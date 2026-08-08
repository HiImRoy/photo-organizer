use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

pub const FIELD_PRIMARY_CATEGORY: &str = "primary_category";
pub const FIELD_AUXILIARY_TAGS: &str = "auxiliary_tags";
pub const FIELD_TONE: &str = "tone";
pub const FIELD_DOMINANT_COLOR_CATEGORY: &str = "dominant_color_category";
pub const FIELD_SATURATION_LEVEL: &str = "saturation_level";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationFieldDescriptor {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub filterable: bool,
    pub supports_manual_override: bool,
    pub supports_restore_auto: bool,
}

pub fn registry_descriptors() -> Vec<ClassificationFieldDescriptor> {
    vec![
        descriptor(FIELD_PRIMARY_CATEGORY, "主类别", "single"),
        descriptor(FIELD_AUXILIARY_TAGS, "辅助标签", "multi"),
        descriptor(FIELD_TONE, "影调", "single"),
        descriptor(FIELD_DOMINANT_COLOR_CATEGORY, "主色", "multi"),
        descriptor(FIELD_SATURATION_LEVEL, "饱和度级别", "single"),
    ]
}

fn descriptor(id: &str, display_name: &str, kind: &str) -> ClassificationFieldDescriptor {
    ClassificationFieldDescriptor {
        id: id.to_owned(),
        display_name: display_name.to_owned(),
        kind: kind.to_owned(),
        filterable: true,
        supports_manual_override: true,
        supports_restore_auto: true,
    }
}

pub fn is_registry_field(field: &str) -> bool {
    matches!(
        field,
        FIELD_PRIMARY_CATEGORY
            | FIELD_AUXILIARY_TAGS
            | FIELD_TONE
            | FIELD_DOMINANT_COLOR_CATEGORY
            | FIELD_SATURATION_LEVEL
    )
}

pub fn normalize_override_value(field: &str, value: Value) -> AppResult<Value> {
    if !is_registry_field(field) || field == FIELD_AUXILIARY_TAGS {
        return Err(AppError::InvalidArgument(format!(
            "field {field} is not a single classification override"
        )));
    }

    if field == FIELD_DOMINANT_COLOR_CATEGORY {
        let values = match value {
            Value::String(value) => vec![value],
            Value::Array(values) => values
                .into_iter()
                .map(|value| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        AppError::InvalidArgument(
                            "dominant color categories must be strings".to_owned(),
                        )
                    })
                })
                .collect::<AppResult<Vec<_>>>()?,
            _ => {
                return Err(AppError::InvalidArgument(
                    "dominant color categories must be a string array".to_owned(),
                ));
            }
        };
        let values = normalize_string_list(values, "dominant color categories")?;
        return Ok(serde_json::to_value(values)?);
    }

    let value = value
        .as_str()
        .ok_or_else(|| AppError::InvalidArgument(format!("{field} override must be a string")))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::InvalidArgument(format!(
            "{field} override cannot be empty"
        )));
    }
    Ok(Value::String(value.to_owned()))
}

pub fn normalize_tag_id(tag_id: &str) -> AppResult<String> {
    let tag_id = tag_id.trim();
    if tag_id.is_empty() {
        return Err(AppError::InvalidArgument(
            "tag id cannot be empty".to_owned(),
        ));
    }
    Ok(tag_id.to_owned())
}

fn normalize_string_list(values: Vec<String>, label: &str) -> AppResult<Vec<String>> {
    let mut result = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            return Err(AppError::InvalidArgument(format!(
                "{label} cannot contain empty values"
            )));
        }
        if seen.insert(value.to_owned()) {
            result.push(value.to_owned());
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClassificationSource {
    None,
    Auto,
    Manual,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationFieldState<T> {
    pub auto: Option<T>,
    pub manual: Option<T>,
    pub effective: Option<T>,
    pub source: ClassificationSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuxiliaryTagState {
    pub auto: Vec<String>,
    pub manual_additions: Vec<String>,
    pub manual_removals: Vec<String>,
    pub effective: Vec<String>,
    pub source: ClassificationSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveClassification {
    pub revision: i64,
    pub primary_category: ClassificationFieldState<String>,
    pub auxiliary_tags: AuxiliaryTagState,
    pub tone: ClassificationFieldState<String>,
    pub dominant_color_categories: ClassificationFieldState<Vec<String>>,
    pub saturation_level: ClassificationFieldState<String>,
}

impl Default for EffectiveClassification {
    fn default() -> Self {
        Self::empty(0)
    }
}

impl EffectiveClassification {
    pub fn empty(revision: i64) -> Self {
        Self {
            revision,
            primary_category: ClassificationFieldState {
                auto: None,
                manual: None,
                effective: None,
                source: ClassificationSource::None,
            },
            auxiliary_tags: AuxiliaryTagState {
                auto: Vec::new(),
                manual_additions: Vec::new(),
                manual_removals: Vec::new(),
                effective: Vec::new(),
                source: ClassificationSource::None,
            },
            tone: ClassificationFieldState {
                auto: None,
                manual: None,
                effective: None,
                source: ClassificationSource::None,
            },
            dominant_color_categories: ClassificationFieldState {
                auto: None,
                manual: None,
                effective: None,
                source: ClassificationSource::None,
            },
            saturation_level: ClassificationFieldState {
                auto: None,
                manual: None,
                effective: None,
                source: ClassificationSource::None,
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AutoClassification {
    pub primary_category: Option<String>,
    pub auxiliary_tags: Vec<String>,
    pub tone: Option<String>,
    pub dominant_color_categories: Vec<String>,
    pub saturation_level: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ManualClassification {
    pub primary_category: Option<String>,
    pub auxiliary_tags: Vec<String>,
    pub dominant_color_categories: Option<Vec<String>>,
    pub tone: Option<String>,
    pub saturation_level: Option<String>,
    pub auxiliary_tag_additions: Vec<String>,
    pub auxiliary_tag_removals: Vec<String>,
}

pub fn resolve_classification(
    revision: i64,
    auto: AutoClassification,
    manual: ManualClassification,
) -> EffectiveClassification {
    let ManualClassification {
        primary_category,
        auxiliary_tag_additions,
        auxiliary_tag_removals,
        dominant_color_categories,
        tone,
        saturation_level,
        ..
    } = manual;
    EffectiveClassification {
        revision,
        primary_category: resolve_single(auto.primary_category, primary_category),
        auxiliary_tags: resolve_tags(
            auto.auxiliary_tags,
            auxiliary_tag_additions,
            auxiliary_tag_removals,
        ),
        tone: resolve_single(auto.tone, tone),
        dominant_color_categories: resolve_single(
            if auto.dominant_color_categories.is_empty() {
                None
            } else {
                Some(auto.dominant_color_categories)
            },
            dominant_color_categories,
        ),
        saturation_level: resolve_single(auto.saturation_level, saturation_level),
    }
}

fn resolve_single<T: Clone>(auto: Option<T>, manual: Option<T>) -> ClassificationFieldState<T> {
    let effective = manual.as_ref().cloned().or_else(|| auto.as_ref().cloned());
    let source = match (auto.is_some(), manual.is_some()) {
        (_, true) => ClassificationSource::Manual,
        (true, false) => ClassificationSource::Auto,
        (false, false) => ClassificationSource::None,
    };
    ClassificationFieldState {
        auto,
        manual,
        effective,
        source,
    }
}

fn resolve_tags(
    auto: Vec<String>,
    manual_additions: Vec<String>,
    manual_removals: Vec<String>,
) -> AuxiliaryTagState {
    let mut effective = auto.clone();
    for tag in &manual_removals {
        effective.retain(|value| value != tag);
    }
    for tag in &manual_additions {
        if !effective.iter().any(|value| value == tag) {
            effective.push(tag.clone());
        }
    }
    effective.sort_by_key(|value| value.to_lowercase());
    let source = match (
        auto.is_empty(),
        manual_additions.is_empty() && manual_removals.is_empty(),
    ) {
        (true, true) => ClassificationSource::None,
        (false, true) => ClassificationSource::Auto,
        (true, false) => ClassificationSource::Manual,
        (false, false) => ClassificationSource::Mixed,
    };
    AuxiliaryTagState {
        auto,
        manual_additions,
        manual_removals,
        effective,
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_closed_and_all_fields_restore_auto() {
        let registry = registry_descriptors();
        assert_eq!(registry.len(), 5);
        assert!(registry.iter().all(|field| {
            field.supports_manual_override && field.supports_restore_auto && field.filterable
        }));
        assert!(!is_registry_field("brightness"));
    }

    #[test]
    fn manual_single_value_wins_over_auto() {
        let result = resolve_classification(
            4,
            AutoClassification {
                primary_category: Some("landscape".to_owned()),
                tone: Some("mid_tone".to_owned()),
                ..AutoClassification::default()
            },
            ManualClassification {
                primary_category: Some("architecture".to_owned()),
                ..ManualClassification::default()
            },
        );
        assert_eq!(
            result.primary_category.effective.as_deref(),
            Some("architecture")
        );
        assert_eq!(result.primary_category.source, ClassificationSource::Manual);
        assert_eq!(result.tone.effective.as_deref(), Some("mid_tone"));
        assert_eq!(result.tone.source, ClassificationSource::Auto);
    }

    #[test]
    fn tag_overrides_add_and_remove_deterministically() {
        let result = resolve_classification(
            0,
            AutoClassification {
                auxiliary_tags: vec!["portrait".to_owned(), "outdoor".to_owned()],
                ..AutoClassification::default()
            },
            ManualClassification {
                auxiliary_tag_additions: vec!["favorite".to_owned(), "portrait".to_owned()],
                auxiliary_tag_removals: vec!["outdoor".to_owned()],
                ..ManualClassification::default()
            },
        );
        assert_eq!(
            result.auxiliary_tags.effective,
            vec!["favorite", "portrait"]
        );
        assert_eq!(result.auxiliary_tags.source, ClassificationSource::Mixed);
    }

    #[test]
    fn failed_auto_can_still_have_manual_effective_value() {
        let result = resolve_classification(
            1,
            AutoClassification::default(),
            ManualClassification {
                saturation_level: Some("high".to_owned()),
                ..ManualClassification::default()
            },
        );
        assert_eq!(result.saturation_level.effective.as_deref(), Some("high"));
        assert_eq!(result.saturation_level.source, ClassificationSource::Manual);
    }
}
