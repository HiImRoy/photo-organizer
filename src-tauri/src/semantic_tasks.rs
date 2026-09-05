use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::{Repository, SemanticAssetCandidate};
use crate::error::{AppError, AppResult};
use crate::gpu::{
    CPU_ANALYSIS_BATCH_LIMIT, MAX_DIRECTML_ANALYSIS_BATCH_SIZE, analysis_batch_limit_for_backend,
};
use crate::models::SemanticProgress;
use crate::semantic::{
    ExecutionBackend, SemanticAnalysisOutput, SemanticClassifier, SemanticPrediction,
    SemanticSimilarity,
};
use crate::subject::{SubjectAnalysisOutput, SubjectClassifier};
use crate::tasks::{SemanticControlSignal, SemanticTaskRegistry};

// Keep CPU inference small enough for ordinary desktops; DirectML may use a wider
// application analysis batch after the GPU capacity tier has been checked.
pub(crate) const SEMANTIC_BATCH_SIZE: usize = 4;

#[allow(clippy::too_many_arguments)]
pub fn spawn_semantic_job<F>(
    repository: Repository,
    classifier: Arc<dyn SemanticClassifier>,
    subject_classifier: Option<Arc<dyn SubjectClassifier>>,
    registry: Arc<SemanticTaskRegistry>,
    job_id: String,
    library_id: i64,
    candidates: Vec<SemanticAssetCandidate>,
    thumbnail_dir: PathBuf,
    batch_size: usize,
    backend: ExecutionBackend,
    emit: F,
) -> AppResult<()>
where
    F: Fn(SemanticProgress) + Send + 'static,
{
    let control = registry.insert(&job_id).ok_or_else(|| {
        AppError::InvalidArgument(format!("semantic job is already running: {job_id}"))
    })?;
    let thread_job_id = job_id.clone();
    let gpu_capabilities = crate::gpu::detect_gpu_capabilities();
    let max_batch_size = analysis_batch_limit_for_backend(&gpu_capabilities, backend);
    debug_assert!(
        max_batch_size
            <= if backend == ExecutionBackend::DirectMl {
                MAX_DIRECTML_ANALYSIS_BATCH_SIZE
            } else {
                CPU_ANALYSIS_BATCH_LIMIT
            }
    );
    let batch_size = if batch_size == 0 {
        SEMANTIC_BATCH_SIZE
    } else {
        batch_size.clamp(1, max_batch_size)
    };
    let subject_backend = subject_classifier
        .as_ref()
        .and_then(|classifier| classifier.status().selected_backend)
        .unwrap_or(backend);
    std::thread::Builder::new()
        .name(format!("semantic-{thread_job_id}"))
        .spawn(move || {
            let mut progress = repository
                .semantic_progress_by_job(&thread_job_id)
                .ok()
                .flatten()
                .unwrap_or_else(|| SemanticProgress {
                    job_id: thread_job_id.clone(),
                    library_id,
                    status: "queued".into(),
                    total: candidates.len() as u64,
                    processed: 0,
                    completed: 0,
                    failed: 0,
                    skipped: 0,
                    current_asset_id: None,
                    current_path: None,
                    execution_backend: Some(
                        match backend {
                            ExecutionBackend::DirectMl => "direct_ml",
                            _ => "cpu",
                        }
                        .into(),
                    ),
                    model_name: classifier.result_metadata().name,
                    model_version: classifier.result_metadata().version,
                    error: None,
                });
            progress.execution_backend = Some(backend.id().into());
            progress.status = "running".into();
            progress.error = None;
            let _ = repository.update_semantic_job_progress(&progress);
            emit(progress.clone());

            for candidate_batch in candidates.chunks(batch_size) {
                if control.wait_until_runnable() == SemanticControlSignal::Cancel {
                    progress.status = "cancelled".into();
                    progress.current_asset_id = None;
                    progress.current_path = None;
                    let _ = repository.cancel_semantic_job(&thread_job_id);
                    let _ = repository.update_semantic_job_progress(&progress);
                    emit(progress.clone());
                    registry.remove(&thread_job_id);
                    return;
                }

                let mut ready = Vec::with_capacity(candidate_batch.len());
                if let Err(error) = repository.mark_semantic_items_running(
                    &thread_job_id,
                    &candidate_batch
                        .iter()
                        .map(|candidate| candidate.id)
                        .collect::<Vec<_>>(),
                ) {
                    // Keep the old per-item failure isolation if the grouped
                    // state update itself cannot be committed.
                    for candidate in candidate_batch {
                        match repository.mark_semantic_item_running(&thread_job_id, candidate.id) {
                            Ok(()) => ready.push(candidate),
                            Err(item_error) => {
                                progress.failed += 1;
                                progress.error = Some(item_error.to_string());
                            }
                        }
                    }
                    log::warn!(
                        "grouped semantic running-state update failed for {thread_job_id}: {error}"
                    );
                } else {
                    ready.extend(candidate_batch.iter());
                }

                let mut cache_ready = Vec::with_capacity(ready.len());
                for candidate in ready {
                    let path = analysis_path(candidate, &thumbnail_dir);
                    if path.as_ref().is_some_and(|path| path.is_file()) {
                        cache_ready.push(candidate);
                    } else {
                        let error = match path {
                            Some(path) => {
                                format!("semantic thumbnail cache is missing: {}", path.display())
                            }
                            None => format!(
                                "semantic analysis path is not an application thumbnail: {}",
                                candidate.analysis_path.display()
                            ),
                        };
                        progress.failed += 1;
                        progress.error = Some(error.clone());
                        let _ = repository.fail_semantic_item(&thread_job_id, candidate.id, &error);
                    }
                }

                if let Some(candidate) = cache_ready.first() {
                    progress.status = "running".into();
                    progress.current_asset_id = Some(candidate.id);
                    progress.current_path =
                        Some(candidate.absolute_path.to_string_lossy().into_owned());
                    emit(progress.clone());

                    let paths = cache_ready
                        .iter()
                        .map(|candidate| {
                            analysis_path(candidate, &thumbnail_dir)
                                .expect("validated semantic thumbnail path")
                        })
                        .collect::<Vec<_>>();
                    let outputs =
                        classify_batch_with_fallback(classifier.as_ref(), &cache_ready, &paths, backend);
                    let subject_outputs = subject_classifier.as_ref().map(|subject_classifier| {
                        classify_subject_batch_with_fallback(
                            subject_classifier.as_ref(),
                            &cache_ready,
                            &paths,
                            subject_backend,
                        )
                    });
                    let mut successful = Vec::with_capacity(cache_ready.len());

                    for (index, ((candidate, path), output)) in
                        cache_ready.iter().zip(paths).zip(outputs).enumerate()
                    {
                        match output {
                            Ok(mut output) => {
                                add_derived_subject_evidence(&mut output);
                                if let Some(subject_outputs) = subject_outputs.as_ref()
                                    && let Some(Ok(subject_output)) = subject_outputs.get(index)
                                {
                                    fuse_topic_with_subject_evidence(&mut output, subject_output);
                                }
                                successful.push((*candidate, output));
                            }
                            Err(error) => {
                                progress.failed += 1;
                                progress.error = Some(error.clone());
                                let _ = repository.fail_semantic_item(
                                    &thread_job_id,
                                    candidate.id,
                                    &error,
                                );
                            }
                        }
                        progress.current_path = Some(path.to_string_lossy().into_owned());
                    }

                    let entries = successful
                        .iter()
                        .map(|(candidate, output)| (*candidate, output))
                        .collect::<Vec<_>>();
                    match repository.save_semantic_results(&thread_job_id, &entries) {
                        Ok((completed, skipped)) => {
                            progress.completed += completed as u64;
                            progress.skipped += skipped as u64;
                            if let Some(subject_outputs) = subject_outputs {
                                let mut subject_successful = Vec::with_capacity(cache_ready.len());
                                for (candidate, output) in
                                    cache_ready.iter().zip(subject_outputs)
                                {
                                    match output {
                                        Ok(output) => {
                                            subject_successful.push((*candidate, output));
                                        }
                                        Err(error) => {
                                            let _ = repository.save_subject_failure(candidate, &error);
                                            progress.error = Some(error);
                                        }
                                    }
                                }
                                let subject_entries = subject_successful
                                    .iter()
                                    .map(|(candidate, output)| (*candidate, output))
                                    .collect::<Vec<_>>();
                                if let Err(error) =
                                    repository.save_subject_results(&subject_entries)
                                {
                                    progress.error = Some(error.to_string());
                                    log::warn!(
                                        "could not save subject results for {thread_job_id}: {error}"
                                    );
                                }
                            }
                        }
                        Err(error) => {
                            progress.failed += successful.len() as u64;
                            progress.error = Some(error.to_string());
                            for (candidate, _) in successful {
                                let _ = repository.fail_semantic_item(
                                    &thread_job_id,
                                    candidate.id,
                                    &error.to_string(),
                                );
                            }
                        }
                    }
                }
                progress.processed = progress.completed + progress.failed + progress.skipped;
                progress.current_asset_id = None;
                progress.current_path = None;
                let _ = repository.update_semantic_job_progress(&progress);
                emit(progress.clone());
            }

            progress.status = "completed".into();
            progress.current_asset_id = None;
            progress.current_path = None;
            if progress.failed == 0 {
                progress.error = None;
            }
            let _ = repository.update_semantic_job_progress(&progress);
            emit(progress);
            registry.remove(&thread_job_id);
        })
        .map_err(AppError::Io)?;
    Ok(())
}

fn add_derived_subject_evidence(output: &mut SemanticAnalysisOutput) {
    let Some((similarity, threshold)) = output
        .predictions
        .iter()
        .find(|prediction| prediction.is_primary && prediction.label_id == "photo_landscape")
        .map(|prediction| (prediction.similarity, prediction.threshold))
    else {
        return;
    };
    if output
        .predictions
        .iter()
        .any(|prediction| prediction.label_id == "scenery")
    {
        return;
    }
    output.predictions.push(SemanticPrediction {
        label_id: "scenery".into(),
        display_name: "风景".into(),
        category_group: "subject".into(),
        similarity,
        threshold,
        is_primary: false,
    });
}

fn fuse_topic_with_subject_evidence(
    output: &mut SemanticAnalysisOutput,
    subject_output: &SubjectAnalysisOutput,
) {
    let Some((label_id, display_name, similarity, threshold)) = subject_output
        .predictions
        .iter()
        .filter_map(|prediction| match prediction.label_id.as_str() {
            "single_person" | "multiple_people" => {
                Some(("photo_portrait", "人像", prediction.similarity, 0.22_f32))
            }
            "animal" => Some(("photo_wildlife", "动物", prediction.similarity, 0.22_f32)),
            "food" => Some((
                "photo_still_life",
                "静物特写",
                prediction.similarity,
                0.21_f32,
            )),
            "plant" => Some(("photo_macro", "植物", prediction.similarity, 0.22_f32)),
            _ => None,
        })
        .max_by(|left, right| left.2.total_cmp(&right.2))
    else {
        return;
    };

    output
        .predictions
        .retain(|prediction| !prediction.is_primary);
    output.predictions.push(SemanticPrediction {
        label_id: label_id.into(),
        display_name: display_name.into(),
        category_group: "scene".into(),
        similarity,
        threshold,
        is_primary: true,
    });

    if let Some(evidence) = output
        .raw_similarities
        .iter_mut()
        .find(|evidence| evidence.label_id == label_id)
    {
        evidence.similarity = evidence.similarity.max(similarity);
    } else {
        output.raw_similarities.push(SemanticSimilarity {
            label_id: label_id.into(),
            display_name: display_name.into(),
            category_group: "topic_subject_fusion".into(),
            similarity,
            threshold,
        });
    }
    output.raw_similarities.sort_by(|left, right| {
        right
            .similarity
            .total_cmp(&left.similarity)
            .then(left.label_id.cmp(&right.label_id))
    });
    output.raw_similarities.truncate(8);
}

fn classify_subject_batch_with_fallback(
    classifier: &dyn SubjectClassifier,
    candidates: &[&SemanticAssetCandidate],
    paths: &[PathBuf],
    backend: ExecutionBackend,
) -> Vec<Result<SubjectAnalysisOutput, String>> {
    classify_subject_batch_adaptively(classifier, candidates, paths, backend)
}

fn classify_subject_batch_adaptively(
    classifier: &dyn SubjectClassifier,
    candidates: &[&SemanticAssetCandidate],
    paths: &[PathBuf],
    backend: ExecutionBackend,
) -> Vec<Result<SubjectAnalysisOutput, String>> {
    debug_assert_eq!(candidates.len(), paths.len());
    match classifier.classify_batch(paths, backend) {
        Ok(outputs) if outputs.len() == paths.len() => outputs.into_iter().map(Ok).collect(),
        Ok(outputs) => {
            let batch_error = format!(
                "subject model returned {} results for {} images",
                outputs.len(),
                paths.len()
            );
            if paths.len() > 1 {
                log::warn!(
                    "subject {} batch returned an invalid result count; backing off from {} images",
                    backend.id(),
                    paths.len()
                );
                let midpoint = paths.len() / 2;
                let mut results = classify_subject_batch_adaptively(
                    classifier,
                    &candidates[..midpoint],
                    &paths[..midpoint],
                    backend,
                );
                results.extend(classify_subject_batch_adaptively(
                    classifier,
                    &candidates[midpoint..],
                    &paths[midpoint..],
                    backend,
                ));
                results
            } else {
                candidates
                    .iter()
                    .map(|candidate| {
                        Err(format!(
                            "{}: {}",
                            candidate.absolute_path.display(),
                            batch_error
                        ))
                    })
                    .collect()
            }
        }
        Err(batch_error) if paths.len() > 1 => {
            log::warn!(
                "subject {} batch of {} thumbnails failed; backing off to smaller batches: {}",
                backend.id(),
                paths.len(),
                batch_error
            );
            let midpoint = paths.len() / 2;
            let mut results = classify_subject_batch_adaptively(
                classifier,
                &candidates[..midpoint],
                &paths[..midpoint],
                backend,
            );
            results.extend(classify_subject_batch_adaptively(
                classifier,
                &candidates[midpoint..],
                &paths[midpoint..],
                backend,
            ));
            results
        }
        Err(batch_error) => candidates
            .iter()
            .zip(paths)
            .map(|(candidate, path)| {
                classify_subject_single_with_fallback(
                    classifier,
                    candidate,
                    path,
                    &batch_error.to_string(),
                    backend,
                )
            })
            .collect(),
    }
}

fn classify_subject_single_with_fallback(
    classifier: &dyn SubjectClassifier,
    candidate: &SemanticAssetCandidate,
    path: &Path,
    batch_error: &str,
    backend: ExecutionBackend,
) -> Result<SubjectAnalysisOutput, String> {
    let retry_paths = [path.to_path_buf()];
    let last_error = match classifier.classify_batch(&retry_paths, backend) {
        Ok(mut outputs) if outputs.len() == 1 => return Ok(outputs.remove(0)),
        Ok(outputs) => format!(
            "subject model returned {} results for one image",
            outputs.len()
        ),
        Err(error) => error.to_string(),
    };
    Err(format!(
        "{batch_error}; single-thumbnail subject retry failed for {}: {last_error}",
        candidate.absolute_path.display()
    ))
}

fn analysis_path(candidate: &SemanticAssetCandidate, thumbnail_dir: &Path) -> Option<PathBuf> {
    // New jobs only schedule assets with a current grid thumbnail. A recovered
    // queued item can have an empty path when its cache became stale; keeping
    // that path strict makes the worker fail visibly instead of reopening the
    // full-resolution original during analysis.
    let file_name = candidate.analysis_path.file_name()?.to_str()?;
    let expected_suffix = format!("-{}.jpg", crate::imaging::THUMBNAIL_SPEC);
    if candidate.analysis_path == candidate.absolute_path
        || !candidate.analysis_path.starts_with(thumbnail_dir)
        || !file_name.ends_with(&expected_suffix)
    {
        return None;
    }
    Some(candidate.analysis_path.clone())
}

fn classify_batch_with_fallback(
    classifier: &dyn SemanticClassifier,
    candidates: &[&SemanticAssetCandidate],
    paths: &[PathBuf],
    backend: ExecutionBackend,
) -> Vec<Result<SemanticAnalysisOutput, String>> {
    classify_batch_adaptively(classifier, candidates, paths, backend)
}

fn classify_batch_adaptively(
    classifier: &dyn SemanticClassifier,
    candidates: &[&SemanticAssetCandidate],
    paths: &[PathBuf],
    backend: ExecutionBackend,
) -> Vec<Result<SemanticAnalysisOutput, String>> {
    debug_assert_eq!(candidates.len(), paths.len());
    match classifier.classify_batch(paths, backend) {
        Ok(outputs) if outputs.len() == paths.len() => outputs.into_iter().map(Ok).collect(),
        Ok(outputs) => {
            let batch_error = format!(
                "semantic model returned {} results for {} images",
                outputs.len(),
                paths.len()
            );
            if paths.len() > 1 {
                log::warn!(
                    "semantic {} batch returned an invalid result count; backing off from {} images",
                    backend.id(),
                    paths.len()
                );
                let midpoint = paths.len() / 2;
                let mut results = classify_batch_adaptively(
                    classifier,
                    &candidates[..midpoint],
                    &paths[..midpoint],
                    backend,
                );
                results.extend(classify_batch_adaptively(
                    classifier,
                    &candidates[midpoint..],
                    &paths[midpoint..],
                    backend,
                ));
                results
            } else {
                candidates
                    .iter()
                    .map(|candidate| {
                        Err(format!(
                            "{}: {}",
                            candidate.absolute_path.display(),
                            batch_error
                        ))
                    })
                    .collect()
            }
        }
        Err(batch_error) if paths.len() > 1 => {
            log::warn!(
                "semantic {} batch of {} thumbnails failed; backing off to smaller batches: {}",
                backend.id(),
                paths.len(),
                batch_error
            );
            let midpoint = paths.len() / 2;
            let mut results = classify_batch_adaptively(
                classifier,
                &candidates[..midpoint],
                &paths[..midpoint],
                backend,
            );
            results.extend(classify_batch_adaptively(
                classifier,
                &candidates[midpoint..],
                &paths[midpoint..],
                backend,
            ));
            results
        }
        Err(batch_error) => candidates
            .iter()
            .zip(paths)
            .map(|(candidate, path)| {
                classify_single_with_fallback(
                    classifier,
                    candidate,
                    path,
                    &batch_error.to_string(),
                    backend,
                )
            })
            .collect(),
    }
}

fn classify_single_with_fallback(
    classifier: &dyn SemanticClassifier,
    candidate: &SemanticAssetCandidate,
    path: &Path,
    batch_error: &str,
    backend: ExecutionBackend,
) -> Result<SemanticAnalysisOutput, String> {
    let retry_paths = [path.to_path_buf()];
    let last_error = match classifier.classify_batch(&retry_paths, backend) {
        Ok(mut outputs) if outputs.len() == 1 => return Ok(outputs.remove(0)),
        Ok(outputs) => {
            format!(
                "semantic model returned {} results for one image",
                outputs.len()
            )
        }
        Err(error) => error.to_string(),
    };
    Err(format!(
        "{}; single-thumbnail retry failed for {}: {}",
        batch_error,
        candidate.absolute_path.display(),
        last_error
    ))
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use super::*;
    use crate::semantic::{ModelMetadata, SemanticError, SemanticRuntimeStatus};
    use crate::subject::{SubjectPrediction, SubjectRuntimeStatus};

    #[test]
    fn application_analysis_uses_batch_size_four() {
        assert_eq!(SEMANTIC_BATCH_SIZE, 4);
    }

    struct RecordingClassifier {
        calls: Mutex<Vec<Vec<PathBuf>>>,
    }

    impl SemanticClassifier for RecordingClassifier {
        fn metadata(&self) -> ModelMetadata {
            ModelMetadata {
                name: "test-model".into(),
                version: "test-version".into(),
                analysis_version: "test-analysis".into(),
                license: Some("test".into()),
                installed: true,
                model_size_bytes: None,
                model_sha256: None,
                supported_backends: vec![ExecutionBackend::Cpu],
            }
        }

        fn status(&self) -> SemanticRuntimeStatus {
            SemanticRuntimeStatus {
                status: "ready".into(),
                message: "test".into(),
                model: self.metadata(),
                topic_model: None,
                selected_backend: Some(ExecutionBackend::Cpu),
            }
        }

        fn classify_batch(
            &self,
            images: &[PathBuf],
            _backend: ExecutionBackend,
        ) -> Result<Vec<SemanticAnalysisOutput>, SemanticError> {
            self.calls.lock().push(images.to_vec());
            Err(SemanticError::Inference("test failure".into()))
        }
    }

    struct RecordingSubjectClassifier {
        calls: Mutex<Vec<Vec<PathBuf>>>,
    }

    impl SubjectClassifier for RecordingSubjectClassifier {
        fn metadata(&self) -> ModelMetadata {
            ModelMetadata {
                name: "test-subject-model".into(),
                version: "test-version".into(),
                analysis_version: "test-analysis".into(),
                license: Some("test".into()),
                installed: true,
                model_size_bytes: None,
                model_sha256: None,
                supported_backends: vec![ExecutionBackend::Cpu],
            }
        }

        fn face_metadata(&self) -> ModelMetadata {
            self.metadata()
        }

        fn status(&self) -> SubjectRuntimeStatus {
            SubjectRuntimeStatus {
                status: "ready".into(),
                message: "test".into(),
                model: self.metadata(),
                face_model: self.face_metadata(),
                selected_backend: Some(ExecutionBackend::Cpu),
            }
        }

        fn classify_batch(
            &self,
            images: &[PathBuf],
            _backend: ExecutionBackend,
        ) -> Result<Vec<SubjectAnalysisOutput>, SemanticError> {
            self.calls.lock().push(images.to_vec());
            Err(SemanticError::Inference("test subject failure".into()))
        }
    }

    struct AdaptiveClassifier {
        calls: Mutex<Vec<Vec<PathBuf>>>,
        max_batch_size: usize,
    }

    impl SemanticClassifier for AdaptiveClassifier {
        fn metadata(&self) -> ModelMetadata {
            ModelMetadata {
                name: "adaptive-test-model".into(),
                version: "test-version".into(),
                analysis_version: "test-analysis".into(),
                license: Some("test".into()),
                installed: true,
                model_size_bytes: None,
                model_sha256: None,
                supported_backends: vec![ExecutionBackend::Cpu],
            }
        }

        fn status(&self) -> SemanticRuntimeStatus {
            SemanticRuntimeStatus {
                status: "ready".into(),
                message: "test".into(),
                model: self.metadata(),
                topic_model: None,
                selected_backend: Some(ExecutionBackend::Cpu),
            }
        }

        fn classify_batch(
            &self,
            images: &[PathBuf],
            _backend: ExecutionBackend,
        ) -> Result<Vec<SemanticAnalysisOutput>, SemanticError> {
            self.calls.lock().push(images.to_vec());
            if images.len() > self.max_batch_size {
                return Err(SemanticError::Inference(format!(
                    "test batch too large: {}",
                    images.len()
                )));
            }
            Ok(images
                .iter()
                .map(|_| SemanticAnalysisOutput {
                    predictions: Vec::new(),
                    embedding: Vec::new(),
                    raw_similarities: Vec::new(),
                })
                .collect())
        }
    }

    struct AdaptiveSubjectClassifier {
        calls: Mutex<Vec<Vec<PathBuf>>>,
        max_batch_size: usize,
    }

    impl SubjectClassifier for AdaptiveSubjectClassifier {
        fn metadata(&self) -> ModelMetadata {
            ModelMetadata {
                name: "adaptive-subject-test-model".into(),
                version: "test-version".into(),
                analysis_version: "test-analysis".into(),
                license: Some("test".into()),
                installed: true,
                model_size_bytes: None,
                model_sha256: None,
                supported_backends: vec![ExecutionBackend::Cpu],
            }
        }

        fn face_metadata(&self) -> ModelMetadata {
            self.metadata()
        }

        fn status(&self) -> SubjectRuntimeStatus {
            SubjectRuntimeStatus {
                status: "ready".into(),
                message: "test".into(),
                model: self.metadata(),
                face_model: self.face_metadata(),
                selected_backend: Some(ExecutionBackend::Cpu),
            }
        }

        fn classify_batch(
            &self,
            images: &[PathBuf],
            _backend: ExecutionBackend,
        ) -> Result<Vec<SubjectAnalysisOutput>, SemanticError> {
            self.calls.lock().push(images.to_vec());
            if images.len() > self.max_batch_size {
                return Err(SemanticError::Inference(format!(
                    "test subject batch too large: {}",
                    images.len()
                )));
            }
            Ok(images
                .iter()
                .map(|_| SubjectAnalysisOutput {
                    predictions: Vec::new(),
                })
                .collect())
        }
    }

    fn adaptive_candidates(count: usize) -> Vec<SemanticAssetCandidate> {
        (0..count)
            .map(|index| SemanticAssetCandidate {
                id: index as i64 + 1,
                absolute_path: PathBuf::from(format!("source/original-{index}.jpg")),
                analysis_path: PathBuf::from(format!("cache/asset-{index}-grid-640-v1.jpg")),
                fingerprint: format!("fingerprint-{index}"),
                model_name: "test-model".into(),
                model_version: "test-version".into(),
                analysis_version: "test-analysis".into(),
                taxonomy_version: crate::semantic::TAXONOMY_VERSION.into(),
            })
            .collect()
    }

    #[test]
    fn semantic_batch_failure_splits_on_the_same_thumbnail_backend() {
        let candidates = adaptive_candidates(4);
        let candidate_refs = candidates.iter().collect::<Vec<_>>();
        let paths = candidates
            .iter()
            .map(|candidate| candidate.analysis_path.clone())
            .collect::<Vec<_>>();
        let classifier = AdaptiveClassifier {
            calls: Mutex::new(Vec::new()),
            max_batch_size: 2,
        };

        let results = classify_batch_with_fallback(
            &classifier,
            &candidate_refs,
            &paths,
            ExecutionBackend::Cpu,
        );

        assert_eq!(results.len(), 4);
        assert!(results.iter().all(Result::is_ok));
        let calls = classifier.calls.lock();
        assert_eq!(calls[0].len(), 4);
        assert!(calls.iter().skip(1).all(|call| call.len() <= 2));
        assert!(
            calls
                .iter()
                .flatten()
                .all(|path| path.to_string_lossy().contains("grid-640-v1"))
        );
        assert!(
            calls
                .iter()
                .flatten()
                .all(|path| !path.to_string_lossy().contains("original"))
        );
    }

    #[test]
    fn subject_batch_failure_splits_on_the_same_thumbnail_backend() {
        let candidates = adaptive_candidates(4);
        let candidate_refs = candidates.iter().collect::<Vec<_>>();
        let paths = candidates
            .iter()
            .map(|candidate| candidate.analysis_path.clone())
            .collect::<Vec<_>>();
        let classifier = AdaptiveSubjectClassifier {
            calls: Mutex::new(Vec::new()),
            max_batch_size: 2,
        };

        let results = classify_subject_batch_with_fallback(
            &classifier,
            &candidate_refs,
            &paths,
            ExecutionBackend::Cpu,
        );

        assert_eq!(results.len(), 4);
        assert!(results.iter().all(Result::is_ok));
        let calls = classifier.calls.lock();
        assert_eq!(calls[0].len(), 4);
        assert!(calls.iter().skip(1).all(|call| call.len() <= 2));
        assert!(
            calls
                .iter()
                .flatten()
                .all(|path| path.to_string_lossy().contains("grid-640-v1"))
        );
        assert!(
            calls
                .iter()
                .flatten()
                .all(|path| !path.to_string_lossy().contains("original"))
        );
    }

    #[test]
    fn failed_batch_retry_never_reopens_original_source() {
        let source = PathBuf::from("source/original.jpg");
        let thumbnail = PathBuf::from("cache/grid-640-v1.jpg");
        let candidate = SemanticAssetCandidate {
            id: 1,
            absolute_path: source,
            analysis_path: thumbnail.clone(),
            fingerprint: "fingerprint".into(),
            model_name: "test-model".into(),
            model_version: "test-version".into(),
            analysis_version: "test-analysis".into(),
            taxonomy_version: crate::semantic::TAXONOMY_VERSION.into(),
        };
        let classifier = RecordingClassifier {
            calls: Mutex::new(Vec::new()),
        };

        let result = classify_single_with_fallback(
            &classifier,
            &candidate,
            &thumbnail,
            "batch failure",
            ExecutionBackend::Cpu,
        );

        assert!(result.is_err());
        assert_eq!(classifier.calls.lock().as_slice(), &[vec![thumbnail]]);
    }

    #[test]
    fn subject_retry_never_reopens_original_source() {
        let source = PathBuf::from("source/original.jpg");
        let thumbnail = PathBuf::from("cache/grid-640-v1.jpg");
        let candidate = SemanticAssetCandidate {
            id: 1,
            absolute_path: source,
            analysis_path: thumbnail.clone(),
            fingerprint: "fingerprint".into(),
            model_name: "test-model".into(),
            model_version: "test-version".into(),
            analysis_version: "test-analysis".into(),
            taxonomy_version: crate::semantic::TAXONOMY_VERSION.into(),
        };
        let classifier = RecordingSubjectClassifier {
            calls: Mutex::new(Vec::new()),
        };

        let result = classify_subject_batch_with_fallback(
            &classifier,
            &[&candidate],
            std::slice::from_ref(&thumbnail),
            ExecutionBackend::Cpu,
        );

        assert!(result[0].is_err());
        assert_eq!(
            classifier.calls.lock().as_slice(),
            &[vec![thumbnail.clone()], vec![thumbnail]]
        );
    }

    #[test]
    fn analysis_path_rejects_source_and_external_paths() {
        let source = PathBuf::from("source/original.jpg");
        let thumbnail = PathBuf::from("cache/asset-grid-640-v1.jpg");
        let mut candidate = SemanticAssetCandidate {
            id: 1,
            absolute_path: source.clone(),
            analysis_path: thumbnail.clone(),
            fingerprint: "fingerprint".into(),
            model_name: "test-model".into(),
            model_version: "test-version".into(),
            analysis_version: "test-analysis".into(),
            taxonomy_version: crate::semantic::TAXONOMY_VERSION.into(),
        };

        assert_eq!(
            analysis_path(&candidate, Path::new("cache")),
            Some(thumbnail)
        );

        candidate.analysis_path = source;
        assert!(analysis_path(&candidate, Path::new("cache")).is_none());

        candidate.analysis_path = PathBuf::from("outside/asset-grid-640-v1.jpg");
        assert!(analysis_path(&candidate, Path::new("cache")).is_none());
    }

    #[test]
    fn subject_evidence_can_replace_a_scene_topic_without_creating_a_subject_label() {
        let mut output = SemanticAnalysisOutput {
            predictions: vec![SemanticPrediction {
                label_id: "photo_landscape".into(),
                display_name: "风光".into(),
                category_group: "scene".into(),
                similarity: 0.42,
                threshold: 0.18,
                is_primary: true,
            }],
            embedding: vec![],
            raw_similarities: vec![SemanticSimilarity {
                label_id: "photo_landscape".into(),
                display_name: "风光".into(),
                category_group: "topic_candidate".into(),
                similarity: 0.42,
                threshold: 0.18,
            }],
        };
        let subject_output = SubjectAnalysisOutput {
            predictions: vec![SubjectPrediction {
                label_id: "single_person".into(),
                display_name: "单人".into(),
                category_group: "subject".into(),
                similarity: 0.91,
                threshold: 0.45,
            }],
        };

        fuse_topic_with_subject_evidence(&mut output, &subject_output);

        assert_eq!(
            output
                .predictions
                .iter()
                .filter(|prediction| prediction.is_primary)
                .map(|prediction| prediction.label_id.as_str())
                .collect::<Vec<_>>(),
            vec!["photo_portrait"]
        );
        assert!(
            !output
                .predictions
                .iter()
                .any(|prediction| prediction.category_group == "subject")
        );
        assert!(
            output
                .raw_similarities
                .iter()
                .any(|evidence| evidence.label_id == "photo_portrait")
        );
    }

    #[test]
    fn landscape_topic_adds_a_non_primary_scenery_subject_label() {
        let mut output = SemanticAnalysisOutput {
            predictions: vec![SemanticPrediction {
                label_id: "photo_landscape".into(),
                display_name: "风光".into(),
                category_group: "scene".into(),
                similarity: 0.42,
                threshold: 0.18,
                is_primary: true,
            }],
            embedding: vec![],
            raw_similarities: vec![],
        };

        add_derived_subject_evidence(&mut output);

        let scenery = output
            .predictions
            .iter()
            .find(|prediction| prediction.label_id == "scenery")
            .expect("derived scenery label");
        assert_eq!(scenery.display_name, "风景");
        assert_eq!(scenery.category_group, "subject");
        assert!(!scenery.is_primary);
    }
}
