use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use parking_lot::{Mutex, RwLock};
use tauri::{Emitter, State};

use crate::classification::registry_descriptors;
use crate::db::{LibrarySourceRoot, Repository};
use crate::error::{AppError, AppResult};
use crate::models::{
    AssetDetail, AssetFilter, AssetPage, AssetQuery, AssetSortField, CancelScanResponse,
    FolderSummary, LibrarySummary, OrganizationIssue, OrganizationPlan, OrganizationPlanRecord,
    OrganizationPlanRequest, ScanProgress, SemanticGroupSummary, SemanticProgress,
    SemanticTaskResponse, SortDirection, StartScanResponse, StartSemanticResponse,
};
use crate::organization;
use crate::paths::AppPaths;
use crate::scanner::{
    ScanOptions, discover_import_source_roots, scan_library_tree_with_options,
    scan_library_with_options, validate_scan_root_with_app_data,
};
use crate::semantic::{
    ExecutionBackend, Places365Classifier, SIGLIP2_ANALYSIS_VERSION, SIGLIP2_MODEL_NAME,
    SIGLIP2_MODEL_VERSION, SemanticClassifier, SemanticLabelDescriptor, SemanticRuntimeStatus,
    TopicModelKind, default_topic_model_metadata, semantic_catalog,
};
use crate::semantic_tasks::{SEMANTIC_BATCH_SIZE, spawn_semantic_job};
use crate::subject::{SubjectClassifier, SubjectModel, SubjectRuntimeStatus};
use crate::tasks::{
    SemanticControlSignal, SemanticTaskRegistry, SourceScanGuard, SourceScanRegistry, TaskRegistry,
};
use crate::workflow;
use crate::workflow::{
    BrowseNode, CollectionDeleteMode, CollectionDetail, CollectionMembershipMutation,
    CollectionSummary, DuplicateGroup, EditExportPlan, EditExportResult, EditRecipe,
    EditRollbackPlan, FaceFeatureStatus, LocalSearchResponse, SimilarAsset,
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
    semantic_status: Arc<RwLock<SemanticRuntimeStatus>>,
    subject_status: Arc<RwLock<SubjectRuntimeStatus>>,
    semantic_preparation: Arc<Mutex<()>>,
    subject_preparation: Arc<Mutex<()>>,
    startup_preparation_started: AtomicBool,
    pub gpu_provider: Arc<RwLock<Option<crate::gpu::GpuProviderStatus>>>,
}

impl AppState {
    pub fn new(
        repository: Repository,
        paths: AppPaths,
        semantic: Arc<dyn SemanticClassifier>,
        subject: Arc<dyn SubjectClassifier>,
    ) -> Self {
        let semantic_status = semantic_loading_status(semantic.status());
        let subject_status = subject_loading_status(subject.status());
        let model_preparation = Arc::new(Mutex::new(()));
        Self {
            repository,
            paths,
            tasks: Arc::new(TaskRegistry::default()),
            source_scans: Arc::new(SourceScanRegistry::default()),
            semantic_tasks: Arc::new(SemanticTaskRegistry::default()),
            semantic: Arc::new(RwLock::new(semantic)),
            subject: Arc::new(RwLock::new(subject)),
            semantic_status: Arc::new(RwLock::new(semantic_status)),
            subject_status: Arc::new(RwLock::new(subject_status)),
            semantic_preparation: model_preparation.clone(),
            subject_preparation: model_preparation,
            startup_preparation_started: AtomicBool::new(false),
            gpu_provider: Arc::new(RwLock::new(None)),
        }
    }

    fn claim_startup_preparation(&self) -> bool {
        !self
            .startup_preparation_started
            .swap(true, Ordering::AcqRel)
    }
}

fn semantic_loading_status(mut status: SemanticRuntimeStatus) -> SemanticRuntimeStatus {
    status.status = "loading".into();
    status.message = "正在后台准备本地语义模型；图库可继续使用。".into();
    status
}

fn semantic_error_status(
    mut status: SemanticRuntimeStatus,
    message: impl std::fmt::Display,
) -> SemanticRuntimeStatus {
    status.status = "error".into();
    status.message = format!("本地语义模型准备失败：{message}");
    status
}

fn subject_loading_status(mut status: SubjectRuntimeStatus) -> SubjectRuntimeStatus {
    status.status = "loading".into();
    status.message = "正在后台准备本地主体模型；图库可继续使用。".into();
    status
}

fn subject_error_status(
    mut status: SubjectRuntimeStatus,
    message: impl std::fmt::Display,
) -> SubjectRuntimeStatus {
    status.status = "error".into();
    status.message = format!("本地主体模型准备失败：{message}");
    status
}

fn current_semantic_status(state: &AppState) -> SemanticRuntimeStatus {
    observe_semantic_runtime(
        &state.repository,
        &state.paths,
        &state.semantic,
        &state.semantic_status,
        &state.gpu_provider,
    )
    .0
}

fn current_subject_status(state: &AppState) -> SubjectRuntimeStatus {
    observe_subject_runtime(&state.subject, &state.subject_status, &state.gpu_provider).0
}

struct SemanticPreparationContext<'a> {
    paths: &'a AppPaths,
    repository: &'a Repository,
    classifier_slot: &'a Arc<RwLock<Arc<dyn SemanticClassifier>>>,
    published_status: &'a Arc<RwLock<SemanticRuntimeStatus>>,
    preparation: &'a Arc<Mutex<()>>,
    gpu_provider: &'a Arc<RwLock<Option<crate::gpu::GpuProviderStatus>>>,
}

fn observe_semantic_runtime(
    repository: &Repository,
    paths: &AppPaths,
    classifier_slot: &Arc<RwLock<Arc<dyn SemanticClassifier>>>,
    published_status: &Arc<RwLock<SemanticRuntimeStatus>>,
    gpu_provider: &Arc<RwLock<Option<crate::gpu::GpuProviderStatus>>>,
) -> (SemanticRuntimeStatus, bool) {
    let classifier = classifier_slot.read().clone();
    let live = classifier.status();
    let mut published = published_status.write();
    let previous = published.clone();

    if previous.status == "loading"
        || (previous.status == "error" && previous.selected_backend == live.selected_backend)
    {
        return (previous, false);
    }

    let fell_back_to_cpu = previous
        .selected_backend
        .is_some_and(ExecutionBackend::is_gpu)
        && live.selected_backend == Some(ExecutionBackend::Cpu);
    let already_fell_back = live.selected_backend == Some(ExecutionBackend::Cpu)
        && previous.message.contains("DirectML")
        && previous.message.contains("回退 CPU");
    let mut observed = live;
    if fell_back_to_cpu {
        observed.message = format!(
            "DirectML 推理失败，已回退 CPU；后续语义任务固定使用 CPU。{}",
            observed.message
        );
    } else if already_fell_back {
        observed.message = if observed.status == previous.status {
            previous.message.clone()
        } else {
            format!(
                "DirectML 已回退 CPU；后续语义任务固定使用 CPU。{}",
                observed.message
            )
        };
    }
    let changed = *published != observed;
    *published = observed.clone();
    drop(published);

    if fell_back_to_cpu {
        let message = format!(
            "语义模型运行时 DirectML 失败，已回退 CPU：{}",
            observed.message
        );
        *gpu_provider.write() = Some(crate::gpu::GpuProviderStatus {
            id: "directml".into(),
            state: "error".into(),
            message,
        });
        if let Err(error) = persist_semantic_backend_after_fallback(repository, paths, &observed) {
            log::error!("could not persist semantic CPU fallback: {error}");
            observed
                .message
                .push_str(&format!(" 活动模型数据库 backend 同步失败：{error}"));
            *published_status.write() = observed.clone();
        }
    }
    (observed, changed)
}

fn observe_subject_runtime(
    classifier_slot: &Arc<RwLock<Arc<dyn SubjectClassifier>>>,
    published_status: &Arc<RwLock<SubjectRuntimeStatus>>,
    gpu_provider: &Arc<RwLock<Option<crate::gpu::GpuProviderStatus>>>,
) -> (SubjectRuntimeStatus, bool) {
    let classifier = classifier_slot.read().clone();
    let live = classifier.status();
    let mut published = published_status.write();
    let previous = published.clone();

    if previous.status == "loading"
        || (previous.status == "error" && previous.selected_backend == live.selected_backend)
    {
        return (previous, false);
    }

    let fell_back_to_cpu = previous
        .selected_backend
        .is_some_and(ExecutionBackend::is_gpu)
        && live.selected_backend == Some(ExecutionBackend::Cpu);
    let observed = live;
    let changed = *published != observed;
    *published = observed.clone();
    drop(published);

    if fell_back_to_cpu {
        *gpu_provider.write() = Some(crate::gpu::GpuProviderStatus {
            id: "directml".into(),
            state: "error".into(),
            message: format!(
                "主体模型 DirectML 执行失败，已回退 CPU：{}",
                observed.message
            ),
        });
    }
    (observed, changed)
}

fn persist_semantic_backend_after_fallback(
    repository: &Repository,
    paths: &AppPaths,
    status: &SemanticRuntimeStatus,
) -> Result<(), String> {
    let active_model = match repository.active_semantic_model_key() {
        Ok(active_model) => active_model,
        Err(error) => return Err(format!("could not read active model: {error}")),
    };
    let fallback_metadata = default_topic_model_metadata();
    let metadata = match status.topic_model.as_ref() {
        Some(metadata)
            if metadata.name == SIGLIP2_MODEL_NAME
                && metadata.version == SIGLIP2_MODEL_VERSION
                && metadata.analysis_version == SIGLIP2_ANALYSIS_VERSION =>
        {
            metadata.clone()
        }
        Some(_) => {
            return Err("loaded topic profile is not the bundled SigLIP 2 Base".into());
        }
        None => {
            let active_is_bundled_base =
                active_model
                    .as_ref()
                    .is_some_and(|(name, version, analysis_version, _)| {
                        name == SIGLIP2_MODEL_NAME
                            && version == SIGLIP2_MODEL_VERSION
                            && analysis_version == SIGLIP2_ANALYSIS_VERSION
                    });
            if !active_is_bundled_base {
                return Err("could not identify the active bundled topic model".into());
            }
            fallback_metadata
        }
    };

    if let Some((name, version, analysis_version, backend)) = active_model.as_ref() {
        if name != &metadata.name
            || version != &metadata.version
            || analysis_version != &metadata.analysis_version
        {
            return Err("active semantic profile changed during CPU fallback".into());
        }
        if backend.as_str() == ExecutionBackend::Cpu.id() {
            return Ok(());
        }
    }

    let model_path = paths
        .siglip2_model_dir
        .join(crate::semantic::SIGLIP2_MODEL_FILE);
    let tokenizer_path = paths
        .siglip2_model_dir
        .join(crate::semantic::SIGLIP2_TOKENIZER_FILE);
    repository
        .register_active_semantic_model(
            &metadata,
            &model_path,
            &tokenizer_path,
            "https://huggingface.co/onnx-community/siglip2-base-patch16-224-ONNX",
            ExecutionBackend::Cpu,
        )
        .map_err(|error| format!("could not write active model backend: {error}"))
}

fn backend_satisfies_request(
    selected_backend: Option<ExecutionBackend>,
    requested_backend: ExecutionBackend,
    previous_message: &str,
) -> bool {
    selected_backend == Some(requested_backend)
        || (requested_backend.is_gpu()
            && selected_backend == Some(ExecutionBackend::Cpu)
            && previous_message.contains("DirectML")
            && previous_message.contains("回退 CPU"))
}

fn set_semantic_failure(
    classifier: &Arc<RwLock<Arc<dyn SemanticClassifier>>>,
    published_status: &Arc<RwLock<SemanticRuntimeStatus>>,
    message: impl std::fmt::Display,
) -> SemanticRuntimeStatus {
    let status = semantic_error_status(classifier.read().status(), message);
    *published_status.write() = status.clone();
    status
}

fn set_subject_failure(
    classifier: &Arc<RwLock<Arc<dyn SubjectClassifier>>>,
    published_status: &Arc<RwLock<SubjectRuntimeStatus>>,
    message: impl std::fmt::Display,
) -> SubjectRuntimeStatus {
    let status = subject_error_status(classifier.read().status(), message);
    *published_status.write() = status.clone();
    status
}

fn prepare_semantic_runtime(
    context: SemanticPreparationContext<'_>,
    topic_model: TopicModelKind,
    requested_backend: ExecutionBackend,
) -> Result<SemanticRuntimeStatus, String> {
    let SemanticPreparationContext {
        paths,
        repository,
        classifier_slot,
        published_status,
        preparation,
        gpu_provider,
    } = context;
    if topic_model != TopicModelKind::Siglip2Base {
        return Err(format!(
            "{} 尚无 bundled 模型资源；当前只打包了 SigLIP 2 Base，不会尝试从 Base 目录装载该 profile。",
            topic_model.display_name()
        ));
    }

    let _guard = preparation.lock();
    let current = classifier_slot.read().clone();
    let current_status = current.status();
    let previous_status = published_status.read().clone();
    let current_topic_matches = current_status
        .topic_model
        .as_ref()
        .is_some_and(|model| model.name == topic_model.model_name());
    if current_status.status == "ready"
        && current.metadata().installed
        && current_topic_matches
        && backend_satisfies_request(
            current_status.selected_backend,
            requested_backend,
            &previous_status.message,
        )
    {
        let mut status = previous_status;
        if status.status == "loading" || status.status == "error" {
            status = current_status;
        }
        *published_status.write() = status.clone();
        return Ok(status);
    }

    *published_status.write() = semantic_loading_status(current_status.clone());
    let loaded = Places365Classifier::load_with_topic_model_with_backend(
        &paths.semantic_model_dir,
        &paths.siglip2_model_dir,
        &paths.onnx_runtime_path,
        topic_model,
        requested_backend,
    );
    let (classifier, mut status) = match loaded {
        Ok(classifier) => {
            let status = classifier.status();
            (classifier, status)
        }
        Err(error) if requested_backend.is_gpu() => {
            log::warn!(
                "DirectML semantic model initialization failed; falling back to CPU: {error}"
            );
            *gpu_provider.write() = Some(crate::gpu::GpuProviderStatus {
                id: "directml".into(),
                state: "error".into(),
                message: format!("模型会话自检失败，已回退 CPU：{error}"),
            });
            if current.metadata().installed
                && current_status.selected_backend == Some(ExecutionBackend::Cpu)
            {
                let mut status = current_status;
                status.message = format!("DirectML 初始化失败，继续使用已就绪的 CPU 模型：{error}");
                *published_status.write() = status.clone();
                return Ok(status);
            }
            match Places365Classifier::load_with_topic_model_with_backend(
                &paths.semantic_model_dir,
                &paths.siglip2_model_dir,
                &paths.onnx_runtime_path,
                topic_model,
                ExecutionBackend::Cpu,
            ) {
                Ok(classifier) => {
                    let mut status = classifier.status();
                    status.message = format!("DirectML 初始化失败，已回退 CPU：{error}");
                    (classifier, status)
                }
                Err(fallback_error) => {
                    let message = format!(
                        "{} DirectML 初始化失败（{error}），CPU 回退也失败：{fallback_error}",
                        topic_model.display_name()
                    );
                    set_semantic_failure(classifier_slot, published_status, &message);
                    return Err(message);
                }
            }
        }
        Err(error) => {
            let message = format!("{} 加载失败：{error}", topic_model.display_name());
            set_semantic_failure(classifier_slot, published_status, &message);
            return Err(message);
        }
    };

    if status
        .topic_model
        .as_ref()
        .is_none_or(|model| model.name != topic_model.model_name())
    {
        let message = format!(
            "{} 未能装载；请检查 bundled 模型资源和 ONNX Runtime。{}",
            topic_model.display_name(),
            status.message
        );
        set_semantic_failure(classifier_slot, published_status, &message);
        return Err(message);
    }

    if requested_backend.is_gpu() && status.selected_backend == Some(requested_backend) {
        *gpu_provider.write() = Some(crate::gpu::GpuProviderStatus {
            id: "directml".into(),
            state: "ready".into(),
            message: "DirectML 模型会话自检通过。".into(),
        });
    }

    if let Err(error) = repository.register_semantic_model(
        &paths.semantic_model_dir.join(crate::semantic::MODEL_FILE),
        &paths
            .semantic_model_dir
            .join(crate::semantic::TOKENIZER_FILE),
    ) {
        log::warn!("could not persist Places365 model metadata: {error}");
        status
            .message
            .push_str(&format!(" 模型元数据未写入数据库：{error}"));
    }
    if let Some(topic_metadata) = status.topic_model.as_ref() {
        let model_path = paths
            .siglip2_model_dir
            .join(crate::semantic::SIGLIP2_MODEL_FILE);
        let tokenizer_path = paths
            .siglip2_model_dir
            .join(crate::semantic::SIGLIP2_TOKENIZER_FILE);
        if let Err(error) = repository.register_active_semantic_model(
            topic_metadata,
            &model_path,
            &tokenizer_path,
            "https://huggingface.co/onnx-community/siglip2-base-patch16-224-ONNX",
            status.selected_backend.unwrap_or(ExecutionBackend::Cpu),
        ) {
            log::warn!("could not persist active semantic model: {error}");
            status
                .message
                .push_str(&format!(" 活动模型状态未写入数据库：{error}"));
        }
    }

    *classifier_slot.write() = Arc::new(classifier);
    *published_status.write() = status.clone();
    Ok(status)
}

fn prepare_subject_runtime(
    paths: &AppPaths,
    classifier_slot: &Arc<RwLock<Arc<dyn SubjectClassifier>>>,
    published_status: &Arc<RwLock<SubjectRuntimeStatus>>,
    preparation: &Arc<Mutex<()>>,
    gpu_provider: &Arc<RwLock<Option<crate::gpu::GpuProviderStatus>>>,
    requested_backend: ExecutionBackend,
) -> Result<SubjectRuntimeStatus, String> {
    let _guard = preparation.lock();
    let current = classifier_slot.read().clone();
    let current_status = current.status();
    let previous_status = published_status.read().clone();
    if matches!(current_status.status.as_str(), "ready" | "partial")
        && current.metadata().installed
        && backend_satisfies_request(
            current_status.selected_backend,
            requested_backend,
            &previous_status.message,
        )
    {
        let mut status = previous_status;
        if status.status == "loading" || status.status == "error" {
            status = current_status;
        }
        *published_status.write() = status.clone();
        return Ok(status);
    }

    *published_status.write() = subject_loading_status(current_status.clone());
    let loaded = SubjectModel::load_with_backend(
        &paths.subject_model_dir,
        &paths.face_model_dir,
        &paths.onnx_runtime_path,
        requested_backend,
    );
    let (classifier, status) = match loaded {
        Ok(classifier) => {
            let status = classifier.status();
            (classifier, status)
        }
        Err(error) if requested_backend.is_gpu() => {
            log::warn!(
                "DirectML subject model initialization failed; falling back to CPU: {error}"
            );
            *gpu_provider.write() = Some(crate::gpu::GpuProviderStatus {
                id: "directml".into(),
                state: "error".into(),
                message: format!("主体模型会话自检失败，已回退 CPU：{error}"),
            });
            if current.metadata().installed
                && current_status.selected_backend == Some(ExecutionBackend::Cpu)
            {
                let mut status = current_status;
                status.message =
                    format!("DirectML 初始化失败，继续使用已就绪的 CPU 主体模型：{error}");
                *published_status.write() = status.clone();
                return Ok(status);
            }
            match SubjectModel::load_with_backend(
                &paths.subject_model_dir,
                &paths.face_model_dir,
                &paths.onnx_runtime_path,
                ExecutionBackend::Cpu,
            ) {
                Ok(classifier) => {
                    let mut status = classifier.status();
                    status.message = format!("DirectML 初始化失败，主体模型已回退 CPU：{error}");
                    (classifier, status)
                }
                Err(fallback_error) => {
                    let message = format!(
                        "PicoDet 主体模型 DirectML 初始化失败（{error}），CPU 回退也失败：{fallback_error}"
                    );
                    set_subject_failure(classifier_slot, published_status, &message);
                    return Err(message);
                }
            }
        }
        Err(error) => {
            let message = format!("PicoDet 主体模型加载失败：{error}");
            set_subject_failure(classifier_slot, published_status, &message);
            return Err(message);
        }
    };

    if !status.model.installed {
        let message = format!("主体模型未能装载：{}", status.message);
        set_subject_failure(classifier_slot, published_status, &message);
        return Err(message);
    }
    if requested_backend.is_gpu() && status.selected_backend == Some(requested_backend) {
        *gpu_provider.write() = Some(crate::gpu::GpuProviderStatus {
            id: "directml".into(),
            state: "ready".into(),
            message: "DirectML 主体模型会话自检通过。".into(),
        });
    }
    *classifier_slot.write() = Arc::new(classifier);
    *published_status.write() = status.clone();
    Ok(status)
}

/// Load the bundled classifiers after the window can be shown. Both models
/// use the same persisted backend choice when available; a new installation
/// starts on CPU. No image analysis is started here.
pub fn prepare_bundled_models(app: tauri::AppHandle, state: &AppState) {
    if !state.claim_startup_preparation() {
        return;
    }
    let (requested_backend, active_model) = match state.repository.active_semantic_model_key() {
        Ok(active_model) => {
            let backend = active_model
                .as_ref()
                .map(|(_, _, _, backend)| ExecutionBackend::parse(Some(backend)))
                .unwrap_or(ExecutionBackend::Cpu);
            (backend, active_model)
        }
        Err(error) => {
            log::warn!("could not inspect persisted semantic model; starting on CPU: {error}");
            (ExecutionBackend::Cpu, None)
        }
    };
    if let Some((name, version, analysis_version, _)) = active_model
        && (name != SIGLIP2_MODEL_NAME
            || version != SIGLIP2_MODEL_VERSION
            || analysis_version != SIGLIP2_ANALYSIS_VERSION)
    {
        log::warn!(
            "migrating persisted semantic model {name} {version} {analysis_version} to bundled SigLIP 2 Base"
        );
    }

    let paths = state.paths.clone();
    let repository = state.repository.clone();
    let tasks = state.tasks.clone();
    let source_scans = state.source_scans.clone();
    let semantic_tasks = state.semantic_tasks.clone();
    let semantic = state.semantic.clone();
    let subject = state.subject.clone();
    let semantic_status = state.semantic_status.clone();
    let subject_status = state.subject_status.clone();
    let semantic_preparation = state.semantic_preparation.clone();
    let subject_preparation = state.subject_preparation.clone();
    let gpu_provider = state.gpu_provider.clone();
    let thread_app = app.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("prepare-bundled-photo-models".into())
        .spawn(move || {
            let semantic_result = prepare_semantic_runtime(
                SemanticPreparationContext {
                    paths: &paths,
                    repository: &repository,
                    classifier_slot: &semantic,
                    published_status: &semantic_status,
                    preparation: &semantic_preparation,
                    gpu_provider: &gpu_provider,
                },
                TopicModelKind::Siglip2Base,
                requested_backend,
            );
            let semantic_state = match semantic_result {
                Ok(status) => status,
                Err(error) => {
                    log::warn!("could not prepare bundled semantic model: {error}");
                    semantic_status.read().clone()
                }
            };
            if let Err(error) = thread_app.emit("semantic-status", semantic_state) {
                log::warn!("could not emit semantic model status: {error}");
            }

            let subject_result = prepare_subject_runtime(
                &paths,
                &subject,
                &subject_status,
                &subject_preparation,
                &gpu_provider,
                requested_backend,
            );
            let subject_state = match subject_result {
                Ok(status) => status,
                Err(error) => {
                    log::warn!("could not prepare bundled subject model: {error}");
                    subject_status.read().clone()
                }
            };
            if let Err(error) = thread_app.emit("subject-status", subject_state) {
                log::warn!("could not emit subject model status: {error}");
            }

            let live_semantic_status = semantic.read().status();
            if !resume_pending_semantic_jobs_if_ready(&live_semantic_status, move || {
                let resume_state = AppState {
                    repository,
                    paths,
                    tasks,
                    source_scans,
                    semantic_tasks,
                    semantic,
                    subject,
                    semantic_status,
                    subject_status,
                    semantic_preparation,
                    subject_preparation,
                    startup_preparation_started: AtomicBool::new(true),
                    gpu_provider,
                };
                resume_pending_semantic_jobs(thread_app, &resume_state);
            }) {
                log::warn!(
                    "leaving queued semantic jobs recoverable because the semantic topic model is not ready: {}",
                    live_semantic_status.message
                );
            }
        })
    {
        let semantic_state = set_semantic_failure(
            &state.semantic,
            &state.semantic_status,
            format!("无法启动模型后台准备线程：{error}"),
        );
        let subject_state = set_subject_failure(
            &state.subject,
            &state.subject_status,
            format!("无法启动模型后台准备线程：{error}"),
        );
        log::error!("could not start bundled model preparation thread: {error}");
        if let Err(emit_error) = app.emit("semantic-status", semantic_state) {
            log::warn!("could not emit semantic model error status: {emit_error}");
        }
        if let Err(emit_error) = app.emit("subject-status", subject_state) {
            log::warn!("could not emit subject model error status: {emit_error}");
        }
    }
}

#[tauri::command]
pub fn list_libraries(state: State<'_, AppState>) -> Result<Vec<LibrarySummary>, String> {
    state.repository.list_libraries().map_err(ipc_error)
}

#[tauri::command]
pub async fn list_assets(
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
    let repository = state.repository.clone();
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(200);
    let filter = filter.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        repository.list_assets(library_id, sort, direction, page, page_size, &filter)
    })
    .await
    .map_err(|error| format!("asset query task failed: {error}"))?
    .map_err(ipc_error)
}

#[tauri::command]
pub async fn query_assets(
    query: AssetQuery,
    state: State<'_, AppState>,
) -> Result<AssetPage, String> {
    let repository = state.repository.clone();
    tauri::async_runtime::spawn_blocking(move || repository.query_assets(&query))
        .await
        .map_err(|error| format!("asset query task failed: {error}"))?
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
    include_subfolder_images: Option<bool>,
    import_worker_count: Option<usize>,
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
    let include_subfolder_images = include_subfolder_images.unwrap_or(true);
    let structured_roots = if include_subfolders.unwrap_or(false) && include_subfolder_images {
        let discovered =
            discover_import_source_roots(&source_identity.source_path).map_err(ipc_error)?;
        Some(
            state
                .repository
                .ensure_library_source_roots(&discovered, include_subfolder_images)
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
        scan_options: ScanOptions {
            include_subfolder_images,
            import_worker_count,
        },
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
    import_worker_count: Option<usize>,
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
    let include_subfolder_images = state
        .repository
        .library_include_subfolder_images(library_id)
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
        scan_options: ScanOptions {
            include_subfolder_images,
            import_worker_count,
        },
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
    scan_options: ScanOptions,
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
        scan_options,
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
                Some(targets) => scan_library_tree_with_options(
                    &repository,
                    &paths.thumbnail_dir,
                    &root,
                    &thread_task_id,
                    &cancellation,
                    targets,
                    scan_options,
                    emit,
                ),
                None => scan_library_with_options(
                    &repository,
                    &paths.thumbnail_dir,
                    &root,
                    &thread_task_id,
                    &cancellation,
                    scan_options,
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
pub async fn remove_library(library_id: i64, state: State<'_, AppState>) -> Result<bool, String> {
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
    let repository = state.repository.clone();
    let paths = state.paths.clone();
    let tasks = state.tasks.clone();
    let semantic_tasks = state.semantic_tasks.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let jobs = repository
            .active_job_ids_for_library(library_id)
            .map_err(ipc_error)?;
        for (job_id, job_type) in jobs {
            if job_type == "scan_and_basic_analysis" {
                tasks.cancel(&job_id);
                let _ = repository.cancel_scan(&job_id, library_id);
            } else {
                let _ = request_semantic_cancel(&repository, &semantic_tasks, &job_id);
            }
        }
        let result = repository
            .remove_library_with_reconciliation(library_id)
            .map_err(ipc_error)?;
        if result.removed {
            for path in &result.removed_thumbnail_cache_paths {
                remove_cache_file_if_safe(&paths.data_dir, "thumbnails", path);
            }
            remove_preview_cache_entries(&paths.data_dir, &result.removed_preview_asset_ids);
        }
        Ok(result.removed)
    })
    .await
    .map_err(|error| format!("remove library task failed: {error}"))?
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
    Ok(current_semantic_status(&state))
}

#[tauri::command]
pub fn get_gpu_capabilities(state: State<'_, AppState>) -> crate::gpu::GpuCapabilities {
    let mut capabilities =
        crate::gpu::detect_gpu_capabilities_with_runtime(&state.paths.onnx_runtime_path);
    if capabilities.dedicated_gpu_available {
        if let Some(cached) = state.gpu_provider.read().clone() {
            capabilities.directml = cached;
        } else {
            *state.gpu_provider.write() = Some(capabilities.directml.clone());
        }
    }
    capabilities
}

#[tauri::command]
pub fn prepare_semantic_model(
    topic_model: Option<String>,
    backend: Option<String>,
    state: State<'_, AppState>,
) -> Result<SemanticRuntimeStatus, String> {
    let topic_model = crate::semantic::TopicModelKind::parse(
        topic_model
            .as_deref()
            .unwrap_or(crate::semantic::DEFAULT_TOPIC_MODEL.id()),
    )
    .ok_or_else(|| "不支持的题材模型，当前 MVP 仅支持 SigLIP 2 Base。".to_string())?;
    let requested_backend = crate::semantic::ExecutionBackend::parse(backend.as_deref());
    prepare_semantic_runtime(
        SemanticPreparationContext {
            paths: &state.paths,
            repository: &state.repository,
            classifier_slot: &state.semantic,
            published_status: &state.semantic_status,
            preparation: &state.semantic_preparation,
            gpu_provider: &state.gpu_provider,
        },
        topic_model,
        requested_backend,
    )
}

#[tauri::command]
pub fn get_subject_status(state: State<'_, AppState>) -> Result<SubjectRuntimeStatus, String> {
    Ok(current_subject_status(&state))
}

#[tauri::command]
pub fn prepare_subject_model(
    backend: Option<String>,
    state: State<'_, AppState>,
) -> Result<SubjectRuntimeStatus, String> {
    let requested_backend = crate::semantic::ExecutionBackend::parse(backend.as_deref());
    prepare_subject_runtime(
        &state.paths,
        &state.subject,
        &state.subject_status,
        &state.subject_preparation,
        &state.gpu_provider,
        requested_backend,
    )
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
    batch_size: Option<usize>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StartSemanticResponse, String> {
    start_semantic_job(
        library_id,
        force.unwrap_or(false),
        None,
        batch_size,
        app,
        &state,
    )
}

#[tauri::command]
pub fn start_semantic_analysis_selected(
    library_id: i64,
    asset_ids: Vec<i64>,
    batch_size: Option<usize>,
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
    spawn_with_app(
        &state,
        app,
        job_id.clone(),
        library_id,
        candidates,
        batch_size.unwrap_or(SEMANTIC_BATCH_SIZE),
    )?;
    Ok(StartSemanticResponse { job_id })
}

#[tauri::command]
pub fn reanalyze_asset(
    library_id: i64,
    asset_id: i64,
    batch_size: Option<usize>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StartSemanticResponse, String> {
    start_semantic_job(library_id, true, Some(asset_id), batch_size, app, &state)
}

#[tauri::command]
pub fn pause_semantic_analysis(
    job_id: String,
    state: State<'_, AppState>,
) -> Result<SemanticTaskResponse, String> {
    let accepted = if let Some(control) = state.semantic_tasks.control(&job_id) {
        control
            .with_job_lock(|job| -> AppResult<bool> {
                if job.is_terminal()
                    || matches!(control.current_signal(), SemanticControlSignal::Cancel)
                {
                    return Ok(false);
                }
                if !state
                    .repository
                    .set_semantic_job_status_if_active(&job_id, "paused")?
                {
                    return Ok(false);
                }
                Ok(control.pause_locked())
            })
            .map_err(ipc_error)?
    } else {
        false
    };
    Ok(SemanticTaskResponse { job_id, accepted })
}

#[tauri::command]
pub fn resume_semantic_analysis(
    job_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SemanticTaskResponse, String> {
    if let Some(control) = state.semantic_tasks.control(&job_id) {
        let accepted = control
            .with_job_lock(|job| -> AppResult<bool> {
                if job.is_terminal()
                    || matches!(control.current_signal(), SemanticControlSignal::Cancel)
                {
                    return Ok(false);
                }
                if !state
                    .repository
                    .set_semantic_job_status_if_active(&job_id, "running")?
                {
                    return Ok(false);
                }
                Ok(control.resume_locked())
            })
            .map_err(ipc_error)?;
        if accepted {
            return Ok(SemanticTaskResponse {
                job_id,
                accepted: true,
            });
        }
        return Ok(SemanticTaskResponse {
            job_id,
            accepted: false,
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
    spawn_with_app(
        &state,
        app,
        job_id.clone(),
        progress.library_id,
        candidates,
        SEMANTIC_BATCH_SIZE,
    )?;
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
    let accepted = request_semantic_cancel(&state.repository, &state.semantic_tasks, &job_id)
        .map_err(ipc_error)?;
    Ok(SemanticTaskResponse { job_id, accepted })
}

fn request_semantic_cancel(
    repository: &Repository,
    registry: &SemanticTaskRegistry,
    job_id: &str,
) -> AppResult<bool> {
    let Some(control) = registry.control(job_id) else {
        return repository.cancel_semantic_job_if_active(job_id);
    };

    control.with_job_lock(|job| {
        if job.is_terminal() {
            return Ok(false);
        }
        if !repository.set_semantic_job_status_if_active(job_id, "cancelling")? {
            return Ok(false);
        }
        Ok(control.cancel_locked())
    })
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
pub fn list_browse_nodes(state: State<'_, AppState>) -> Result<Vec<BrowseNode>, String> {
    workflow::list_browse_nodes(&state.repository).map_err(ipc_error)
}

#[tauri::command]
pub fn create_collection(
    name: String,
    description: Option<String>,
    parent_collection_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<CollectionSummary, String> {
    workflow::create_collection_under(
        &state.repository,
        &name,
        description.as_deref().unwrap_or_default(),
        parent_collection_id,
    )
    .map_err(ipc_error)
}

#[tauri::command]
pub fn rename_collection(
    collection_id: i64,
    name: String,
    state: State<'_, AppState>,
) -> Result<CollectionSummary, String> {
    workflow::rename_collection(&state.repository, collection_id, &name).map_err(ipc_error)
}

#[tauri::command]
pub fn move_collection(
    collection_id: i64,
    parent_collection_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<CollectionSummary, String> {
    workflow::move_collection(&state.repository, collection_id, parent_collection_id)
        .map_err(ipc_error)
}

#[tauri::command]
pub fn delete_collection(
    collection_id: i64,
    mode: Option<CollectionDeleteMode>,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    workflow::delete_collection_with_mode(
        &state.repository,
        collection_id,
        mode.unwrap_or(CollectionDeleteMode::DeleteSubtree),
    )
    .map_err(ipc_error)
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
pub fn add_assets_to_collections(
    collection_ids: Vec<i64>,
    asset_ids: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<CollectionSummary>, String> {
    workflow::add_assets_to_collections(&state.repository, &collection_ids, &asset_ids)
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
pub fn move_assets_between_collections(
    source_collection_id: i64,
    target_collection_id: i64,
    asset_ids: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<CollectionMembershipMutation, String> {
    workflow::move_assets_between_collections(
        &state.repository,
        source_collection_id,
        target_collection_id,
        &asset_ids,
    )
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
                            SEMANTIC_BATCH_SIZE,
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

fn semantic_status_ready_for_jobs(status: &SemanticRuntimeStatus) -> bool {
    status.status == "ready"
        && status.model.installed
        && status
            .topic_model
            .as_ref()
            .is_some_and(|model| model.installed)
}

fn resume_pending_semantic_jobs_if_ready(
    status: &SemanticRuntimeStatus,
    resume: impl FnOnce(),
) -> bool {
    if !semantic_status_ready_for_jobs(status) {
        return false;
    }
    resume();
    true
}

fn start_semantic_job(
    library_id: i64,
    force: bool,
    only_asset_id: Option<i64>,
    batch_size: Option<usize>,
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
    spawn_with_app(
        state,
        app,
        job_id.clone(),
        library_id,
        candidates,
        batch_size.unwrap_or(SEMANTIC_BATCH_SIZE),
    )?;
    Ok(StartSemanticResponse { job_id })
}

fn spawn_with_app(
    state: &AppState,
    app: tauri::AppHandle,
    job_id: String,
    library_id: i64,
    candidates: Vec<crate::db::SemanticAssetCandidate>,
    batch_size: usize,
) -> Result<(), String> {
    let repository = state.repository.clone();
    let paths = state.paths.clone();
    let semantic_slot = state.semantic.clone();
    let subject_slot = state.subject.clone();
    let semantic_status = state.semantic_status.clone();
    let subject_status = state.subject_status.clone();
    let gpu_provider = state.gpu_provider.clone();
    let classifier = state.semantic.read().clone();
    let backend = classifier
        .status()
        .selected_backend
        .unwrap_or(crate::semantic::ExecutionBackend::Cpu);
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
        batch_size,
        backend,
        move |progress| {
            let (semantic_state, semantic_changed) = observe_semantic_runtime(
                &repository,
                &paths,
                &semantic_slot,
                &semantic_status,
                &gpu_provider,
            );
            if semantic_changed && let Err(error) = app.emit("semantic-status", semantic_state) {
                log::warn!("could not emit updated semantic runtime status: {error}");
            }

            let (subject_state, subject_changed) =
                observe_subject_runtime(&subject_slot, &subject_status, &gpu_provider);
            if subject_changed && let Err(error) = app.emit("subject-status", subject_state) {
                log::warn!("could not emit updated subject runtime status: {error}");
            }

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

    let cache_name = format!(
        "{asset_id}-{fingerprint}-{}-{max_width}-{max_height}.jpg",
        crate::imaging::SCREEN_PREVIEW_SPEC
    );
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

fn remove_preview_cache_entries(application_data_root: &Path, removed_asset_ids: &[i64]) {
    if removed_asset_ids.is_empty() {
        return;
    }
    let Some(directory) = private_app_cache_root(application_data_root, "previews") else {
        return;
    };
    let Ok(entries) = fs::read_dir(&directory) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((raw_asset_id, _)) = name.split_once('-') else {
            continue;
        };
        let Ok(asset_id) = raw_asset_id.parse::<i64>() else {
            continue;
        };
        if removed_asset_ids.binary_search(&asset_id).is_ok()
            && path.parent().is_some_and(|parent| parent == directory)
        {
            let _ = fs::remove_file(path);
        }
    }
}

fn remove_cache_file_if_safe(application_data_root: &Path, cache_name: &str, path: &Path) {
    let Some(root) = private_app_cache_root(application_data_root, cache_name) else {
        return;
    };
    let Ok(target) = canonical_or_absolute(path) else {
        return;
    };
    if target.starts_with(&root) {
        let _ = fs::remove_file(target);
    }
}

fn private_app_cache_root(application_data_root: &Path, cache_name: &str) -> Option<PathBuf> {
    let cache_root = application_data_root.join(cache_name);
    let app_metadata = fs::symlink_metadata(application_data_root).ok()?;
    let cache_metadata = fs::symlink_metadata(&cache_root).ok()?;
    if !app_metadata.is_dir()
        || !cache_metadata.is_dir()
        || is_symlink_or_reparse_point(&app_metadata)
        || is_symlink_or_reparse_point(&cache_metadata)
    {
        return None;
    }

    let application_data_root = application_data_root.canonicalize().ok()?;
    let cache_root = cache_root.canonicalize().ok()?;
    (cache_root.parent() == Some(application_data_root.as_path())).then_some(cache_root)
}

fn is_symlink_or_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn preview_cache_cleanup_handles_large_cache_in_one_pass_and_keeps_source_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let application_data_root = temp.path().join("application-data");
        let preview_dir = application_data_root.join("previews");
        fs::create_dir_all(&preview_dir).expect("create preview cache");
        let source_dir = temp.path().join("图库 😀");
        fs::create_dir_all(&source_dir).expect("create source fixture");

        let matching_first = preview_dir.join("31-old-fingerprint-screen-1920.jpg");
        let matching_second = preview_dir.join("31-new-fingerprint-screen-2560.jpg");
        let unrelated_same_prefix = preview_dir.join("310-old-fingerprint-screen-1920.jpg");
        fs::write(&matching_first, b"cache").expect("write matching cache");
        fs::write(&matching_second, b"cache").expect("write matching cache version");
        fs::write(&unrelated_same_prefix, b"cache").expect("write unrelated cache");
        for index in 0..512 {
            fs::write(
                preview_dir.join(format!("unrelated-{index:04}-preview.jpg")),
                b"cache",
            )
            .expect("write unrelated cache entry");
        }
        let matching_directory = preview_dir.join("31-not-a-cache-file");
        fs::create_dir(&matching_directory).expect("create matching-named directory");

        let original = source_dir.join("31-old-fingerprint-screen-1920.jpg");
        fs::write(&original, b"original fixture bytes").expect("write original fixture");
        let original_bytes = fs::read(&original).expect("read original fixture");

        remove_preview_cache_entries(&application_data_root, &[31]);

        assert!(!matching_first.exists());
        assert!(!matching_second.exists());
        assert!(unrelated_same_prefix.is_file());
        assert!(matching_directory.is_dir());
        assert_eq!(
            fs::read(&original).expect("source file remains"),
            original_bytes
        );
        assert_eq!(
            fs::read_dir(&preview_dir)
                .expect("read preview cache")
                .count(),
            514
        );
    }

    #[test]
    fn cache_cleanup_root_must_be_a_direct_app_data_child() {
        let temp = tempfile::tempdir().expect("temp dir");
        let application_data_root = temp.path().join("application-data");
        let preview_dir = application_data_root.join("previews");
        fs::create_dir_all(&preview_dir).expect("create preview cache");
        let source_dir = temp.path().join("图库");
        fs::create_dir_all(&source_dir).expect("create source fixture");

        assert_eq!(
            private_app_cache_root(&application_data_root, "previews"),
            preview_dir.canonicalize().ok()
        );
        assert!(private_app_cache_root(&application_data_root, "../图库").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn preview_cache_cleanup_rejects_symlinked_cache_directory() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temp dir");
        let application_data_root = temp.path().join("application-data");
        fs::create_dir_all(&application_data_root).expect("create app data root");
        let source_dir = temp.path().join("图库");
        fs::create_dir_all(&source_dir).expect("create source fixture");
        let source_preview = source_dir.join("31-original.jpg");
        fs::write(&source_preview, b"original fixture bytes").expect("write source file");
        symlink(&source_dir, application_data_root.join("previews"))
            .expect("create cache directory symlink");

        remove_preview_cache_entries(&application_data_root, &[31]);

        assert_eq!(
            fs::read(source_preview).expect("source file remains"),
            b"original fixture bytes"
        );
    }

    #[test]
    fn fresh_app_state_publishes_loading_status_without_an_active_model() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        assert!(
            repository
                .active_semantic_model_key()
                .expect("read active semantic model")
                .is_none()
        );
        let mut paths = AppPaths::initialize_with_resources(
            temp.path().join("app-data"),
            temp.path().join("resources"),
        )
        .expect("initialize app paths");
        paths.semantic_model_dir = temp.path().join("missing/places365");
        paths.siglip2_model_dir = temp.path().join("missing/siglip2-base");
        paths.subject_model_dir = temp.path().join("missing/picodet");
        paths.face_model_dir = temp.path().join("missing/yunet");
        paths.onnx_runtime_path = temp.path().join("missing/onnxruntime.dll");
        let state = AppState::new(
            repository,
            paths,
            Arc::new(crate::semantic::UnavailableClassifier::default()),
            Arc::new(crate::subject::UnavailableSubjectClassifier::default()),
        );

        assert_eq!(current_semantic_status(&state).status, "loading");
        assert_eq!(current_subject_status(&state).status, "loading");

        let semantic_result = prepare_semantic_runtime(
            SemanticPreparationContext {
                paths: &state.paths,
                repository: &state.repository,
                classifier_slot: &state.semantic,
                published_status: &state.semantic_status,
                preparation: &state.semantic_preparation,
                gpu_provider: &state.gpu_provider,
            },
            TopicModelKind::Siglip2Base,
            ExecutionBackend::Cpu,
        );
        let subject_result = prepare_subject_runtime(
            &state.paths,
            &state.subject,
            &state.subject_status,
            &state.subject_preparation,
            &state.gpu_provider,
            ExecutionBackend::Cpu,
        );
        let semantic_error = semantic_result.expect_err("missing bundled semantic resources");
        let subject_error = subject_result.expect_err("missing bundled subject resources");
        let semantic_status = current_semantic_status(&state);
        let subject_status = current_subject_status(&state);
        assert_eq!(semantic_status.status, "error");
        assert!(semantic_status.message.contains("SigLIP"));
        assert_eq!(subject_status.status, "error");
        assert!(subject_status.message.contains("主体模型"));
        assert!(semantic_error.contains("SigLIP"));
        assert!(subject_error.contains("PicoDet"));
    }

    #[test]
    fn startup_preparation_is_claimed_once_and_model_loads_share_a_gate() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let paths = AppPaths::initialize_with_resources(
            temp.path().join("app-data"),
            temp.path().join("resources"),
        )
        .expect("initialize app paths");
        let state = AppState::new(
            repository,
            paths,
            Arc::new(crate::semantic::UnavailableClassifier::default()),
            Arc::new(crate::subject::UnavailableSubjectClassifier::default()),
        );

        assert!(Arc::ptr_eq(
            &state.semantic_preparation,
            &state.subject_preparation
        ));
        assert!(state.claim_startup_preparation());
        assert!(!state.claim_startup_preparation());
    }

    #[test]
    fn directml_cpu_fallback_satisfies_repeated_prepare_request() {
        let fallback_message = "DirectML 初始化失败，已回退 CPU：provider unavailable";
        assert!(backend_satisfies_request(
            Some(ExecutionBackend::Cpu),
            ExecutionBackend::DirectMl,
            fallback_message,
        ));
        assert!(backend_satisfies_request(
            Some(ExecutionBackend::DirectMl),
            ExecutionBackend::DirectMl,
            "ready",
        ));
        assert!(!backend_satisfies_request(
            Some(ExecutionBackend::Cpu),
            ExecutionBackend::DirectMl,
            "CPU 模型已就绪",
        ));
        assert!(!backend_satisfies_request(
            Some(ExecutionBackend::DirectMl),
            ExecutionBackend::Cpu,
            fallback_message,
        ));
        assert!(backend_satisfies_request(
            Some(ExecutionBackend::Cpu),
            ExecutionBackend::DirectMl,
            "DirectML 推理失败，已回退 CPU；后续语义任务固定使用 CPU。",
        ));
    }

    #[test]
    fn unsupported_so400m_profile_is_rejected_before_using_the_base_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let mut paths = AppPaths::initialize_with_resources(
            temp.path().join("app-data"),
            temp.path().join("resources"),
        )
        .expect("initialize app paths");
        paths.semantic_model_dir = temp.path().join("missing/places365");
        paths.siglip2_model_dir = temp.path().join("models/siglip2-base");
        paths.onnx_runtime_path = temp.path().join("missing/onnxruntime.dll");
        let state = AppState::new(
            repository,
            paths,
            Arc::new(crate::semantic::UnavailableClassifier::default()),
            Arc::new(crate::subject::UnavailableSubjectClassifier::default()),
        );

        let error = prepare_semantic_runtime(
            SemanticPreparationContext {
                paths: &state.paths,
                repository: &state.repository,
                classifier_slot: &state.semantic,
                published_status: &state.semantic_status,
                preparation: &state.semantic_preparation,
                gpu_provider: &state.gpu_provider,
            },
            TopicModelKind::Siglip2So400m14_384,
            ExecutionBackend::Cpu,
        )
        .expect_err("SO400M is not bundled");

        assert!(error.contains("SO400M"));
        assert!(error.contains("不会尝试从 Base 目录"));
        assert_eq!(current_semantic_status(&state).status, "loading");
        assert!(
            state
                .repository
                .active_semantic_model_key()
                .expect("read active model")
                .is_none()
        );
    }

    struct MutableSemanticClassifier {
        status: parking_lot::RwLock<SemanticRuntimeStatus>,
    }

    impl SemanticClassifier for MutableSemanticClassifier {
        fn metadata(&self) -> crate::semantic::ModelMetadata {
            self.status.read().model.clone()
        }

        fn status(&self) -> SemanticRuntimeStatus {
            self.status.read().clone()
        }

        fn result_metadata(&self) -> crate::semantic::ModelMetadata {
            let status = self.status.read();
            status
                .topic_model
                .clone()
                .unwrap_or_else(|| status.model.clone())
        }

        fn classify_batch(
            &self,
            _images: &[PathBuf],
            _backend: ExecutionBackend,
        ) -> Result<Vec<crate::semantic::SemanticAnalysisOutput>, crate::semantic::SemanticError>
        {
            Err(crate::semantic::SemanticError::ModelUnavailable)
        }
    }

    #[test]
    fn semantic_runtime_unavailable_is_reloaded_instead_of_reused() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let mut paths = AppPaths::initialize_with_resources(
            temp.path().join("app-data"),
            temp.path().join("resources"),
        )
        .expect("initialize app paths");
        paths.semantic_model_dir = temp.path().join("missing/places365");
        paths.siglip2_model_dir = temp.path().join("missing/siglip2-base");
        paths.onnx_runtime_path = temp.path().join("missing/onnxruntime.dll");

        let topic_metadata = default_topic_model_metadata();
        let unavailable = SemanticRuntimeStatus {
            status: "runtime_unavailable".into(),
            message: "CPU runtime rebuild failed".into(),
            model: topic_metadata.clone(),
            topic_model: Some(topic_metadata),
            selected_backend: Some(ExecutionBackend::Cpu),
        };
        let classifier = Arc::new(MutableSemanticClassifier {
            status: parking_lot::RwLock::new(unavailable.clone()),
        });
        let state = AppState::new(
            repository,
            paths,
            classifier,
            Arc::new(crate::subject::UnavailableSubjectClassifier::default()),
        );
        *state.semantic_status.write() = unavailable;

        let error = prepare_semantic_runtime(
            SemanticPreparationContext {
                paths: &state.paths,
                repository: &state.repository,
                classifier_slot: &state.semantic,
                published_status: &state.semantic_status,
                preparation: &state.semantic_preparation,
                gpu_provider: &state.gpu_provider,
            },
            TopicModelKind::Siglip2Base,
            ExecutionBackend::Cpu,
        )
        .expect_err("unavailable runtime must attempt a fresh load");

        assert!(error.contains("加载失败"));
        assert_eq!(state.semantic_status.read().status, "error");
        assert!(state.semantic_status.read().message.contains("加载失败"));
    }

    #[test]
    fn semantic_cpu_runtime_fallback_updates_status_provider_and_active_model() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let mut paths = AppPaths::initialize_with_resources(
            temp.path().join("app-data"),
            temp.path().join("resources"),
        )
        .expect("initialize app paths");
        let model_dir = temp.path().join("models/siglip2-base");
        std::fs::create_dir_all(&model_dir).expect("create model directory");
        let model_path = model_dir.join(crate::semantic::SIGLIP2_MODEL_FILE);
        let tokenizer_path = model_dir.join(crate::semantic::SIGLIP2_TOKENIZER_FILE);
        std::fs::write(&tokenizer_path, b"test tokenizer").expect("write tokenizer");
        paths.siglip2_model_dir = model_dir;
        let topic_metadata = default_topic_model_metadata();
        repository
            .register_active_semantic_model(
                &topic_metadata,
                &model_path,
                &tokenizer_path,
                "https://huggingface.co/onnx-community/siglip2-base-patch16-224-ONNX",
                ExecutionBackend::DirectMl,
            )
            .expect("register DirectML model");

        let initial = SemanticRuntimeStatus {
            status: "ready".into(),
            message: "DirectML model ready".into(),
            model: topic_metadata.clone(),
            topic_model: Some(topic_metadata),
            selected_backend: Some(ExecutionBackend::DirectMl),
        };
        let classifier = Arc::new(MutableSemanticClassifier {
            status: parking_lot::RwLock::new(initial.clone()),
        });
        let classifier_slot: Arc<RwLock<Arc<dyn SemanticClassifier>>> =
            Arc::new(RwLock::new(classifier.clone()));
        let published_status = Arc::new(RwLock::new(initial));
        let gpu_provider = Arc::new(RwLock::new(Some(crate::gpu::GpuProviderStatus {
            id: "directml".into(),
            state: "ready".into(),
            message: "DirectML ready".into(),
        })));
        {
            let mut runtime = classifier.status.write();
            runtime.selected_backend = Some(ExecutionBackend::Cpu);
            runtime.message = "CPU runtime ready".into();
        }

        let (status, changed) = observe_semantic_runtime(
            &repository,
            &paths,
            &classifier_slot,
            &published_status,
            &gpu_provider,
        );
        assert!(changed);
        assert_eq!(status.selected_backend, Some(ExecutionBackend::Cpu));
        assert!(status.message.contains("DirectML 推理失败"));
        assert_eq!(
            gpu_provider
                .read()
                .as_ref()
                .expect("cached provider status")
                .state,
            "error"
        );
        assert_eq!(
            repository
                .active_semantic_model_key()
                .expect("read active model")
                .expect("active model")
                .3,
            "cpu"
        );

        let (status_again, changed_again) = observe_semantic_runtime(
            &repository,
            &paths,
            &classifier_slot,
            &published_status,
            &gpu_provider,
        );
        assert!(!changed_again);
        assert!(status_again.message.contains("回退 CPU"));
        assert!(backend_satisfies_request(
            status_again.selected_backend,
            ExecutionBackend::DirectMl,
            &status_again.message,
        ));
    }

    struct MutableSubjectClassifier {
        status: parking_lot::RwLock<SubjectRuntimeStatus>,
    }

    impl SubjectClassifier for MutableSubjectClassifier {
        fn metadata(&self) -> crate::semantic::ModelMetadata {
            self.status.read().model.clone()
        }

        fn face_metadata(&self) -> crate::semantic::ModelMetadata {
            self.status.read().face_model.clone()
        }

        fn status(&self) -> SubjectRuntimeStatus {
            self.status.read().clone()
        }

        fn classify_batch(
            &self,
            _images: &[PathBuf],
            _backend: ExecutionBackend,
        ) -> Result<Vec<crate::subject::SubjectAnalysisOutput>, crate::semantic::SemanticError>
        {
            Err(crate::semantic::SemanticError::ModelUnavailable)
        }
    }

    #[test]
    fn subject_error_status_is_reloaded_instead_of_reused() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let mut paths = AppPaths::initialize_with_resources(
            temp.path().join("app-data"),
            temp.path().join("resources"),
        )
        .expect("initialize app paths");
        paths.subject_model_dir = temp.path().join("missing/picodet");
        paths.face_model_dir = temp.path().join("missing/yunet");
        paths.onnx_runtime_path = temp.path().join("missing/onnxruntime.dll");

        let metadata = default_topic_model_metadata();
        let failed = SubjectRuntimeStatus {
            status: "error".into(),
            message: "previous subject load failed".into(),
            model: metadata.clone(),
            face_model: metadata,
            selected_backend: Some(ExecutionBackend::Cpu),
        };
        let classifier = Arc::new(MutableSubjectClassifier {
            status: parking_lot::RwLock::new(failed.clone()),
        });
        let state = AppState::new(
            repository,
            paths,
            Arc::new(crate::semantic::UnavailableClassifier::default()),
            classifier,
        );
        *state.subject_status.write() = failed;

        let error = prepare_subject_runtime(
            &state.paths,
            &state.subject,
            &state.subject_status,
            &state.subject_preparation,
            &state.gpu_provider,
            ExecutionBackend::Cpu,
        )
        .expect_err("failed subject runtime must attempt a fresh load");

        assert!(error.contains("主体模型加载失败"));
        assert!(error.contains("PicoDet"));
        assert_eq!(state.subject_status.read().status, "error");
    }

    #[test]
    fn subject_status_query_publishes_live_cpu_fallback_and_gpu_failure() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let paths = AppPaths::initialize_with_resources(
            temp.path().join("app-data"),
            temp.path().join("resources"),
        )
        .expect("initialize app paths");
        let metadata = default_topic_model_metadata();
        let initial = SubjectRuntimeStatus {
            status: "ready".into(),
            message: "DirectML subject ready".into(),
            model: metadata.clone(),
            face_model: metadata,
            selected_backend: Some(ExecutionBackend::DirectMl),
        };
        let classifier = Arc::new(MutableSubjectClassifier {
            status: parking_lot::RwLock::new(initial.clone()),
        });
        let state = AppState::new(
            repository,
            paths,
            Arc::new(crate::semantic::UnavailableClassifier::default()),
            classifier.clone(),
        );
        *state.subject_status.write() = initial;
        let model = classifier.metadata();
        let face_model = classifier.face_metadata();
        *classifier.status.write() = SubjectRuntimeStatus {
            status: "ready".into(),
            message: "DirectML 执行失败（test），已一次性回退 CPU。".into(),
            model,
            face_model,
            selected_backend: Some(ExecutionBackend::Cpu),
        };

        let status = current_subject_status(&state);
        assert_eq!(status.selected_backend, Some(ExecutionBackend::Cpu));
        assert!(status.message.contains("回退 CPU"));
        assert_eq!(
            state
                .gpu_provider
                .read()
                .as_ref()
                .expect("cached provider status")
                .state,
            "error"
        );
    }

    #[test]
    fn pending_semantic_jobs_resume_only_with_a_ready_topic_model() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        repository
            .open_for_tests()
            .expect("open database")
            .execute(
                "INSERT INTO analysis_jobs(
                    id, job_type, status, progress_current, progress_total, created_at, updated_at
                 ) VALUES(?1, 'semantic_classification', 'queued', 0, 1, ?2, ?2)",
                params!["pending-recovery", "2026-09-23T00:00:00Z"],
            )
            .expect("insert recoverable job");

        let mut status = crate::semantic::UnavailableClassifier::default().status();
        status.model.installed = true;
        status.topic_model = Some(crate::semantic::default_topic_model_metadata());
        status.selected_backend = Some(ExecutionBackend::Cpu);

        let mut resumed = false;
        status.status = "loading".into();
        assert!(!resume_pending_semantic_jobs_if_ready(&status, || {
            resumed = true
        }));
        assert!(!resumed, "loading models must leave jobs queued");

        status.status = "runtime_unavailable".into();
        assert!(!resume_pending_semantic_jobs_if_ready(&status, || {
            resumed = true
        }));
        assert!(!resumed, "failed runtimes must leave jobs queued");
        let persisted_status: String = repository
            .open_for_tests()
            .expect("open database")
            .query_row(
                "SELECT status FROM analysis_jobs WHERE id=?1",
                ["pending-recovery"],
                |row| row.get(0),
            )
            .expect("read queued job status");
        assert_eq!(persisted_status, "queued");

        status.status = "ready".into();
        status.topic_model = None;
        assert!(!resume_pending_semantic_jobs_if_ready(&status, || {
            resumed = true
        }));
        assert!(
            !resumed,
            "a ready Places365 shell is insufficient without topics"
        );

        status.topic_model = Some(crate::semantic::default_topic_model_metadata());
        assert!(resume_pending_semantic_jobs_if_ready(&status, || resumed = true));
        assert!(resumed, "queued jobs resume after the topic model is ready");
    }

    #[test]
    fn persisted_terminal_semantic_job_rejects_registry_absent_cancel() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        repository
            .open_for_tests()
            .expect("open database")
            .execute(
                "INSERT INTO analysis_jobs(
                    id, job_type, status, progress_current, progress_total, created_at, updated_at
                 ) VALUES(?1, 'semantic_classification', 'completed', 1, 1, ?2, ?2)",
                params!["persisted-terminal", "2026-09-14T00:00:00Z"],
            )
            .expect("insert terminal semantic job");

        let registry = SemanticTaskRegistry::default();
        assert!(
            !request_semantic_cancel(&repository, &registry, "persisted-terminal")
                .expect("cancel terminal semantic job")
        );

        let status: String = repository
            .open_for_tests()
            .expect("reopen database")
            .query_row(
                "SELECT status FROM analysis_jobs WHERE id=?1",
                ["persisted-terminal"],
                |row| row.get(0),
            )
            .expect("read terminal semantic status");
        assert_eq!(status, "completed");
    }
}
