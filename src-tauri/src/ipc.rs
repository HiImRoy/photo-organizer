use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use parking_lot::RwLock;
use tauri::{Emitter, State};

use crate::db::Repository;
use crate::error::{AppError, AppResult};
use crate::models::{
    AssetFilter, AssetPage, AssetSortField, CancelScanResponse, FolderSummary, LibrarySummary,
    ScanProgress, SemanticGroupSummary, SemanticProgress, SemanticTaskResponse, SortDirection,
    StartScanResponse, StartSemanticResponse,
};
use crate::paths::AppPaths;
use crate::scanner::{scan_library, validate_scan_root};
use crate::semantic::{
    SemanticClassifier, SemanticLabelDescriptor, SemanticRuntimeStatus, TinyClipClassifier,
    semantic_catalog,
};
use crate::semantic_tasks::spawn_semantic_job;
use crate::tasks::{SemanticTaskRegistry, TaskRegistry};

pub struct AppState {
    pub repository: Repository,
    pub paths: AppPaths,
    pub tasks: Arc<TaskRegistry>,
    pub semantic_tasks: Arc<SemanticTaskRegistry>,
    pub semantic: Arc<RwLock<Arc<dyn SemanticClassifier>>>,
}

impl AppState {
    pub fn new(
        repository: Repository,
        paths: AppPaths,
        semantic: Arc<dyn SemanticClassifier>,
    ) -> Self {
        Self {
            repository,
            paths,
            tasks: Arc::new(TaskRegistry::default()),
            semantic_tasks: Arc::new(SemanticTaskRegistry::default()),
            semantic: Arc::new(RwLock::new(semantic)),
        }
    }
}

#[tauri::command]
pub fn list_libraries(state: State<'_, AppState>) -> Result<Vec<LibrarySummary>, String> {
    state.repository.list_libraries().map_err(ipc_error)
}

#[tauri::command]
pub fn list_assets(
    library_id: i64,
    sort: Option<String>,
    direction: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    filter: Option<AssetFilter>,
    state: State<'_, AppState>,
) -> Result<AssetPage, String> {
    let sort_value = sort.unwrap_or_else(|| "file_name".into());
    let sort = AssetSortField::parse(&sort_value)
        .ok_or_else(|| format!("invalid sort field: {sort_value}"))?;
    let direction_value = direction.unwrap_or_else(|| "asc".into());
    let direction = SortDirection::parse(&direction_value)
        .ok_or_else(|| format!("invalid sort direction: {direction_value}"))?;
    state
        .repository
        .list_assets(
            library_id,
            sort,
            direction,
            page.unwrap_or(1).max(1),
            page_size.unwrap_or(200),
            &filter.unwrap_or_default(),
        )
        .map_err(ipc_error)
}

#[tauri::command]
pub fn start_scan(
    root_path: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StartScanResponse, String> {
    let canonical_root = validate_scan_root(Path::new(&root_path)).map_err(ipc_error)?;
    let (task_id, cancellation) = state.tasks.create();
    let response = StartScanResponse {
        task_id: task_id.clone(),
    };
    let repository = state.repository.clone();
    let paths = state.paths.clone();
    let tasks = state.tasks.clone();
    let thread_task_id = task_id.clone();

    std::thread::Builder::new()
        .name(format!("scan-{thread_task_id}"))
        .spawn(move || {
            let result = scan_library(
                &repository,
                &paths.thumbnail_dir,
                &canonical_root,
                &thread_task_id,
                &cancellation,
                |progress| {
                    if let Err(error) = app.emit("scan-progress", progress) {
                        log::warn!("could not emit scan progress: {error}");
                    }
                },
            );
            if let Err(error) = result {
                let root = canonical_root.to_string_lossy();
                let library_id = repository.list_libraries().ok().and_then(|libraries| {
                    libraries
                        .into_iter()
                        .find(|library| library.root_path == root)
                        .map(|library| library.id)
                });
                if let Err(database_error) =
                    repository.fail_scan(&thread_task_id, library_id, &error.to_string())
                {
                    log::error!("could not persist failed scan: {database_error}");
                }
                let mut progress = ScanProgress::starting(&thread_task_id);
                progress.library_id = library_id;
                progress.status = "failed".into();
                progress.stage = "failed".into();
                progress.error = Some(error.to_string());
                let _ = app.emit("scan-progress", progress);
                log::error!("scan {thread_task_id} failed: {error}");
            }
            tasks.remove(&thread_task_id);
        })
        .map_err(|error| {
            state.tasks.remove(&task_id);
            ipc_error(AppError::Io(error))
        })?;

    Ok(response)
}

#[tauri::command]
pub fn cancel_scan(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<CancelScanResponse, String> {
    let accepted = state.tasks.cancel(&task_id);
    Ok(CancelScanResponse { task_id, accepted })
}

#[tauri::command]
pub fn get_thumbnail_data_url(asset_id: i64, state: State<'_, AppState>) -> Result<String, String> {
    load_thumbnail_data_url(&state.repository, &state.paths.thumbnail_dir, asset_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn get_semantic_status(state: State<'_, AppState>) -> Result<SemanticRuntimeStatus, String> {
    Ok(state.semantic.read().status())
}

#[tauri::command]
pub fn prepare_semantic_model(state: State<'_, AppState>) -> Result<SemanticRuntimeStatus, String> {
    let classifier = TinyClipClassifier::load(
        &state.paths.semantic_model_dir,
        &state.paths.onnx_runtime_path,
    )
    .map_err(|error| error.to_string())?;
    state
        .repository
        .register_semantic_model(
            &state
                .paths
                .semantic_model_dir
                .join(crate::semantic::MODEL_FILE),
            &state
                .paths
                .semantic_model_dir
                .join(crate::semantic::TOKENIZER_FILE),
        )
        .map_err(ipc_error)?;
    let classifier: Arc<dyn SemanticClassifier> = Arc::new(classifier);
    let status = classifier.status();
    *state.semantic.write() = classifier;
    Ok(status)
}

#[tauri::command]
pub fn get_semantic_catalog() -> Vec<SemanticLabelDescriptor> {
    semantic_catalog()
}

#[tauri::command]
pub fn list_library_folders(
    library_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<FolderSummary>, String> {
    state
        .repository
        .list_library_folders(library_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn list_semantic_groups(
    library_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<SemanticGroupSummary>, String> {
    state
        .repository
        .list_semantic_groups(library_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn get_semantic_progress(
    library_id: i64,
    state: State<'_, AppState>,
) -> Result<Option<SemanticProgress>, String> {
    state
        .repository
        .latest_semantic_progress(library_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn start_semantic_analysis(
    library_id: i64,
    force: Option<bool>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StartSemanticResponse, String> {
    start_semantic_job(library_id, force.unwrap_or(false), None, app, &state)
}

#[tauri::command]
pub fn reanalyze_asset(
    library_id: i64,
    asset_id: i64,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StartSemanticResponse, String> {
    start_semantic_job(library_id, true, Some(asset_id), app, &state)
}

#[tauri::command]
pub fn pause_semantic_analysis(
    job_id: String,
    state: State<'_, AppState>,
) -> Result<SemanticTaskResponse, String> {
    let accepted = state.semantic_tasks.pause(&job_id);
    if accepted {
        state
            .repository
            .set_semantic_job_status(&job_id, "paused")
            .map_err(ipc_error)?;
    }
    Ok(SemanticTaskResponse { job_id, accepted })
}

#[tauri::command]
pub fn resume_semantic_analysis(
    job_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SemanticTaskResponse, String> {
    if state.semantic_tasks.resume(&job_id) {
        state
            .repository
            .set_semantic_job_status(&job_id, "running")
            .map_err(ipc_error)?;
        return Ok(SemanticTaskResponse {
            job_id,
            accepted: true,
        });
    }
    let progress = state
        .repository
        .semantic_progress_by_job(&job_id)
        .map_err(ipc_error)?;
    let Some(progress) = progress else {
        return Ok(SemanticTaskResponse {
            job_id,
            accepted: false,
        });
    };
    if !matches!(
        progress.status.as_str(),
        "queued" | "paused" | "interrupted"
    ) {
        return Ok(SemanticTaskResponse {
            job_id,
            accepted: false,
        });
    }
    let candidates = state
        .repository
        .pending_semantic_candidates(&job_id)
        .map_err(ipc_error)?;
    spawn_with_app(&state, app, job_id.clone(), progress.library_id, candidates)?;
    Ok(SemanticTaskResponse {
        job_id,
        accepted: true,
    })
}

#[tauri::command]
pub fn cancel_semantic_analysis(
    job_id: String,
    state: State<'_, AppState>,
) -> Result<SemanticTaskResponse, String> {
    let accepted = if state.semantic_tasks.cancel(&job_id) {
        state
            .repository
            .set_semantic_job_status(&job_id, "cancelling")
            .map_err(ipc_error)?;
        true
    } else if state
        .repository
        .semantic_progress_by_job(&job_id)
        .map_err(ipc_error)?
        .is_some()
    {
        state
            .repository
            .cancel_semantic_job(&job_id)
            .map_err(ipc_error)?;
        true
    } else {
        false
    };
    Ok(SemanticTaskResponse { job_id, accepted })
}

pub fn resume_pending_semantic_jobs(app: tauri::AppHandle, state: &AppState) {
    match state.repository.recoverable_semantic_jobs() {
        Ok(jobs) => {
            for (job_id, library_id) in jobs {
                match state.repository.pending_semantic_candidates(&job_id) {
                    Ok(candidates) => {
                        if let Err(error) = spawn_with_app(
                            state,
                            app.clone(),
                            job_id.clone(),
                            library_id,
                            candidates,
                        ) {
                            log::error!("could not resume semantic job {job_id}: {error}");
                        }
                    }
                    Err(error) => {
                        log::error!("could not load semantic job {job_id}: {error}");
                    }
                }
            }
        }
        Err(error) => log::error!("could not load recoverable semantic jobs: {error}"),
    }
}

fn start_semantic_job(
    library_id: i64,
    force: bool,
    only_asset_id: Option<i64>,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<StartSemanticResponse, String> {
    let classifier = state.semantic.read().clone();
    if !classifier.metadata().installed {
        return Err("本地语义模型尚未就绪，请先准备模型。".into());
    }
    let job_id = uuid::Uuid::new_v4().to_string();
    let candidates = state
        .repository
        .create_semantic_job(&job_id, library_id, force, only_asset_id)
        .map_err(ipc_error)?;
    spawn_with_app(state, app, job_id.clone(), library_id, candidates)?;
    Ok(StartSemanticResponse { job_id })
}

fn spawn_with_app(
    state: &AppState,
    app: tauri::AppHandle,
    job_id: String,
    library_id: i64,
    candidates: Vec<crate::db::SemanticAssetCandidate>,
) -> Result<(), String> {
    let classifier = state.semantic.read().clone();
    spawn_semantic_job(
        state.repository.clone(),
        classifier,
        state.semantic_tasks.clone(),
        job_id,
        library_id,
        candidates,
        move |progress| {
            if let Err(error) = app.emit("semantic-progress", progress) {
                log::warn!("could not emit semantic progress: {error}");
            }
        },
    )
    .map_err(ipc_error)
}

fn load_thumbnail_data_url(
    repository: &Repository,
    thumbnail_root: &Path,
    asset_id: i64,
) -> AppResult<String> {
    let registered_path = repository.thumbnail_path(asset_id)?;
    let root = canonical_or_absolute(thumbnail_root)?;
    let thumbnail = registered_path.canonicalize()?;
    if !thumbnail.starts_with(&root) {
        return Err(AppError::UnsafePath(thumbnail));
    }
    let metadata = fs::metadata(&thumbnail)?;
    if metadata.len() > 20 * 1024 * 1024 {
        return Err(AppError::InvalidArgument(
            "thumbnail exceeds the 20 MiB IPC safety limit".into(),
        ));
    }
    let bytes = fs::read(&thumbnail)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn canonical_or_absolute(path: &Path) -> AppResult<PathBuf> {
    path.canonicalize().or_else(|_| {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            std::env::current_dir()
                .map(|current| current.join(path))
                .map_err(AppError::from)
        }
    })
}

fn ipc_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
