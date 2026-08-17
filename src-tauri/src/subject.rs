use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::semantic::{ExecutionBackend, ModelMetadata, SemanticError, SemanticLabelDescriptor};

pub const MODEL_NAME: &str = "PicoDet-S-COCO";
pub const MODEL_VERSION: &str = "onnx-2026-08-10";
pub const ANALYSIS_VERSION: &str = "photo-organizer-subject-picodet-yunet-v1";
pub const TAXONOMY_VERSION: &str = "photo-organizer-subject-tags-v2";
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

const VEHICLE_CLASSES: &[usize] = &[1, 2, 3, 4, 5, 6, 7, 8];
const ANIMAL_CLASSES: &[usize] = &[14, 15, 16, 17, 18, 19, 20, 21, 22, 23];
const FOOD_CLASSES: &[usize] = &[39, 40, 41, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55];
const PLANT_CLASSES: &[usize] = &[58];

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
        display_name: "食品",
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

pub struct SubjectModel {
    detector: Mutex<Session>,
    detector_input_name: String,
    scale_factor_input_name: String,
    detector_output_name: String,
    face_detector: Option<Mutex<Session>>,
    face_input_name: Option<String>,
    face_output_names: Option<Vec<String>>,
    model_size_bytes: u64,
    face_model_size_bytes: Option<u64>,
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
        let model_path = model_dir.join(MODEL_FILE);
        let labels_path = model_dir.join(LABELS_FILE);
        crate::semantic::verify_sha256(&model_path, MODEL_SHA256)?;
        crate::semantic::verify_sha256(runtime_path, crate::semantic::RUNTIME_SHA256)?;
        let labels = load_coco_labels(&labels_path)?;
        if labels.len() != COCO_LABEL_COUNT {
            return Err(SemanticError::Inference(format!(
                "COCO label resource contains {} labels; expected {COCO_LABEL_COUNT}",
                labels.len()
            )));
        }
        crate::semantic::initialize_ort(runtime_path)?;

        let detector = create_session(&model_path, "PicoDet")?;
        let (detector_input_name, scale_factor_input_name, detector_output_name) =
            validate_picodet_contract(&detector)?;

        let face_model_path = face_model_dir.join(FACE_MODEL_FILE);
        let (face_detector, face_input_name, face_output_names, face_model_size_bytes) =
            match crate::semantic::verify_sha256(&face_model_path, FACE_MODEL_SHA256)
                .and_then(|_| create_session(&face_model_path, "YuNet"))
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

        let model_size_bytes = std::fs::metadata(model_path)
            .map_err(|error| SemanticError::Inference(error.to_string()))?
            .len();
        Ok(Self {
            detector: Mutex::new(detector),
            detector_input_name,
            scale_factor_input_name,
            detector_output_name,
            face_detector,
            face_input_name,
            face_output_names,
            model_size_bytes,
            face_model_size_bytes,
        })
    }

    pub fn model_contract(&self) -> (String, String) {
        (
            self.detector_input_name.clone(),
            self.detector_output_name.clone(),
        )
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
        let (status, message) = if self.face_detector.is_some() {
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
        SubjectRuntimeStatus {
            status: status.into(),
            message: message.into(),
            model: self.metadata(),
            face_model: self.face_metadata(),
            selected_backend: Some(ExecutionBackend::Cpu),
        }
    }

    fn classify_batch(
        &self,
        images: &[PathBuf],
        backend: ExecutionBackend,
    ) -> Result<Vec<SubjectAnalysisOutput>, SemanticError> {
        if !matches!(backend, ExecutionBackend::Auto | ExecutionBackend::Cpu) {
            return Err(SemanticError::BackendUnavailable(backend));
        }
        let mut results = Vec::with_capacity(images.len());
        for path in images {
            let rgb = crate::imaging::load_analysis_thumbnail(path).map_err(|error| {
                SemanticError::Inference(format!("{}: {error}", path.display()))
            })?;
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
                    .map_err(crate::semantic::inference_error)?;
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
                        .map_err(crate::semantic::inference_error)?;
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

fn model_metadata(installed: bool, size: Option<u64>) -> ModelMetadata {
    ModelMetadata {
        name: MODEL_NAME.into(),
        version: MODEL_VERSION.into(),
        analysis_version: ANALYSIS_VERSION.into(),
        license: Some("Apache-2.0".into()),
        installed,
        model_size_bytes: size,
        model_sha256: Some(MODEL_SHA256.into()),
        supported_backends: vec![ExecutionBackend::Cpu],
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
        supported_backends: vec![ExecutionBackend::Cpu],
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

fn create_session(path: &Path, model_name: &str) -> Result<Session, SemanticError> {
    let builder = Session::builder().map_err(|error| {
        SemanticError::Inference(format!(
            "could not create {model_name} ONNX session: {error}"
        ))
    })?;
    let builder = builder
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|error| {
            SemanticError::Inference(format!(
                "could not optimize {model_name} ONNX graph: {error}"
            ))
        })?;
    let mut builder = builder
        .with_intra_threads(crate::semantic::cpu_thread_count())
        .map_err(|error| {
            SemanticError::Inference(format!(
                "could not configure {model_name} ONNX threads: {error}"
            ))
        })?;
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

fn parse_picodet_output(shape: &[i64], data: &[f32]) -> Result<Vec<(usize, f32)>, SemanticError> {
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
        detections.push((class_id as usize, score));
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

fn aggregate_subjects(detections: &[(usize, f32)], face_score: f32) -> SubjectAnalysisOutput {
    let mut class_scores = [0.0_f32; COCO_LABEL_COUNT];
    let mut person_scores = Vec::new();
    for (class_id, score) in detections {
        class_scores[*class_id] = class_scores[*class_id].max(*score);
        if *class_id == 0 && *score >= PERSON_SCORE_THRESHOLD {
            person_scores.push(*score);
        }
    }
    person_scores.sort_by(|left, right| right.total_cmp(left));

    let mut predictions = Vec::new();
    match person_scores.as_slice() {
        [] if face_score >= FACE_SCORE_THRESHOLD => {
            // A clear face without a surviving full-body person box is still
            // useful evidence for the mutually exclusive single-person tag.
            predictions.push(prediction("single_person", face_score));
        }
        [] => {}
        [score] => {
            predictions.push(prediction("single_person", (*score).max(face_score)));
        }
        scores => {
            // The second strongest box is a conservative confidence for the
            // presence of more than one person.
            predictions.push(prediction("multiple_people", scores[1]));
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
    fn detections_are_aggregated_without_forcing_a_primary_scene() {
        let output = aggregate_subjects(&[(0, 0.91), (0, 0.86), (2, 0.88), (16, 0.79)], 0.82);
        let labels = output
            .predictions
            .iter()
            .map(|prediction| prediction.label_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, &["multiple_people", "animal", "vehicle"]);
    }

    #[test]
    fn person_labels_are_mutually_exclusive_and_face_fallback_is_single_person() {
        let one_person = aggregate_subjects(&[(0, 0.91)], 0.82);
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
    fn unsupported_detections_are_ignored() {
        let output = parse_picodet_output(&[1, 2, 6], &[100.0, 0.9, 0.0, 0.0, 1.0, 1.0])
            .expect("valid shape");
        assert_eq!(output.len(), 0);
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
