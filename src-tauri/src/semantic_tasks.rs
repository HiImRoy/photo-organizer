use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::{Repository, SemanticAssetCandidate};
use crate::error::{AppError, AppResult};
use crate::models::SemanticProgress;
use crate::semantic::{ExecutionBackend, SemanticAnalysisOutput, SemanticClassifier};
use crate::tasks::{SemanticControlSignal, SemanticTaskRegistry};

const SEMANTIC_BATCH_SIZE: usize = 8;

pub fn spawn_semantic_job<F>(
    repository: Repository,
    classifier: Arc<dyn SemanticClassifier>,
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
                for candidate in candidate_batch {
                    if let Err(error) =
                        repository.mark_semantic_item_running(&thread_job_id, candidate.id)
                    {
                        progress.failed += 1;
                        progress.error = Some(error.to_string());
                    } else {
                        ready.push(candidate);
                    }
                }

                if let Some(candidate) = ready.first() {
                    progress.status = "running".into();
                    progress.current_asset_id = Some(candidate.id);
                    progress.current_path =
                        Some(candidate.absolute_path.to_string_lossy().into_owned());
                    emit(progress.clone());

                    let paths = ready
                        .iter()
                        .map(|candidate| analysis_path(candidate))
                        .collect::<Vec<_>>();
                    let outputs = classify_batch_with_fallback(classifier.as_ref(), &ready, &paths);

                    for ((candidate, path), output) in ready.iter().zip(paths).zip(outputs) {
                        match output {
                            Ok(output) => match repository.save_semantic_result(
                                &thread_job_id,
                                candidate,
                                &output,
                            ) {
                                Ok(true) => progress.completed += 1,
                                Ok(false) => progress.skipped += 1,
                                Err(error) => {
                                    progress.failed += 1;
                                    progress.error = Some(error.to_string());
                                    let _ = repository.fail_semantic_item(
                                        &thread_job_id,
                                        candidate.id,
                                        &error.to_string(),
                                    );
                                }
                            },
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

fn analysis_path(candidate: &SemanticAssetCandidate) -> PathBuf {
    if candidate.analysis_path.is_file() {
        candidate.analysis_path.clone()
    } else {
        candidate.absolute_path.clone()
    }
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
    let mut paths = vec![path.to_path_buf()];
    if path != candidate.absolute_path {
        paths.push(candidate.absolute_path.clone());
    }

    let mut last_error = batch_error.to_string();
    for path in paths {
        match classifier.classify_batch(std::slice::from_ref(&path), ExecutionBackend::Cpu) {
            Ok(mut outputs) if outputs.len() == 1 => return Ok(outputs.remove(0)),
            Ok(outputs) => {
                last_error = format!(
                    "semantic model returned {} results for one image",
                    outputs.len()
                );
            }
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(format!(
        "{}; single-image fallback failed for {}: {}",
        batch_error,
        candidate.absolute_path.display(),
        last_error
    ))
}
