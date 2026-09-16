use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use ort::session::Session;
use ort::value::Tensor;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::semantic::{ExecutionBackend, ModelMetadata, SemanticError, SemanticLabelDescriptor};

pub const MODEL_NAME: &str = "PicoDet-S-COCO";
pub const MODEL_VERSION: &str = "onnx-2026-08-10";
pub const ANALYSIS_VERSION: &str = "photo-organizer-subject-picodet-yunet-v2";
pub const TAXONOMY_VERSION: &str = "photo-organizer-subject-tags-v4";
pub const MODEL_FILE: &str = "picodet_s_320_lcnet_postprocessed.onnx";
pub const LABELS_FILE: &str = "coco80.txt";
pub const MODEL_SHA256: &str = "09fc88131be8ad224f13739a5cf8fc838600d76a77539af7f0400fa90506c5f3";
pub const FACE_MODEL_NAME: &str = "YuNet-FaceDetector";
pub const FACE_MODEL_VERSION: &str = "onnx-2023mar";
pub const FACE_MODEL_FILE: &str = "face_detection_yunet_2023mar.onnx";
pub const FACE_MODEL_SHA256: &str =
    "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4";

const PICO_IMAGE_SIZE: usize = 320;
// YuNet's official 2023mar export has a fixed 640×640 input contract.
const FACE_IMAGE_SIZE: usize = 640;
const DETECTION_SCORE_THRESHOLD: f32 = 0.40;
const PERSON_SCORE_THRESHOLD: f32 = 0.45;
const FACE_SCORE_THRESHOLD: f32 = 0.65;
const COCO_LABEL_COUNT: usize = 80;
const YUNET_STRIDES: [usize; 3] = [8, 16, 32];
const YUNET_OUTPUT_NAMES: [&str; 12] = [
    "cls_8", "cls_16", "cls_32", "obj_8", "obj_16", "obj_32", "bbox_8", "bbox_16", "bbox_32",
    "kps_8", "kps_16", "kps_32",
];

const ANIMAL_CLASSES: &[usize] = &[14, 15, 16, 17, 18, 19, 20, 21, 22, 23];
const VEHICLE_CLASSES: &[usize] = &[2, 3, 4, 5, 6, 7, 8];
const FOOD_CLASSES: &[usize] = &[39, 40, 41, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55];
const PLANT_CLASSES: &[usize] = &[58];
const PERSON_DUPLICATE_IOU_THRESHOLD: f32 = 0.85;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectPrediction {
    pub label_id: String,
    pub display_name: String,
    pub category_group: String,
    pub similarity: f32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectAnalysisOutput {
    pub predictions: Vec<SubjectPrediction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectRuntimeStatus {
    pub status: String,
    pub message: String,
    pub model: ModelMetadata,
    pub face_model: ModelMetadata,
    pub selected_backend: Option<ExecutionBackend>,
}

#[derive(Debug, Clone, Copy)]
struct SubjectLabelDefinition {
    id: &'static str,
    display_name: &'static str,
    threshold: f32,
}

const SUBJECT_LABELS: [SubjectLabelDefinition; 6] = [
    SubjectLabelDefinition {
        id: "single_person",
        display_name: "单人",
        threshold: PERSON_SCORE_THRESHOLD,
    },
    SubjectLabelDefinition {
        id: "multiple_people",
        display_name: "多人",
        threshold: PERSON_SCORE_THRESHOLD,
    },
    SubjectLabelDefinition {
        id: "animal",
        display_name: "动物",
        threshold: DETECTION_SCORE_THRESHOLD,
    },
    SubjectLabelDefinition {
        id: "vehicle",
        display_name: "车辆",
        threshold: DETECTION_SCORE_THRESHOLD,
    },
    SubjectLabelDefinition {
        id: "food",
        display_name: "食物",
        threshold: DETECTION_SCORE_THRESHOLD,
    },
    SubjectLabelDefinition {
        id: "plant",
        display_name: "植物",
        threshold: DETECTION_SCORE_THRESHOLD,
    },
];

pub fn subject_catalog() -> Vec<SemanticLabelDescriptor> {
    SUBJECT_LABELS
        .iter()
        .map(|label| SemanticLabelDescriptor {
            id: label.id.into(),
            display_name: label.display_name.into(),
            category_group: "subject".into(),
            threshold: label.threshold,
            is_primary_category: false,
            taxonomy_version: TAXONOMY_VERSION.into(),
        })
        .collect()
}

pub trait SubjectClassifier: Send + Sync {
    fn metadata(&self) -> ModelMetadata;
    fn face_metadata(&self) -> ModelMetadata;
    fn status(&self) -> SubjectRuntimeStatus;
    fn classify_batch(
        &self,
        images: &[PathBuf],
        backend: ExecutionBackend,
    ) -> Result<Vec<SubjectAnalysisOutput>, SemanticError>;
}

#[derive(Debug, Default)]
pub struct UnavailableSubjectClassifier {
    message: Option<String>,
}

impl UnavailableSubjectClassifier {
    pub fn with_message(message: impl Into<String>) -> Self {
        Self {
            message: Some(message.into()),
        }
    }
}

impl SubjectClassifier for UnavailableSubjectClassifier {
    fn metadata(&self) -> ModelMetadata {
        model_metadata(false, None)
    }

    fn face_metadata(&self) -> ModelMetadata {
        face_model_metadata(false, None)
    }

    fn status(&self) -> SubjectRuntimeStatus {
        SubjectRuntimeStatus {
            status: "model_unavailable".into(),
            message: self
                .message
                .clone()
                .unwrap_or_else(|| "本地主体模型不可用；主体标签不会被自动生成。".into()),
            model: self.metadata(),
            face_model: self.face_metadata(),
            selected_backend: None,
        }
    }

    fn classify_batch(
        &self,
        _images: &[PathBuf],
        _backend: ExecutionBackend,
    ) -> Result<Vec<SubjectAnalysisOutput>, SemanticError> {
        Err(SemanticError::ModelUnavailable)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SubjectBackendState {
    Active(ExecutionBackend),
    SwitchingToCpu { reason: String },
    CpuFallback { reason: String },
    Failed { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubjectFallbackDecision {
    RetryCpu,
    AlreadyAttempted,
    NotRequired,
}

fn begin_subject_cpu_fallback(
    state: &mut SubjectBackendState,
    error: &SemanticError,
) -> SubjectFallbackDecision {
    if !matches!(error, SemanticError::Inference(_)) {
        return SubjectFallbackDecision::NotRequired;
    }
    match state {
        SubjectBackendState::Active(ExecutionBackend::DirectMl) => {
            *state = SubjectBackendState::SwitchingToCpu {
                reason: error.to_string(),
            };
            SubjectFallbackDecision::RetryCpu
        }
        SubjectBackendState::Active(_) => SubjectFallbackDecision::NotRequired,
        SubjectBackendState::SwitchingToCpu { .. }
        | SubjectBackendState::CpuFallback { .. }
        | SubjectBackendState::Failed { .. } => SubjectFallbackDecision::AlreadyAttempted,
    }
}

fn run_subject_batch_with_fallback<T, DirectMl, RebuildCpu, Cpu>(
    backend_state: &Mutex<SubjectBackendState>,
    active_backend: ExecutionBackend,
    directml_batch: DirectMl,
    rebuild_cpu_sessions: RebuildCpu,
    cpu_batch: Cpu,
) -> Result<T, SemanticError>
where
    DirectMl: FnOnce() -> Result<T, SemanticError>,
    RebuildCpu: FnOnce() -> Result<(), SemanticError>,
    Cpu: FnOnce() -> Result<T, SemanticError>,
{
    match directml_batch() {
        Ok(results) => Ok(results),
        Err(error) if active_backend == ExecutionBackend::DirectMl => {
            let reason = error.to_string();
            let decision = {
                let mut state = backend_state.lock();
                begin_subject_cpu_fallback(&mut state, &error)
            };
            if decision != SubjectFallbackDecision::RetryCpu {
                return Err(error);
            }
            if let Err(fallback_error) = rebuild_cpu_sessions() {
                let combined =
                    format!("DirectML 主体模型执行失败：{reason}；CPU 回退失败：{fallback_error}");
                *backend_state.lock() = SubjectBackendState::Failed {
                    reason: combined.clone(),
                };
                log::error!(
                    "DirectML subject inference failed and CPU fallback could not be \
                     initialized; DirectML will not be retried: {combined}"
                );
                return Err(SemanticError::Inference(combined));
            }
            *backend_state.lock() = SubjectBackendState::CpuFallback {
                reason: reason.clone(),
            };
            log::warn!(
                "DirectML subject inference failed ({reason}); switched to CPU once; \
                 subsequent thumbnails will not retry DirectML"
            );
            match cpu_batch() {
                Ok(results) => Ok(results),
                Err(cpu_error) => {
                    let combined = format!(
                        "DirectML 主体模型执行失败：{reason}；CPU 回退重试失败：{cpu_error}"
                    );
                    // Keep CPU fallback active. The task layer will split
                    // this failed batch and retry individual thumbnails;
                    // one problematic image must not poison the model for
                    // all remaining assets.
                    *backend_state.lock() = SubjectBackendState::CpuFallback { reason };
                    log::warn!(
                        "CPU subject fallback batch failed; keeping CPU fallback for \
                         per-thumbnail recovery: {combined}"
                    );
                    Err(SemanticError::Inference(combined))
                }
            }
        }
        Err(error) => Err(error),
    }
}

impl SubjectBackendState {
    fn selected_backend(&self) -> ExecutionBackend {
        match self {
            Self::Active(backend) if *backend != ExecutionBackend::Auto => *backend,
            Self::Active(_)
            | Self::SwitchingToCpu { .. }
            | Self::CpuFallback { .. }
            | Self::Failed { .. } => ExecutionBackend::Cpu,
        }
    }
}

pub struct SubjectModel {
    inference_lock: Mutex<()>,
    detector: Mutex<Session>,
    detector_input_name: String,
    scale_factor_input_name: String,
    detector_output_name: String,
    face_detector: Option<Mutex<Session>>,
    face_input_name: Option<String>,
    face_output_names: Option<Vec<String>>,
    model_size_bytes: u64,
    face_model_size_bytes: Option<u64>,
    detector_model_path: PathBuf,
    face_model_path: Option<PathBuf>,
    backend_state: Mutex<SubjectBackendState>,
}

impl std::fmt::Debug for SubjectModel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SubjectModel")
            .field("model", &MODEL_NAME)
            .field("face_model_installed", &self.face_detector.is_some())
            .finish_non_exhaustive()
    }
}

impl SubjectModel {
    pub fn load(
        model_dir: &Path,
        face_model_dir: &Path,
        runtime_path: &Path,
    ) -> Result<Self, SemanticError> {
        Self::load_with_backend(
            model_dir,
            face_model_dir,
            runtime_path,
            ExecutionBackend::Cpu,
        )
    }

    pub fn load_with_backend(
        model_dir: &Path,
        face_model_dir: &Path,
        runtime_path: &Path,
        backend: ExecutionBackend,
    ) -> Result<Self, SemanticError> {
        let model_path = model_dir.join(MODEL_FILE);
        let labels_path = model_dir.join(LABELS_FILE);
        verify_subject_model_resources(&model_path, None)?;
        crate::semantic::verify_sha256(runtime_path, crate::semantic::RUNTIME_SHA256)?;
        let labels = load_coco_labels(&labels_path)?;
        if labels.len() != COCO_LABEL_COUNT {
            return Err(SemanticError::Inference(format!(
                "COCO label resource contains {} labels; expected {COCO_LABEL_COUNT}",
                labels.len()
            )));
        }
        crate::semantic::initialize_ort(runtime_path)?;

        let detector = create_session(&model_path, "PicoDet", backend)?;
        let (detector_input_name, scale_factor_input_name, detector_output_name) =
            validate_picodet_contract(&detector)?;

        let face_model_path = face_model_dir.join(FACE_MODEL_FILE);
        let (face_detector, face_input_name, face_output_names, face_model_size_bytes) =
            match crate::semantic::verify_sha256(&face_model_path, FACE_MODEL_SHA256)
                .and_then(|_| create_session(&face_model_path, "YuNet", backend))
                .and_then(|session| {
                    let (input, outputs) = validate_yunet_contract(&session)?;
                    Ok((session, input, outputs))
                }) {
                Ok((session, input, outputs)) => {
                    let size = std::fs::metadata(&face_model_path)
                        .map_err(|error| SemanticError::Inference(error.to_string()))?
                        .len();
                    (
                        Some(Mutex::new(session)),
                        Some(input),
                        Some(outputs),
                        Some(size),
                    )
                }
                Err(error) => {
                    log::warn!("YuNet face helper unavailable: {error}");
                    (None, None, None, None)
                }
            };

        let model_size_bytes = std::fs::metadata(&model_path)
            .map_err(|error| SemanticError::Inference(error.to_string()))?
            .len();
        let face_model_path = face_detector.as_ref().map(|_| face_model_path.clone());
        Ok(Self {
            inference_lock: Mutex::new(()),
            detector: Mutex::new(detector),
            detector_input_name,
            scale_factor_input_name,
            detector_output_name,
            face_detector,
            face_input_name,
            face_output_names,
            model_size_bytes,
            face_model_size_bytes,
            detector_model_path: model_path,
            face_model_path,
            backend_state: Mutex::new(SubjectBackendState::Active(backend)),
        })
    }

    pub fn model_contract(&self) -> (String, String) {
        (
            self.detector_input_name.clone(),
            self.detector_output_name.clone(),
        )
    }

    fn backend_for_request(
        &self,
        requested: ExecutionBackend,
    ) -> Result<ExecutionBackend, SemanticError> {
        let state = self.backend_state.lock();
        match &*state {
            SubjectBackendState::Active(active) => {
                if crate::semantic::backend_matches(*active, requested) {
                    Ok(*active)
                } else {
                    Err(SemanticError::BackendUnavailable(requested))
                }
            }
            SubjectBackendState::CpuFallback { .. } => {
                if matches!(
                    requested,
                    ExecutionBackend::Auto | ExecutionBackend::Cpu | ExecutionBackend::DirectMl
                ) {
                    Ok(ExecutionBackend::Cpu)
                } else {
                    Err(SemanticError::BackendUnavailable(requested))
                }
            }
            SubjectBackendState::SwitchingToCpu { .. } => Err(SemanticError::Inference(
                "主体模型正在执行 DirectML 到 CPU 的一次性回退".into(),
            )),
            SubjectBackendState::Failed { reason } => Err(SemanticError::Inference(reason.clone())),
        }
    }

    fn rebuild_cpu_sessions(&self) -> Result<(), SemanticError> {
        verify_subject_model_resources(&self.detector_model_path, self.face_model_path.as_deref())?;
        let cpu_detector =
            create_session(&self.detector_model_path, "PicoDet", ExecutionBackend::Cpu)?;
        validate_picodet_contract(&cpu_detector)?;

        let cpu_face_detector = self
            .face_model_path
            .as_ref()
            .map(|path| {
                create_session(path, "YuNet", ExecutionBackend::Cpu).and_then(|session| {
                    validate_yunet_contract(&session)?;
                    Ok(session)
                })
            })
            .transpose()?;

        *self.detector.lock() = cpu_detector;
        if let (Some(face_detector), Some(cpu_face_detector)) =
            (&self.face_detector, cpu_face_detector)
        {
            *face_detector.lock() = cpu_face_detector;
        }
        Ok(())
    }

    fn classify_batch_once(
        &self,
        images: &[PathBuf],
    ) -> Result<Vec<SubjectAnalysisOutput>, SemanticError> {
        let mut results = Vec::with_capacity(images.len());
        for path in images {
            let rgb = load_subject_thumbnail(path)?;
            let pico_pixels = preprocess_pico(&rgb);
            let detections = {
                let input = Tensor::from_array((
                    [1_usize, 3, PICO_IMAGE_SIZE, PICO_IMAGE_SIZE],
                    pico_pixels.into_boxed_slice(),
                ))
                .map_err(crate::semantic::inference_error)?;
                let scale_factor =
                    Tensor::from_array(([1_usize, 2], vec![1.0_f32, 1.0_f32].into_boxed_slice()))
                        .map_err(crate::semantic::inference_error)?;
                let mut detector = self.detector.lock();
                let outputs = detector
                    .run(ort::inputs! {
                        self.detector_input_name.as_str() => input,
                        self.scale_factor_input_name.as_str() => scale_factor,
                    })
                    .map_err(|error| {
                        SemanticError::Inference(format!("PicoDet 执行失败：{error}"))
                    })?;
                let output = outputs
                    .get(self.detector_output_name.as_str())
                    .ok_or_else(|| {
                        SemanticError::Inference(format!(
                            "PicoDet output is missing: {}",
                            self.detector_output_name
                        ))
                    })?;
                let (shape, data) = output
                    .try_extract_tensor::<f32>()
                    .map_err(crate::semantic::inference_error)?;
                parse_picodet_output(shape.as_ref(), data)
            }?;

            let face_score = match (
                &self.face_detector,
                &self.face_input_name,
                &self.face_output_names,
            ) {
                (Some(face_detector), Some(input_name), Some(output_names)) => {
                    let face_pixels = preprocess_yunet(&rgb);
                    let input = Tensor::from_array((
                        [1_usize, 3, FACE_IMAGE_SIZE, FACE_IMAGE_SIZE],
                        face_pixels.into_boxed_slice(),
                    ))
                    .map_err(crate::semantic::inference_error)?;
                    let mut detector = face_detector.lock();
                    let outputs = detector
                        .run(ort::inputs! { input_name.as_str() => input })
                        .map_err(|error| {
                            SemanticError::Inference(format!("YuNet 执行失败：{error}"))
                        })?;
                    let mut blobs = Vec::with_capacity(output_names.len());
                    for output_name in output_names {
                        let output = outputs.get(output_name.as_str()).ok_or_else(|| {
                            SemanticError::Inference(format!(
                                "YuNet output is missing: {output_name}"
                            ))
                        })?;
                        let (shape, data) = output
                            .try_extract_tensor::<f32>()
                            .map_err(crate::semantic::inference_error)?;
                        blobs.push((shape.to_vec(), data.to_vec()));
                    }
                    parse_yunet_outputs(&blobs)?
                }
                _ => 0.0,
            };
            results.push(aggregate_subjects(&detections, face_score));
        }
        Ok(results)
    }
}

impl SubjectClassifier for SubjectModel {
    fn metadata(&self) -> ModelMetadata {
        model_metadata(true, Some(self.model_size_bytes))
    }

    fn face_metadata(&self) -> ModelMetadata {
        face_model_metadata(self.face_detector.is_some(), self.face_model_size_bytes)
    }

    fn status(&self) -> SubjectRuntimeStatus {
        let (base_status, base_message) = if self.face_detector.is_some() {
            (
                "ready",
                "PicoDet 主体检测与 YuNet 人像辅助模型均已就绪；结果仅基于缩略图。",
            )
        } else {
            (
                "partial",
                "PicoDet 主体检测已就绪；YuNet 人像辅助模型不可用，人像标签将暂不生成。",
            )
        };
        let backend_state = self.backend_state.lock().clone();
        let (status, message) = match &backend_state {
            SubjectBackendState::Active(_) => (base_status, base_message.to_owned()),
            SubjectBackendState::SwitchingToCpu { reason } => (
                base_status,
                format!("{base_message} DirectML 执行失败，正在一次性回退 CPU：{reason}"),
            ),
            SubjectBackendState::CpuFallback { reason } => (
                base_status,
                format!(
                    "{base_message} DirectML 执行失败（{reason}），已一次性回退 CPU；后续图片不再重试 DirectML。"
                ),
            ),
            SubjectBackendState::Failed { reason } => (
                "error",
                format!(
                    "主体模型不可用：DirectML 执行失败且 CPU 回退失败；已停止重复尝试。{reason}"
                ),
            ),
        };
        SubjectRuntimeStatus {
            status: status.into(),
            message,
            model: self.metadata(),
            face_model: self.face_metadata(),
            selected_backend: Some(backend_state.selected_backend()),
        }
    }

    fn classify_batch(
        &self,
        images: &[PathBuf],
        backend: ExecutionBackend,
    ) -> Result<Vec<SubjectAnalysisOutput>, SemanticError> {
        if images.is_empty() {
            return Ok(Vec::new());
        }
        // Keep the state transition and the CPU session rebuild serialized with
        // inference. A second caller must wait for the first DirectML failure
        // to finish switching instead of observing SwitchingToCpu and failing
        // its own batch.
        let _inference_guard = self.inference_lock.lock();
        let active_backend = self.backend_for_request(backend)?;
        run_subject_batch_with_fallback(
            &self.backend_state,
            active_backend,
            || self.classify_batch_once(images),
            || self.rebuild_cpu_sessions(),
            || self.classify_batch_once(images),
        )
    }
}

fn verify_subject_model_resources(
    detector_model_path: &Path,
    face_model_path: Option<&Path>,
) -> Result<(), SemanticError> {
    crate::semantic::verify_sha256(detector_model_path, MODEL_SHA256)?;
    if let Some(face_model_path) = face_model_path {
        crate::semantic::verify_sha256(face_model_path, FACE_MODEL_SHA256)?;
    }
    Ok(())
}

fn load_subject_thumbnail(path: &Path) -> Result<image::RgbImage, SemanticError> {
    crate::imaging::load_analysis_thumbnail(path)
        .map_err(|error| SemanticError::InvalidInput(format!("{}: {error}", path.display())))
}

fn model_metadata(installed: bool, size: Option<u64>) -> ModelMetadata {
    ModelMetadata {
        name: MODEL_NAME.into(),
        version: MODEL_VERSION.into(),
        analysis_version: ANALYSIS_VERSION.into(),
        license: Some("Apache-2.0".into()),
        installed,
        model_size_bytes: size,
        model_sha256: Some(MODEL_SHA256.into()),
        supported_backends: vec![ExecutionBackend::Cpu, ExecutionBackend::DirectMl],
    }
}

fn face_model_metadata(installed: bool, size: Option<u64>) -> ModelMetadata {
    ModelMetadata {
        name: FACE_MODEL_NAME.into(),
        version: FACE_MODEL_VERSION.into(),
        analysis_version: ANALYSIS_VERSION.into(),
        license: Some("MIT".into()),
        installed,
        model_size_bytes: size,
        model_sha256: Some(FACE_MODEL_SHA256.into()),
        supported_backends: vec![ExecutionBackend::Cpu, ExecutionBackend::DirectMl],
    }
}

fn load_coco_labels(path: &Path) -> Result<Vec<String>, SemanticError> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| SemanticError::Integrity(format!("{}: {error}", path.display())))?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn create_session(
    path: &Path,
    model_name: &str,
    backend: ExecutionBackend,
) -> Result<Session, SemanticError> {
    let mut builder = crate::semantic::build_session_builder(model_name, backend)?;
    builder.commit_from_file(path).map_err(|error| {
        SemanticError::Inference(format!("could not load {model_name} ONNX graph: {error}"))
    })
}

fn validate_yunet_contract(session: &Session) -> Result<(String, Vec<String>), SemanticError> {
    let inputs = session
        .inputs()
        .iter()
        .map(|outlet| outlet.name().to_owned())
        .collect::<Vec<_>>();
    let outputs = session
        .outputs()
        .iter()
        .map(|outlet| outlet.name().to_owned())
        .collect::<Vec<_>>();
    let input = inputs.iter().find(|name| name.as_str() == "input").cloned();
    let ordered_outputs = YUNET_OUTPUT_NAMES
        .iter()
        .map(|expected| {
            outputs
                .iter()
                .find(|name| name.as_str() == *expected)
                .cloned()
        })
        .collect::<Option<Vec<_>>>();
    match (input, ordered_outputs) {
        (Some(input), Some(outputs)) => Ok((input, outputs)),
        _ => Err(SemanticError::Inference(format!(
            "YuNet ONNX contract mismatch: inputs={inputs:?}, outputs={outputs:?}"
        ))),
    }
}

fn validate_picodet_contract(session: &Session) -> Result<(String, String, String), SemanticError> {
    let inputs = session
        .inputs()
        .iter()
        .map(|outlet| outlet.name().to_owned())
        .collect::<Vec<_>>();
    let outputs = session
        .outputs()
        .iter()
        .map(|outlet| outlet.name().to_owned())
        .collect::<Vec<_>>();
    let image = inputs.iter().find(|name| name.as_str() == "image").cloned();
    let scale_factor = inputs
        .iter()
        .find(|name| name.as_str() == "scale_factor")
        .cloned();
    let output = outputs.first().cloned();
    match (image, scale_factor, output) {
        (Some(image), Some(scale_factor), Some(output)) => Ok((image, scale_factor, output)),
        _ => Err(SemanticError::Inference(format!(
            "PicoDet ONNX contract mismatch: inputs={inputs:?}, outputs={outputs:?}"
        ))),
    }
}

fn preprocess_pico(image: &image::RgbImage) -> Vec<f32> {
    let resized = image::imageops::resize(
        image,
        PICO_IMAGE_SIZE as u32,
        PICO_IMAGE_SIZE as u32,
        FilterType::Triangle,
    );
    let mut values = Vec::with_capacity(3 * PICO_IMAGE_SIZE * PICO_IMAGE_SIZE);
    for channel in 0..3 {
        for pixel in resized.pixels() {
            values.push(f32::from(pixel[channel]) / 255.0);
        }
    }
    values
}

fn preprocess_yunet(image: &image::RgbImage) -> Vec<f32> {
    let resized = image::imageops::resize(
        image,
        FACE_IMAGE_SIZE as u32,
        FACE_IMAGE_SIZE as u32,
        FilterType::Triangle,
    );
    // YuNet is exported through OpenCV's BGR face-detector path. The model
    // expects the original 0..255 scale, in channel-first BGR order.
    let mut values = Vec::with_capacity(3 * FACE_IMAGE_SIZE * FACE_IMAGE_SIZE);
    for channel in [2_usize, 1, 0] {
        for pixel in resized.pixels() {
            values.push(f32::from(pixel[channel]));
        }
    }
    values
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Detection {
    class_id: usize,
    score: f32,
    bbox: [f32; 4],
}

fn parse_picodet_output(shape: &[i64], data: &[f32]) -> Result<Vec<Detection>, SemanticError> {
    if shape.last().copied() != Some(6) || !data.len().is_multiple_of(6) {
        return Err(SemanticError::Inference(format!(
            "unexpected PicoDet output shape {shape:?}; expected [..., 6]"
        )));
    }
    let mut detections = Vec::new();
    for row in data.chunks_exact(6) {
        let class_id = row[0].round();
        let score = row[1];
        if !class_id.is_finite()
            || class_id < 0.0
            || class_id >= COCO_LABEL_COUNT as f32
            || !score.is_finite()
            || score < DETECTION_SCORE_THRESHOLD
        {
            continue;
        }
        let bbox = [row[2], row[3], row[4], row[5]];
        if bbox.iter().any(|value| !value.is_finite()) || bbox[2] <= bbox[0] || bbox[3] <= bbox[1] {
            continue;
        }
        detections.push(Detection {
            class_id: class_id as usize,
            score,
            bbox,
        });
    }
    Ok(detections)
}

fn parse_yunet_outputs(blobs: &[(Vec<i64>, Vec<f32>)]) -> Result<f32, SemanticError> {
    if blobs.len() != YUNET_OUTPUT_NAMES.len() {
        return Err(SemanticError::Inference(format!(
            "YuNet returned {} outputs; expected {}",
            blobs.len(),
            YUNET_OUTPUT_NAMES.len()
        )));
    }
    let mut best_score = 0.0_f32;
    for (stride_index, stride) in YUNET_STRIDES.iter().enumerate() {
        let rows = FACE_IMAGE_SIZE / stride;
        let cols = FACE_IMAGE_SIZE / stride;
        let count = rows * cols;
        let cls = checked_yunet_blob(&blobs[stride_index], count, 1, "cls")?;
        let obj = checked_yunet_blob(&blobs[3 + stride_index], count, 1, "obj")?;
        let bbox = checked_yunet_blob(&blobs[6 + stride_index], count, 4, "bbox")?;
        let kps = checked_yunet_blob(&blobs[9 + stride_index], count, 10, "kps")?;
        for index in 0..count {
            // This is the same decode used by OpenCV's FaceDetectorYN:
            // clamp class/objectness probabilities, combine them, then use
            // the anchor-grid offsets for the box and landmarks. The box and
            // landmarks are intentionally discarded after decoding because
            // this product only needs an anonymous single-person signal.
            let cls_score = cls[index].clamp(0.0, 1.0);
            let obj_score = obj[index].clamp(0.0, 1.0);
            let score = (cls_score * obj_score).sqrt();
            if score < FACE_SCORE_THRESHOLD {
                continue;
            }
            let row = index / cols;
            let col = index % cols;
            let _x = ((col as f32 + bbox[index * 4]) * *stride as f32)
                - bbox[index * 4 + 2].exp() * *stride as f32 / 2.0;
            let _y = ((row as f32 + bbox[index * 4 + 1]) * *stride as f32)
                - bbox[index * 4 + 3].exp() * *stride as f32 / 2.0;
            let _landmark_anchor = kps[index * 10];
            best_score = best_score.max(score);
        }
    }
    Ok(best_score)
}

fn checked_yunet_blob<'a>(
    blob: &'a (Vec<i64>, Vec<f32>),
    expected_instances: usize,
    values_per_instance: usize,
    kind: &str,
) -> Result<&'a [f32], SemanticError> {
    let (shape, data) = blob;
    if shape.last().copied() != Some(values_per_instance as i64)
        || data.len() != expected_instances * values_per_instance
    {
        return Err(SemanticError::Inference(format!(
            "unexpected YuNet {kind} output shape {shape:?}; expected [{expected_instances}, {values_per_instance}]"
        )));
    }
    Ok(data)
}

fn aggregate_subjects(detections: &[Detection], face_score: f32) -> SubjectAnalysisOutput {
    let mut class_scores = [0.0_f32; COCO_LABEL_COUNT];
    let mut person_detections = Vec::new();
    for detection in detections {
        class_scores[detection.class_id] = class_scores[detection.class_id].max(detection.score);
        if detection.class_id == 0 && detection.score >= PERSON_SCORE_THRESHOLD {
            person_detections.push(*detection);
        }
    }
    person_detections.sort_by(|left, right| right.score.total_cmp(&left.score));
    let mut distinct_persons = Vec::with_capacity(person_detections.len());
    for detection in person_detections {
        if distinct_persons.iter().all(|kept: &Detection| {
            bbox_iou(&kept.bbox, &detection.bbox) < PERSON_DUPLICATE_IOU_THRESHOLD
        }) {
            distinct_persons.push(detection);
        }
    }

    let mut predictions = Vec::new();
    match distinct_persons.as_slice() {
        [] if face_score >= FACE_SCORE_THRESHOLD => {
            // A clear face without a surviving full-body person box is still
            // useful evidence for the mutually exclusive single-person tag.
            predictions.push(prediction("single_person", face_score));
        }
        [] => {}
        [detection] => {
            predictions.push(prediction("single_person", detection.score.max(face_score)));
        }
        detections => {
            // The second strongest box is a conservative confidence for the
            // presence of more than one person.
            predictions.push(prediction("multiple_people", detections[1].score));
        }
    }
    if let Some(score) = max_for_classes(&class_scores, ANIMAL_CLASSES) {
        predictions.push(prediction("animal", score));
    }
    if let Some(score) = max_for_classes(&class_scores, VEHICLE_CLASSES) {
        predictions.push(prediction("vehicle", score));
    }
    if let Some(score) = max_for_classes(&class_scores, FOOD_CLASSES) {
        predictions.push(prediction("food", score));
    }
    if let Some(score) = max_for_classes(&class_scores, PLANT_CLASSES) {
        predictions.push(prediction("plant", score));
    }
    SubjectAnalysisOutput { predictions }
}

fn bbox_iou(left: &[f32; 4], right: &[f32; 4]) -> f32 {
    let intersection_left = left[0].max(right[0]);
    let intersection_top = left[1].max(right[1]);
    let intersection_right = left[2].min(right[2]);
    let intersection_bottom = left[3].min(right[3]);
    let intersection_width = (intersection_right - intersection_left).max(0.0);
    let intersection_height = (intersection_bottom - intersection_top).max(0.0);
    let intersection = intersection_width * intersection_height;
    let left_area = (left[2] - left[0]).max(0.0) * (left[3] - left[1]).max(0.0);
    let right_area = (right[2] - right[0]).max(0.0) * (right[3] - right[1]).max(0.0);
    let union = left_area + right_area - intersection;
    if union <= f32::EPSILON {
        0.0
    } else {
        intersection / union
    }
}

fn max_for_classes(scores: &[f32; COCO_LABEL_COUNT], classes: &[usize]) -> Option<f32> {
    let score = classes
        .iter()
        .filter_map(|class_id| scores.get(*class_id).copied())
        .fold(0.0_f32, f32::max);
    (score >= DETECTION_SCORE_THRESHOLD).then_some(score)
}

fn prediction(label_id: &str, similarity: f32) -> SubjectPrediction {
    let descriptor = SUBJECT_LABELS
        .iter()
        .find(|label| label.id == label_id)
        .expect("subject prediction must be declared in catalog");
    SubjectPrediction {
        label_id: label_id.into(),
        display_name: descriptor.display_name.into(),
        category_group: "subject".into(),
        similarity,
        threshold: descriptor.threshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_catalog_is_non_primary_and_chinese() {
        let catalog = subject_catalog();
        assert_eq!(catalog.len(), 6);
        assert_eq!(
            catalog
                .iter()
                .map(|label| (label.id.as_str(), label.display_name.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("single_person", "单人"),
                ("multiple_people", "多人"),
                ("animal", "动物"),
                ("vehicle", "车辆"),
                ("food", "食物"),
                ("plant", "植物"),
            ]
        );
        assert!(
            catalog
                .iter()
                .all(|label| label.category_group == "subject")
        );
        assert!(catalog.iter().all(|label| !label.is_primary_category));
        assert!(
            catalog
                .iter()
                .all(|label| label.display_name.chars().any(|c| c >= '\u{4e00}'))
        );
    }

    #[test]
    fn directml_failure_transitions_to_cpu_once() {
        let mut state = SubjectBackendState::Active(ExecutionBackend::DirectMl);
        let error = SemanticError::Inference("Squeeze_4 failed with 0x80070057".into());
        let decision = begin_subject_cpu_fallback(&mut state, &error);

        assert_eq!(decision, SubjectFallbackDecision::RetryCpu);
        assert!(matches!(
            state,
            SubjectBackendState::SwitchingToCpu { ref reason }
                if reason.contains("Squeeze_4") && reason.contains("0x80070057")
        ));
        let second_error = SemanticError::Inference("second DirectML failure".into());
        assert_eq!(
            begin_subject_cpu_fallback(&mut state, &second_error),
            SubjectFallbackDecision::AlreadyAttempted
        );
        state = SubjectBackendState::CpuFallback {
            reason: "Squeeze_4 failed with 0x80070057".into(),
        };
        let third_error = SemanticError::Inference("third DirectML failure".into());
        assert_eq!(
            begin_subject_cpu_fallback(&mut state, &third_error),
            SubjectFallbackDecision::AlreadyAttempted
        );
    }

    #[test]
    fn cpu_execution_does_not_request_a_directml_fallback() {
        let mut state = SubjectBackendState::Active(ExecutionBackend::Cpu);
        let error = SemanticError::Inference("CPU inference failure".into());

        assert_eq!(
            begin_subject_cpu_fallback(&mut state, &error),
            SubjectFallbackDecision::NotRequired
        );
        assert_eq!(state, SubjectBackendState::Active(ExecutionBackend::Cpu));
    }

    #[test]
    fn invalid_thumbnail_does_not_request_a_directml_fallback() {
        let mut state = SubjectBackendState::Active(ExecutionBackend::DirectMl);
        let error = SemanticError::InvalidInput("bad grid thumbnail".into());

        assert_eq!(
            begin_subject_cpu_fallback(&mut state, &error),
            SubjectFallbackDecision::NotRequired
        );
        assert_eq!(
            state,
            SubjectBackendState::Active(ExecutionBackend::DirectMl)
        );
    }

    #[test]
    fn broken_subject_thumbnail_is_reported_as_invalid_input() {
        let error = load_subject_thumbnail(Path::new("test-data/missing-subject-grid-640-v1.jpg"))
            .expect_err("missing subject thumbnail must fail");

        assert!(matches!(error, SemanticError::InvalidInput(_)));
    }

    #[test]
    fn cpu_batch_retry_failure_keeps_fallback_for_next_thumbnail() {
        let state = Mutex::new(SubjectBackendState::Active(ExecutionBackend::DirectMl));
        let result: Result<(), SemanticError> = run_subject_batch_with_fallback(
            &state,
            ExecutionBackend::DirectMl,
            || Err(SemanticError::Inference("DirectML batch failure".into())),
            || Ok(()),
            || Err(SemanticError::Inference("CPU batch retry failure".into())),
        );

        assert!(matches!(
            result,
            Err(SemanticError::Inference(message))
                if message.contains("CPU batch retry failure")
        ));
        assert!(matches!(
            &*state.lock(),
            SubjectBackendState::CpuFallback { reason }
                if reason.contains("DirectML batch failure")
        ));

        let recovered = run_subject_batch_with_fallback(
            &state,
            ExecutionBackend::Cpu,
            || Ok::<_, SemanticError>("single thumbnail recovered"),
            || Ok::<(), SemanticError>(()),
            || Ok::<_, SemanticError>("unused CPU closure"),
        )
        .expect("next thumbnail should be allowed to recover on CPU");
        assert_eq!(recovered, "single thumbnail recovered");
    }

    #[test]
    fn concurrent_fallback_requests_have_one_state_transition() {
        let state = std::sync::Arc::new(Mutex::new(SubjectBackendState::Active(
            ExecutionBackend::DirectMl,
        )));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let mut handles = Vec::new();
        for index in 0..2 {
            let state = std::sync::Arc::clone(&state);
            let barrier = std::sync::Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                let error = SemanticError::Inference(format!("concurrent failure {index}"));
                let mut state = state.lock();
                begin_subject_cpu_fallback(&mut state, &error)
            }));
        }

        let decisions = handles
            .into_iter()
            .map(|handle| handle.join().expect("fallback transition thread"))
            .collect::<Vec<_>>();
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == SubjectFallbackDecision::RetryCpu)
                .count(),
            1
        );
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == SubjectFallbackDecision::AlreadyAttempted)
                .count(),
            1
        );
    }

    #[test]
    fn cpu_fallback_rechecks_subject_model_hashes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let detector_path = temp.path().join(MODEL_FILE);
        std::fs::write(&detector_path, b"changed model").expect("write invalid model fixture");

        let error = verify_subject_model_resources(&detector_path, None)
            .expect_err("changed model must fail integrity validation");
        assert!(matches!(error, SemanticError::Integrity(_)));
    }

    #[test]
    fn detections_are_aggregated_without_forcing_a_primary_scene() {
        let output = aggregate_subjects(
            &[
                detection(0, 0.91, [0.0, 0.0, 0.4, 1.0]),
                detection(0, 0.86, [0.6, 0.0, 1.0, 1.0]),
                detection(2, 0.88, [0.0, 0.0, 1.0, 1.0]),
                detection(16, 0.79, [0.0, 0.0, 1.0, 1.0]),
            ],
            0.82,
        );
        let labels = output
            .predictions
            .iter()
            .map(|prediction| prediction.label_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, &["multiple_people", "animal", "vehicle"]);
    }

    #[test]
    fn person_labels_are_mutually_exclusive_and_face_fallback_is_single_person() {
        let one_person = aggregate_subjects(&[detection(0, 0.91, [0.0, 0.0, 1.0, 1.0])], 0.82);
        assert_eq!(
            one_person
                .predictions
                .iter()
                .map(|prediction| prediction.label_id.as_str())
                .collect::<Vec<_>>(),
            vec!["single_person"]
        );

        let face_only = aggregate_subjects(&[], 0.82);
        assert_eq!(face_only.predictions[0].label_id, "single_person");
        assert!(
            !face_only
                .predictions
                .iter()
                .any(|prediction| prediction.label_id == "multiple_people")
        );
    }

    #[test]
    fn overlapping_person_boxes_do_not_count_the_same_person_twice() {
        let output = aggregate_subjects(
            &[
                detection(0, 0.91, [0.0, 0.0, 1.0, 1.0]),
                detection(0, 0.86, [0.02, 0.02, 0.98, 0.98]),
            ],
            0.0,
        );

        assert_eq!(
            output
                .predictions
                .iter()
                .map(|prediction| prediction.label_id.as_str())
                .collect::<Vec<_>>(),
            vec!["single_person"]
        );
    }

    #[test]
    fn ordinary_objects_cannot_create_a_person_label() {
        let output = aggregate_subjects(&[detection(2, 0.99, [0.0, 0.0, 1.0, 1.0])], 0.0);

        assert_eq!(
            output
                .predictions
                .iter()
                .map(|prediction| prediction.label_id.as_str())
                .collect::<Vec<_>>(),
            vec!["vehicle"]
        );
    }

    #[test]
    fn unsupported_detections_are_ignored() {
        let output = parse_picodet_output(&[1, 2, 6], &[100.0, 0.9, 0.0, 0.0, 1.0, 1.0])
            .expect("valid shape");
        assert_eq!(output.len(), 0);
    }

    fn detection(class_id: usize, score: f32, bbox: [f32; 4]) -> Detection {
        Detection {
            class_id,
            score,
            bbox,
        }
    }

    #[test]
    fn yunet_decoder_accepts_official_multiscale_outputs() {
        let mut cls_outputs = Vec::new();
        let mut obj_outputs = Vec::new();
        for stride in YUNET_STRIDES {
            let count = (FACE_IMAGE_SIZE / stride).pow(2);
            let mut cls = vec![0.0; count];
            cls[0] = 1.0;
            let mut obj = vec![0.0; count];
            obj[0] = 1.0;
            cls_outputs.push((vec![1, count as i64, 1], cls));
            obj_outputs.push((vec![1, count as i64, 1], obj));
        }
        let mut ordered = Vec::with_capacity(YUNET_OUTPUT_NAMES.len());
        ordered.extend(cls_outputs);
        ordered.extend(obj_outputs);
        for stride in YUNET_STRIDES {
            let count = (FACE_IMAGE_SIZE / stride).pow(2);
            ordered.push((vec![1, count as i64, 4], vec![0.0; count * 4]));
        }
        for stride in YUNET_STRIDES {
            let count = (FACE_IMAGE_SIZE / stride).pow(2);
            ordered.push((vec![1, count as i64, 10], vec![0.0; count * 10]));
        }

        assert_eq!(parse_yunet_outputs(&ordered).expect("decode YuNet"), 1.0);
    }
}
