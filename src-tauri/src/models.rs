use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySummary {
    pub id: i64,
    pub root_path: String,
    pub created_at: String,
    pub last_scan_at: Option<String>,
    pub status: String,
    pub asset_count: i64,
    pub present_count: i64,
    pub missing_count: i64,
    pub semantic_pending_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetListItem {
    pub id: i64,
    pub library_id: i64,
    pub absolute_path: String,
    pub relative_path: String,
    pub file_name: String,
    pub extension: String,
    pub file_size: i64,
    pub modified_at: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub orientation: Option<u32>,
    pub capture_time: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub exposure_time: Option<String>,
    pub aperture: Option<f64>,
    pub iso: Option<i64>,
    pub focal_length: Option<f64>,
    pub file_status: String,
    pub scan_status: String,
    pub analysis_status: String,
    pub error_message: Option<String>,
    pub thumbnail_available: bool,
    pub brightness: Option<f64>,
    pub contrast: Option<f64>,
    pub tone_label: Option<String>,
    pub saturation: Option<f64>,
    pub chroma: Option<f64>,
    pub saturation_label: Option<String>,
    pub dominant_color: Option<String>,
    pub dominant_color_category: Option<String>,
    pub neutral_ratio: Option<f64>,
    pub dominant_color_coverage: Option<f64>,
    pub semantic_status: String,
    pub semantic_error: Option<String>,
    pub semantic_analyzed_at: Option<String>,
    pub semantic_labels: Vec<SemanticLabelResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticLabelResult {
    pub label_id: String,
    pub display_name: String,
    pub similarity: f64,
    pub threshold: f64,
    pub model_name: String,
    pub model_version: String,
    pub analysis_version: String,
    pub analyzed_at: String,
    pub is_manual: bool,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetPage {
    pub items: Vec<AssetListItem>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SemanticMatchMode {
    #[default]
    Any,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetFilter {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub semantic_labels: Vec<String>,
    #[serde(default)]
    pub semantic_match: SemanticMatchMode,
    #[serde(default)]
    pub tone_labels: Vec<String>,
    #[serde(default)]
    pub color_categories: Vec<String>,
    #[serde(default)]
    pub brightness_min: Option<f64>,
    #[serde(default)]
    pub brightness_max: Option<f64>,
    #[serde(default)]
    pub saturation_min: Option<f64>,
    #[serde(default)]
    pub saturation_max: Option<f64>,
    #[serde(default)]
    pub captured_from: Option<String>,
    #[serde(default)]
    pub captured_to: Option<String>,
    #[serde(default)]
    pub folder_prefix: Option<String>,
    #[serde(default)]
    pub semantic_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FolderSummary {
    pub relative_path: String,
    pub asset_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticGroupSummary {
    pub label_id: String,
    pub display_name: String,
    pub asset_count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssetSortField {
    #[default]
    FileName,
    CaptureTime,
    ModifiedTime,
    Brightness,
    Saturation,
}

impl AssetSortField {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "file_name" => Some(Self::FileName),
            "capture_time" => Some(Self::CaptureTime),
            "modified_time" => Some(Self::ModifiedTime),
            "brightness" => Some(Self::Brightness),
            "saturation" => Some(Self::Saturation),
            _ => None,
        }
    }

    pub fn sql_expression(self) -> &'static str {
        match self {
            Self::FileName => "a.file_name COLLATE NOCASE",
            Self::CaptureTime => {
                "CASE WHEN a.capture_time IS NULL THEN 1 ELSE 0 END, a.capture_time"
            }
            Self::ModifiedTime => "a.modified_at",
            Self::Brightness => {
                "CASE WHEN tf.brightness_mean IS NULL THEN 1 ELSE 0 END, tf.brightness_mean"
            }
            Self::Saturation => {
                "CASE WHEN cf.saturation_mean IS NULL THEN 1 ELSE 0 END, cf.saturation_mean"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "asc" => Some(Self::Asc),
            "desc" => Some(Self::Desc),
            _ => None,
        }
    }

    pub fn sql(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartScanResponse {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CancelScanResponse {
    pub task_id: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartSemanticResponse {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTaskResponse {
    pub job_id: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticProgress {
    pub job_id: String,
    pub library_id: i64,
    pub status: String,
    pub total: u64,
    pub processed: u64,
    pub completed: u64,
    pub failed: u64,
    pub skipped: u64,
    pub current_asset_id: Option<i64>,
    pub current_path: Option<String>,
    pub execution_backend: Option<String>,
    pub model_name: String,
    pub model_version: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub task_id: String,
    pub library_id: Option<i64>,
    pub status: String,
    pub stage: String,
    pub discovered: u64,
    pub processed: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub skipped: u64,
    pub missing: u64,
    pub current_path: Option<String>,
    pub error: Option<String>,
}

impl ScanProgress {
    pub fn starting(task_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            library_id: None,
            status: "running".into(),
            stage: "preparing".into(),
            discovered: 0,
            processed: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            missing: 0,
            current_path: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub task_id: String,
    pub library_id: i64,
    pub status: String,
    pub discovered: u64,
    pub processed: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub skipped: u64,
    pub missing: u64,
}

#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub absolute_path: String,
    pub relative_path: String,
    pub file_name: String,
    pub extension: String,
    pub file_size: i64,
    pub modified_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingAssetSnapshot {
    pub id: i64,
    pub file_size: i64,
    pub modified_at: i64,
    pub analysis_status: String,
    pub analysis_algorithm_version: Option<String>,
    pub thumbnail_status: Option<String>,
    pub cache_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BasicImageFeatures {
    pub brightness_mean: f64,
    pub brightness_median: f64,
    pub brightness_low_percentile: f64,
    pub brightness_high_percentile: f64,
    pub shadow_ratio: f64,
    pub highlight_ratio: f64,
    pub contrast: f64,
    pub dynamic_range: f64,
    pub tone_label: String,
    pub exposure_label: String,
    pub contrast_label: String,
    pub saturation_mean: f64,
    pub saturation_median: f64,
    pub chroma_mean: f64,
    pub dominant_color_rgb: String,
    pub dominant_color_category: String,
    pub dominant_colors_json: String,
    pub hue_histogram_json: String,
    pub warmth_score: f64,
    pub neutral_ratio: f64,
    pub colorfulness: f64,
    pub monochrome_probability: f64,
    pub dominant_color_coverage: f64,
    pub saturation_label: String,
    pub algorithm_version: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExifMetadata {
    pub orientation: u32,
    pub capture_time: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub exposure_time: Option<String>,
    pub aperture: Option<f64>,
    pub iso: Option<i64>,
    pub focal_length: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ProcessedImage {
    pub width: u32,
    pub height: u32,
    pub exif: ExifMetadata,
    pub thumbnail_path: String,
    pub features: BasicImageFeatures,
}

/// The source set used to generate a read-only organization preview.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationScope {
    #[default]
    All,
    Filtered,
    Selected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationLevelKind {
    Year,
    Month,
    Day,
    OriginalDirectory,
    PrimarySemantic,
    Tone,
    DominantColor,
    Saturation,
}

impl OrganizationLevelKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Year => "year",
            Self::Month => "month",
            Self::Day => "day",
            Self::OriginalDirectory => "original_directory",
            Self::PrimarySemantic => "primary_semantic",
            Self::Tone => "tone",
            Self::DominantColor => "dominant_color",
            Self::Saturation => "saturation",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationMissingFallback {
    ModificationTime,
    #[default]
    Unknown,
    Skip,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationConflictStrategy {
    Skip,
    #[default]
    Sequence,
    ShortHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationLevel {
    pub kind: OrganizationLevelKind,
    #[serde(default)]
    pub fallback: OrganizationMissingFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationRules {
    #[serde(default = "organization_rules_version")]
    pub version: String,
    #[serde(default)]
    pub levels: Vec<OrganizationLevel>,
    pub template: String,
    #[serde(default = "organization_sequence_start")]
    pub sequence_start: u32,
    #[serde(default = "organization_sequence_width")]
    pub sequence_width: u8,
    #[serde(default)]
    pub missing_fallback: OrganizationMissingFallback,
    #[serde(default)]
    pub conflict_strategy: OrganizationConflictStrategy,
}

impl Default for OrganizationRules {
    fn default() -> Self {
        Self {
            version: organization_rules_version(),
            levels: vec![
                OrganizationLevel {
                    kind: OrganizationLevelKind::Year,
                    fallback: OrganizationMissingFallback::ModificationTime,
                },
                OrganizationLevel {
                    kind: OrganizationLevelKind::Month,
                    fallback: OrganizationMissingFallback::ModificationTime,
                },
                OrganizationLevel {
                    kind: OrganizationLevelKind::PrimarySemantic,
                    fallback: OrganizationMissingFallback::Unknown,
                },
            ],
            template: "{capture_time:yyyyMMdd_HHmmss}_{semantic}_{original_stem}_{sequence:0000}"
                .into(),
            sequence_start: organization_sequence_start(),
            sequence_width: organization_sequence_width(),
            missing_fallback: OrganizationMissingFallback::Unknown,
            conflict_strategy: OrganizationConflictStrategy::Sequence,
        }
    }
}

fn organization_rules_version() -> String {
    "organization-rules-v1".into()
}

fn organization_sequence_start() -> u32 {
    1
}

fn organization_sequence_width() -> u8 {
    4
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPlanRequest {
    pub library_id: i64,
    pub target_root: String,
    #[serde(default)]
    pub scope: OrganizationScope,
    #[serde(default)]
    pub filter: AssetFilter,
    #[serde(default)]
    pub selected_asset_ids: Vec<i64>,
    #[serde(default)]
    pub rules: OrganizationRules,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationItemStatus {
    Ready,
    Warning,
    Error,
    SkippedConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationIssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationIssue {
    pub code: String,
    pub severity: OrganizationIssueSeverity,
    pub source_path: Option<String>,
    pub target_path: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPlanItem {
    pub ordinal: u32,
    pub asset_id: i64,
    pub source_path: String,
    pub source_relative_path: String,
    pub source_fingerprint: String,
    pub target_relative_path: String,
    pub target_path: String,
    pub file_size: u64,
    pub status: OrganizationItemStatus,
    pub variables: std::collections::BTreeMap<String, String>,
    pub issues: Vec<OrganizationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationTreeNode {
    pub name: String,
    pub relative_path: String,
    pub file_count: u64,
    pub byte_count: u64,
    pub children: Vec<OrganizationTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPlanSummary {
    pub plan_id: String,
    pub library_id: i64,
    pub source_root: String,
    pub target_root: String,
    pub scope: OrganizationScope,
    pub item_count: u64,
    pub conflict_count: u64,
    pub error_count: u64,
    pub warning_count: u64,
    pub estimated_bytes: u64,
    pub target_available_bytes: Option<u64>,
    pub generated_at: String,
    pub status: String,
    pub source_snapshot: String,
    pub rules: OrganizationRules,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPlan {
    pub summary: OrganizationPlanSummary,
    pub items: Vec<OrganizationPlanItem>,
    pub tree: OrganizationTreeNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPlanRecord {
    pub plan_id: String,
    pub library_id: i64,
    pub target_root: String,
    pub scope: OrganizationScope,
    pub rules: OrganizationRules,
    pub summary: OrganizationPlanSummary,
    pub created_at: String,
    pub updated_at: String,
}
