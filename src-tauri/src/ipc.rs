use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use parking_lot::RwLock;
use tauri::{Emitter, State};

use crate::db::Repository;
use crate::error::{AppError, AppResult};
use crate::models::{
    AssetFilter, AssetPage, AssetSortField, CancelScanResponse, FolderSummary, LibrarySummary,
    OrganizationIssue, OrganizationPlan, OrganizationPlanRecord, OrganizationPlanRequest,
    ScanProgress, SemanticGroupSummary, SemanticProgress, SemanticTaskResponse, SortDirection,
    StartScanResponse, StartSemanticResponse,
};
use crate::organization;
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
pub fn get_preview_data_url(
    asset_id: i64,
    tier: Option<String>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    load_preview_data_url(
        &state.repository,
        &state.paths,
        asset_id,
        tier.as_deref().unwrap_or("screen"),
        max_width.unwrap_or(2560).clamp(640, 4096),
        max_height.unwrap_or(1600).clamp(480, 4096),
    )
    .map_err(ipc_error)
}

#[tauri::command]
pub fn remove_library(library_id: i64, state: State<'_, AppState>) -> Result<bool, String> {
    let cache_entries = state
        .repository
        .library_cache_entries(library_id)
        .map_err(ipc_error)?;
    let jobs = state
        .repository
        .active_job_ids_for_library(library_id)
        .map_err(ipc_error)?;
    for (job_id, job_type) in jobs {
        if job_type == "scan_and_basic_analysis" {
            state.tasks.cancel(&job_id);
            let _ = state.repository.cancel_scan(&job_id, library_id);
        } else {
            state.semantic_tasks.cancel(&job_id);
            let _ = state.repository.cancel_semantic_job(&job_id);
        }
    }

    let removed = state
        .repository
        .remove_library(library_id)
        .map_err(ipc_error)?;
    if removed {
        for (asset_id, fingerprint, thumbnail_path) in cache_entries {
            if let Some(path) = thumbnail_path {
                remove_cache_file_if_safe(&state.paths.thumbnail_dir, &path);
            }
            remove_cache_entries(
                &state.paths.preview_dir,
                &format!("{asset_id}-{fingerprint}-"),
            );
        }
    }
    Ok(removed)
}

#[tauri::command]
pub fn open_library_in_explorer(root_path: String) -> Result<(), String> {
    if !Path::new(&root_path).is_dir() {
        return Err("原始目录不可访问".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(&root_path)
        .spawn()
        .map(|_| ())
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
pub fn start_semantic_analysis_selected(
    library_id: i64,
    asset_ids: Vec<i64>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StartSemanticResponse, String> {
    if asset_ids.is_empty() {
        return Err("请先选择至少一张图片。".into());
    }
    let classifier = state.semantic.read().clone();
    if !classifier.metadata().installed {
        return Err("本地语义模型尚未就绪，请先准备模型。".into());
    }
    let job_id = uuid::Uuid::new_v4().to_string();
    let candidates = state
        .repository
        .create_semantic_job_for_assets(&job_id, library_id, &asset_ids)
        .map_err(ipc_error)?;
    spawn_with_app(&state, app, job_id.clone(), library_id, candidates)?;
    Ok(StartSemanticResponse { job_id })
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

#[tauri::command]
pub fn validate_organization_rules(request: OrganizationPlanRequest) -> Vec<OrganizationIssue> {
    organization::validate_rules(&request.rules)
}

#[tauri::command]
pub fn preview_organization_plan(
    request: OrganizationPlanRequest,
    state: State<'_, AppState>,
) -> Result<OrganizationPlan, String> {
    let library = state
        .repository
        .list_libraries()
        .map_err(ipc_error)?
        .into_iter()
        .find(|library| library.id == request.library_id)
        .ok_or_else(|| format!("library {} not found", request.library_id))?;
    let filter = match request.scope {
        crate::models::OrganizationScope::Filtered => request.filter.clone(),
        _ => AssetFilter::default(),
    };
    let selected = match request.scope {
        crate::models::OrganizationScope::Selected => Some(request.selected_asset_ids.as_slice()),
        _ => None,
    };
    let assets = state
        .repository
        .list_assets_for_organization(request.library_id, &filter, selected)
        .map_err(ipc_error)?;
    let plan = organization::build_plan(&request, &library.root_path, assets).map_err(ipc_error)?;
    state
        .repository
        .save_organization_plan(&plan)
        .map_err(ipc_error)?;
    Ok(plan)
}

#[tauri::command]
pub fn get_organization_plan(
    plan_id: String,
    state: State<'_, AppState>,
) -> Result<Option<OrganizationPlanRecord>, String> {
    state
        .repository
        .get_organization_plan(&plan_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn list_organization_issues(
    plan_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<OrganizationIssue>, String> {
    state
        .repository
        .list_organization_issues(&plan_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn export_organization_manifest(
    plan: OrganizationPlan,
    output_path: String,
    format: String,
) -> Result<(), String> {
    organization::export_manifest(&plan, Path::new(&output_path), &format).map_err(ipc_error)
}

#[tauri::command]
pub fn discard_organization_plan(
    plan_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .repository
        .delete_organization_plan(&plan_id)
        .map_err(ipc_error)
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

fn load_preview_data_url(
    repository: &Repository,
    paths: &AppPaths,
    asset_id: i64,
    tier: &str,
    max_width: u32,
    max_height: u32,
) -> AppResult<String> {
    let (source_path, fingerprint) = repository.asset_source(asset_id)?;
    let source = source_path.canonicalize()?;
    let metadata = fs::metadata(&source)?;
    if !metadata.is_file() {
        return Err(AppError::NotFound(format!("source for asset {asset_id}")));
    }

    if tier == "original" {
        if metadata.len() > 96 * 1024 * 1024 {
            return Err(AppError::InvalidArgument(
                "original preview exceeds the 96 MiB IPC safety limit".into(),
            ));
        }
        let bytes = fs::read(&source)?;
        return Ok(format!(
            "{}{}",
            mime_for_path(&source),
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ));
    }

    let cache_name = format!("{asset_id}-{fingerprint}-{}-{}.jpg", max_width, max_height);
    let cache_path = paths.preview_dir.join(cache_name);
    let bytes = if cache_path.is_file() {
        fs::read(&cache_path)?
    } else {
        let image = crate::imaging::load_oriented_image(&source)?;
        let preview = image.resize(max_width, max_height, FilterType::Lanczos3);
        let mut encoded = Vec::new();
        JpegEncoder::new_with_quality(&mut encoded, 91).encode_image(&preview)?;
        let temp_path = cache_path.with_extension("tmp");
        fs::write(&temp_path, &encoded)?;
        match fs::rename(&temp_path, &cache_path) {
            Ok(()) => encoded,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::read(&cache_path)?
            }
            Err(error) => return Err(AppError::Io(error)),
        }
    };
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn mime_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "data:image/png;base64,",
        Some("webp") => "data:image/webp;base64,",
        _ => "data:image/jpeg;base64,",
    }
}

fn remove_cache_entries(directory: &Path, prefix: &str) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let matches = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(prefix));
        if matches {
            let _ = fs::remove_file(path);
        }
    }
}

fn remove_cache_file_if_safe(root: &Path, path: &Path) {
    let Ok(root) = canonical_or_absolute(root) else {
        return;
    };
    let Ok(target) = canonical_or_absolute(path) else {
        return;
    };
    if target.starts_with(&root) {
        let _ = fs::remove_file(target);
    }
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
