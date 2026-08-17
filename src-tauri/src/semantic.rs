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

use crate::{places365, topics};

pub const MODEL_NAME: &str = "Places365-ResNet18";
pub const MODEL_VERSION: &str = "onnx-2026-08-11";
pub const ANALYSIS_VERSION: &str = "photo-organizer-semantic-topic-candidates-tinyclip-v2";
pub const TAXONOMY_VERSION: &str = topics::TAXONOMY_VERSION;
pub const MODEL_FILE: &str = "resnet18_places365.onnx";
pub const TOKENIZER_FILE: &str = "categories_places365.txt";
pub const IO_FILE: &str = "IO_places365.txt";
pub const MODEL_SHA256: &str = "3c3cd0d42693e2957fcaa0bc365ce78e169a2e1162356742adfbd11077e8f7bf";
pub const TOKENIZER_SHA256: &str =
    "6cc3f1f8eae85b7016dc634e2d333cdcce5fd16cfada4afd87977fff5f8b12ba";
pub const IO_SHA256: &str = "d7e6abfeb228d789720326e630bedd231a7eaedcae8fd13d6d9dcd8eca95f59e";
pub const TINYCLIP_MODEL_NAME: &str = "TinyCLIP-ViT-8M-16-Text-3M-YFCC15M";
pub const TINYCLIP_MODEL_VERSION: &str = "onnx-int8-2025-08-06";
pub const TINYCLIP_MODEL_FILE: &str = "model-int8.onnx";
pub const TINYCLIP_TOKENIZER_FILE: &str = "tokenizer.json";
pub const TINYCLIP_MODEL_SHA256: &str =
    "10921310ddef06557ec1598d1260470a0a4db53f70ffe0deb60b946dcad6d27a";
pub const TINYCLIP_TOKENIZER_SHA256: &str =
    "6d9109cc838977f3ca94a379eec36aecc7c807e1785cd729660ca2fc0171fb35";
pub const SIGLIP2_MODEL_NAME: &str = "SigLIP2-Base-Patch16-224";
pub const SIGLIP2_MODEL_VERSION: &str = "onnx-int8-2026-08-11";
pub const SIGLIP2_ANALYSIS_VERSION: &str = "photo-organizer-semantic-topic-candidates-siglip2-v2";
pub const SIGLIP2_MODEL_FILE: &str = "model_int8.onnx";
pub const SIGLIP2_TOKENIZER_FILE: &str = "tokenizer.json";
pub const SIGLIP2_MODEL_SHA256: &str =
    "bfe28fe2ccdb685874586648035ea349593e487ce33bd0939b28813681a8f167";
pub const SIGLIP2_TOKENIZER_SHA256: &str =
    "cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322";
pub const MOBILECLIP_MODEL_NAME: &str = "MobileCLIP-S0";
pub const MOBILECLIP_MODEL_VERSION: &str = "onnx-int8-2026-08-11";
pub const MOBILECLIP_ANALYSIS_VERSION: &str =
    "photo-organizer-semantic-topic-candidates-mobileclip-s0-v1";
pub const MOBILECLIP_VISION_FILE: &str = "vision_model_int8.onnx";
pub const MOBILECLIP_TEXT_FILE: &str = "text_model_int8.onnx";
pub const MOBILECLIP_TOKENIZER_FILE: &str = "tokenizer.json";
pub const MOBILECLIP_VISION_SHA256: &str =
    "7a1b45f57fb9f3cde9d325759883e9451d7281336caeb9c576ae918e72080f0b";
pub const MOBILECLIP_TEXT_SHA256: &str =
    "fc8d87978623385c17a46331ffb9cb5ab7fe8b61c513c094602b85f08edd0a0b";
pub const MOBILECLIP_TOKENIZER_SHA256: &str =
    "72ed5c96db5729294468543e4bc75fce14ca63f58e37300290189ba1c1e52b85";
pub const RUNTIME_SHA256: &str = "8a1aad8d59d02a5337d4e3f5bbd1158c3f7bf84fe3b3f0052f957dd3e75a91cb";
pub const EMBEDDING_DIMENSIONS: usize = 512;

const IMAGE_SIZE: usize = 224;
const PLACES365_IMAGE_SIZE: usize = 224;
const TOKEN_LENGTH: usize = 77;
const PAD_TOKEN_ID: u32 = 49_407;
const MAX_LABELS: usize = 8;
const PLACES365_TOPIC_MIN_SCORE: f32 = 0.24;
const PLACES365_TOPIC_MIN_MARGIN: f32 = 0.045;
const IMAGE_MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const IMAGE_STD: [f32; 3] = [0.268_629_54, 0.261_302_6, 0.275_777_1];
const PLACES365_IMAGE_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const PLACES365_IMAGE_STD: [f32; 3] = [0.229, 0.224, 0.225];

const SIGLIP_IMAGE_SIZE: usize = 224;
const SIGLIP_TOKEN_LENGTH: usize = 64;
const SIGLIP_PAD_TOKEN_ID: u32 = 0;
const MOBILECLIP_IMAGE_SIZE: usize = 256;
const MOBILECLIP_TOKEN_LENGTH: usize = 77;
const MOBILECLIP_PAD_TOKEN_ID: u32 = 0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TopicModelKind {
    Tinyclip,
    Siglip2Base,
    MobileclipS0,
}

pub const DEFAULT_TOPIC_MODEL: TopicModelKind = TopicModelKind::Siglip2Base;

impl TopicModelKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "siglip2" | "siglip2-base" | "siglip2-base-patch16-224" => Some(Self::Siglip2Base),
            _ => None,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Tinyclip => "tinyclip",
            Self::Siglip2Base => "siglip2-base",
            Self::MobileclipS0 => "mobileclip-s0",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Tinyclip => "TinyCLIP INT8",
            Self::Siglip2Base => "SigLIP 2 Base INT8",
            Self::MobileclipS0 => "MobileCLIP-S0 INT8",
        }
    }

    pub const fn model_name(self) -> &'static str {
        match self {
            Self::Tinyclip => TINYCLIP_MODEL_NAME,
            Self::Siglip2Base => SIGLIP2_MODEL_NAME,
            Self::MobileclipS0 => MOBILECLIP_MODEL_NAME,
        }
    }

    pub const fn analysis_version(self) -> &'static str {
        match self {
            Self::Tinyclip => ANALYSIS_VERSION,
            Self::Siglip2Base => SIGLIP2_ANALYSIS_VERSION,
            Self::MobileclipS0 => MOBILECLIP_ANALYSIS_VERSION,
        }
    }
}

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

pub fn default_topic_model_metadata() -> ModelMetadata {
    ModelMetadata {
        name: SIGLIP2_MODEL_NAME.into(),
        version: SIGLIP2_MODEL_VERSION.into(),
        analysis_version: SIGLIP2_ANALYSIS_VERSION.into(),
        license: Some("Apache-2.0".into()),
        installed: true,
        model_size_bytes: None,
        model_sha256: Some(SIGLIP2_MODEL_SHA256.into()),
        supported_backends: vec![ExecutionBackend::Cpu],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticPrediction {
    pub label_id: String,
    pub display_name: String,
    pub category_group: String,
    pub similarity: f32,
    pub threshold: f32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSimilarity {
    pub label_id: String,
    pub display_name: String,
    pub category_group: String,
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
    #[serde(default)]
    pub topic_model: Option<ModelMetadata>,
    pub selected_backend: Option<ExecutionBackend>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLabelDescriptor {
    pub id: String,
    pub display_name: String,
    pub category_group: String,
    pub threshold: f32,
    pub is_primary_category: bool,
    pub taxonomy_version: String,
}

const CONTEXT_LABELS: [(&str, &str, f32); 2] =
    [("indoor", "室内", 0.55), ("outdoor", "室外", 0.55)];

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
    /// Metadata for the model whose labels and embeddings are persisted.
    ///
    /// The Places365 adapter is a composite classifier: it owns the scene
    /// model, but its persisted topic results come from the selected topic
    /// model. Standalone adapters simply return their own metadata.
    fn result_metadata(&self) -> ModelMetadata {
        self.metadata()
    }
    fn encode_text(&self, _queries: &[String]) -> Result<Vec<Vec<f32>>, SemanticError> {
        Err(SemanticError::ModelUnavailable)
    }
    fn classify_batch(
        &self,
        images: &[PathBuf],
        backend: ExecutionBackend,
    ) -> Result<Vec<SemanticAnalysisOutput>, SemanticError>;
}

pub fn semantic_catalog() -> Vec<SemanticLabelDescriptor> {
    let mut catalog = topics::TOPIC_LABELS
        .iter()
        .map(|label| SemanticLabelDescriptor {
            id: label.id.into(),
            display_name: label.display_name.into(),
            category_group: "scene".into(),
            threshold: label.threshold,
            is_primary_category: true,
            taxonomy_version: TAXONOMY_VERSION.into(),
        })
        .collect::<Vec<_>>();
    catalog.extend(CONTEXT_LABELS.iter().map(|(id, display_name, threshold)| {
        SemanticLabelDescriptor {
            id: (*id).into(),
            display_name: (*display_name).into(),
            category_group: "context".into(),
            threshold: *threshold,
            is_primary_category: false,
            taxonomy_version: TAXONOMY_VERSION.into(),
        }
    }));
    catalog.extend(crate::subject::subject_catalog());
    catalog
}

/// Map values written by older taxonomy versions to the current user-facing
/// taxonomy. This is intentionally kept at the read boundary so existing
/// manual classifications and organization rules remain readable after a
/// taxonomy refresh.
pub fn canonical_label_id(label_id: &str) -> &str {
    match label_id {
        "unknown" | "photo_documentary" => "photo_abstract",
        "person" | "portrait" => "single_person",
        "group" => "multiple_people",
        "pet" => "animal",
        "photo_urban" => "photo_street",
        "photo_event" => "photo_activity",
        "photo_transport" => "photo_vehicle",
        "photo_plant" => "photo_macro",
        _ => label_id,
    }
}

pub fn known_display_name_for_label_id(label_id: &str) -> Option<&'static str> {
    let canonical_id = canonical_label_id(label_id);
    if canonical_id != label_id {
        return known_display_name_for_label_id(canonical_id);
    }
    let legacy_name = match label_id {
        "single_person" => Some("单人"),
        "multiple_people" => Some("多人"),
        "landscape" => Some("风景"),
        "architecture" => Some("建筑"),
        "product" => Some("产品"),
        "still_life" => Some("静物"),
        "food" => Some("食品"),
        "animal" => Some("动物"),
        "screenshot" => Some("截图"),
        "document" => Some("文档"),
        "abstract" => Some("抽象"),
        "vehicle" => Some("车辆"),
        "plant" => Some("植物"),
        "flower" => Some("花卉"),
        "mountain" => Some("山"),
        "water" => Some("水体"),
        "forest" => Some("森林"),
        "street" => Some("街道"),
        "night" => Some("夜景"),
        "sunset" => Some("日落"),
        _ => None,
    };
    if legacy_name.is_some() {
        return legacy_name;
    }
    topics::TOPIC_LABELS
        .iter()
        .find(|label| label.id == label_id)
        .map(|label| label.display_name)
        .or_else(|| {
            CONTEXT_LABELS
                .iter()
                .find(|(id, _, _)| *id == label_id)
                .map(|(_, display_name, _)| *display_name)
        })
}

pub fn category_group_for_label_id(label_id: &str) -> Option<&'static str> {
    let canonical_id = canonical_label_id(label_id);
    if canonical_id != label_id {
        return category_group_for_label_id(canonical_id);
    }
    let legacy_group = match label_id {
        "single_person" | "multiple_people" | "animal" | "vehicle" | "food" | "plant" => {
            Some("subject")
        }
        "landscape" | "architecture" | "product" | "still_life" | "screenshot" | "document"
        | "abstract" => Some("scene"),
        "flower" | "mountain" | "water" | "forest" => Some("subject"),
        "street" | "night" | "sunset" | "indoor" | "outdoor" => Some("context"),
        _ => None,
    };
    if legacy_group.is_some() {
        return legacy_group;
    }
    topics::TOPIC_LABELS
        .iter()
        .find(|label| label.id == label_id)
        .map(|_| "scene")
        .or_else(|| {
            CONTEXT_LABELS
                .iter()
                .find(|(id, _, _)| *id == label_id)
                .map(|_| "context")
        })
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
            topic_model: None,
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

pub struct Places365Classifier {
    session: Mutex<Session>,
    input_name: String,
    output_name: String,
    categories: Vec<String>,
    leaf_cluster_indexes: Vec<usize>,
    outdoor_by_leaf: Vec<bool>,
    model_size_bytes: u64,
    topic_classifier: Option<Box<dyn SemanticClassifier>>,
}

impl std::fmt::Debug for Places365Classifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Places365Classifier")
            .field("model", &MODEL_NAME)
            .field("version", &MODEL_VERSION)
            .field("categories", &self.categories.len())
            .field("has_topic_classifier", &self.topic_classifier.is_some())
            .finish_non_exhaustive()
    }
}

impl Places365Classifier {
    pub fn load(
        model_dir: &Path,
        embedding_model_dir: &Path,
        runtime_path: &Path,
    ) -> Result<Self, SemanticError> {
        Self::load_with_topic_model(
            model_dir,
            embedding_model_dir,
            runtime_path,
            DEFAULT_TOPIC_MODEL,
        )
    }

    pub fn load_with_topic_model(
        model_dir: &Path,
        topic_model_dir: &Path,
        runtime_path: &Path,
        topic_model: TopicModelKind,
    ) -> Result<Self, SemanticError> {
        let model_path = model_dir.join(MODEL_FILE);
        let categories_path = model_dir.join(TOKENIZER_FILE);
        let io_path = model_dir.join(IO_FILE);
        verify_sha256(&model_path, MODEL_SHA256)?;
        verify_sha256(&categories_path, TOKENIZER_SHA256)?;
        verify_sha256(&io_path, IO_SHA256)?;
        verify_sha256(runtime_path, RUNTIME_SHA256)?;
        initialize_ort(runtime_path)?;

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
            SemanticError::Inference(format!("could not load Places365 ONNX graph: {error}"))
        })?;
        let (input_name, output_name) = validate_places365_model_contract(&session)?;
        let categories = load_places365_categories(&categories_path)?;
        let outdoor_by_leaf = load_places365_io(&io_path)?;
        if categories.len() != places365::PLACES365_LEAF_COUNT
            || outdoor_by_leaf.len() != categories.len()
        {
            return Err(SemanticError::Inference(format!(
                "Places365 resource contract mismatch: categories={}, indoor/outdoor={}",
                categories.len(),
                outdoor_by_leaf.len()
            )));
        }
        let leaf_cluster_indexes = categories
            .iter()
            .map(|label| {
                places365::scene_cluster_index_for_leaf(label).ok_or_else(|| {
                    SemanticError::Inference(format!(
                        "Places365 leaf is not mapped to a product scene cluster: {label}"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let topic_classifier =
            match load_topic_classifier(topic_model, topic_model_dir, runtime_path) {
                Ok(classifier) => Some(classifier),
                Err(error) => {
                    log::warn!(
                        "{} topic adapter unavailable: {error}",
                        topic_model.display_name()
                    );
                    None
                }
            };

        Ok(Self {
            session: Mutex::new(session),
            input_name,
            output_name,
            categories,
            leaf_cluster_indexes,
            outdoor_by_leaf,
            model_size_bytes: std::fs::metadata(model_path)
                .map_err(|error| SemanticError::Inference(error.to_string()))?
                .len(),
            topic_classifier,
        })
    }

    pub fn model_contract(&self) -> (String, String) {
        (self.input_name.clone(), self.output_name.clone())
    }
}

impl SemanticClassifier for Places365Classifier {
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
        let message = if self.topic_classifier.is_some() {
            format!(
                "Places365 ResNet-18 已就绪；环境证据与摄影题材候选已启用 {}。",
                self.topic_classifier
                    .as_ref()
                    .map(|classifier| classifier.metadata().name)
                    .unwrap_or_default()
            )
        } else {
            "Places365 ResNet-18 已就绪；环境证据可用，但题材候选模型未就绪。".into()
        };
        SemanticRuntimeStatus {
            status: "ready".into(),
            message,
            model: self.metadata(),
            topic_model: self
                .topic_classifier
                .as_ref()
                .map(|classifier| classifier.metadata()),
            selected_backend: Some(ExecutionBackend::Cpu),
        }
    }

    fn result_metadata(&self) -> ModelMetadata {
        self.topic_classifier
            .as_ref()
            .map(|classifier| classifier.metadata())
            .unwrap_or_else(|| self.metadata())
    }

    fn encode_text(&self, queries: &[String]) -> Result<Vec<Vec<f32>>, SemanticError> {
        self.topic_classifier
            .as_ref()
            .ok_or(SemanticError::ModelUnavailable)?
            .encode_text(queries)
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

        let image_count = images.len();
        let mut session = self.session.lock();
        let leaf_count = self.categories.len();
        let mut probability_rows = Vec::with_capacity(image_count);
        for image in images {
            let pixel_values = preprocess_places365_images(std::slice::from_ref(image))?;
            let pixel_values = Tensor::from_array((
                [1_usize, 3, PLACES365_IMAGE_SIZE, PLACES365_IMAGE_SIZE],
                pixel_values.into_boxed_slice(),
            ))
            .map_err(inference_error)?;
            let outputs = session
                .run(ort::inputs![self.input_name.as_str() => pixel_values])
                .map_err(inference_error)?;
            let (shape, data) = outputs[self.output_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(inference_error)?;
            let shape_description = format!("{shape:?}");
            let data = data.to_vec();
            if data.len() != leaf_count {
                return Err(SemanticError::Inference(format!(
                    "unexpected Places365 logits shape: {shape_description}; values={}",
                    data.len()
                )));
            }
            probability_rows.push(softmax(&data));
        }
        drop(session);

        let embedding_outputs = self.topic_classifier.as_ref().and_then(|classifier| {
            match classifier.classify_batch(images, ExecutionBackend::Cpu) {
                Ok(outputs) if outputs.len() == image_count => Some(outputs),
                Ok(outputs) => {
                    log::warn!(
                        "topic model returned {} embeddings for {} images",
                        outputs.len(),
                        image_count
                    );
                    None
                }
                Err(error) => {
                    log::warn!("topic model embedding batch failed: {error}");
                    None
                }
            }
        });
        let mut results = Vec::with_capacity(image_count);
        for (image_index, probabilities) in probability_rows.into_iter().enumerate() {
            let topic_output = embedding_outputs
                .as_ref()
                .and_then(|outputs| outputs.get(image_index));
            let mut predictions =
                select_places365_topic(&probabilities, &self.leaf_cluster_indexes)
                    .into_iter()
                    .collect::<Vec<_>>();
            if let Some((label_id, score)) =
                select_places365_environment(&probabilities, &self.outdoor_by_leaf)
            {
                predictions.push(SemanticPrediction {
                    label_id: label_id.into(),
                    display_name: known_display_name_for_label_id(label_id)
                        .unwrap_or("环境")
                        .into(),
                    category_group: "context".into(),
                    similarity: score,
                    threshold: 0.55,
                    is_primary: false,
                });
            }
            let mut raw_similarities = topic_output
                .map(|output| output.raw_similarities.clone())
                .unwrap_or_default();
            raw_similarities.extend(places365_raw_similarities(
                &probabilities,
                &self.categories,
                &self.leaf_cluster_indexes,
            ));
            raw_similarities.sort_by(|left, right| {
                right
                    .similarity
                    .total_cmp(&left.similarity)
                    .then(left.label_id.cmp(&right.label_id))
            });
            raw_similarities.truncate(MAX_LABELS);
            let embedding = embedding_outputs
                .as_ref()
                .and_then(|outputs| outputs.get(image_index))
                .map(|output| output.embedding.clone())
                .unwrap_or_default();
            results.push(SemanticAnalysisOutput {
                predictions,
                embedding,
                raw_similarities,
            });
        }
        Ok(results)
    }
}

pub struct TinyClipClassifier {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    prompt_input_ids: Vec<i64>,
    prompt_attention_mask: Vec<i64>,
    prompt_label_indexes: Vec<usize>,
    model_size_bytes: u64,
}

impl std::fmt::Debug for TinyClipClassifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TinyClipClassifier")
            .field("model", &TINYCLIP_MODEL_NAME)
            .field("version", &TINYCLIP_MODEL_VERSION)
            .finish_non_exhaustive()
    }
}

impl TinyClipClassifier {
    pub fn load(model_dir: &Path, runtime_path: &Path) -> Result<Self, SemanticError> {
        let model_path = model_dir.join(TINYCLIP_MODEL_FILE);
        let tokenizer_path = model_dir.join(TINYCLIP_TOKENIZER_FILE);
        verify_sha256(&model_path, TINYCLIP_MODEL_SHA256)?;
        verify_sha256(&tokenizer_path, TINYCLIP_TOKENIZER_SHA256)?;
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
        let (prompt_input_ids, prompt_attention_mask, prompt_label_indexes) =
            tokenize_topic_prompts(&tokenizer)?;

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            prompt_input_ids,
            prompt_attention_mask,
            prompt_label_indexes,
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
            name: TINYCLIP_MODEL_NAME.into(),
            version: TINYCLIP_MODEL_VERSION.into(),
            analysis_version: ANALYSIS_VERSION.into(),
            license: Some("MIT".into()),
            installed: true,
            model_size_bytes: Some(self.model_size_bytes),
            model_sha256: Some(TINYCLIP_MODEL_SHA256.into()),
            supported_backends: vec![ExecutionBackend::Cpu],
        }
    }

    fn status(&self) -> SemanticRuntimeStatus {
        SemanticRuntimeStatus {
            status: "ready".into(),
            message: "TinyCLIP INT8 已通过完整性校验，可用于题材候选和本地文本/相似搜索。".into(),
            model: self.metadata(),
            topic_model: Some(self.metadata()),
            selected_backend: Some(ExecutionBackend::Cpu),
        }
    }

    fn encode_text(&self, queries: &[String]) -> Result<Vec<Vec<f32>>, SemanticError> {
        if queries.is_empty() {
            return Ok(Vec::new());
        }
        let prompts = queries
            .iter()
            .map(|query| format!("a photo of {query}"))
            .collect::<Vec<_>>();
        let (input_ids, attention_mask) = tokenize_texts(&self.tokenizer, &prompts)?;
        let query_count = prompts.len();
        let input_ids =
            Tensor::from_array(([query_count, TOKEN_LENGTH], input_ids.into_boxed_slice()))
                .map_err(inference_error)?;
        let attention_mask = Tensor::from_array((
            [query_count, TOKEN_LENGTH],
            attention_mask.into_boxed_slice(),
        ))
        .map_err(inference_error)?;
        // The exported graph has a combined image/text contract. Text output is
        // independent from this placeholder image tensor, but the input remains
        // required by ONNX Runtime.
        let pixel_values = Tensor::from_array((
            [1_usize, 3, IMAGE_SIZE, IMAGE_SIZE],
            vec![0_f32; 3 * IMAGE_SIZE * IMAGE_SIZE].into_boxed_slice(),
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
        let (shape, data) = outputs["text_embeds"]
            .try_extract_tensor::<f32>()
            .map_err(inference_error)?;
        if shape.as_ref() != [query_count as i64, EMBEDDING_DIMENSIONS as i64] {
            return Err(SemanticError::Inference(format!(
                "unexpected text embedding shape: {shape:?}"
            )));
        }
        Ok(data
            .chunks_exact(EMBEDDING_DIMENSIONS)
            .map(|embedding| embedding.to_vec())
            .collect())
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

        // The label prompts never change for a classifier instance. Reuse the
        // tokenized tensors across batches; the ONNX graph still requires the
        // text inputs, but tokenization no longer runs once per batch.
        let input_ids = self.prompt_input_ids.clone();
        let attention_mask = self.prompt_attention_mask.clone();
        let pixel_values = preprocess_images(images)?;
        let prompt_count = self.prompt_label_indexes.len();
        let image_count = images.len();

        let input_ids =
            Tensor::from_array(([prompt_count, TOKEN_LENGTH], input_ids.into_boxed_slice()))
                .map_err(inference_error)?;
        let attention_mask = Tensor::from_array((
            [prompt_count, TOKEN_LENGTH],
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
            || text_shape.as_ref() != [prompt_count as i64, EMBEDDING_DIMENSIONS as i64]
        {
            return Err(SemanticError::Inference(format!(
                "unexpected embedding shapes: image={image_shape:?}, text={text_shape:?}"
            )));
        }

        let mut results = Vec::with_capacity(image_count);
        for image_index in 0..image_count {
            let start = image_index * EMBEDDING_DIMENSIONS;
            let embedding = image_data[start..start + EMBEDDING_DIMENSIONS].to_vec();
            let mut prompt_scores = Vec::with_capacity(prompt_count);
            for prompt_index in 0..prompt_count {
                let text_start = prompt_index * EMBEDDING_DIMENSIONS;
                let similarity = cosine_similarity(
                    &embedding,
                    &text_data[text_start..text_start + EMBEDDING_DIMENSIONS],
                );
                prompt_scores.push(similarity);
            }
            let scores =
                topics::aggregate_prompt_scores(&prompt_scores, &self.prompt_label_indexes);
            results.push(SemanticAnalysisOutput {
                predictions: select_topic_predictions(&scores),
                embedding,
                raw_similarities: rank_topic_similarities(&scores),
            });
        }
        Ok(results)
    }
}

#[derive(Debug, Clone, Copy)]
enum OpenClipVariant {
    Siglip2Base,
    MobileclipS0,
}

impl OpenClipVariant {
    const fn topic_model(self) -> TopicModelKind {
        match self {
            Self::Siglip2Base => TopicModelKind::Siglip2Base,
            Self::MobileclipS0 => TopicModelKind::MobileclipS0,
        }
    }

    const fn image_size(self) -> usize {
        match self {
            Self::Siglip2Base => SIGLIP_IMAGE_SIZE,
            Self::MobileclipS0 => MOBILECLIP_IMAGE_SIZE,
        }
    }

    const fn token_length(self) -> usize {
        match self {
            Self::Siglip2Base => SIGLIP_TOKEN_LENGTH,
            Self::MobileclipS0 => MOBILECLIP_TOKEN_LENGTH,
        }
    }

    const fn pad_token_id(self) -> u32 {
        match self {
            Self::Siglip2Base => SIGLIP_PAD_TOKEN_ID,
            Self::MobileclipS0 => MOBILECLIP_PAD_TOKEN_ID,
        }
    }

    const fn pad_token(self) -> &'static str {
        match self {
            Self::Siglip2Base => "<pad>",
            Self::MobileclipS0 => "!",
        }
    }

    const fn embedding_dimensions(self) -> usize {
        match self {
            Self::Siglip2Base => 768,
            Self::MobileclipS0 => 512,
        }
    }
}

enum OpenClipGraph {
    Joint(Mutex<Session>),
    Split {
        vision: Mutex<Session>,
        text: Mutex<Session>,
    },
}

struct OpenClipImageBatch {
    embeddings: Vec<f32>,
    siglip_logits: Option<Vec<f32>>,
}

pub struct OpenVocabularyClipClassifier {
    variant: OpenClipVariant,
    graph: OpenClipGraph,
    tokenizer: Tokenizer,
    prompt_input_ids: Vec<i64>,
    prompt_label_indexes: Vec<usize>,
    prompt_text_embeddings: Vec<f32>,
    embedding_dimensions: usize,
    model_size_bytes: u64,
}

impl std::fmt::Debug for OpenVocabularyClipClassifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenVocabularyClipClassifier")
            .field("model", &self.variant.topic_model().model_name())
            .field("version", &self.variant.topic_model().analysis_version())
            .field("embedding_dimensions", &self.embedding_dimensions)
            .finish_non_exhaustive()
    }
}

impl OpenVocabularyClipClassifier {
    pub fn load(
        topic_model: TopicModelKind,
        model_dir: &Path,
        runtime_path: &Path,
    ) -> Result<Self, SemanticError> {
        let variant = match topic_model {
            TopicModelKind::Siglip2Base => OpenClipVariant::Siglip2Base,
            TopicModelKind::MobileclipS0 => OpenClipVariant::MobileclipS0,
            TopicModelKind::Tinyclip => {
                return Err(SemanticError::Inference(
                    "TinyCLIP must use its combined graph adapter".into(),
                ));
            }
        };
        verify_sha256(runtime_path, RUNTIME_SHA256)?;
        initialize_ort(runtime_path)?;

        let tokenizer_path = model_dir.join(match variant {
            OpenClipVariant::Siglip2Base => SIGLIP2_TOKENIZER_FILE,
            OpenClipVariant::MobileclipS0 => MOBILECLIP_TOKENIZER_FILE,
        });
        let model_size_bytes = match variant {
            OpenClipVariant::Siglip2Base => {
                let model_path = model_dir.join(SIGLIP2_MODEL_FILE);
                verify_sha256(&model_path, SIGLIP2_MODEL_SHA256)?;
                std::fs::metadata(&model_path)
                    .map_err(|error| SemanticError::Integrity(error.to_string()))?
                    .len()
            }
            OpenClipVariant::MobileclipS0 => {
                let vision_path = model_dir.join(MOBILECLIP_VISION_FILE);
                let text_path = model_dir.join(MOBILECLIP_TEXT_FILE);
                verify_sha256(&vision_path, MOBILECLIP_VISION_SHA256)?;
                verify_sha256(&text_path, MOBILECLIP_TEXT_SHA256)?;
                std::fs::metadata(&vision_path)
                    .and_then(|vision| {
                        std::fs::metadata(&text_path).map(|text| vision.len() + text.len())
                    })
                    .map_err(|error| SemanticError::Integrity(error.to_string()))?
            }
        };
        verify_sha256(
            &tokenizer_path,
            match variant {
                OpenClipVariant::Siglip2Base => SIGLIP2_TOKENIZER_SHA256,
                OpenClipVariant::MobileclipS0 => MOBILECLIP_TOKENIZER_SHA256,
            },
        )?;

        let mut tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|error| {
            SemanticError::Inference(format!("could not load tokenizer: {error}"))
        })?;
        let token_length = variant.token_length();
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: token_length,
                ..TruncationParams::default()
            }))
            .map_err(|error| {
                SemanticError::Inference(format!("could not configure tokenizer: {error}"))
            })?;
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::Fixed(token_length),
            pad_id: variant.pad_token_id(),
            pad_token: variant.pad_token().into(),
            ..PaddingParams::default()
        }));

        let graph = match variant {
            OpenClipVariant::Siglip2Base => {
                let model_path = model_dir.join(SIGLIP2_MODEL_FILE);
                let session = build_optimized_session(&model_path, "SigLIP 2")?;
                validate_siglip2_model_contract(&session)?;
                OpenClipGraph::Joint(Mutex::new(session))
            }
            OpenClipVariant::MobileclipS0 => {
                let vision_path = model_dir.join(MOBILECLIP_VISION_FILE);
                let text_path = model_dir.join(MOBILECLIP_TEXT_FILE);
                let vision = build_optimized_session(&vision_path, "MobileCLIP vision")?;
                let text = build_optimized_session(&text_path, "MobileCLIP text")?;
                validate_mobileclip_vision_contract(&vision)?;
                validate_mobileclip_text_contract(&text)?;
                OpenClipGraph::Split {
                    vision: Mutex::new(vision),
                    text: Mutex::new(text),
                }
            }
        };

        let (prompt_input_ids, _, prompt_label_indexes) =
            tokenize_topic_prompts_for_variant(&tokenizer, variant)?;
        let mut classifier = Self {
            variant,
            graph,
            tokenizer,
            prompt_input_ids,
            prompt_label_indexes,
            prompt_text_embeddings: Vec::new(),
            embedding_dimensions: variant.embedding_dimensions(),
            model_size_bytes,
        };
        classifier.prompt_text_embeddings = classifier.run_text_embeddings(
            &classifier.prompt_input_ids,
            classifier.prompt_label_indexes.len(),
        )?;
        if classifier.prompt_text_embeddings.len()
            != classifier.prompt_label_indexes.len() * classifier.embedding_dimensions
        {
            return Err(SemanticError::Inference(format!(
                "unexpected topic prompt embedding size: {}",
                classifier.prompt_text_embeddings.len()
            )));
        }
        Ok(classifier)
    }

    fn run_text_embeddings(
        &self,
        input_ids: &[i64],
        batch_size: usize,
    ) -> Result<Vec<f32>, SemanticError> {
        let token_length = self.variant.token_length();
        let input_ids = Tensor::from_array((
            [batch_size, token_length],
            input_ids.to_vec().into_boxed_slice(),
        ))
        .map_err(inference_error)?;
        let data = match &self.graph {
            OpenClipGraph::Joint(session) => {
                let pixel_values = Tensor::from_array((
                    [1_usize, 3, SIGLIP_IMAGE_SIZE, SIGLIP_IMAGE_SIZE],
                    vec![0_f32; 3 * SIGLIP_IMAGE_SIZE * SIGLIP_IMAGE_SIZE].into_boxed_slice(),
                ))
                .map_err(inference_error)?;
                let mut session = session.lock();
                let outputs = session
                    .run(ort::inputs! {
                        "input_ids" => input_ids,
                        "pixel_values" => pixel_values,
                    })
                    .map_err(inference_error)?;
                outputs["text_embeds"]
                    .try_extract_tensor::<f32>()
                    .map_err(inference_error)
                    .map(|(_, data)| data.to_vec())?
            }
            OpenClipGraph::Split { text, .. } => {
                let mut session = text.lock();
                let outputs = session
                    .run(ort::inputs! { "input_ids" => input_ids })
                    .map_err(inference_error)?;
                outputs["text_embeds"]
                    .try_extract_tensor::<f32>()
                    .map_err(inference_error)
                    .map(|(_, data)| data.to_vec())?
            }
        };
        if data.len() != batch_size * self.embedding_dimensions {
            return Err(SemanticError::Inference(format!(
                "unexpected text embedding size: {}; expected {}",
                data.len(),
                batch_size * self.embedding_dimensions
            )));
        }
        Ok(data)
    }

    fn run_image_embeddings(
        &self,
        images: &[PathBuf],
    ) -> Result<OpenClipImageBatch, SemanticError> {
        let image_count = images.len();
        let pixel_values = preprocess_open_clip_images(images, self.variant)?;
        let pixel_values = Tensor::from_array((
            [
                image_count,
                3,
                self.variant.image_size(),
                self.variant.image_size(),
            ],
            pixel_values.into_boxed_slice(),
        ))
        .map_err(inference_error)?;
        let (data, siglip_logits) = match &self.graph {
            OpenClipGraph::Joint(session) => {
                let prompt_count = self.prompt_label_indexes.len();
                let input_ids = Tensor::from_array((
                    [prompt_count, self.variant.token_length()],
                    self.prompt_input_ids.clone().into_boxed_slice(),
                ))
                .map_err(inference_error)?;
                let mut session = session.lock();
                let outputs = session
                    .run(ort::inputs! {
                        "input_ids" => input_ids,
                        "pixel_values" => pixel_values,
                    })
                    .map_err(inference_error)?;
                let image_data = outputs["image_embeds"]
                    .try_extract_tensor::<f32>()
                    .map_err(inference_error)
                    .map(|(_, data)| data.to_vec())?;
                let prompt_count = self.prompt_label_indexes.len();
                let (logit_shape, logits) = outputs["logits_per_image"]
                    .try_extract_tensor::<f32>()
                    .map_err(inference_error)?;
                if logit_shape.as_ref() != [image_count as i64, prompt_count as i64] {
                    return Err(SemanticError::Inference(format!(
                        "unexpected SigLIP 2 logit shape: {logit_shape:?}"
                    )));
                }
                (image_data, Some(logits.to_vec()))
            }
            OpenClipGraph::Split { vision, .. } => {
                let mut session = vision.lock();
                let outputs = session
                    .run(ort::inputs! { "pixel_values" => pixel_values })
                    .map_err(inference_error)?;
                let image_data = outputs["image_embeds"]
                    .try_extract_tensor::<f32>()
                    .map_err(inference_error)
                    .map(|(_, data)| data.to_vec())?;
                (image_data, None)
            }
        };
        if data.len() != image_count * self.embedding_dimensions {
            return Err(SemanticError::Inference(format!(
                "unexpected image embedding size: {}; expected {}",
                data.len(),
                image_count * self.embedding_dimensions
            )));
        }
        Ok(OpenClipImageBatch {
            embeddings: data,
            siglip_logits,
        })
    }
}

impl SemanticClassifier for OpenVocabularyClipClassifier {
    fn metadata(&self) -> ModelMetadata {
        let topic_model = self.variant.topic_model();
        let (model_sha256, license) = match self.variant {
            OpenClipVariant::Siglip2Base => (SIGLIP2_MODEL_SHA256, "Apache-2.0"),
            OpenClipVariant::MobileclipS0 => (MOBILECLIP_VISION_SHA256, "Apple AMLR 2.0"),
        };
        ModelMetadata {
            name: topic_model.model_name().into(),
            version: match self.variant {
                OpenClipVariant::Siglip2Base => SIGLIP2_MODEL_VERSION,
                OpenClipVariant::MobileclipS0 => MOBILECLIP_MODEL_VERSION,
            }
            .into(),
            analysis_version: topic_model.analysis_version().into(),
            license: Some(license.into()),
            installed: true,
            model_size_bytes: Some(self.model_size_bytes),
            model_sha256: Some(model_sha256.into()),
            supported_backends: vec![ExecutionBackend::Cpu],
        }
    }

    fn status(&self) -> SemanticRuntimeStatus {
        let metadata = self.metadata();
        SemanticRuntimeStatus {
            status: "ready".into(),
            message: format!(
                "{} 已通过完整性校验，可用于摄影题材候选和本地文本/相似搜索。",
                self.variant.topic_model().display_name()
            ),
            model: metadata.clone(),
            topic_model: Some(metadata),
            selected_backend: Some(ExecutionBackend::Cpu),
        }
    }

    fn encode_text(&self, queries: &[String]) -> Result<Vec<Vec<f32>>, SemanticError> {
        if queries.is_empty() {
            return Ok(Vec::new());
        }
        let prompts = queries
            .iter()
            .map(|query| match self.variant {
                OpenClipVariant::Siglip2Base => format!("This is a photo of {query}."),
                OpenClipVariant::MobileclipS0 => format!("a photo of {query}"),
            })
            .collect::<Vec<_>>();
        let (input_ids, _) =
            tokenize_texts_with_length(&self.tokenizer, &prompts, self.variant.token_length())?;
        let data = self.run_text_embeddings(&input_ids, prompts.len())?;
        Ok(data
            .chunks_exact(self.embedding_dimensions)
            .map(|embedding| embedding.to_vec())
            .collect())
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
        let image_batch = self.run_image_embeddings(images)?;
        let prompt_count = self.prompt_label_indexes.len();
        let mut results = Vec::with_capacity(images.len());
        for image_index in 0..images.len() {
            let start = image_index * self.embedding_dimensions;
            let embedding =
                image_batch.embeddings[start..start + self.embedding_dimensions].to_vec();
            let prompt_scores = if let Some(logits) = image_batch.siglip_logits.as_ref() {
                logits[image_index * prompt_count..(image_index + 1) * prompt_count]
                    .iter()
                    .map(|logit| sigmoid(*logit))
                    .collect::<Vec<_>>()
            } else {
                let cosine_scores = (0..prompt_count)
                    .map(|prompt_index| {
                        let text_start = prompt_index * self.embedding_dimensions;
                        cosine_similarity(
                            &embedding,
                            &self.prompt_text_embeddings
                                [text_start..text_start + self.embedding_dimensions],
                        )
                    })
                    .collect::<Vec<_>>();
                match self.variant {
                    OpenClipVariant::MobileclipS0 => softmax(
                        &cosine_scores
                            .iter()
                            .map(|score| score * 100.0)
                            .collect::<Vec<_>>(),
                    ),
                    OpenClipVariant::Siglip2Base => cosine_scores,
                }
            };
            let scores =
                topics::aggregate_prompt_scores(&prompt_scores, &self.prompt_label_indexes);
            results.push(SemanticAnalysisOutput {
                predictions: select_topic_predictions(&scores),
                embedding,
                raw_similarities: rank_topic_similarities(&scores),
            });
        }
        Ok(results)
    }
}

fn load_topic_classifier(
    topic_model: TopicModelKind,
    model_dir: &Path,
    runtime_path: &Path,
) -> Result<Box<dyn SemanticClassifier>, SemanticError> {
    match topic_model {
        TopicModelKind::Tinyclip => {
            Ok(Box::new(TinyClipClassifier::load(model_dir, runtime_path)?))
        }
        TopicModelKind::Siglip2Base | TopicModelKind::MobileclipS0 => Ok(Box::new(
            OpenVocabularyClipClassifier::load(topic_model, model_dir, runtime_path)?,
        )),
    }
}

fn build_optimized_session(model_path: &Path, model_name: &str) -> Result<Session, SemanticError> {
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
        .with_intra_threads(cpu_thread_count())
        .map_err(|error| {
            SemanticError::Inference(format!(
                "could not configure {model_name} ONNX threads: {error}"
            ))
        })?;
    builder.commit_from_file(model_path).map_err(|error| {
        SemanticError::Inference(format!("could not load {model_name} ONNX graph: {error}"))
    })
}

fn validate_siglip2_model_contract(session: &Session) -> Result<(), SemanticError> {
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
    for name in ["input_ids", "pixel_values"] {
        if !input_names.contains(&name) {
            return Err(SemanticError::Inference(format!(
                "SigLIP 2 model input is missing: {name}"
            )));
        }
    }
    for name in ["image_embeds", "text_embeds", "logits_per_image"] {
        if !output_names.contains(&name) {
            return Err(SemanticError::Inference(format!(
                "SigLIP 2 model output is missing: {name}"
            )));
        }
    }
    Ok(())
}

fn validate_mobileclip_vision_contract(session: &Session) -> Result<(), SemanticError> {
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
    if !input_names.contains(&"pixel_values") || !output_names.contains(&"image_embeds") {
        return Err(SemanticError::Inference(
            "MobileCLIP vision graph must expose pixel_values and image_embeds".into(),
        ));
    }
    Ok(())
}

fn validate_mobileclip_text_contract(session: &Session) -> Result<(), SemanticError> {
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
    if !input_names.contains(&"input_ids") || !output_names.contains(&"text_embeds") {
        return Err(SemanticError::Inference(
            "MobileCLIP text graph must expose input_ids and text_embeds".into(),
        ));
    }
    Ok(())
}

fn validate_places365_model_contract(session: &Session) -> Result<(String, String), SemanticError> {
    let input_name = session
        .inputs()
        .first()
        .map(|input| input.name().to_owned())
        .ok_or_else(|| SemanticError::Inference("Places365 model has no input".into()))?;
    let output_name = session
        .outputs()
        .first()
        .map(|output| output.name().to_owned())
        .ok_or_else(|| SemanticError::Inference("Places365 model has no output".into()))?;
    Ok((input_name, output_name))
}

fn load_places365_categories(path: &Path) -> Result<Vec<String>, SemanticError> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| SemanticError::Integrity(format!("{}: {error}", path.display())))?;
    let categories = content
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|label| {
            label
                .strip_prefix('/')
                .and_then(|value| value.split_once('/').map(|(_, leaf)| leaf))
                .unwrap_or(label)
                .to_owned()
        })
        .collect::<Vec<_>>();
    if categories.len() != places365::PLACES365_LEAF_COUNT {
        return Err(SemanticError::Integrity(format!(
            "{} contains {} categories; expected {}",
            path.display(),
            categories.len(),
            places365::PLACES365_LEAF_COUNT
        )));
    }
    Ok(categories)
}

fn load_places365_io(path: &Path) -> Result<Vec<bool>, SemanticError> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| SemanticError::Integrity(format!("{}: {error}", path.display())))?;
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value = line
                .split_whitespace()
                .last()
                .ok_or_else(|| SemanticError::Integrity(format!("invalid IO label: {line}")))?
                .parse::<u8>()
                .map_err(|error| {
                    SemanticError::Integrity(format!("invalid IO label {line}: {error}"))
                })?;
            match value {
                1 => Ok(false),
                2 => Ok(true),
                _ => Err(SemanticError::Integrity(format!(
                    "IO label must be 1 (indoor) or 2 (outdoor): {line}"
                ))),
            }
        })
        .collect()
}

fn preprocess_places365_images(images: &[PathBuf]) -> Result<Vec<f32>, SemanticError> {
    let mut values =
        Vec::with_capacity(images.len() * 3 * PLACES365_IMAGE_SIZE * PLACES365_IMAGE_SIZE);
    for path in images {
        let image = crate::imaging::load_analysis_thumbnail(path)
            .map_err(|error| SemanticError::Inference(format!("{}: {error}", path.display())))?;
        if image.width() == 0 || image.height() == 0 {
            return Err(SemanticError::Inference(format!(
                "image has zero dimensions: {}",
                path.display()
            )));
        }
        // The official ResNet-18 Places365 preprocessing resizes to 256x256
        // and takes the centered 224x224 crop.
        let resized = image::imageops::resize(&image, 256, 256, FilterType::CatmullRom);
        let offset = ((256 - PLACES365_IMAGE_SIZE) / 2) as u32;
        let cropped = image::imageops::crop_imm(
            &resized,
            offset,
            offset,
            PLACES365_IMAGE_SIZE as u32,
            PLACES365_IMAGE_SIZE as u32,
        )
        .to_image();
        for channel in 0..3 {
            for pixel in cropped.pixels() {
                let normalized = (f32::from(pixel[channel]) / 255.0
                    - PLACES365_IMAGE_MEAN[channel])
                    / PLACES365_IMAGE_STD[channel];
                values.push(normalized);
            }
        }
    }
    Ok(values)
}

fn softmax(values: &[f32]) -> Vec<f32> {
    let maximum = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut probabilities = values
        .iter()
        .map(|value| (*value - maximum).exp())
        .collect::<Vec<_>>();
    let total = probabilities.iter().sum::<f32>();
    if total > f32::EPSILON {
        for probability in &mut probabilities {
            *probability /= total;
        }
    }
    probabilities
}

fn select_places365_environment(
    probabilities: &[f32],
    outdoor_by_leaf: &[bool],
) -> Option<(&'static str, f32)> {
    let mut ranked = probabilities.iter().enumerate().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(left.1).then(left.0.cmp(&right.0)));
    let mut indoor = 0.0_f32;
    let mut outdoor = 0.0_f32;
    for (index, probability) in ranked.into_iter().take(10) {
        if outdoor_by_leaf.get(index).copied().unwrap_or(false) {
            outdoor += probability;
        } else {
            indoor += probability;
        }
    }
    let total = indoor + outdoor;
    if total <= f32::EPSILON {
        return None;
    }
    let outdoor_ratio = outdoor / total;
    if outdoor_ratio >= 0.65 {
        Some(("outdoor", outdoor_ratio))
    } else if outdoor_ratio <= 0.35 {
        Some(("indoor", 1.0 - outdoor_ratio))
    } else {
        None
    }
}

fn places365_raw_similarities(
    probabilities: &[f32],
    categories: &[String],
    leaf_cluster_indexes: &[usize],
) -> Vec<SemanticSimilarity> {
    let mut ranked = probabilities.iter().enumerate().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(left.1).then(left.0.cmp(&right.0)));
    ranked
        .into_iter()
        .take(MAX_LABELS)
        .filter_map(|(index, probability)| {
            let cluster = places365::SCENE_CLUSTERS.get(*leaf_cluster_indexes.get(index)?)?;
            Some(SemanticSimilarity {
                label_id: categories.get(index)?.clone(),
                // Raw leaf IDs stay internal.  The visible explanation uses
                // the Chinese product cluster name instead of an English ID.
                display_name: cluster.display_name.into(),
                category_group: "places365_leaf".into(),
                similarity: *probability,
                threshold: 0.0,
            })
        })
        .collect()
}

fn select_places365_topic(
    probabilities: &[f32],
    leaf_cluster_indexes: &[usize],
) -> Option<SemanticPrediction> {
    let mut scores = vec![0.0_f32; topics::TOPIC_LABELS.len()];
    for (index, probability) in probabilities.iter().enumerate() {
        let Some(cluster_index) = leaf_cluster_indexes.get(index) else {
            continue;
        };
        let Some(cluster) = places365::SCENE_CLUSTERS.get(*cluster_index) else {
            continue;
        };
        let Some(topic_id) = places365_cluster_to_topic(cluster.id) else {
            continue;
        };
        let Some(topic_index) = topics::label_index(topic_id) else {
            continue;
        };
        scores[topic_index] += probability;
    }

    let mut ranked = scores
        .iter()
        .enumerate()
        .filter_map(|(index, score)| {
            topics::TOPIC_LABELS
                .get(index)
                .map(|label| (index, *score, label))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
    let (_, score, label) = ranked.first().copied()?;
    let second_score = ranked.get(1).map(|(_, score, _)| *score).unwrap_or(0.0);
    if score < PLACES365_TOPIC_MIN_SCORE || score - second_score < PLACES365_TOPIC_MIN_MARGIN {
        return None;
    }
    Some(SemanticPrediction {
        label_id: label.id.into(),
        display_name: label.display_name.into(),
        category_group: "scene".into(),
        similarity: score,
        threshold: label.threshold,
        is_primary: true,
    })
}

fn places365_cluster_to_topic(cluster_id: &str) -> Option<&'static str> {
    match cluster_id {
        "photo_landscape" => Some("photo_landscape"),
        "photo_urban" => Some("photo_street"),
        "photo_architecture" => Some("photo_architecture"),
        "photo_food" => Some("photo_food"),
        "photo_commercial" => Some("photo_still_life"),
        "photo_event" => Some("photo_activity"),
        "photo_transport" => Some("photo_vehicle"),
        "photo_plant" => Some("photo_macro"),
        // Industrial/work scenes are deliberately not a photographer-facing
        // topic; leave that Places365 evidence unassigned.
        "photo_documentary" => None,
        // Residential/public indoor and travel are useful evidence, but are
        // intentionally not forced into a photographer-facing topic.
        "photo_indoor" | "photo_travel" => None,
        _ => None,
    }
}

pub(crate) fn initialize_ort(runtime_path: &Path) -> Result<(), SemanticError> {
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

pub(crate) fn verify_sha256(path: &Path, expected: &str) -> Result<(), SemanticError> {
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

type TokenizedTopicPrompts = (Vec<i64>, Vec<i64>, Vec<usize>);

fn tokenize_topic_prompts(tokenizer: &Tokenizer) -> Result<TokenizedTopicPrompts, SemanticError> {
    tokenize_topic_prompts_with_config(tokenizer, TOKEN_LENGTH)
}

fn tokenize_topic_prompts_with_config(
    tokenizer: &Tokenizer,
    token_length: usize,
) -> Result<TokenizedTopicPrompts, SemanticError> {
    tokenize_topic_prompts_for_template(tokenizer, token_length, None)
}

fn tokenize_topic_prompts_for_variant(
    tokenizer: &Tokenizer,
    variant: OpenClipVariant,
) -> Result<TokenizedTopicPrompts, SemanticError> {
    let template = match variant {
        OpenClipVariant::Siglip2Base => Some("This is a photo of {label}."),
        OpenClipVariant::MobileclipS0 => None,
    };
    tokenize_topic_prompts_for_template(tokenizer, variant.token_length(), template)
}

fn tokenize_topic_prompts_for_template(
    tokenizer: &Tokenizer,
    token_length: usize,
    template: Option<&str>,
) -> Result<TokenizedTopicPrompts, SemanticError> {
    let specs = topics::prompt_specs();
    let prompts = specs
        .iter()
        .map(|spec| {
            template
                .map(|template| template.replace("{label}", spec.prompt))
                .unwrap_or_else(|| spec.prompt.to_string())
        })
        .collect::<Vec<_>>();
    let label_indexes = specs
        .iter()
        .map(|spec| spec.label_index)
        .collect::<Vec<_>>();
    let (input_ids, attention_mask) =
        tokenize_texts_with_length(tokenizer, &prompts, token_length)?;
    Ok((input_ids, attention_mask, label_indexes))
}

fn tokenize_texts<S: AsRef<str>>(
    tokenizer: &Tokenizer,
    prompts: &[S],
) -> Result<(Vec<i64>, Vec<i64>), SemanticError> {
    tokenize_texts_with_length(tokenizer, prompts, TOKEN_LENGTH)
}

fn tokenize_texts_with_length<S: AsRef<str>>(
    tokenizer: &Tokenizer,
    prompts: &[S],
    token_length: usize,
) -> Result<(Vec<i64>, Vec<i64>), SemanticError> {
    let encodings = tokenizer
        .encode_batch(
            prompts
                .iter()
                .map(|prompt| prompt.as_ref().to_string())
                .collect::<Vec<_>>(),
            true,
        )
        .map_err(|error| SemanticError::Inference(format!("tokenization failed: {error}")))?;
    let mut input_ids = Vec::with_capacity(prompts.len() * token_length);
    let mut attention_mask = Vec::with_capacity(prompts.len() * token_length);
    for encoding in encodings {
        if encoding.len() != token_length {
            return Err(SemanticError::Inference(format!(
                "tokenizer returned {} tokens; expected {token_length}",
                encoding.len(),
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
        let image = crate::imaging::load_analysis_thumbnail(path)
            .map_err(|error| SemanticError::Inference(format!("{}: {error}", path.display())))?;
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

fn preprocess_open_clip_images(
    images: &[PathBuf],
    variant: OpenClipVariant,
) -> Result<Vec<f32>, SemanticError> {
    let image_size = variant.image_size() as u32;
    let mut values =
        Vec::with_capacity(images.len() * 3 * variant.image_size() * variant.image_size());
    for path in images {
        let image = crate::imaging::load_analysis_thumbnail(path)
            .map_err(|error| SemanticError::Inference(format!("{}: {error}", path.display())))?;
        if image.width() == 0 || image.height() == 0 {
            return Err(SemanticError::Inference(format!(
                "image has zero dimensions: {}",
                path.display()
            )));
        }
        let cropped = match variant {
            OpenClipVariant::Siglip2Base => {
                image::imageops::resize(&image, image_size, image_size, FilterType::CatmullRom)
            }
            OpenClipVariant::MobileclipS0 => {
                let shortest = image.width().min(image.height());
                let scale = f64::from(image_size) / f64::from(shortest);
                let resized_width = (f64::from(image.width()) * scale)
                    .round()
                    .max(f64::from(image_size)) as u32;
                let resized_height = (f64::from(image.height()) * scale)
                    .round()
                    .max(f64::from(image_size)) as u32;
                let resized = image::imageops::resize(
                    &image,
                    resized_width,
                    resized_height,
                    FilterType::CatmullRom,
                );
                let left = (resized_width - image_size) / 2;
                let top = (resized_height - image_size) / 2;
                image::imageops::crop_imm(&resized, left, top, image_size, image_size).to_image()
            }
        };
        for channel in 0..3 {
            for pixel in cropped.pixels() {
                let value = f32::from(pixel[channel]) / 255.0;
                let value = match variant {
                    OpenClipVariant::Siglip2Base => (value - 0.5) / 0.5,
                    OpenClipVariant::MobileclipS0 => value,
                };
                values.push(value);
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

fn sigmoid(value: f32) -> f32 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exponential = value.exp();
        exponential / (1.0 + exponential)
    }
}

fn select_topic_predictions(scores: &[f32]) -> Vec<SemanticPrediction> {
    let Some((index, similarity)) = topics::select_primary(scores) else {
        return Vec::new();
    };
    let label = &topics::TOPIC_LABELS[index];
    vec![SemanticPrediction {
        label_id: label.id.into(),
        display_name: label.display_name.into(),
        category_group: "scene".into(),
        similarity,
        threshold: label.threshold,
        is_primary: true,
    }]
}

fn rank_topic_similarities(scores: &[f32]) -> Vec<SemanticSimilarity> {
    let mut ranked = scores
        .iter()
        .enumerate()
        .filter_map(|(index, similarity)| {
            topics::TOPIC_LABELS
                .get(index)
                .map(|label| SemanticSimilarity {
                    label_id: label.id.into(),
                    display_name: label.display_name.into(),
                    category_group: "topic_candidate".into(),
                    similarity: *similarity,
                    threshold: label.threshold,
                })
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .similarity
            .total_cmp(&left.similarity)
            .then(left.label_id.cmp(&right.label_id))
    });
    ranked.truncate(topics::MAX_RAW_CANDIDATES);
    ranked
}

pub(crate) fn cpu_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get().clamp(1, 8))
        .unwrap_or(1)
}

pub(crate) fn inference_error(error: impl std::fmt::Display) -> SemanticError {
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
    pub raw_similarities: Vec<SemanticSimilarity>,
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
                        raw_similarities: result.raw_similarities,
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
                topic_model: None,
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
    fn catalog_has_stable_unique_ids_and_group_metadata() {
        let catalog = semantic_catalog();
        assert_eq!(catalog.len(), topics::TOPIC_LABELS.len() + 2 + 6);
        let mut ids = catalog
            .iter()
            .map(|label| label.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), catalog.len());
        assert_eq!(
            known_display_name_for_label_id("photo_landscape"),
            Some("风光自然")
        );
        assert_eq!(known_display_name_for_label_id("unknown"), Some("抽象艺术"));
        assert_eq!(
            known_display_name_for_label_id("photo_documentary"),
            Some("抽象艺术")
        );
        assert_eq!(known_display_name_for_label_id("person"), Some("单人"));
        assert_eq!(known_display_name_for_label_id("pet"), Some("动物"));
        assert_eq!(
            catalog
                .iter()
                .filter(|label| label.is_primary_category)
                .count(),
            topics::TOPIC_LABELS.len()
        );
        assert_eq!(
            catalog
                .iter()
                .filter(|label| label.category_group == "subject")
                .count(),
            6
        );
        assert!(catalog
            .iter()
            .filter(|label| label.category_group == "scene" || label.category_group == "context")
            .all(|label| label.taxonomy_version == TAXONOMY_VERSION));
        assert!(
            catalog
                .iter()
                .filter(|label| label.is_primary_category)
                .all(|label| label.category_group == "scene")
        );
        assert!(
            !catalog
                .iter()
                .find(|label| label.id == "outdoor")
                .unwrap()
                .is_primary_category
        );
        assert!(!catalog.iter().any(|label| label.id == "unknown"));
        assert!(!catalog.iter().any(|label| label.id == "photo_documentary"));
    }

    #[test]
    fn legacy_labels_canonicalize_to_the_consolidated_taxonomy() {
        assert_eq!(canonical_label_id("unknown"), "photo_abstract");
        assert_eq!(canonical_label_id("photo_documentary"), "photo_abstract");
        assert_eq!(canonical_label_id("person"), "single_person");
        assert_eq!(canonical_label_id("portrait"), "single_person");
        assert_eq!(canonical_label_id("group"), "multiple_people");
        assert_eq!(canonical_label_id("pet"), "animal");
    }

    #[test]
    fn unknown_is_not_a_model_candidate() {
        let mut scores = vec![0.05; topics::TOPIC_LABELS.len()];
        scores[topics::label_index("photo_landscape").unwrap()] = 0.30;

        let predictions = select_topic_predictions(&scores);

        assert!(
            predictions
                .iter()
                .any(|label| label.label_id == "photo_landscape")
        );
        assert!(!predictions.iter().any(|label| label.label_id == "unknown"));
    }

    #[test]
    fn siglip2_is_the_default_topic_model() {
        assert_eq!(DEFAULT_TOPIC_MODEL, TopicModelKind::Siglip2Base);
        assert_eq!(
            TopicModelKind::parse("siglip2-base"),
            Some(TopicModelKind::Siglip2Base)
        );
        assert!(TopicModelKind::parse("tinyclip").is_none());
        assert!(TopicModelKind::parse("mobileclip-s0").is_none());
        let metadata = default_topic_model_metadata();
        assert_eq!(metadata.name, SIGLIP2_MODEL_NAME);
        assert_eq!(metadata.version, SIGLIP2_MODEL_VERSION);
        assert_eq!(metadata.analysis_version, SIGLIP2_ANALYSIS_VERSION);
        assert_eq!(metadata.license.as_deref(), Some("Apache-2.0"));
        assert_eq!(metadata.model_sha256.as_deref(), Some(SIGLIP2_MODEL_SHA256));
    }

    #[test]
    fn primary_label_uses_highest_accepted_similarity() {
        let mut scores = vec![0.10; topics::TOPIC_LABELS.len()];
        scores[topics::label_index("photo_landscape").unwrap()] = 0.21;
        scores[topics::label_index("photo_food").unwrap()] = 0.30;
        let predictions = select_topic_predictions(&scores);
        assert_eq!(predictions[0].label_id, "photo_food");
        assert!(predictions[0].is_primary);
        assert_eq!(
            predictions.iter().filter(|label| label.is_primary).count(),
            1
        );
        assert_eq!(predictions.len(), 1);
    }

    #[test]
    fn low_confidence_success_is_empty_for_the_abstract_fallback() {
        let scores = vec![0.01; topics::TOPIC_LABELS.len()];
        let predictions = select_topic_predictions(&scores);
        assert!(predictions.is_empty());
        assert!(!predictions.iter().any(|label| label.label_id == "unknown"));
    }

    #[test]
    fn places365_evidence_maps_to_a_photography_topic() {
        let landscape_cluster = places365::SCENE_CLUSTERS
            .iter()
            .position(|cluster| cluster.id == "photo_landscape")
            .unwrap();
        let street_cluster = places365::SCENE_CLUSTERS
            .iter()
            .position(|cluster| cluster.id == "photo_urban")
            .unwrap();
        let prediction =
            select_places365_topic(&[0.42, 0.08], &[landscape_cluster, street_cluster]).unwrap();
        assert_eq!(prediction.label_id, "photo_landscape");
        assert!(prediction.is_primary);
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
