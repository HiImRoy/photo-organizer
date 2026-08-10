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

use crate::places365;

pub const MODEL_NAME: &str = "Places365-ResNet18";
pub const MODEL_VERSION: &str = "onnx-2026-08-10";
pub const ANALYSIS_VERSION: &str = "photo-organizer-semantic-places365-photography-v1";
pub const TAXONOMY_VERSION: &str = places365::TAXONOMY_VERSION;
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
pub const RUNTIME_SHA256: &str = "8a1aad8d59d02a5337d4e3f5bbd1158c3f7bf84fe3b3f0052f957dd3e75a91cb";
pub const EMBEDDING_DIMENSIONS: usize = 512;

const IMAGE_SIZE: usize = 224;
const PLACES365_IMAGE_SIZE: usize = 224;
const TOKEN_LENGTH: usize = 77;
const PAD_TOKEN_ID: u32 = 49_407;
const MAX_LABELS: usize = 8;
const TOP_SCORE_WINDOW: f32 = 0.055;
const SCENE_SCORE_MARGIN: f32 = 0.025;
const PLACES365_SCENE_MIN_PROBABILITY: f32 = 0.24;
const PLACES365_SCENE_MIN_MARGIN: f32 = 0.045;
const IMAGE_MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const IMAGE_STD: [f32; 3] = [0.268_629_54, 0.261_302_6, 0.275_777_1];
const PLACES365_IMAGE_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const PLACES365_IMAGE_STD: [f32; 3] = [0.229, 0.224, 0.225];

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

#[derive(Debug, Clone, Copy)]
struct LabelDefinition {
    id: &'static str,
    display_name: &'static str,
    prompt: &'static str,
    category_group: &'static str,
    threshold: f32,
}

const LABELS: [LabelDefinition; 13] = [
    LabelDefinition {
        id: "photo_landscape",
        display_name: "风光自然",
        prompt: "a landscape or nature photograph with mountains, water, forest, coast, or other landform",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_urban",
        display_name: "城市街拍",
        prompt: "a street photography scene in a city, town, neighborhood, village, or public outdoor area",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_architecture",
        display_name: "建筑与空间",
        prompt: "an architectural photograph of a building, landmark, historical site, bridge, castle, temple, or religious space",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_food",
        display_name: "美食餐饮",
        prompt: "a food or dining photograph showing a dish, restaurant, cafe, market, kitchen, or dining space",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_commercial",
        display_name: "商业与静物",
        prompt: "a commercial, product, shop, market, salon, or still life photograph",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_indoor",
        display_name: "室内与生活",
        prompt: "an interior or everyday life photograph inside a home, office, school, library, laboratory, hospital, or public room",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_travel",
        display_name: "旅行人文",
        prompt: "a travel or cultural documentary photograph showing a destination, museum, hotel, airport, station, harbor, or local experience",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_event",
        display_name: "活动与运动",
        prompt: "an event, sports, performance, entertainment, stadium, theater, amusement park, or pool photograph",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_transport",
        display_name: "交通与汽车",
        prompt: "a transportation or automobile photograph showing a vehicle, road, garage, parking area, rail track, or transport facility",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_plant",
        display_name: "植物与园艺",
        prompt: "a plant, garden, orchard, greenhouse, cultivated field, park, or horticulture photograph",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "photo_documentary",
        display_name: "纪实与工业",
        prompt: "an industrial, construction, worksite, utility, repair, military, or documentary photograph",
        category_group: "scene",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "indoor",
        display_name: "室内",
        prompt: "an indoor scene inside a room or building",
        category_group: "context",
        threshold: 0.16,
    },
    LabelDefinition {
        id: "outdoor",
        display_name: "室外",
        prompt: "an outdoor scene under the open sky",
        category_group: "context",
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
    let mut catalog = LABELS
        .iter()
        .map(|label| SemanticLabelDescriptor {
            id: label.id.into(),
            display_name: label.display_name.into(),
            category_group: label.category_group.into(),
            threshold: label.threshold,
            is_primary_category: is_primary_category(label.id),
            taxonomy_version: TAXONOMY_VERSION.into(),
        })
        .collect::<Vec<_>>();
    catalog.extend(crate::subject::subject_catalog());
    catalog
}

pub fn known_display_name_for_label_id(label_id: &str) -> Option<&'static str> {
    if label_id == "unknown" {
        return Some("未知");
    }
    let legacy_name = match label_id {
        "person" => Some("人物"),
        "portrait" => Some("人像"),
        "group" => Some("多人"),
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
        "pet" => Some("宠物"),
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
    LABELS
        .iter()
        .find(|label| label.id == label_id)
        .map(|label| label.display_name)
}

pub fn category_group_for_label_id(label_id: &str) -> Option<&'static str> {
    if label_id == "unknown" {
        return Some("scene");
    }
    let legacy_group = match label_id {
        "person" | "portrait" | "group" | "animal" | "pet" | "vehicle" | "plant" => Some("subject"),
        "landscape" | "architecture" | "product" | "still_life" | "food" | "screenshot"
        | "document" | "abstract" => Some("scene"),
        "flower" | "mountain" | "water" | "forest" => Some("subject"),
        "street" | "night" | "sunset" | "indoor" | "outdoor" => Some("context"),
        _ => None,
    };
    if legacy_group.is_some() {
        return legacy_group;
    }
    LABELS
        .iter()
        .find(|label| label.id == label_id)
        .map(|label| label.category_group)
}

fn is_primary_category(label_id: &str) -> bool {
    LABELS
        .iter()
        .find(|label| label.id == label_id)
        .is_some_and(|label| label.category_group == "scene")
}

fn is_active_label(label_id: &str) -> bool {
    LABELS.iter().any(|label| label.id == label_id)
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

pub struct Places365Classifier {
    session: Mutex<Session>,
    input_name: String,
    output_name: String,
    categories: Vec<String>,
    leaf_cluster_indexes: Vec<usize>,
    outdoor_by_leaf: Vec<bool>,
    model_size_bytes: u64,
    embedding_classifier: Option<TinyClipClassifier>,
}

impl std::fmt::Debug for Places365Classifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Places365Classifier")
            .field("model", &MODEL_NAME)
            .field("version", &MODEL_VERSION)
            .field("categories", &self.categories.len())
            .field(
                "has_embedding_classifier",
                &self.embedding_classifier.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl Places365Classifier {
    pub fn load(
        model_dir: &Path,
        embedding_model_dir: &Path,
        runtime_path: &Path,
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

        let embedding_classifier = match TinyClipClassifier::load(embedding_model_dir, runtime_path)
        {
            Ok(classifier) => Some(classifier),
            Err(error) => {
                log::warn!("TinyCLIP embedding adapter unavailable: {error}");
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
            embedding_classifier,
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
        let message = if self.embedding_classifier.is_some() {
            "Places365 ResNet-18 已就绪；场景分类使用本地 365 类模型，向量搜索使用本地 TinyCLIP。"
        } else {
            "Places365 ResNet-18 已就绪；场景分类可用，但本地文本与相似搜索向量模型未就绪。"
        };
        SemanticRuntimeStatus {
            status: "ready".into(),
            message: message.into(),
            model: self.metadata(),
            selected_backend: Some(ExecutionBackend::Cpu),
        }
    }

    fn encode_text(&self, queries: &[String]) -> Result<Vec<Vec<f32>>, SemanticError> {
        self.embedding_classifier
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

        let embedding_outputs = self.embedding_classifier.as_ref().and_then(|classifier| {
            match classifier.classify_batch(images, ExecutionBackend::Cpu) {
                Ok(outputs) if outputs.len() == image_count => Some(outputs),
                Ok(outputs) => {
                    log::warn!(
                        "TinyCLIP returned {} embeddings for {} images",
                        outputs.len(),
                        image_count
                    );
                    None
                }
                Err(error) => {
                    log::warn!("TinyCLIP embedding batch failed: {error}");
                    None
                }
            }
        });
        let mut results = Vec::with_capacity(image_count);
        for (image_index, probabilities) in probability_rows.into_iter().enumerate() {
            let primary = select_places365_primary(
                &probabilities,
                &self.leaf_cluster_indexes,
                places365::SCENE_CLUSTERS.len(),
            );
            let mut predictions = Vec::with_capacity(2);
            if let Some((cluster_index, score)) = primary {
                let cluster = &places365::SCENE_CLUSTERS[cluster_index];
                predictions.push(SemanticPrediction {
                    label_id: cluster.id.into(),
                    display_name: cluster.display_name.into(),
                    category_group: "scene".into(),
                    similarity: score,
                    threshold: PLACES365_SCENE_MIN_PROBABILITY,
                    is_primary: true,
                });
            }
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
            let raw_similarities = places365_raw_similarities(
                &probabilities,
                &self.categories,
                &self.leaf_cluster_indexes,
            );
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
        let (prompt_input_ids, prompt_attention_mask) = tokenize_prompts(&tokenizer)?;

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            prompt_input_ids,
            prompt_attention_mask,
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
            message: "TinyCLIP INT8 向量模型已通过完整性校验，可用于本地文本与相似搜索。".into(),
            model: self.metadata(),
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
        let image = image::open(path)
            .map_err(|error| SemanticError::Inference(format!("{}: {error}", path.display())))?
            .to_rgb8();
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

fn select_places365_primary(
    probabilities: &[f32],
    leaf_cluster_indexes: &[usize],
    cluster_count: usize,
) -> Option<(usize, f32)> {
    let mut scores = vec![0.0_f32; cluster_count];
    for (probability, cluster_index) in probabilities.iter().zip(leaf_cluster_indexes) {
        if let Some(score) = scores.get_mut(*cluster_index) {
            *score += probability;
        }
    }
    let mut ranked = scores.into_iter().enumerate().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
    let (top_index, top_score) = ranked.first().copied()?;
    let second_score = ranked.get(1).map(|(_, score)| *score).unwrap_or(0.0);
    (top_score >= PLACES365_SCENE_MIN_PROBABILITY
        && top_score - second_score >= PLACES365_SCENE_MIN_MARGIN)
        .then_some((top_index, top_score))
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

fn tokenize_prompts(tokenizer: &Tokenizer) -> Result<(Vec<i64>, Vec<i64>), SemanticError> {
    let prompts = LABELS.iter().map(|label| label.prompt).collect::<Vec<_>>();
    tokenize_texts(tokenizer, &prompts)
}

fn tokenize_texts<S: AsRef<str>>(
    tokenizer: &Tokenizer,
    prompts: &[S],
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
    let mut input_ids = Vec::with_capacity(prompts.len() * TOKEN_LENGTH);
    let mut attention_mask = Vec::with_capacity(prompts.len() * TOKEN_LENGTH);
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
    let mut accepted = Vec::<(usize, f32)>::new();

    // `scene` is the compatibility primary category. It is deliberately
    // selected separately from attributes so two unrelated labels cannot
    // force one another into the primary slot.
    let mut scene_candidates = candidates_for_group(scores, "scene");
    scene_candidates.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
    if let Some((index, score)) = scene_candidates.first().copied()
        && score >= LABELS[index].threshold
        && scene_candidates
            .get(1)
            .is_none_or(|(_, second_score)| score - *second_score >= SCENE_SCORE_MARGIN)
    {
        accepted.push((index, score));
    }

    for group in ["subject", "context"] {
        let mut candidates = candidates_for_group(scores, group);
        candidates.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
        let Some((_, top_score)) = candidates.first().copied() else {
            continue;
        };
        accepted.extend(
            candidates
                .into_iter()
                .filter(|(index, score)| {
                    *score >= LABELS[*index].threshold && *score >= top_score - TOP_SCORE_WINDOW
                })
                .take(MAX_LABELS),
        );
    }

    accepted.sort_by(|left, right| {
        is_primary_category(LABELS[left.0].id)
            .cmp(&is_primary_category(LABELS[right.0].id))
            .reverse()
            .then(right.1.total_cmp(&left.1))
            .then(left.0.cmp(&right.0))
    });
    accepted.truncate(MAX_LABELS);

    let primary_index = accepted
        .iter()
        .find(|(index, _)| is_primary_category(LABELS[*index].id))
        .map(|(index, _)| *index);
    accepted
        .into_iter()
        .map(|(index, similarity)| SemanticPrediction {
            label_id: LABELS[index].id.into(),
            display_name: LABELS[index].display_name.into(),
            category_group: LABELS[index].category_group.into(),
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
        .filter(|(index, _)| is_active_label(LABELS[*index].id))
        .map(|(index, similarity)| SemanticSimilarity {
            label_id: LABELS[index].id.into(),
            display_name: LABELS[index].display_name.into(),
            category_group: LABELS[index].category_group.into(),
            similarity: *similarity,
            threshold: LABELS[index].threshold,
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
    ranked
}

fn candidates_for_group(scores: &[f32], group: &str) -> Vec<(usize, f32)> {
    scores
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            is_active_label(LABELS[*index].id) && LABELS[*index].category_group == group
        })
        .map(|(index, score)| (index, *score))
        .collect()
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
    fn catalog_has_stable_unique_ids_and_group_metadata() {
        let catalog = semantic_catalog();
        assert_eq!(catalog.len(), 21);
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
        assert_eq!(known_display_name_for_label_id("unknown"), Some("未知"));
        assert_eq!(
            catalog
                .iter()
                .filter(|label| label.is_primary_category)
                .count(),
            11
        );
        assert_eq!(
            catalog
                .iter()
                .filter(|label| label.category_group == "subject")
                .count(),
            8
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
    }

    #[test]
    fn unknown_is_not_a_model_candidate() {
        let mut scores = vec![0.05; LABELS.len()];
        scores[LABELS
            .iter()
            .position(|label| label.id == "photo_landscape")
            .unwrap()] = 0.30;

        let predictions = select_predictions(&scores);

        assert!(
            predictions
                .iter()
                .any(|label| label.label_id == "photo_landscape")
        );
        assert!(!predictions.iter().any(|label| label.label_id == "unknown"));
    }

    #[test]
    fn primary_label_uses_highest_accepted_similarity() {
        let mut scores = vec![0.10; LABELS.len()];
        scores[LABELS
            .iter()
            .position(|label| label.id == "photo_landscape")
            .unwrap()] = 0.21;
        scores[LABELS
            .iter()
            .position(|label| label.id == "photo_food")
            .unwrap()] = 0.24;
        scores[LABELS
            .iter()
            .position(|label| label.id == "outdoor")
            .unwrap()] = 0.24;
        let predictions = select_predictions(&scores);
        assert_eq!(predictions[0].label_id, "photo_food");
        assert!(predictions[0].is_primary);
        assert_eq!(
            predictions.iter().filter(|label| label.is_primary).count(),
            1
        );
        assert!(predictions.iter().any(|label| label.label_id == "outdoor"));
    }

    #[test]
    fn low_confidence_success_is_empty_and_resolves_to_virtual_unknown() {
        let scores = vec![0.01; LABELS.len()];
        let predictions = select_predictions(&scores);
        assert!(predictions.is_empty());
        assert!(!predictions.iter().any(|label| label.label_id == "unknown"));
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
