//! Photographer-facing topic candidates.
//!
//! This taxonomy is intentionally independent from Places365 scene leaves and
//! from subject detection. The bundled SigLIP 2 adapter is the single topic
//! candidate provider; prompts and selection rules are versioned separately
//! from the database contract.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TopicLabelDefinition {
    pub id: &'static str,
    pub display_name: &'static str,
    pub prompts: &'static [&'static str],
    pub threshold: f32,
}

pub const TAXONOMY_VERSION: &str = "photo-organizer-photography-topics-v3";
pub const SCORE_MARGIN: f32 = 0.035;
pub const MAX_RAW_CANDIDATES: usize = 16;

const PORTRAIT_PROMPTS: &[&str] = &["a portrait", "a person", "people"];
const LANDSCAPE_PROMPTS: &[&str] = &[
    "a landscape",
    "natural scenery",
    "mountains, coast, or open land",
];
const STREET_PROMPTS: &[&str] = &[
    "a street scene",
    "an urban scene",
    "a candid moment in a city",
];
const ARCHITECTURE_PROMPTS: &[&str] = &[
    "architecture",
    "a building",
    "an interior or designed space",
];
const STILL_LIFE_PROMPTS: &[&str] = &["a still life", "a product", "arranged objects"];
const FOOD_PROMPTS: &[&str] = &["food", "a meal or dish", "restaurant food"];
const WILDLIFE_PROMPTS: &[&str] = &["wildlife", "an animal", "a pet or bird"];
const MACRO_PROMPTS: &[&str] = &["a plant", "a flower or plant", "botanical photography"];
const ACTIVITY_PROMPTS: &[&str] = &["a sport", "sports photography", "an action scene"];
const VEHICLE_PROMPTS: &[&str] = &["a vehicle", "an automobile", "transportation"];
const DOCUMENT_PROMPTS: &[&str] = &["a document", "a screenshot", "a page with text"];
const ABSTRACT_PROMPTS: &[&str] = &[
    "an abstract image",
    "shapes, patterns, and textures",
    "experimental art",
];

/// Active labels are deliberately limited to visual photographic genres.
/// Travel, commercial projects, weddings, and similar intent-level labels
/// should be inferred at session level or confirmed by the user instead of
/// being forced from one image.
pub const TOPIC_LABELS: &[TopicLabelDefinition] = &[
    TopicLabelDefinition {
        id: "photo_portrait",
        display_name: "人像",
        prompts: PORTRAIT_PROMPTS,
        threshold: 0.22,
    },
    TopicLabelDefinition {
        id: "photo_landscape",
        display_name: "风光自然",
        prompts: LANDSCAPE_PROMPTS,
        threshold: 0.18,
    },
    TopicLabelDefinition {
        id: "photo_street",
        display_name: "街拍纪实",
        prompts: STREET_PROMPTS,
        threshold: 0.19,
    },
    TopicLabelDefinition {
        id: "photo_architecture",
        display_name: "建筑",
        prompts: ARCHITECTURE_PROMPTS,
        threshold: 0.19,
    },
    TopicLabelDefinition {
        id: "photo_still_life",
        display_name: "静物产品",
        prompts: STILL_LIFE_PROMPTS,
        threshold: 0.21,
    },
    TopicLabelDefinition {
        id: "photo_food",
        display_name: "美食",
        prompts: FOOD_PROMPTS,
        threshold: 0.21,
    },
    TopicLabelDefinition {
        id: "photo_wildlife",
        display_name: "动物",
        prompts: WILDLIFE_PROMPTS,
        threshold: 0.22,
    },
    TopicLabelDefinition {
        id: "photo_macro",
        display_name: "植物",
        prompts: MACRO_PROMPTS,
        threshold: 0.22,
    },
    TopicLabelDefinition {
        id: "photo_activity",
        display_name: "运动",
        prompts: ACTIVITY_PROMPTS,
        threshold: 0.21,
    },
    TopicLabelDefinition {
        id: "photo_vehicle",
        display_name: "交通工具",
        prompts: VEHICLE_PROMPTS,
        threshold: 0.22,
    },
    TopicLabelDefinition {
        id: "photo_document",
        display_name: "文档截图",
        prompts: DOCUMENT_PROMPTS,
        threshold: 0.25,
    },
    TopicLabelDefinition {
        id: "photo_abstract",
        display_name: "抽象艺术",
        prompts: ABSTRACT_PROMPTS,
        threshold: 0.21,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptSpec {
    pub label_index: usize,
    pub prompt: &'static str,
}

pub fn prompt_specs() -> Vec<PromptSpec> {
    TOPIC_LABELS
        .iter()
        .enumerate()
        .flat_map(|(label_index, label)| {
            label.prompts.iter().map(move |prompt| PromptSpec {
                label_index,
                prompt,
            })
        })
        .collect()
}

pub fn aggregate_prompt_scores(prompt_scores: &[f32], prompt_label_indexes: &[usize]) -> Vec<f32> {
    let mut sums = vec![0.0_f32; TOPIC_LABELS.len()];
    let mut counts = vec![0_u32; TOPIC_LABELS.len()];
    for (score, label_index) in prompt_scores.iter().zip(prompt_label_indexes) {
        if let Some(sum) = sums.get_mut(*label_index) {
            *sum += *score;
        }
        if let Some(count) = counts.get_mut(*label_index) {
            *count += 1;
        }
    }
    sums.into_iter()
        .zip(counts)
        .map(
            |(sum, count)| {
                if count == 0 { 0.0 } else { sum / count as f32 }
            },
        )
        .collect()
}

pub fn select_primary(scores: &[f32]) -> Option<(usize, f32)> {
    let mut ranked = scores
        .iter()
        .enumerate()
        .filter_map(|(index, score)| TOPIC_LABELS.get(index).map(|label| (index, *score, label)))
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
    let (top_index, top_score, top_label) = ranked.first().copied()?;
    let second_score = ranked.get(1).map(|(_, score, _)| *score).unwrap_or(0.0);
    (top_score >= top_label.threshold && top_score - second_score >= SCORE_MARGIN)
        .then_some((top_index, top_score))
}

pub fn label_index(label_id: &str) -> Option<usize> {
    TOPIC_LABELS.iter().position(|label| label.id == label_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_ids_are_unique_and_have_prompt_ensembles() {
        let mut ids = TOPIC_LABELS
            .iter()
            .map(|label| label.id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), TOPIC_LABELS.len());
        assert!(TOPIC_LABELS.iter().all(|label| label.prompts.len() >= 3));
        assert!(prompt_specs().len() >= TOPIC_LABELS.len() * 3);
        assert_eq!(
            TOPIC_LABELS
                .iter()
                .find(|label| label.id == "photo_architecture")
                .unwrap()
                .display_name,
            "建筑"
        );
        assert_eq!(
            TOPIC_LABELS
                .iter()
                .find(|label| label.id == "photo_vehicle")
                .unwrap()
                .display_name,
            "交通工具"
        );
        assert!(
            !TOPIC_LABELS
                .iter()
                .any(|label| label.id == "photo_documentary")
        );
    }

    #[test]
    fn prompt_scores_are_averaged_per_topic() {
        let specs = prompt_specs();
        let mut scores = vec![0.0; specs.len()];
        let portrait_count = specs.iter().filter(|spec| spec.label_index == 0).count();
        for (score, spec) in scores.iter_mut().zip(specs) {
            if spec.label_index == 0 {
                *score = 0.30;
            }
        }
        let label_indexes = prompt_specs()
            .iter()
            .map(|spec| spec.label_index)
            .collect::<Vec<_>>();
        let aggregated = aggregate_prompt_scores(&scores, &label_indexes);
        assert_eq!(aggregated[0], 0.30);
        assert_eq!(portrait_count, TOPIC_LABELS[0].prompts.len());
    }

    #[test]
    fn selection_requires_threshold_and_margin() {
        let portrait = label_index("photo_portrait").unwrap();
        let landscape = label_index("photo_landscape").unwrap();
        let mut scores = vec![0.0; TOPIC_LABELS.len()];
        scores[portrait] = 0.30;
        scores[landscape] = 0.20;
        assert_eq!(select_primary(&scores), Some((portrait, 0.30)));

        scores[landscape] = 0.28;
        assert_eq!(select_primary(&scores), None);

        scores[landscape] = 0.01;
        scores[portrait] = 0.10;
        assert_eq!(select_primary(&scores), None);
    }
}
