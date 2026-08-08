use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use image::imageops::FilterType;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

pub const MODEL_NAME: &str = "TinyCLIP-ViT-8M-16-Text-3M-YFCC15M";
pub const MODEL_VERSION: &str = "onnx-int8-2025-08-06";
pub const ANALYSIS_VERSION: &str = "photo-organizer-semantic-v2";
pub const MODEL_FILE: &str = "model-int8.onnx";
pub const TOKENIZER_FILE: &str = "tokenizer.json";
pub const MODEL_SHA256: &str = "10921310ddef06557ec1598d1260470a0a4db53f70ffe0deb60b946dcad6d27a";
pub const TOKENIZER_SHA256: &str =
    "6d9109cc838977f3ca94a379eec36aecc7c807e1785cd729660ca2fc0171fb35";
pub const RUNTIME_SHA256: &str = "8a1aad8d59d02a5337d4e3f5bbd1158c3f7bf84fe3b3f0052f957dd3e75a91cb";
pub const EMBEDDING_DIMENSIONS: usize = 512;

const IMAGE_SIZE: usize = 224;
const TOKEN_LENGTH: usize = 77;
const PAD_TOKEN_ID: u32 = 49_407;
const MAX_LABELS: usize = 4;
const TOP_SCORE_WINDOW: f32 = 0.055;
const IMAGE_MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const IMAGE_STD: [f32; 3] = [0.268_629_54, 0.261_302_6, 0.275_777_1];

static ORT_RUNTIME: OnceLock<Result<PathBuf, String>> = OnceLock::new();

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionBackend {
    Auto,
    Cpu,
    DirectMl,
    Cuda,
    CoreMl,
    Npu,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetadata {
    pub name: String,
    pub version: String,
    pub analysis_version: String,
    pub license: Option<String>,
    pub installed: bool,
    pub model_size_bytes: Option<u64>,
    pub model_sha256: Option<String>,
    pub supported_backends: Vec<ExecutionBackend>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticPrediction {
    pub label_id: String,
    pub display_name: String,
    pub similarity: f32,
    pub threshold: f32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSimilarity {
    pub label_id: String,
    pub display_name: String,
    pub similarity: f32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticAnalysisOutput {
    pub predictions: Vec<SemanticPrediction>,
    pub embedding: Vec<f32>,
    pub raw_similarities: Vec<SemanticSimilarity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticRuntimeStatus {
    pub status: String,
    pub message: String,
    pub model: ModelMetadata,
    pub selected_backend: Option<ExecutionBackend>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLabelDescriptor {
    pub id: String,
    pub display_name: String,
    pub threshold: f32,
    pub is_primary_category: bool,
}

#[derive(Debug, Clone, Copy)]
struct LabelDefinition {
    id: &'static str,
    display_name: &'static str,
    prompt: &'static str,
    threshold: f32,
}

const LABELS: [LabelDefinition; 21] = [
    LabelDefinition {
        id: "portrait",
        display_name: "人像",
        prompt: "a portrait photograph of one person",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "group",
        display_name: "多人",
        prompt: "a photograph of a group of people",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "landscape",
        display_name: "风景",
        prompt: "a landscape photography scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "architecture",
        display_name: "城市 / 建筑",
        prompt: "an architectural photograph of a building",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "indoor",
        display_name: "室内",
        prompt: "an indoor room or interior photograph",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "street",
        display_name: "街道",
        prompt: "a street photography scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "vehicle",
        display_name: "车辆",
        prompt: "a photograph of a car, truck, train, or other vehicle",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "product",
        display_name: "静物 / 产品",
        prompt: "a commercial product photograph",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "still_life",
        display_name: "静物",
        prompt: "a still life photograph of arranged objects",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "food",
        display_name: "食品",
        prompt: "a food photography image",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "animal",
        display_name: "动物",
        prompt: "a photograph of an animal",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "screenshot",
        display_name: "截图",
        prompt: "a computer or phone screenshot of a user interface",
        threshold: 0.17,
    },
    LabelDefinition {
        id: "document",
        display_name: "文档 / 截图",
        prompt: "a scanned document or a photographed page with text",
        threshold: 0.17,
    },
    LabelDefinition {
        id: "night",
        display_name: "夜景",
        prompt: "a night photography scene after dark",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "flower",
        display_name: "花卉",
        prompt: "a close photograph of flowers or blossoms",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "mountain",
        display_name: "山",
        prompt: "a mountain landscape photograph",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "water",
        display_name: "水体",
        prompt: "a photograph of the ocean, a lake, river, or other water",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "forest",
        display_name: "森林",
        prompt: "a forest or woodland photograph",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "sunset",
        display_name: "日落",
        prompt: "a sunset or sunrise photograph",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "abstract",
        display_name: "抽象",
        prompt: "an abstract image with shapes, patterns, or textures",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "unknown",
        display_name: "未知",
        prompt: "an unrecognizable or unclassifiable image",
        threshold: 0.16,
    },
];

#[derive(Debug, thiserror::Error)]
pub enum SemanticError {
    #[error("semantic model is not installed or enabled")]
    ModelUnavailable,
    #[error("execution backend is unavailable: {0:?}")]
    BackendUnavailable(ExecutionBackend),
    #[error("semantic model integrity check failed: {0}")]
    Integrity(String),
    #[error("semantic inference failed: {0}")]
    Inference(String),
}

pub trait SemanticClassifier: Send + Sync {
    fn metadata(&self) -> ModelMetadata;
    fn status(&self) -> SemanticRuntimeStatus;
    fn classify_batch(
        &self,
        images: &[PathBuf],
        backend: ExecutionBackend,
    ) -> Result<Vec<SemanticAnalysisOutput>, SemanticError>;
}

pub fn semantic_catalog() -> Vec<SemanticLabelDescriptor> {
    LABELS
        .iter()
        .map(|label| SemanticLabelDescriptor {
            id: label.id.into(),
            display_name: label.display_name.into(),
            threshold: label.threshold,
            is_primary_category: is_primary_category(label.id),
        })
        .collect()
}

const PRIMARY_CATEGORY_IDS: [&str; 7] = [
    "portrait",
    "landscape",
    "architecture",
    "product",
    "animal",
    "document",
    "unknown",
];

fn is_primary_category(label_id: &str) -> bool {
    PRIMARY_CATEGORY_IDS.contains(&label_id)
}

#[derive(Debug, Default)]
pub struct UnavailableClassifier {
    message: Option<String>,
}

impl UnavailableClassifier {
    pub fn with_message(message: impl Into<String>) -> Self {
        Self {
            message: Some(message.into()),
        }
    }
}

impl SemanticClassifier for UnavailableClassifier {
    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            name: MODEL_NAME.into(),
            version: MODEL_VERSION.into(),
            analysis_version: ANALYSIS_VERSION.into(),
            license: Some("MIT".into()),
            installed: false,
            model_size_bytes: None,
            model_sha256: Some(MODEL_SHA256.into()),
            supported_backends: vec![ExecutionBackend::Cpu],
        }
    }

    fn status(&self) -> SemanticRuntimeStatus {
        SemanticRuntimeStatus {
            status: "model_unavailable".into(),
            message: self.message.clone().unwrap_or_else(|| {
                "本地语义模型不可用；基础图库、缩略图和影调色彩分析仍可正常使用。".into()
            }),
            model: self.metadata(),
            selected_backend: None,
        }
    }

    fn classify_batch(
        &self,
        _images: &[PathBuf],
        _backend: ExecutionBackend,
    ) -> Result<Vec<SemanticAnalysisOutput>, SemanticError> {
        Err(SemanticError::ModelUnavailable)
    }
}

pub struct TinyClipClassifier {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    model_size_bytes: u64,
}

impl std::fmt::Debug for TinyClipClassifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TinyClipClassifier")
            .field("model", &MODEL_NAME)
            .field("version", &MODEL_VERSION)
            .finish_non_exhaustive()
    }
}

impl TinyClipClassifier {
    pub fn load(model_dir: &Path, runtime_path: &Path) -> Result<Self, SemanticError> {
        let model_path = model_dir.join(MODEL_FILE);
        let tokenizer_path = model_dir.join(TOKENIZER_FILE);
        verify_sha256(&model_path, MODEL_SHA256)?;
        verify_sha256(&tokenizer_path, TOKENIZER_SHA256)?;
        verify_sha256(runtime_path, RUNTIME_SHA256)?;
        initialize_ort(runtime_path)?;

        let mut tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|error| {
            SemanticError::Inference(format!("could not load tokenizer: {error}"))
        })?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: TOKEN_LENGTH,
                ..TruncationParams::default()
            }))
            .map_err(|error| {
                SemanticError::Inference(format!("could not configure tokenizer: {error}"))
            })?;
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::Fixed(TOKEN_LENGTH),
            pad_id: PAD_TOKEN_ID,
            pad_token: "<|endoftext|>".into(),
            ..PaddingParams::default()
        }));

        let builder = Session::builder().map_err(|error| {
            SemanticError::Inference(format!("could not create ONNX session: {error}"))
        })?;
        let builder = builder
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|error| {
                SemanticError::Inference(format!("could not optimize ONNX graph: {error}"))
            })?;
        let mut builder = builder
            .with_intra_threads(cpu_thread_count())
            .map_err(|error| {
                SemanticError::Inference(format!("could not configure ONNX threads: {error}"))
            })?;
        let session = builder.commit_from_file(&model_path).map_err(|error| {
            SemanticError::Inference(format!("could not load ONNX graph: {error}"))
        })?;
        validate_model_contract(&session)?;

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            model_size_bytes: std::fs::metadata(model_path)
                .map_err(|error| SemanticError::Inference(error.to_string()))?
                .len(),
        })
    }

    pub fn model_contract(&self) -> (Vec<String>, Vec<String>) {
        let session = self.session.lock();
        (
            session
                .inputs()
                .iter()
                .map(|outlet| outlet.name().into())
                .collect(),
            session
                .outputs()
                .iter()
                .map(|outlet| outlet.name().into())
                .collect(),
        )
    }
}

impl SemanticClassifier for TinyClipClassifier {
    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            name: MODEL_NAME.into(),
            version: MODEL_VERSION.into(),
            analysis_version: ANALYSIS_VERSION.into(),
            license: Some("MIT".into()),
            installed: true,
            model_size_bytes: Some(self.model_size_bytes),
            model_sha256: Some(MODEL_SHA256.into()),
            supported_backends: vec![ExecutionBackend::Cpu],
        }
    }

    fn status(&self) -> SemanticRuntimeStatus {
        SemanticRuntimeStatus {
            status: "ready".into(),
            message: "TinyCLIP INT8 已通过完整性校验，CPU 执行后端可用。".into(),
            model: self.metadata(),
            selected_backend: Some(ExecutionBackend::Cpu),
        }
    }

    fn classify_batch(
        &self,
        images: &[PathBuf],
        backend: ExecutionBackend,
    ) -> Result<Vec<SemanticAnalysisOutput>, SemanticError> {
        if !matches!(backend, ExecutionBackend::Auto | ExecutionBackend::Cpu) {
            return Err(SemanticError::BackendUnavailable(backend));
        }
        if images.is_empty() {
            return Ok(Vec::new());
        }

        let (input_ids, attention_mask) = tokenize_prompts(&self.tokenizer)?;
        let pixel_values = preprocess_images(images)?;
        let label_count = LABELS.len();
        let image_count = images.len();

        let input_ids =
            Tensor::from_array(([label_count, TOKEN_LENGTH], input_ids.into_boxed_slice()))
                .map_err(inference_error)?;
        let attention_mask = Tensor::from_array((
            [label_count, TOKEN_LENGTH],
            attention_mask.into_boxed_slice(),
        ))
        .map_err(inference_error)?;
        let pixel_values = Tensor::from_array((
            [image_count, 3, IMAGE_SIZE, IMAGE_SIZE],
            pixel_values.into_boxed_slice(),
        ))
        .map_err(inference_error)?;

        let mut session = self.session.lock();
        let outputs = session
            .run(ort::inputs! {
                "input_ids" => input_ids,
                "pixel_values" => pixel_values,
                "attention_mask" => attention_mask,
            })
            .map_err(inference_error)?;
        let (image_shape, image_data) = outputs["image_embeds"]
            .try_extract_tensor::<f32>()
            .map_err(inference_error)?;
        let (text_shape, text_data) = outputs["text_embeds"]
            .try_extract_tensor::<f32>()
            .map_err(inference_error)?;

        if image_shape.as_ref() != [image_count as i64, EMBEDDING_DIMENSIONS as i64]
            || text_shape.as_ref() != [label_count as i64, EMBEDDING_DIMENSIONS as i64]
        {
            return Err(SemanticError::Inference(format!(
                "unexpected embedding shapes: image={image_shape:?}, text={text_shape:?}"
            )));
        }

        let mut results = Vec::with_capacity(image_count);
        for image_index in 0..image_count {
            let start = image_index * EMBEDDING_DIMENSIONS;
            let embedding = image_data[start..start + EMBEDDING_DIMENSIONS].to_vec();
            let mut scores = Vec::with_capacity(label_count);
            for label_index in 0..label_count {
                let text_start = label_index * EMBEDDING_DIMENSIONS;
                let similarity = cosine_similarity(
                    &embedding,
                    &text_data[text_start..text_start + EMBEDDING_DIMENSIONS],
                );
                scores.push(similarity);
            }
            results.push(SemanticAnalysisOutput {
                predictions: select_predictions(&scores),
                embedding,
                raw_similarities: rank_similarities(&scores),
            });
        }
        Ok(results)
    }
}

fn initialize_ort(runtime_path: &Path) -> Result<(), SemanticError> {
    let canonical = runtime_path.canonicalize().map_err(|error| {
        SemanticError::Inference(format!("could not locate ONNX Runtime: {error}"))
    })?;
    let result = ORT_RUNTIME.get_or_init(|| {
        let builder = ort::init_from(&canonical).map_err(|error| error.to_string())?;
        let _created = builder.with_name("PhotoOrganizer").commit();
        Ok(canonical.clone())
    });
    match result {
        Ok(loaded) if loaded == &canonical => Ok(()),
        Ok(loaded) => Err(SemanticError::Inference(format!(
            "ONNX Runtime was already initialized from {}",
            loaded.display()
        ))),
        Err(error) => Err(SemanticError::Inference(format!(
            "could not initialize ONNX Runtime: {error}"
        ))),
    }
}

fn validate_model_contract(session: &Session) -> Result<(), SemanticError> {
    let input_names = session
        .inputs()
        .iter()
        .map(|input| input.name())
        .collect::<Vec<_>>();
    let output_names = session
        .outputs()
        .iter()
        .map(|output| output.name())
        .collect::<Vec<_>>();
    for name in ["input_ids", "pixel_values", "attention_mask"] {
        if !input_names.contains(&name) {
            return Err(SemanticError::Inference(format!(
                "model input is missing: {name}"
            )));
        }
    }
    for name in ["image_embeds", "text_embeds"] {
        if !output_names.contains(&name) {
            return Err(SemanticError::Inference(format!(
                "model output is missing: {name}"
            )));
        }
    }
    Ok(())
}

fn verify_sha256(path: &Path, expected: &str) -> Result<(), SemanticError> {
    let mut file = File::open(path)
        .map_err(|error| SemanticError::Integrity(format!("{}: {error}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes = file
            .read(&mut buffer)
            .map_err(|error| SemanticError::Integrity(error.to_string()))?;
        if bytes == 0 {
            break;
        }
        hasher.update(&buffer[..bytes]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual == expected {
        Ok(())
    } else {
        Err(SemanticError::Integrity(format!(
            "{} expected {expected}, received {actual}",
            path.display()
        )))
    }
}

fn tokenize_prompts(tokenizer: &Tokenizer) -> Result<(Vec<i64>, Vec<i64>), SemanticError> {
    let prompts = LABELS.iter().map(|label| label.prompt).collect::<Vec<_>>();
    let encodings = tokenizer
        .encode_batch(prompts, true)
        .map_err(|error| SemanticError::Inference(format!("tokenization failed: {error}")))?;
    let mut input_ids = Vec::with_capacity(LABELS.len() * TOKEN_LENGTH);
    let mut attention_mask = Vec::with_capacity(LABELS.len() * TOKEN_LENGTH);
    for encoding in encodings {
        if encoding.len() != TOKEN_LENGTH {
            return Err(SemanticError::Inference(format!(
                "tokenizer returned {} tokens; expected {TOKEN_LENGTH}",
                encoding.len()
            )));
        }
        input_ids.extend(encoding.get_ids().iter().map(|value| i64::from(*value)));
        attention_mask.extend(
            encoding
                .get_attention_mask()
                .iter()
                .map(|value| i64::from(*value)),
        );
    }
    Ok((input_ids, attention_mask))
}

fn preprocess_images(images: &[PathBuf]) -> Result<Vec<f32>, SemanticError> {
    let mut values = Vec::with_capacity(images.len() * 3 * IMAGE_SIZE * IMAGE_SIZE);
    for path in images {
        let image = image::open(path)
            .map_err(|error| SemanticError::Inference(format!("{}: {error}", path.display())))?
            .to_rgb8();
        let shortest = image.width().min(image.height());
        if shortest == 0 {
            return Err(SemanticError::Inference(format!(
                "image has zero dimensions: {}",
                path.display()
            )));
        }
        let scale = IMAGE_SIZE as f64 / f64::from(shortest);
        let resized_width = (f64::from(image.width()) * scale)
            .round()
            .max(IMAGE_SIZE as f64) as u32;
        let resized_height = (f64::from(image.height()) * scale)
            .round()
            .max(IMAGE_SIZE as f64) as u32;
        let resized = image::imageops::resize(
            &image,
            resized_width,
            resized_height,
            FilterType::CatmullRom,
        );
        let left = (resized_width - IMAGE_SIZE as u32) / 2;
        let top = (resized_height - IMAGE_SIZE as u32) / 2;
        let cropped =
            image::imageops::crop_imm(&resized, left, top, IMAGE_SIZE as u32, IMAGE_SIZE as u32)
                .to_image();
        for channel in 0..3 {
            for pixel in cropped.pixels() {
                let normalized =
                    (f32::from(pixel[channel]) / 255.0 - IMAGE_MEAN[channel]) / IMAGE_STD[channel];
                values.push(normalized);
            }
        }
    }
    Ok(values)
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (left, right) in left.iter().zip(right) {
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    let denominator = left_norm.sqrt() * right_norm.sqrt();
    if denominator <= f32::EPSILON {
        0.0
    } else {
        dot / denominator
    }
}

fn select_predictions(scores: &[f32]) -> Vec<SemanticPrediction> {
    let unknown_index = LABELS.len() - 1;
    let mut concrete_primaries = scores
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            is_primary_category(LABELS[*index].id) && LABELS[*index].id != "unknown"
        })
        .map(|(index, score)| (index, *score))
        .collect::<Vec<_>>();
    concrete_primaries.sort_by(|left, right| right.1.total_cmp(&left.1));
    let confident_primary = concrete_primaries.first().is_some_and(|(index, score)| {
        *score >= LABELS[*index].threshold
            && concrete_primaries
                .get(1)
                .is_none_or(|(_, second_score)| *score - *second_score >= TOP_SCORE_WINDOW)
    });
    let best_primary = scores
        .iter()
        .enumerate()
        .filter(|(index, _)| is_primary_category(LABELS[*index].id))
        .max_by(|left, right| left.1.total_cmp(right.1).then(right.0.cmp(&left.0)))
        .map(|(index, score)| (index, *score));
    let mut accepted = scores
        .iter()
        .enumerate()
        .filter(|(index, score)| **score >= LABELS[*index].threshold)
        .map(|(index, score)| (index, *score))
        .collect::<Vec<_>>();
    accepted.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));

    if accepted
        .iter()
        .all(|(index, _)| !is_primary_category(LABELS[*index].id))
        && let Some(primary) = best_primary
    {
        accepted.push(primary);
    }
    if let Some((_, top_score)) = accepted.first().copied() {
        accepted.retain(|(index, score)| {
            *score >= top_score - TOP_SCORE_WINDOW || is_primary_category(LABELS[*index].id)
        });
    }
    if accepted.iter().any(|(index, _)| *index != unknown_index) {
        accepted.retain(|(index, _)| *index != unknown_index);
    }
    accepted.truncate(MAX_LABELS);
    if !accepted
        .iter()
        .any(|(index, _)| is_primary_category(LABELS[*index].id))
        && let Some(primary) = best_primary
    {
        if accepted.len() == MAX_LABELS {
            accepted.pop();
        }
        accepted.push(primary);
        accepted.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
    }
    if accepted.is_empty() {
        accepted.push((
            unknown_index,
            scores.get(unknown_index).copied().unwrap_or(0.0),
        ));
    }

    if !confident_primary {
        accepted.retain(|(index, _)| !is_primary_category(LABELS[*index].id));
        accepted.push((
            unknown_index,
            scores.get(unknown_index).copied().unwrap_or(0.0),
        ));
    }

    let primary_index = accepted
        .iter()
        .filter(|(index, _)| is_primary_category(LABELS[*index].id))
        .max_by(|left, right| left.1.total_cmp(&right.1).then(right.0.cmp(&left.0)))
        .map(|(index, _)| *index);
    accepted
        .into_iter()
        .map(|(index, similarity)| SemanticPrediction {
            label_id: LABELS[index].id.into(),
            display_name: LABELS[index].display_name.into(),
            similarity,
            threshold: LABELS[index].threshold,
            is_primary: primary_index == Some(index),
        })
        .collect()
}

fn rank_similarities(scores: &[f32]) -> Vec<SemanticSimilarity> {
    let mut ranked = scores
        .iter()
        .enumerate()
        .map(|(index, similarity)| SemanticSimilarity {
            label_id: LABELS[index].id.into(),
            display_name: LABELS[index].display_name.into(),
            similarity: *similarity,
            threshold: LABELS[index].threshold,
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
    ranked
}

fn cpu_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get().clamp(1, 8))
        .unwrap_or(1)
}

fn inference_error(error: impl std::fmt::Display) -> SemanticError {
    SemanticError::Inference(error.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReport {
    pub schema_version: u32,
    pub requested_model: String,
    pub model: ModelMetadata,
    pub backend: ExecutionBackend,
    pub batch_size: usize,
    pub sample_count: usize,
    pub failure_count: usize,
    pub mean_latency_ms: Option<f64>,
    pub p50_latency_ms: Option<f64>,
    pub p95_latency_ms: Option<f64>,
    pub throughput_per_second: Option<f64>,
    pub peak_memory_bytes: Option<u64>,
    pub sample_predictions: Vec<BenchmarkSamplePrediction>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkSamplePrediction {
    pub path: String,
    pub labels: Vec<SemanticPrediction>,
}

pub fn benchmark_classifier(
    classifier: &dyn SemanticClassifier,
    requested_model: impl Into<String>,
    images: &[PathBuf],
    backend: ExecutionBackend,
    batch_size: usize,
) -> BenchmarkReport {
    let requested_model = requested_model.into();
    let metadata = classifier.metadata();
    let batch_size = batch_size.max(1);
    if !metadata.installed {
        return unavailable_benchmark(requested_model, metadata, backend, batch_size, images.len());
    }

    let overall_start = Instant::now();
    let mut batch_latencies = Vec::new();
    let mut failure_count = 0;
    let mut first_error = None;
    let mut sample_predictions = Vec::new();
    for batch in images.chunks(batch_size) {
        let start = Instant::now();
        match classifier.classify_batch(batch, backend) {
            Ok(results) if results.len() == batch.len() => {
                for (path, result) in batch.iter().zip(results).take(8 - sample_predictions.len()) {
                    sample_predictions.push(BenchmarkSamplePrediction {
                        path: path.to_string_lossy().into_owned(),
                        labels: result.predictions,
                    });
                }
            }
            Ok(_) => failure_count += batch.len(),
            Err(error) => {
                failure_count += batch.len();
                first_error.get_or_insert_with(|| error.to_string());
            }
        }
        batch_latencies.push(start.elapsed().as_secs_f64() * 1000.0 / batch.len() as f64);
    }
    batch_latencies.sort_by(f64::total_cmp);
    let elapsed = overall_start.elapsed().as_secs_f64();
    BenchmarkReport {
        schema_version: 2,
        requested_model,
        model: metadata,
        backend,
        batch_size,
        sample_count: images.len(),
        failure_count,
        mean_latency_ms: (!batch_latencies.is_empty())
            .then(|| batch_latencies.iter().sum::<f64>() / batch_latencies.len() as f64),
        p50_latency_ms: percentile(&batch_latencies, 0.50),
        p95_latency_ms: percentile(&batch_latencies, 0.95),
        throughput_per_second: (elapsed > 0.0).then_some(images.len() as f64 / elapsed),
        peak_memory_bytes: None,
        sample_predictions,
        status: if failure_count == 0 {
            "completed"
        } else {
            "completed_with_errors"
        }
        .into(),
        error: first_error,
    }
}

fn unavailable_benchmark(
    requested_model: String,
    metadata: ModelMetadata,
    backend: ExecutionBackend,
    batch_size: usize,
    sample_count: usize,
) -> BenchmarkReport {
    BenchmarkReport {
        schema_version: 2,
        requested_model,
        model: metadata,
        backend,
        batch_size,
        sample_count,
        failure_count: sample_count,
        mean_latency_ms: None,
        p50_latency_ms: None,
        p95_latency_ms: None,
        throughput_per_second: None,
        peak_memory_bytes: None,
        sample_predictions: Vec::new(),
        status: "model_unavailable".into(),
        error: Some("semantic model is not installed or enabled".into()),
    }
}

fn percentile(values: &[f64], quantile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let index = ((values.len() - 1) as f64 * quantile).ceil() as usize;
    values.get(index).copied()
}

pub fn discover_benchmark_images(root: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| {
                    matches!(
                        value.to_ascii_lowercase().as_str(),
                        "jpg" | "jpeg" | "png" | "webp"
                    )
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeClassifier;

    impl SemanticClassifier for FakeClassifier {
        fn metadata(&self) -> ModelMetadata {
            ModelMetadata {
                name: "fake-test-only".into(),
                version: "1".into(),
                analysis_version: "1".into(),
                license: Some("test-only".into()),
                installed: true,
                model_size_bytes: Some(1),
                model_sha256: None,
                supported_backends: vec![ExecutionBackend::Cpu],
            }
        }

        fn status(&self) -> SemanticRuntimeStatus {
            SemanticRuntimeStatus {
                status: "ready".into(),
                message: "test double".into(),
                model: self.metadata(),
                selected_backend: Some(ExecutionBackend::Cpu),
            }
        }

        fn classify_batch(
            &self,
            images: &[PathBuf],
            _backend: ExecutionBackend,
        ) -> Result<Vec<SemanticAnalysisOutput>, SemanticError> {
            Ok(images
                .iter()
                .map(|_| SemanticAnalysisOutput {
                    predictions: Vec::new(),
                    embedding: vec![0.0; 512],
                    raw_similarities: Vec::new(),
                })
                .collect())
        }
    }

    #[test]
    fn unavailable_classifier_never_returns_labels() {
        let classifier = UnavailableClassifier::default();
        let result =
            classifier.classify_batch(&[PathBuf::from("fixture.jpg")], ExecutionBackend::Cpu);
        assert!(matches!(result, Err(SemanticError::ModelUnavailable)));
        assert_eq!(classifier.status().status, "model_unavailable");
    }

    #[test]
    fn catalog_has_stable_unique_ids_and_unknown_last() {
        let catalog = semantic_catalog();
        assert_eq!(catalog.len(), 21);
        assert_eq!(
            catalog.last().map(|label| label.id.as_str()),
            Some("unknown")
        );
        let mut ids = catalog
            .iter()
            .map(|label| label.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), catalog.len());
        assert_eq!(
            catalog
                .iter()
                .filter(|label| label.is_primary_category)
                .count(),
            7
        );
        assert!(
            !catalog
                .iter()
                .find(|label| label.id == "night")
                .unwrap()
                .is_primary_category
        );
    }

    #[test]
    fn primary_label_uses_highest_accepted_similarity() {
        let mut scores = vec![0.10; LABELS.len()];
        scores[0] = 0.21;
        scores[13] = 0.24;
        let predictions = select_predictions(&scores);
        assert_eq!(predictions[0].label_id, "night");
        assert!(!predictions[0].is_primary);
        assert!(
            predictions
                .iter()
                .any(|label| label.label_id == "portrait" && label.is_primary)
        );
    }

    #[test]
    fn low_confidence_success_is_explicit_unknown_not_a_forced_category() {
        let scores = vec![0.01; LABELS.len()];
        let predictions = select_predictions(&scores);
        assert_eq!(
            predictions
                .iter()
                .find(|label| label.is_primary)
                .map(|label| label.label_id.as_str()),
            Some("unknown")
        );
        assert_eq!(
            predictions.iter().filter(|label| label.is_primary).count(),
            1
        );
    }

    #[test]
    fn benchmark_framework_calculates_stats_with_test_double() {
        let images = vec![PathBuf::from("a.jpg"), PathBuf::from("b.png")];
        let report = benchmark_classifier(
            &FakeClassifier,
            "fake-test-only",
            &images,
            ExecutionBackend::Cpu,
            1,
        );
        assert_eq!(report.status, "completed");
        assert_eq!(report.failure_count, 0);
        assert!(report.mean_latency_ms.is_some());
    }
}
