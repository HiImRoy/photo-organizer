use std::sync::Arc;

use crate::db::{Repository, SemanticAssetCandidate};
use crate::error::{AppError, AppResult};
use crate::models::SemanticProgress;
use crate::semantic::{ExecutionBackend, SemanticClassifier};
use crate::tasks::{SemanticControlSignal, SemanticTaskRegistry};

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

            for candidate in candidates {
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

                progress.status = "running".into();
                progress.current_asset_id = Some(candidate.id);
                progress.current_path =
                    Some(candidate.absolute_path.to_string_lossy().into_owned());
                if let Err(error) =
                    repository.mark_semantic_item_running(&thread_job_id, candidate.id)
                {
                    progress.failed += 1;
                    progress.processed = progress.completed + progress.failed + progress.skipped;
                    progress.error = Some(error.to_string());
                    let _ = repository.update_semantic_job_progress(&progress);
                    emit(progress.clone());
                    continue;
                }
                emit(progress.clone());

                match classifier.classify_batch(
                    std::slice::from_ref(&candidate.absolute_path),
                    ExecutionBackend::Cpu,
                ) {
                    Ok(mut outputs) if outputs.len() == 1 => {
                        match repository.save_semantic_result(
                            &thread_job_id,
                            &candidate,
                            &outputs.remove(0),
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
                        }
                    }
                    Ok(outputs) => {
                        let error = format!(
                            "semantic model returned {} results for one image",
                            outputs.len()
                        );
                        progress.failed += 1;
                        progress.error = Some(error.clone());
                        let _ = repository.fail_semantic_item(&thread_job_id, candidate.id, &error);
                    }
                    Err(error) => {
                        progress.failed += 1;
                        progress.error = Some(error.to_string());
                        let _ = repository.fail_semantic_item(
                            &thread_job_id,
                            candidate.id,
                            &error.to_string(),
                        );
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
