use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::{Repository, SemanticAssetCandidate};
use crate::error::{AppError, AppResult};
use crate::models::SemanticProgress;
use crate::semantic::{ExecutionBackend, SemanticAnalysisOutput, SemanticClassifier};
use crate::subject::{SubjectAnalysisOutput, SubjectClassifier};
use crate::tasks::{SemanticControlSignal, SemanticTaskRegistry};

const SEMANTIC_BATCH_SIZE: usize = 32;

#[allow(clippy::too_many_arguments)]
pub fn spawn_semantic_job<F>(
    repository: Repository,
    classifier: Arc<dyn SemanticClassifier>,
    subject_classifier: Option<Arc<dyn SubjectClassifier>>,
    registry: Arc<SemanticTaskRegistry>,
    job_id: String,
    library_id: i64,
    candidates: Vec<SemanticAssetCandidate>,
    emit: F,
) -> AppResult<()>
where
    F: Fn(SemanticProgress) + Send + 'static,
{
    let control = registry.insert(&job_id).ok_or_else(|| {
        AppError::InvalidArgument(format!("semantic job is already running: {job_id}"))
    })?;
    let thread_job_id = job_id.clone();
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
                    execution_backend: Some("cpu".into()),
                    model_name: classifier.metadata().name,
                    model_version: classifier.metadata().version,
                    error: None,
                });
            progress.status = "running".into();
            progress.error = None;
            let _ = repository.update_semantic_job_progress(&progress);
            emit(progress.clone());

            for candidate_batch in candidates.chunks(SEMANTIC_BATCH_SIZE) {
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
                    let path = analysis_path(candidate);
                    if path.is_file() {
                        cache_ready.push(candidate);
                    } else {
                        let error =
                            format!("semantic thumbnail cache is missing: {}", path.display());
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
                        .map(|candidate| analysis_path(candidate))
                        .collect::<Vec<_>>();
                    let outputs =
                        classify_batch_with_fallback(classifier.as_ref(), &cache_ready, &paths);
                    let subject_outputs = subject_classifier.as_ref().map(|subject_classifier| {
                        classify_subject_batch_with_fallback(
                            subject_classifier.as_ref(),
                            &cache_ready,
                            &paths,
                        )
                    });
                    let mut successful = Vec::with_capacity(cache_ready.len());

                    for ((candidate, path), output) in cache_ready.iter().zip(paths).zip(outputs) {
                        match output {
                            Ok(output) => successful.push((*candidate, output)),
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

fn classify_subject_batch_with_fallback(
    classifier: &dyn SubjectClassifier,
    candidates: &[&SemanticAssetCandidate],
    paths: &[PathBuf],
) -> Vec<Result<SubjectAnalysisOutput, String>> {
    match classifier.classify_batch(paths, ExecutionBackend::Cpu) {
        Ok(outputs) if outputs.len() == paths.len() => outputs.into_iter().map(Ok).collect(),
        Ok(outputs) => {
            let error = format!(
                "subject model returned {} results for {} images",
                outputs.len(),
                paths.len()
            );
            candidates
                .iter()
                .map(|candidate| Err(format!("{}: {error}", candidate.absolute_path.display())))
                .collect()
        }
        Err(batch_error) => {
            let batch_error = batch_error.to_string();
            candidates
                .iter()
                .zip(paths)
                .map(|(candidate, path)| {
                    classify_subject_single_with_fallback(classifier, candidate, path, &batch_error)
                })
                .collect()
        }
    }
}

fn classify_subject_single_with_fallback(
    classifier: &dyn SubjectClassifier,
    candidate: &SemanticAssetCandidate,
    path: &Path,
    batch_error: &str,
) -> Result<SubjectAnalysisOutput, String> {
    let retry_paths = [path.to_path_buf()];
    let last_error = match classifier.classify_batch(&retry_paths, ExecutionBackend::Cpu) {
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

fn analysis_path(candidate: &SemanticAssetCandidate) -> PathBuf {
    // New jobs only schedule assets with a current grid thumbnail. A recovered
    // queued item can have an empty path when its cache became stale; keeping
    // that path strict makes the worker fail visibly instead of reopening the
    // full-resolution original during analysis.
    candidate.analysis_path.clone()
}

fn classify_batch_with_fallback(
    classifier: &dyn SemanticClassifier,
    candidates: &[&SemanticAssetCandidate],
    paths: &[PathBuf],
) -> Vec<Result<SemanticAnalysisOutput, String>> {
    match classifier.classify_batch(paths, ExecutionBackend::Cpu) {
        Ok(outputs) if outputs.len() == paths.len() => outputs.into_iter().map(Ok).collect(),
        Ok(outputs) => {
            let error = format!(
                "semantic model returned {} results for {} images",
                outputs.len(),
                paths.len()
            );
            candidates
                .iter()
                .map(|candidate| Err(format!("{}: {}", candidate.absolute_path.display(), error)))
                .collect()
        }
        Err(batch_error) => {
            let batch_error = batch_error.to_string();
            candidates
                .iter()
                .zip(paths)
                .map(|(candidate, path)| {
                    classify_single_with_fallback(classifier, candidate, path, &batch_error)
                })
                .collect()
        }
    }
}

fn classify_single_with_fallback(
    classifier: &dyn SemanticClassifier,
    candidate: &SemanticAssetCandidate,
    path: &Path,
    batch_error: &str,
) -> Result<SemanticAnalysisOutput, String> {
    let retry_paths = [path.to_path_buf()];
    let last_error = match classifier.classify_batch(&retry_paths, ExecutionBackend::Cpu) {
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
    use crate::subject::SubjectRuntimeStatus;

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

    #[test]
    fn failed_batch_retry_never_reopens_original_source() {
        let source = PathBuf::from("source/original.jpg");
        let thumbnail = PathBuf::from("cache/grid-640-v1.jpg");
        let candidate = SemanticAssetCandidate {
            id: 1,
            absolute_path: source,
            analysis_path: thumbnail.clone(),
            fingerprint: "fingerprint".into(),
        };
        let classifier = RecordingClassifier {
            calls: Mutex::new(Vec::new()),
        };

        let result =
            classify_single_with_fallback(&classifier, &candidate, &thumbnail, "batch failure");

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
        };
        let classifier = RecordingSubjectClassifier {
            calls: Mutex::new(Vec::new()),
        };

        let result = classify_subject_batch_with_fallback(
            &classifier,
            &[&candidate],
            std::slice::from_ref(&thumbnail),
        );

        assert!(result[0].is_err());
        assert_eq!(
            classifier.calls.lock().as_slice(),
            &[vec![thumbnail.clone()], vec![thumbnail]]
        );
    }
}
