use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use parking_lot::RwLock;
use tauri::{Emitter, State};

use crate::classification::registry_descriptors;
use crate::db::{LibrarySourceRoot, Repository};
use crate::error::{AppError, AppResult};
use crate::models::{
    AssetDetail, AssetFilter, AssetPage, AssetSortField, CancelScanResponse, FolderSummary,
    LibrarySummary, OrganizationIssue, OrganizationPlan, OrganizationPlanRecord,
    OrganizationPlanRequest, ScanProgress, SemanticGroupSummary, SemanticProgress,
    SemanticTaskResponse, SortDirection, StartScanResponse, StartSemanticResponse,
};
use crate::organization;
use crate::paths::AppPaths;
use crate::scanner::{
    discover_import_source_roots, scan_library, scan_library_tree, validate_scan_root_with_app_data,
};
use crate::semantic::{
    Places365Classifier, SIGLIP2_ANALYSIS_VERSION, SIGLIP2_MODEL_NAME, SIGLIP2_MODEL_VERSION,
    SemanticClassifier, SemanticLabelDescriptor, SemanticRuntimeStatus, TopicModelKind,
    semantic_catalog,
};
use crate::semantic_tasks::spawn_semantic_job;
use crate::subject::{SubjectClassifier, SubjectModel, SubjectRuntimeStatus};
use crate::tasks::{SemanticTaskRegistry, SourceScanGuard, SourceScanRegistry, TaskRegistry};
use crate::workflow;
use crate::workflow::{
    CollectionDetail, CollectionSummary, DuplicateGroup, EditExportPlan, EditExportResult,
    EditRecipe, EditRollbackPlan, FaceFeatureStatus, LocalSearchResponse, SimilarAsset,
    SimilarityClusterResponse, WorkflowAsset,
};

pub struct AppState {
    pub repository: Repository,
    pub paths: AppPaths,
    pub tasks: Arc<TaskRegistry>,
    pub source_scans: Arc<SourceScanRegistry>,
    pub semantic_tasks: Arc<SemanticTaskRegistry>,
    pub semantic: Arc<RwLock<Arc<dyn SemanticClassifier>>>,
    pub subject: Arc<RwLock<Arc<dyn SubjectClassifier>>>,
}

impl AppState {
    pub fn new(
        repository: Repository,
        paths: AppPaths,
        semantic: Arc<dyn SemanticClassifier>,
        subject: Arc<dyn SubjectClassifier>,
    ) -> Self {
        Self {
            repository,
            paths,
            tasks: Arc::new(TaskRegistry::default()),
            source_scans: Arc::new(SourceScanRegistry::default()),
            semantic_tasks: Arc::new(SemanticTaskRegistry::default()),
            semantic: Arc::new(RwLock::new(semantic)),
            subject: Arc::new(RwLock::new(subject)),
        }
    }
}

/// Restore a model that was explicitly prepared in an earlier session.
///
/// Loading is intentionally performed after the Tauri state has been
/// installed and on a background thread. This keeps the WebView responsive on
/// cold start while allowing the next session to reuse the user's last model
/// choice without another button click.
pub fn restore_persisted_models(app: tauri::AppHandle, state: &AppState) {
    let active_model = match state.repository.active_semantic_model_key() {
        Ok(active_model) => active_model,
        Err(error) => {
            log::warn!("could not inspect persisted semantic model: {error}");
            return;
        }
    };
    let Some((name, version, analysis_version)) = active_model else {
        return;
    };
    if name != SIGLIP2_MODEL_NAME
        || version != SIGLIP2_MODEL_VERSION
        || analysis_version != SIGLIP2_ANALYSIS_VERSION
    {
        log::warn!(
            "migrating persisted semantic model {name} {version} {analysis_version} to the bundled SigLIP 2 model"
        );
    }

    let paths = state.paths.clone();
    let repository = state.repository.clone();
    let tasks = state.tasks.clone();
    let source_scans = state.source_scans.clone();
    let semantic_tasks = state.semantic_tasks.clone();
    let semantic = state.semantic.clone();
    let subject = state.subject.clone();
    let thread_app = app.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("restore-photo-models".into())
        .spawn(move || {
            let classifier = match Places365Classifier::load_with_topic_model(
                &paths.semantic_model_dir,
                &paths.siglip2_model_dir,
                &paths.onnx_runtime_path,
                TopicModelKind::Siglip2Base,
            ) {
                Ok(classifier) => classifier,
                Err(error) => {
                    log::warn!("could not restore persisted semantic model: {error}");
                    return;
                }
            };
            let semantic_status = classifier.status();
            if let Some(topic_model) = semantic_status.topic_model.as_ref() {
                let model_path = paths
                    .siglip2_model_dir
                    .join(crate::semantic::SIGLIP2_MODEL_FILE);
                let tokenizer_path = paths
                    .siglip2_model_dir
                    .join(crate::semantic::SIGLIP2_TOKENIZER_FILE);
                if let Err(error) = repository.register_active_semantic_model(
                    topic_model,
                    &model_path,
                    &tokenizer_path,
                    "https://huggingface.co/onnx-community/siglip2-base-patch16-224-ONNX",
                ) {
                    log::warn!("could not persist restored semantic model: {error}");
                }
            }
            *semantic.write() = Arc::new(classifier);
            if let Err(error) = thread_app.emit("semantic-status", semantic_status) {
                log::warn!("could not emit restored semantic status: {error}");
            }

            match SubjectModel::load(
                &paths.subject_model_dir,
                &paths.face_model_dir,
                &paths.onnx_runtime_path,
            ) {
                Ok(classifier) => {
                    let subject_status = classifier.status();
                    *subject.write() = Arc::new(classifier);
                    if let Err(error) = thread_app.emit("subject-status", subject_status) {
                        log::warn!("could not emit restored subject status: {error}");
                    }
                }
                Err(error) => {
                    log::warn!("could not restore persisted subject model: {error}");
                }
            }

            // Pending jobs must be resumed only after the classifier has been
            // restored; otherwise the old startup path would mark them as
            // failed against the unavailable placeholder.
            let resume_state = AppState {
                repository,
                paths,
                tasks,
                source_scans,
                semantic_tasks,
                semantic,
                subject,
            };
            resume_pending_semantic_jobs(thread_app, &resume_state);
        })
    {
        log::error!("could not start persisted model restore thread: {error}");
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
pub fn get_classification_registry() -> Vec<crate::classification::ClassificationFieldDescriptor> {
    registry_descriptors()
}

#[tauri::command]
pub fn get_asset_detail(asset_id: i64, state: State<'_, AppState>) -> Result<AssetDetail, String> {
    state
        .repository
        .get_asset_detail(asset_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn update_classification_override(
    asset_id: i64,
    field: String,
    value: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<AssetDetail, String> {
    state
        .repository
        .update_classification_override(asset_id, &field, value)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn update_asset_rating(
    asset_id: i64,
    rating: i64,
    state: State<'_, AppState>,
) -> Result<AssetDetail, String> {
    state
        .repository
        .update_asset_rating(asset_id, rating)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn update_asset_color_label(
    asset_id: i64,
    color_label: Option<String>,
    state: State<'_, AppState>,
) -> Result<AssetDetail, String> {
    state
        .repository
        .update_asset_color_label(asset_id, color_label.as_deref())
        .map_err(ipc_error)
}

#[tauri::command]
pub fn update_tag_override(
    asset_id: i64,
    tag_id: String,
    state: Option<String>,
    app_state: State<'_, AppState>,
) -> Result<AssetDetail, String> {
    app_state
        .repository
        .update_tag_override(asset_id, &tag_id, state.as_deref())
        .map_err(ipc_error)
}

#[tauri::command]
pub fn restore_auto_classification(
    asset_id: i64,
    field: Option<String>,
    state: State<'_, AppState>,
) -> Result<AssetDetail, String> {
    state
        .repository
        .restore_auto_classification(asset_id, field.as_deref())
        .map_err(ipc_error)
}

#[tauri::command]
pub fn batch_update_classification(
    asset_ids: Vec<i64>,
    field: String,
    value: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    state
        .repository
        .batch_update_classification(&asset_ids, &field, value)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn set_library_parent(
    library_id: i64,
    parent_library_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    state
        .repository
        .set_library_parent(library_id, parent_library_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn assign_asset_to_library(
    asset_id: i64,
    target_library_id: i64,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    state
        .repository
        .assign_asset_to_library(asset_id, target_library_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn start_scan(
    root_path: String,
    include_subfolders: Option<bool>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StartScanResponse, String> {
    let source_identity =
        validate_scan_root_with_app_data(Path::new(&root_path), &state.paths.data_dir)
            .map_err(ipc_error)?;
    let scan_guard = state
        .source_scans
        .try_acquire(&source_identity.identity_key)
        .ok_or_else(|| "该图库或其嵌套图库正在扫描，请稍后重试".to_owned())?;
    let structured_roots = if include_subfolders.unwrap_or(false) {
        let discovered =
            discover_import_source_roots(&source_identity.source_path).map_err(ipc_error)?;
        Some(
            state
                .repository
                .ensure_library_source_roots(&discovered)
                .map_err(ipc_error)?,
        )
    } else {
        None
    };
    let library_id_hint = structured_roots.as_ref().and_then(|roots| {
        roots
            .iter()
            .find(|root| root.identity_key == source_identity.identity_key)
            .map(|root| root.library_id)
    });
    let (task_id, cancellation) = state.tasks.create();
    let response = StartScanResponse {
        task_id: task_id.clone(),
    };
    spawn_scan_task(ScanTask {
        app,
        repository: state.repository.clone(),
        paths: state.paths.clone(),
        tasks: state.tasks.clone(),
        task_id: task_id.clone(),
        cancellation,
        root: source_identity.source_path,
        scan_guard,
        library_id_hint,
        structured_roots,
    })
    .map_err(ipc_error)
    .inspect_err(|_| {
        state.tasks.remove(&task_id);
    })?;

    Ok(response)
}

#[tauri::command]
pub fn rescan_library(
    library_id: i64,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StartScanResponse, String> {
    let source = state
        .repository
        .library_source_root(library_id)
        .map_err(ipc_error)?
        .ok_or_else(|| format!("library {library_id} does not exist"))?;
    let source_identity =
        validate_scan_root_with_app_data(&source.source_path, &state.paths.data_dir)
            .map_err(ipc_error)?;
    let scan_guard = state
        .source_scans
        .try_acquire(&source_identity.identity_key)
        .ok_or_else(|| "该图库或其嵌套图库正在扫描，请稍后重试".to_owned())?;
    let mut structured_roots = state
        .repository
        .nested_source_roots(library_id)
        .map_err(ipc_error)?;
    structured_roots.insert(
        0,
        LibrarySourceRoot {
            library_id,
            source_path: source_identity.source_path.clone(),
            identity_key: source_identity.identity_key.clone(),
        },
    );
    let (task_id, cancellation) = state.tasks.create();
    let response = StartScanResponse {
        task_id: task_id.clone(),
    };
    spawn_scan_task(ScanTask {
        app,
        repository: state.repository.clone(),
        paths: state.paths.clone(),
        tasks: state.tasks.clone(),
        task_id: task_id.clone(),
        cancellation,
        root: source_identity.source_path,
        scan_guard,
        library_id_hint: Some(library_id),
        structured_roots: Some(structured_roots),
    })
    .map_err(ipc_error)
    .inspect_err(|_| {
        state.tasks.remove(&task_id);
    })?;
    Ok(response)
}

struct ScanTask {
    app: tauri::AppHandle,
    repository: Repository,
    paths: AppPaths,
    tasks: Arc<TaskRegistry>,
    task_id: String,
    cancellation: Arc<std::sync::atomic::AtomicBool>,
    root: PathBuf,
    scan_guard: SourceScanGuard,
    library_id_hint: Option<i64>,
    structured_roots: Option<Vec<LibrarySourceRoot>>,
}

fn spawn_scan_task(task: ScanTask) -> AppResult<()> {
    let ScanTask {
        app,
        repository,
        paths,
        tasks,
        task_id,
        cancellation,
        root,
        scan_guard,
        library_id_hint,
        structured_roots,
    } = task;
    let thread_task_id = task_id.clone();
    std::thread::Builder::new()
        .name(format!("scan-{thread_task_id}"))
        .spawn(move || {
            let emit = |progress| {
                if let Err(error) = app.emit("scan-progress", progress) {
                    log::warn!("could not emit scan progress: {error}");
                }
            };
            let result = match structured_roots {
                Some(targets) => scan_library_tree(
                    &repository,
                    &paths.thumbnail_dir,
                    &root,
                    &thread_task_id,
                    &cancellation,
                    targets,
                    emit,
                ),
                None => scan_library(
                    &repository,
                    &paths.thumbnail_dir,
                    &root,
                    &thread_task_id,
                    &cancellation,
                    emit,
                ),
            };
            if let Err(error) = result {
                let root_string = root.to_string_lossy().into_owned();
                let library_id = library_id_hint.or_else(|| {
                    repository.list_libraries().ok().and_then(|libraries| {
                        libraries
                            .into_iter()
                            .find(|library| library.source_path == root_string)
                            .map(|library| library.id)
                    })
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
            drop(scan_guard);
        })
        .map(|_| ())
        .map_err(AppError::Io)
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
pub async fn get_preview_data_url(
    asset_id: i64,
    tier: Option<String>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let repository = state.repository.clone();
    let paths = state.paths.clone();
    let tier = tier.unwrap_or_else(|| "screen".into());
    let max_width = max_width.unwrap_or(2560).clamp(640, 4096);
    let max_height = max_height.unwrap_or(1600).clamp(480, 4096);
    tauri::async_runtime::spawn_blocking(move || {
        load_preview_data_url(&repository, &paths, asset_id, &tier, max_width, max_height)
    })
    .await
    .map_err(|error| format!("preview task failed: {error}"))?
    .map_err(ipc_error)
}

#[tauri::command]
pub fn remove_library(library_id: i64, state: State<'_, AppState>) -> Result<bool, String> {
    let _scan_guard = state
        .repository
        .library_source_root(library_id)
        .map_err(ipc_error)?
        .map(|source| {
            state
                .source_scans
                .try_acquire(&source.identity_key)
                .ok_or_else(|| "该图库或其嵌套图库正在扫描，请稍后重试".to_owned())
        })
        .transpose()?;
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
    let result = state
        .repository
        .remove_library_with_reconciliation(library_id)
        .map_err(ipc_error)?;
    if result.removed {
        for (asset_id, fingerprint, thumbnail_path) in result.removed_cache_entries {
            if let Some(path) = thumbnail_path {
                remove_cache_file_if_safe(&state.paths.thumbnail_dir, &path);
            }
            remove_cache_entries(
                &state.paths.preview_dir,
                &format!("{asset_id}-{fingerprint}-"),
            );
        }
    }
    Ok(result.removed)
}

#[tauri::command]
pub fn open_library_in_explorer(library_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let source = state
        .repository
        .library_source_root(library_id)
        .map_err(ipc_error)?
        .ok_or_else(|| format!("library {library_id} does not exist"))?;
    let root_path = validate_scan_root_with_app_data(&source.source_path, &state.paths.data_dir)
        .map_err(ipc_error)?
        .source_path;
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
pub fn prepare_semantic_model(
    topic_model: Option<String>,
    state: State<'_, AppState>,
) -> Result<SemanticRuntimeStatus, String> {
    let topic_model = crate::semantic::TopicModelKind::parse(
        topic_model
            .as_deref()
            .unwrap_or(crate::semantic::DEFAULT_TOPIC_MODEL.id()),
    )
    .ok_or_else(|| "不支持的题材模型，当前 MVP 仅支持 SigLIP 2 Base。".to_string())?;
    let classifier = Places365Classifier::load_with_topic_model(
        &state.paths.semantic_model_dir,
        &state.paths.siglip2_model_dir,
        &state.paths.onnx_runtime_path,
        topic_model,
    )
    .map_err(|error| error.to_string())?;
    let status = classifier.status();
    let selected_topic_model = status
        .topic_model
        .as_ref()
        .ok_or_else(|| "题材模型未能装载，请检查模型资源和 ONNX Runtime。".to_string())?;
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
    let model_path = state
        .paths
        .siglip2_model_dir
        .join(crate::semantic::SIGLIP2_MODEL_FILE);
    let tokenizer_path = state
        .paths
        .siglip2_model_dir
        .join(crate::semantic::SIGLIP2_TOKENIZER_FILE);
    let source_url = "https://huggingface.co/onnx-community/siglip2-base-patch16-224-ONNX";
    state
        .repository
        .register_active_semantic_model(
            selected_topic_model,
            &model_path,
            &tokenizer_path,
            source_url,
        )
        .map_err(ipc_error)?;
    let classifier: Arc<dyn SemanticClassifier> = Arc::new(classifier);
    *state.semantic.write() = classifier;
    Ok(status)
}

#[tauri::command]
pub fn get_subject_status(state: State<'_, AppState>) -> Result<SubjectRuntimeStatus, String> {
    Ok(state.subject.read().status())
}

#[tauri::command]
pub fn prepare_subject_model(state: State<'_, AppState>) -> Result<SubjectRuntimeStatus, String> {
    let classifier = SubjectModel::load(
        &state.paths.subject_model_dir,
        &state.paths.face_model_dir,
        &state.paths.onnx_runtime_path,
    )
    .map_err(|error| error.to_string())?;
    let classifier: Arc<dyn SubjectClassifier> = Arc::new(classifier);
    let status = classifier.status();
    *state.subject.write() = classifier;
    Ok(status)
}

#[tauri::command]
pub fn clear_subject_data(state: State<'_, AppState>) -> Result<u64, String> {
    state.repository.clear_subject_data().map_err(ipc_error)
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
    let subject_model = state.subject.read().metadata();
    let subject_model = subject_model.installed.then_some(subject_model);
    let semantic_model = classifier.result_metadata();
    let candidates = state
        .repository
        .create_semantic_job_for_assets_with_semantic_model(
            &job_id,
            library_id,
            &asset_ids,
            &semantic_model,
            subject_model.as_ref(),
        )
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

#[tauri::command]
pub fn list_favorite_asset_ids(
    library_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<i64>, String> {
    workflow::list_favorite_asset_ids(&state.repository, library_id).map_err(ipc_error)
}

#[tauri::command]
pub fn list_favorite_assets(
    library_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<WorkflowAsset>, String> {
    workflow::list_favorite_assets(&state.repository, library_id).map_err(ipc_error)
}

#[tauri::command]
pub fn set_asset_favorite(
    asset_id: i64,
    favorite: bool,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    workflow::set_favorite(&state.repository, asset_id, favorite).map_err(ipc_error)
}

#[tauri::command]
pub fn list_collections(state: State<'_, AppState>) -> Result<Vec<CollectionSummary>, String> {
    workflow::list_collections(&state.repository).map_err(ipc_error)
}

#[tauri::command]
pub fn create_collection(
    name: String,
    description: Option<String>,
    state: State<'_, AppState>,
) -> Result<CollectionSummary, String> {
    workflow::create_collection(
        &state.repository,
        &name,
        description.as_deref().unwrap_or_default(),
    )
    .map_err(ipc_error)
}

#[tauri::command]
pub fn delete_collection(collection_id: i64, state: State<'_, AppState>) -> Result<bool, String> {
    workflow::delete_collection(&state.repository, collection_id).map_err(ipc_error)
}

#[tauri::command]
pub fn get_collection(
    collection_id: i64,
    state: State<'_, AppState>,
) -> Result<CollectionDetail, String> {
    workflow::get_collection(&state.repository, collection_id).map_err(ipc_error)
}

#[tauri::command]
pub fn add_assets_to_collection(
    collection_id: i64,
    asset_ids: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<CollectionSummary, String> {
    workflow::add_assets_to_collection(&state.repository, collection_id, &asset_ids)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn remove_assets_from_collection(
    collection_id: i64,
    asset_ids: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<CollectionSummary, String> {
    workflow::remove_assets_from_collection(&state.repository, collection_id, &asset_ids)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn list_duplicate_groups(
    library_id: i64,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<DuplicateGroup>, String> {
    workflow::list_duplicate_groups(&state.repository, library_id, limit.unwrap_or(100))
        .map_err(ipc_error)
}

#[tauri::command]
pub async fn search_local_images(
    library_id: i64,
    query: String,
    limit: Option<u32>,
    minimum_similarity: Option<f32>,
    state: State<'_, AppState>,
) -> Result<LocalSearchResponse, String> {
    let repository = state.repository.clone();
    let classifier = state.semantic.read().clone();
    tauri::async_runtime::spawn_blocking(move || {
        workflow::search_by_text(
            &repository,
            &classifier,
            library_id,
            &query,
            limit.unwrap_or(80),
            minimum_similarity.unwrap_or(0.05),
        )
    })
    .await
    .map_err(|error| format!("local search task failed: {error}"))?
    .map_err(ipc_error)
}

#[tauri::command]
pub async fn find_similar_assets(
    library_id: i64,
    asset_id: i64,
    limit: Option<u32>,
    minimum_similarity: Option<f32>,
    state: State<'_, AppState>,
) -> Result<Vec<SimilarAsset>, String> {
    let repository = state.repository.clone();
    tauri::async_runtime::spawn_blocking(move || {
        workflow::find_similar_assets(
            &repository,
            library_id,
            asset_id,
            limit.unwrap_or(80),
            minimum_similarity.unwrap_or(0.7),
        )
    })
    .await
    .map_err(|error| format!("similarity task failed: {error}"))?
    .map_err(ipc_error)
}

#[tauri::command]
pub async fn build_similarity_clusters(
    library_id: i64,
    threshold: Option<f32>,
    state: State<'_, AppState>,
) -> Result<SimilarityClusterResponse, String> {
    let repository = state.repository.clone();
    tauri::async_runtime::spawn_blocking(move || {
        workflow::build_similarity_clusters(&repository, library_id, threshold.unwrap_or(0.92))
    })
    .await
    .map_err(|error| format!("clustering task failed: {error}"))?
    .map_err(ipc_error)
}

#[tauri::command]
pub fn get_face_feature_status(state: State<'_, AppState>) -> Result<FaceFeatureStatus, String> {
    workflow::face_feature_status(&state.repository).map_err(ipc_error)
}

#[tauri::command]
pub fn clear_face_data(state: State<'_, AppState>) -> Result<FaceFeatureStatus, String> {
    workflow::clear_face_data(&state.repository).map_err(ipc_error)
}

#[tauri::command]
pub async fn render_edit_preview(
    asset_id: i64,
    recipe: EditRecipe,
    max_width: Option<u32>,
    max_height: Option<u32>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let repository = state.repository.clone();
    tauri::async_runtime::spawn_blocking(move || {
        workflow::render_edit_preview(
            &repository,
            asset_id,
            &recipe,
            max_width.unwrap_or(1_920),
            max_height.unwrap_or(1_200),
        )
    })
    .await
    .map_err(|error| format!("edit preview task failed: {error}"))?
    .map_err(ipc_error)
}

#[tauri::command]
pub fn preview_edit_export(
    asset_id: i64,
    target_path: String,
    recipe: EditRecipe,
    state: State<'_, AppState>,
) -> Result<EditExportPlan, String> {
    workflow::preview_edit_export(
        &state.repository,
        asset_id,
        Path::new(&target_path),
        &recipe,
    )
    .map_err(ipc_error)
}

#[tauri::command]
pub async fn execute_edit_export(
    plan_id: String,
    state: State<'_, AppState>,
) -> Result<EditExportResult, String> {
    let repository = state.repository.clone();
    tauri::async_runtime::spawn_blocking(move || {
        workflow::execute_edit_export(&repository, &plan_id)
    })
    .await
    .map_err(|error| format!("edit export task failed: {error}"))?
    .map_err(ipc_error)
}

#[tauri::command]
pub fn preview_edit_rollback(
    plan_id: String,
    state: State<'_, AppState>,
) -> Result<EditRollbackPlan, String> {
    workflow::preview_edit_rollback(&state.repository, &plan_id).map_err(ipc_error)
}

#[tauri::command]
pub async fn execute_edit_rollback(
    plan_id: String,
    state: State<'_, AppState>,
) -> Result<EditExportResult, String> {
    let repository = state.repository.clone();
    tauri::async_runtime::spawn_blocking(move || {
        workflow::execute_edit_rollback(&repository, &plan_id)
    })
    .await
    .map_err(|error| format!("edit rollback task failed: {error}"))?
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
    let subject_model = state.subject.read().metadata();
    let subject_model = subject_model.installed.then_some(subject_model);
    let semantic_model = classifier.result_metadata();
    let candidates = state
        .repository
        .create_semantic_job_with_semantic_model(
            &job_id,
            library_id,
            force,
            only_asset_id,
            &semantic_model,
            subject_model.as_ref(),
        )
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
    let subject_classifier = {
        let subject = state.subject.read().clone();
        subject.metadata().installed.then_some(subject)
    };
    spawn_semantic_job(
        state.repository.clone(),
        classifier,
        subject_classifier,
        state.semantic_tasks.clone(),
        job_id,
        library_id,
        candidates,
        state.paths.thumbnail_dir.clone(),
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
        let image =
            crate::imaging::load_oriented_bounded_image(&source, max_width.max(max_height))?;
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
