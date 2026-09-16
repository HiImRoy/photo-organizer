use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::{Repository, SemanticAssetCandidate, is_terminal_semantic_job_status};
use crate::error::{AppError, AppResult};
use crate::gpu::{
    CPU_ANALYSIS_BATCH_LIMIT, MAX_DIRECTML_ANALYSIS_BATCH_SIZE, analysis_batch_limit_for_backend,
};
use crate::models::SemanticProgress;
use crate::semantic::{ExecutionBackend, SemanticAnalysisOutput, SemanticClassifier};
use crate::subject::{SubjectAnalysisOutput, SubjectClassifier};
use crate::tasks::{SemanticControlSignal, SemanticTaskJobState, SemanticTaskRegistry};

// Keep CPU inference small enough for ordinary desktops; DirectML may use a wider
// application analysis batch after the GPU capacity tier has been checked.
pub(crate) const SEMANTIC_BATCH_SIZE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticWorkerAction {
    Continue,
    WaitForResume,
    Stop,
}

fn persist_semantic_progress_and_emit<F>(
    repository: &Repository,
    progress: &SemanticProgress,
    emit: &F,
) -> Result<bool, String>
where
    F: Fn(SemanticProgress),
{
    match repository.update_semantic_job_progress_if_active(progress) {
        Ok(true) => {
            emit(progress.clone());
            Ok(true)
        }
        Ok(false) => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn publish_semantic_persistence_failure<F>(
    job: &mut SemanticTaskJobState,
    repository: &Repository,
    progress: &mut SemanticProgress,
    error: String,
    emit: &F,
) where
    F: Fn(SemanticProgress),
{
    log::error!(
        "could not persist semantic job {}: {error}",
        progress.job_id
    );
    progress.status = "failed".into();
    progress.error = Some(error);
    progress.current_asset_id = None;
    progress.current_path = None;
    match repository.update_semantic_job_progress_if_active(progress) {
        Ok(true) => {}
        Ok(false) => log::error!(
            "semantic job {} failure status was not persisted",
            progress.job_id
        ),
        Err(error) => log::error!(
            "could not persist semantic job {} failure status: {error}",
            progress.job_id
        ),
    }
    job.mark_terminal();
    emit(progress.clone());
}

fn cancel_semantic_job_locked<F>(
    job: &mut SemanticTaskJobState,
    repository: &Repository,
    progress: &mut SemanticProgress,
    emit: &F,
) where
    F: Fn(SemanticProgress),
{
    if job.is_terminal() {
        return;
    }
    progress.status = "cancelled".into();
    progress.current_asset_id = None;
    progress.current_path = None;
    let cancelled = match repository.cancel_semantic_job_if_active(&progress.job_id) {
        Ok(cancelled) => cancelled,
        Err(error) => {
            publish_semantic_persistence_failure(
                job,
                repository,
                progress,
                format!("could not cancel semantic job: {error}"),
                emit,
            );
            return;
        }
    };
    job.mark_terminal();
    if cancelled {
        match persist_semantic_progress_and_emit(repository, progress, emit) {
            Ok(true) => {}
            Ok(false) => emit(progress.clone()),
            Err(error) => {
                log::warn!(
                    "could not persist cancelled semantic job {} progress: {error}",
                    progress.job_id
                );
                emit(progress.clone());
            }
        }
    }
}

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
            progress.error = None;
            let startup_action = control.with_job_lock(|job| {
                if is_terminal_semantic_job_status(&progress.status) {
                    job.mark_terminal();
                    return SemanticWorkerAction::Stop;
                }
                match control.current_signal() {
                    SemanticControlSignal::Cancel => {
                        cancel_semantic_job_locked(job, &repository, &mut progress, &emit);
                        SemanticWorkerAction::Stop
                    }
                    SemanticControlSignal::Paused => {
                        progress.status = "paused".into();
                        match persist_semantic_progress_and_emit(&repository, &progress, &emit) {
                            Ok(true) => SemanticWorkerAction::Continue,
                            Ok(false) => {
                                job.mark_terminal();
                                SemanticWorkerAction::Stop
                            }
                            Err(error) => {
                                publish_semantic_persistence_failure(
                                    job,
                                    &repository,
                                    &mut progress,
                                    error,
                                    &emit,
                                );
                                SemanticWorkerAction::Stop
                            }
                        }
                    }
                    SemanticControlSignal::Continue => {
                        progress.status = "running".into();
                        match persist_semantic_progress_and_emit(&repository, &progress, &emit) {
                            Ok(true) => SemanticWorkerAction::Continue,
                            Ok(false) => {
                                job.mark_terminal();
                                SemanticWorkerAction::Stop
                            }
                            Err(error) => {
                                publish_semantic_persistence_failure(
                                    job,
                                    &repository,
                                    &mut progress,
                                    error,
                                    &emit,
                                );
                                SemanticWorkerAction::Stop
                            }
                        }
                    }
                }
            });
            if startup_action == SemanticWorkerAction::Stop {
                registry.remove(&thread_job_id);
                return;
            }

            for candidate_batch in candidates.chunks(batch_size) {
                loop {
                    match control.wait_until_runnable() {
                        SemanticControlSignal::Cancel => {
                            control.with_job_lock(|job| {
                                cancel_semantic_job_locked(
                                    job,
                                    &repository,
                                    &mut progress,
                                    &emit,
                                );
                            });
                            registry.remove(&thread_job_id);
                            return;
                        }
                        SemanticControlSignal::Paused => continue,
                        SemanticControlSignal::Continue => {
                            let signal = control.with_job_lock(|job| {
                                if job.is_terminal() {
                                    None
                                } else {
                                    Some(control.current_signal())
                                }
                            });
                            match signal {
                                None => {
                                    registry.remove(&thread_job_id);
                                    return;
                                }
                                Some(SemanticControlSignal::Cancel) => {
                                    control.with_job_lock(|job| {
                                        cancel_semantic_job_locked(
                                            job,
                                            &repository,
                                            &mut progress,
                                            &emit,
                                        );
                                    });
                                    registry.remove(&thread_job_id);
                                    return;
                                }
                                Some(SemanticControlSignal::Paused) => continue,
                                Some(SemanticControlSignal::Continue) => break,
                            }
                        }
                    }
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
                    loop {
                        match control.wait_until_runnable() {
                            SemanticControlSignal::Cancel => {
                                control.with_job_lock(|job| {
                                    cancel_semantic_job_locked(
                                        job,
                                        &repository,
                                        &mut progress,
                                        &emit,
                                    );
                                });
                                registry.remove(&thread_job_id);
                                return;
                            }
                            SemanticControlSignal::Paused => continue,
                            SemanticControlSignal::Continue => {
                                let action = control.with_job_lock(|job| {
                                    if job.is_terminal() {
                                        return SemanticWorkerAction::Stop;
                                    }
                                    match control.current_signal() {
                                        SemanticControlSignal::Cancel => {
                                            cancel_semantic_job_locked(
                                                job,
                                                &repository,
                                                &mut progress,
                                                &emit,
                                            );
                                            SemanticWorkerAction::Stop
                                        }
                                        SemanticControlSignal::Paused => {
                                            progress.status = "paused".into();
                                            progress.current_asset_id = None;
                                            progress.current_path = None;
                                            match persist_semantic_progress_and_emit(
                                                &repository,
                                                &progress,
                                                &emit,
                                            ) {
                                                Ok(true) => SemanticWorkerAction::WaitForResume,
                                                Ok(false) => {
                                                    job.mark_terminal();
                                                    SemanticWorkerAction::Stop
                                                }
                                                Err(error) => {
                                                    publish_semantic_persistence_failure(
                                                        job,
                                                        &repository,
                                                        &mut progress,
                                                        error,
                                                        &emit,
                                                    );
                                                    SemanticWorkerAction::Stop
                                                }
                                            }
                                        }
                                        SemanticControlSignal::Continue => {
                                            progress.status = "running".into();
                                            progress.current_asset_id = Some(candidate.id);
                                            progress.current_path = Some(
                                                candidate
                                                    .absolute_path
                                                    .to_string_lossy()
                                                    .into_owned(),
                                            );
                                            match persist_semantic_progress_and_emit(
                                                &repository,
                                                &progress,
                                                &emit,
                                            ) {
                                                Ok(true) => SemanticWorkerAction::Continue,
                                                Ok(false) => {
                                                    job.mark_terminal();
                                                    SemanticWorkerAction::Stop
                                                }
                                                Err(error) => {
                                                    publish_semantic_persistence_failure(
                                                        job,
                                                        &repository,
                                                        &mut progress,
                                                        error,
                                                        &emit,
                                                    );
                                                    SemanticWorkerAction::Stop
                                                }
                                            }
                                        }
                                    }
                                });
                                match action {
                                    SemanticWorkerAction::Continue => break,
                                    SemanticWorkerAction::WaitForResume => {
                                        let _ = control.wait_until_runnable();
                                        continue;
                                    }
                                    SemanticWorkerAction::Stop => {
                                        registry.remove(&thread_job_id);
                                        return;
                                    }
                                }
                            }
                        }
                    }

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

                    for ((candidate, path), output) in cache_ready.iter().zip(paths).zip(outputs)
                    {
                        match output {
                            Ok(output) => {
                                // Topic predictions are persisted independently from subject
                                // predictions. A detected person, animal, or vehicle is
                                // evidence about the contents of the frame, not a replacement
                                // for the photographer-facing topic selected by SigLIP2.
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
                let batch_action = control.with_job_lock(|job| {
                    if job.is_terminal() {
                        return SemanticWorkerAction::Stop;
                    }
                    progress.processed = progress.completed + progress.failed + progress.skipped;
                    progress.current_asset_id = None;
                    progress.current_path = None;
                    match control.current_signal() {
                        SemanticControlSignal::Cancel => {
                            cancel_semantic_job_locked(
                                job,
                                &repository,
                                &mut progress,
                                &emit,
                            );
                            SemanticWorkerAction::Stop
                        }
                        SemanticControlSignal::Paused => {
                            progress.status = "paused".into();
                            match persist_semantic_progress_and_emit(
                                &repository,
                                &progress,
                                &emit,
                            ) {
                                Ok(true) => SemanticWorkerAction::WaitForResume,
                                Ok(false) => {
                                    job.mark_terminal();
                                    SemanticWorkerAction::Stop
                                }
                                Err(error) => {
                                    publish_semantic_persistence_failure(
                                        job,
                                        &repository,
                                        &mut progress,
                                        error,
                                        &emit,
                                    );
                                    SemanticWorkerAction::Stop
                                }
                            }
                        }
                        SemanticControlSignal::Continue => {
                            progress.status = "running".into();
                            match persist_semantic_progress_and_emit(
                                &repository,
                                &progress,
                                &emit,
                            ) {
                                Ok(true) => SemanticWorkerAction::Continue,
                                Ok(false) => {
                                    job.mark_terminal();
                                    SemanticWorkerAction::Stop
                                }
                                Err(error) => {
                                    publish_semantic_persistence_failure(
                                        job,
                                        &repository,
                                        &mut progress,
                                        error,
                                        &emit,
                                    );
                                    SemanticWorkerAction::Stop
                                }
                            }
                        }
                    }
                });
                if batch_action == SemanticWorkerAction::Stop {
                    registry.remove(&thread_job_id);
                    return;
                }
            }

            loop {
                let finish_action = control.with_job_lock(|job| {
                    if job.is_terminal() {
                        return SemanticWorkerAction::Stop;
                    }
                    match control.current_signal() {
                        SemanticControlSignal::Cancel => {
                            cancel_semantic_job_locked(
                                job,
                                &repository,
                                &mut progress,
                                &emit,
                            );
                            SemanticWorkerAction::Stop
                        }
                        SemanticControlSignal::Paused => {
                            progress.status = "paused".into();
                            progress.current_asset_id = None;
                            progress.current_path = None;
                            match persist_semantic_progress_and_emit(
                                &repository,
                                &progress,
                                &emit,
                            ) {
                                Ok(true) => SemanticWorkerAction::WaitForResume,
                                Ok(false) => {
                                    job.mark_terminal();
                                    SemanticWorkerAction::Stop
                                }
                                Err(error) => {
                                    publish_semantic_persistence_failure(
                                        job,
                                        &repository,
                                        &mut progress,
                                        error,
                                        &emit,
                                    );
                                    SemanticWorkerAction::Stop
                                }
                            }
                        }
                        SemanticControlSignal::Continue => {
                            progress.status = "completed".into();
                            progress.current_asset_id = None;
                            progress.current_path = None;
                            if progress.failed == 0 {
                                progress.error = None;
                            }
                            job.mark_terminal();
                            if let Err(error) = persist_semantic_progress_and_emit(
                                &repository,
                                &progress,
                                &emit,
                            ) {
                                publish_semantic_persistence_failure(
                                    job,
                                    &repository,
                                    &mut progress,
                                    error,
                                    &emit,
                                );
                            }
                            SemanticWorkerAction::Stop
                        }
                    }
                });
                if finish_action != SemanticWorkerAction::WaitForResume {
                    break;
                }
                let _ = control.wait_until_runnable();
            }
            registry.remove(&thread_job_id);
        })
        .map_err(AppError::Io)?;
    Ok(())
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
    use std::sync::{Arc, Barrier};
    use std::time::Duration;

    use parking_lot::Mutex;
    use rusqlite::params;

    use super::*;
    use crate::db::Repository;
    use crate::semantic::{ModelMetadata, SemanticError, SemanticRuntimeStatus};
    use crate::subject::SubjectRuntimeStatus;

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

    struct BlockingClassifier {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    }

    impl SemanticClassifier for BlockingClassifier {
        fn metadata(&self) -> ModelMetadata {
            ModelMetadata {
                name: "blocking-test-model".into(),
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
            self.entered.wait();
            self.release.wait();
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
    fn batch_tail_persists_pause_before_final_finish() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let library_root = temp.path().join("library");
        std::fs::create_dir_all(&library_root).expect("library root");
        let (library_id, _) = repository
            .begin_scan(
                library_root.to_string_lossy().as_ref(),
                "semantic-pause-scan",
            )
            .expect("begin scan");
        let source_path = library_root.join("photo.jpg");
        let thumbnail_dir = temp.path().join("thumbnails");
        std::fs::create_dir_all(&thumbnail_dir).expect("thumbnail directory");
        let thumbnail_path = thumbnail_dir.join("asset-grid-640-v1.jpg");
        std::fs::write(&thumbnail_path, b"thumbnail fixture").expect("thumbnail fixture");

        let connection = repository.open_for_tests().expect("open database");
        connection
            .execute(
                "INSERT INTO assets(
                    library_id, asset_identity_key, absolute_path, relative_path, file_name,
                    extension, file_size, modified_at, fingerprint, width, height, orientation,
                    file_status, scan_status, analysis_status, first_seen_at, last_seen_at,
                    last_seen_scan
                 ) VALUES(?1, 'semantic-pause-asset', ?2, 'photo.jpg', 'photo.jpg', 'jpg',
                          100, 1, 'semantic-pause-fingerprint', 1000, 800, 1,
                          'present', 'indexed', 'completed', ?3, ?3, 1)",
                params![
                    library_id,
                    source_path.to_string_lossy().into_owned(),
                    "2026-09-14T00:00:00Z"
                ],
            )
            .expect("insert semantic asset");
        let asset_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO thumbnails(
                    asset_id, cache_path, spec, source_modified_at, source_size,
                    status, updated_at
                 ) VALUES(?1, ?2, ?3, 1, 100, 'ready', ?4)",
                params![
                    asset_id,
                    thumbnail_path.to_string_lossy().into_owned(),
                    crate::imaging::THUMBNAIL_SPEC,
                    "2026-09-14T00:00:00Z"
                ],
            )
            .expect("insert semantic thumbnail");
        connection
            .execute(
                "UPDATE libraries SET status='ready' WHERE id=?1",
                [library_id],
            )
            .expect("finish seed library");
        drop(connection);

        let job_id = "semantic-pause-job";
        let candidates = repository
            .create_semantic_job(job_id, library_id, false, None)
            .expect("create semantic job");
        assert_eq!(candidates.len(), 1);
        let registry = Arc::new(SemanticTaskRegistry::default());
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let classifier = Arc::new(BlockingClassifier {
            entered: entered.clone(),
            release: release.clone(),
        });
        let emitted_statuses = Arc::new(Mutex::new(Vec::<String>::new()));
        let emitted_statuses_for_worker = emitted_statuses.clone();
        spawn_semantic_job(
            repository.clone(),
            classifier,
            None,
            registry.clone(),
            job_id.into(),
            library_id,
            candidates,
            thumbnail_dir,
            1,
            ExecutionBackend::Cpu,
            move |progress| {
                emitted_statuses_for_worker.lock().push(progress.status);
            },
        )
        .expect("spawn semantic worker");

        entered.wait();
        assert!(registry.pause(job_id));
        release.wait();

        let paused = (0..200).find_map(|_| {
            let progress = repository
                .semantic_progress_by_job(job_id)
                .expect("read semantic progress");
            if progress
                .as_ref()
                .is_some_and(|progress| progress.status == "paused")
            {
                progress
            } else {
                std::thread::sleep(Duration::from_millis(5));
                None
            }
        });
        assert!(
            paused.is_some(),
            "worker did not persist paused batch progress"
        );
        assert!(
            emitted_statuses
                .lock()
                .iter()
                .any(|status| status == "paused")
        );

        assert!(registry.resume(job_id));
        let completed = (0..200).find_map(|_| {
            let progress = repository
                .semantic_progress_by_job(job_id)
                .expect("read completed semantic progress");
            if progress
                .as_ref()
                .is_some_and(|progress| progress.status == "completed")
            {
                progress
            } else {
                std::thread::sleep(Duration::from_millis(5));
                None
            }
        });
        assert!(completed.is_some(), "worker did not finish after resume");
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
    fn cpu_subject_batch_failure_recovers_each_thumbnail_individually() {
        let candidates = adaptive_candidates(4);
        let candidate_refs = candidates.iter().collect::<Vec<_>>();
        let paths = candidates
            .iter()
            .map(|candidate| candidate.analysis_path.clone())
            .collect::<Vec<_>>();
        let classifier = AdaptiveSubjectClassifier {
            calls: Mutex::new(Vec::new()),
            max_batch_size: 1,
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
        assert_eq!(
            calls.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![4, 2, 1, 1, 2, 1, 1]
        );
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
    fn topic_and_subject_results_keep_independent_layers() {
        let topic_output = SemanticAnalysisOutput {
            predictions: vec![crate::semantic::SemanticPrediction {
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
        let subject_output = SubjectAnalysisOutput {
            predictions: vec![crate::subject::SubjectPrediction {
                label_id: "single_person".into(),
                display_name: "单人".into(),
                category_group: "subject".into(),
                similarity: 0.91,
                threshold: 0.45,
            }],
        };

        let merged =
            crate::semantic::merge_topic_and_environment_predictions(Some(&topic_output), None);
        assert_eq!(
            merged
                .iter()
                .filter(|prediction| prediction.is_primary)
                .map(|prediction| prediction.label_id.as_str())
                .collect::<Vec<_>>(),
            vec!["photo_landscape"]
        );
        assert!(
            !merged
                .iter()
                .any(|prediction| prediction.category_group == "subject")
        );
        assert_eq!(subject_output.predictions[0].label_id, "single_person");
        assert_eq!(subject_output.predictions[0].category_group, "subject");
        assert!(
            !subject_output
                .predictions
                .iter()
                .any(|prediction| prediction.label_id == "photo_portrait")
        );
    }

    #[test]
    fn street_topic_and_animal_subject_keep_independent_layers() {
        let topic_output = SemanticAnalysisOutput {
            predictions: vec![crate::semantic::SemanticPrediction {
                label_id: "photo_street".into(),
                display_name: "街拍".into(),
                category_group: "scene".into(),
                similarity: 0.44,
                threshold: 0.18,
                is_primary: true,
            }],
            embedding: vec![],
            raw_similarities: vec![],
        };
        let subject_output = SubjectAnalysisOutput {
            predictions: vec![crate::subject::SubjectPrediction {
                label_id: "animal".into(),
                display_name: "动物".into(),
                category_group: "subject".into(),
                similarity: 0.88,
                threshold: 0.45,
            }],
        };

        let merged =
            crate::semantic::merge_topic_and_environment_predictions(Some(&topic_output), None);
        assert_eq!(
            merged
                .iter()
                .filter(|prediction| prediction.is_primary)
                .map(|prediction| prediction.label_id.as_str())
                .collect::<Vec<_>>(),
            vec!["photo_street"]
        );
        assert_eq!(subject_output.predictions[0].label_id, "animal");
        assert_eq!(subject_output.predictions[0].category_group, "subject");
        assert!(
            !subject_output
                .predictions
                .iter()
                .any(|prediction| prediction.label_id == "photo_wildlife")
        );
    }
}
