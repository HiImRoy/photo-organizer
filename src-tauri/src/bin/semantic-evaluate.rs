use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use chrono::Utc;
use clap::{Parser, ValueEnum};
use photo_organizer_lib::semantic::{
    ExecutionBackend, ModelMetadata, OpenVocabularyClipClassifier, SemanticClassifier,
    SemanticSimilarity, TopicModelKind, semantic_catalog,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    name = "semantic-evaluate",
    about = "Evaluate a bundled topic model against an explicit, locally licensed photography set",
    long_about = "Reads only the supplied evaluation directory. Each top-level folder is a stable topic label ID or a '+'-joined set with optional context/subject labels. Reports are created outside the dataset and never overwrite an existing file."
)]
struct Arguments {
    #[arg(long, default_value = "evaluation-data", value_name = "DIR")]
    data: PathBuf,

    #[arg(
        long,
        default_value = "benchmark-output/photo-evaluation.json",
        value_name = "REPORT.json"
    )]
    output: PathBuf,

    #[arg(long, default_value = "siglip2-base", value_name = "MODEL")]
    model: String,

    #[arg(long, value_name = "DIR")]
    model_dir: Option<PathBuf>,

    #[arg(long, value_name = "onnxruntime.dll")]
    runtime: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = BackendArgument::Cpu)]
    backend: BackendArgument,

    #[arg(long, default_value_t = 4)]
    batch_size: usize,

    #[arg(long, default_value_t = false)]
    calibrate: bool,

    #[arg(long, default_value_t = 0.85)]
    target_precision: f64,

    #[arg(long, default_value_t = 8)]
    minimum_samples_per_class: usize,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BackendArgument {
    Auto,
    Cpu,
}

impl From<BackendArgument> for ExecutionBackend {
    fn from(value: BackendArgument) -> Self {
        match value {
            BackendArgument::Auto => Self::Auto,
            BackendArgument::Cpu => Self::Cpu,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EvaluationInput {
    path: PathBuf,
    expected_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvaluatedSample {
    path: String,
    expected_labels: Vec<String>,
    predicted_labels: Vec<String>,
    top1: Option<String>,
    top3: Vec<String>,
    raw_similarities: Vec<SemanticSimilarity>,
    latency_ms: f64,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClassMetrics {
    label_id: String,
    display_name: String,
    image_count: usize,
    predicted_count: usize,
    true_positive_count: usize,
    precision: Option<f64>,
    recall: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfusionCount {
    expected_label: String,
    predicted_top1: String,
    count: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ProcessStats {
    peak_memory_bytes: Option<u64>,
    cpu_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalibrationLabel {
    label_id: String,
    display_name: String,
    sample_count: usize,
    positive_count: usize,
    current_threshold: f32,
    recommended_threshold: Option<f32>,
    accepted_count: usize,
    true_positive_count: usize,
    false_positive_count: usize,
    precision: Option<f64>,
    recall: Option<f64>,
    eligible: bool,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalibrationMarginSummary {
    margin: f32,
    accepted_count: usize,
    true_positive_count: usize,
    precision: Option<f64>,
    macro_recall: Option<f64>,
    coverage: Option<f64>,
    meets_target_precision: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalibrationReport {
    status: String,
    score_semantics: String,
    target_precision: f64,
    minimum_samples_per_class: usize,
    evaluated_sample_count: usize,
    eligible_label_count: usize,
    recommended_margin: Option<f32>,
    labels: Vec<CalibrationLabel>,
    margin_sweep: Vec<CalibrationMarginSummary>,
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvaluationReport {
    schema_version: u32,
    generated_at: String,
    status: String,
    dataset_root: String,
    label_convention: String,
    model: ModelMetadata,
    backend: ExecutionBackend,
    sample_count: usize,
    successful_count: usize,
    failure_count: usize,
    class_counts: BTreeMap<String, usize>,
    top1_accuracy: Option<f64>,
    top3_accuracy: Option<f64>,
    multilabel_micro_precision: Option<f64>,
    multilabel_micro_recall: Option<f64>,
    multilabel_macro_precision: Option<f64>,
    multilabel_macro_recall: Option<f64>,
    unknown_ratio: Option<f64>,
    per_class: Vec<ClassMetrics>,
    confusion: Vec<ConfusionCount>,
    model_load_ms: f64,
    inference_ms: f64,
    end_to_end_ms: f64,
    peak_memory_bytes: Option<u64>,
    process_cpu_seconds: Option<f64>,
    average_cpu_core_percent: Option<f64>,
    calibration: Option<CalibrationReport>,
    samples: Vec<EvaluatedSample>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("semantic-evaluate: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    if !arguments.data.is_dir() {
        return Err(format!(
            "--data must name a readable evaluation directory: {}",
            arguments.data.display()
        )
        .into());
    }
    let inputs = discover_evaluation_inputs(&arguments.data)?;
    if inputs.is_empty() {
        return Err("the evaluation directory contains no supported images".into());
    }
    if !(1..=1024).contains(&arguments.batch_size) {
        return Err("--batch-size must be between 1 and 1024".into());
    }
    if !(0.0..=1.0).contains(&arguments.target_precision) {
        return Err("--target-precision must be between 0 and 1".into());
    }
    if arguments.minimum_samples_per_class == 0 {
        return Err("--minimum-samples-per-class must be greater than 0".into());
    }

    let topic_model = TopicModelKind::parse(&arguments.model).ok_or_else(|| {
        format!(
            "unsupported --model '{}'; use siglip2-base",
            arguments.model
        )
    })?;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_dir = arguments.model_dir.unwrap_or_else(|| {
        manifest_dir
            .join("resources")
            .join("models")
            .join(match topic_model {
                TopicModelKind::Siglip2Base => "siglip2-base-patch16-224",
                _ => unreachable!("only the bundled SigLIP 2 topic model is accepted"),
            })
    });
    let runtime = arguments.runtime.unwrap_or_else(|| {
        manifest_dir
            .join("resources")
            .join("runtime")
            .join("onnxruntime.dll")
    });
    let process_start = collect_process_stats();
    let overall_start = Instant::now();
    let load_start = Instant::now();
    let classifier: Box<dyn SemanticClassifier> = Box::new(OpenVocabularyClipClassifier::load(
        topic_model,
        &model_dir,
        &runtime,
    )?);
    let model_load_ms = load_start.elapsed().as_secs_f64() * 1000.0;
    let metadata = classifier.metadata();
    let backend: ExecutionBackend = arguments.backend.into();
    let actual_backend = classifier.status().selected_backend.unwrap_or(backend);

    let inference_start = Instant::now();
    let mut samples = Vec::with_capacity(inputs.len());
    for batch in inputs.chunks(arguments.batch_size) {
        let paths = batch
            .iter()
            .map(|input| input.path.clone())
            .collect::<Vec<_>>();
        let batch_start = Instant::now();
        let result = classifier.classify_batch(&paths, backend);
        let batch_latency_ms = batch_start.elapsed().as_secs_f64() * 1000.0;
        let per_sample_latency_ms = batch_latency_ms / batch.len() as f64;
        match result {
            Ok(outputs) if outputs.len() == batch.len() => {
                for (input, output) in batch.iter().zip(outputs) {
                    let relative_path = input
                        .path
                        .strip_prefix(&arguments.data)
                        .unwrap_or(&input.path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    samples.push(evaluated_sample(
                        input,
                        relative_path,
                        per_sample_latency_ms,
                        output,
                    ));
                }
            }
            Ok(outputs) => {
                let error = format!(
                    "classifier returned {} outputs for {} inputs",
                    outputs.len(),
                    batch.len()
                );
                for input in batch {
                    let relative_path = input
                        .path
                        .strip_prefix(&arguments.data)
                        .unwrap_or(&input.path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    samples.push(failed_sample(
                        input,
                        relative_path,
                        per_sample_latency_ms,
                        error.clone(),
                    ));
                }
            }
            Err(error) => {
                for input in batch {
                    let relative_path = input
                        .path
                        .strip_prefix(&arguments.data)
                        .unwrap_or(&input.path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    samples.push(failed_sample(
                        input,
                        relative_path,
                        per_sample_latency_ms,
                        error.to_string(),
                    ));
                }
            }
        }
    }
    let inference_ms = inference_start.elapsed().as_secs_f64() * 1000.0;
    let end_to_end_ms = overall_start.elapsed().as_secs_f64() * 1000.0;
    let process_stats = process_stats_delta(process_start, collect_process_stats());
    let mut report = build_report(
        &arguments.data,
        metadata,
        actual_backend,
        samples,
        model_load_ms,
        inference_ms,
        end_to_end_ms,
        process_stats,
    );
    if arguments.calibrate {
        report.calibration = Some(build_calibration_report(
            &report.samples,
            arguments.target_precision,
            arguments.minimum_samples_per_class,
            score_semantics(topic_model),
        ));
    }
    write_report(&arguments.output, &report)?;
    println!("wrote {}", arguments.output.display());
    Ok(())
}

fn evaluated_sample(
    input: &EvaluationInput,
    relative_path: String,
    latency_ms: f64,
    output: photo_organizer_lib::semantic::SemanticAnalysisOutput,
) -> EvaluatedSample {
    let top3 = output
        .raw_similarities
        .iter()
        .take(3)
        .map(|score| score.label_id.clone())
        .collect::<Vec<_>>();
    let top1 = top3.first().cloned();
    let mut predicted_labels = output
        .predictions
        .iter()
        .map(|prediction| prediction.label_id.clone())
        .collect::<Vec<_>>();
    if predicted_labels.is_empty() {
        predicted_labels.push("unknown".into());
    }
    EvaluatedSample {
        path: relative_path,
        expected_labels: input.expected_labels.clone(),
        predicted_labels,
        top1,
        top3,
        raw_similarities: output.raw_similarities,
        latency_ms,
        error: None,
    }
}

fn failed_sample(
    input: &EvaluationInput,
    relative_path: String,
    latency_ms: f64,
    error: String,
) -> EvaluatedSample {
    EvaluatedSample {
        path: relative_path,
        expected_labels: input.expected_labels.clone(),
        predicted_labels: Vec::new(),
        top1: None,
        top3: Vec::new(),
        raw_similarities: Vec::new(),
        latency_ms,
        error: Some(error),
    }
}

fn discover_evaluation_inputs(
    root: &Path,
) -> Result<Vec<EvaluationInput>, Box<dyn std::error::Error>> {
    let known_labels = semantic_catalog()
        .into_iter()
        .map(|label| label.id)
        .chain(std::iter::once("unknown".into()))
        .collect::<HashSet<_>>();
    let mut inputs = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let folder_name = entry.file_name().to_string_lossy().into_owned();
        if folder_name.starts_with('.') {
            continue;
        }
        let mut expected_labels = folder_name
            .split('+')
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        expected_labels.sort();
        expected_labels.dedup();
        if expected_labels.is_empty()
            || expected_labels
                .iter()
                .any(|label| !known_labels.contains(label))
        {
            return Err(format!(
                "invalid evaluation label folder '{folder_name}'; use stable label IDs joined with '+'"
            )
            .into());
        }
        for candidate in walkdir::WalkDir::new(entry.path()).follow_links(false) {
            let candidate = candidate?;
            if candidate.file_type().is_file() && is_supported_image(candidate.path()) {
                inputs.push(EvaluationInput {
                    path: candidate.into_path(),
                    expected_labels: expected_labels.clone(),
                });
            }
        }
    }
    inputs.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(inputs)
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp"
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn build_report(
    dataset_root: &Path,
    model: ModelMetadata,
    backend: ExecutionBackend,
    samples: Vec<EvaluatedSample>,
    model_load_ms: f64,
    inference_ms: f64,
    end_to_end_ms: f64,
    process_stats: ProcessStats,
) -> EvaluationReport {
    let sample_count = samples.len();
    let successful_count = samples
        .iter()
        .filter(|sample| sample.error.is_none())
        .count();
    let failure_count = sample_count - successful_count;
    let mut class_counts = BTreeMap::<String, usize>::new();
    let mut predicted_counts = BTreeMap::<String, usize>::new();
    let mut true_positives = BTreeMap::<String, usize>::new();
    let mut confusion = BTreeMap::<(String, String), usize>::new();
    let mut top1_hits = 0_usize;
    let mut top3_hits = 0_usize;
    let mut unknown_count = 0_usize;
    let mut micro_true_positive = 0_usize;
    let mut micro_false_positive = 0_usize;
    let mut micro_false_negative = 0_usize;

    for sample in &samples {
        let expected_all = sample
            .expected_labels
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let predicted_all = sample
            .predicted_labels
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected = expected_all
            .iter()
            .filter(|label| label.as_str() != "unknown")
            .cloned()
            .collect::<BTreeSet<_>>();
        let predicted = predicted_all
            .iter()
            .filter(|label| label.as_str() != "unknown")
            .cloned()
            .collect::<BTreeSet<_>>();
        for label in &sample.expected_labels {
            *class_counts.entry(label.clone()).or_default() += 1;
        }
        for label in &sample.predicted_labels {
            *predicted_counts.entry(label.clone()).or_default() += 1;
        }
        for label in expected_all.intersection(&predicted_all) {
            *true_positives.entry(label.clone()).or_default() += 1;
        }
        micro_true_positive += expected.intersection(&predicted).count();
        micro_false_positive += predicted.difference(&expected).count();
        micro_false_negative += expected.difference(&predicted).count();
        if sample
            .top1
            .as_ref()
            .is_some_and(|label| sample.expected_labels.contains(label))
        {
            top1_hits += 1;
        }
        if sample
            .top3
            .iter()
            .any(|label| sample.expected_labels.contains(label))
        {
            top3_hits += 1;
        }
        if sample
            .predicted_labels
            .iter()
            .any(|label| label == "unknown")
        {
            unknown_count += 1;
        }
        let predicted_top1 = sample
            .top1
            .clone()
            .unwrap_or_else(|| "inference_error".into());
        for expected_label in &sample.expected_labels {
            *confusion
                .entry((expected_label.clone(), predicted_top1.clone()))
                .or_default() += 1;
        }
    }

    let catalog = semantic_catalog();
    let per_class = catalog
        .iter()
        .map(|label| {
            let image_count = class_counts.get(&label.id).copied().unwrap_or(0);
            let predicted_count = predicted_counts.get(&label.id).copied().unwrap_or(0);
            let true_positive_count = true_positives.get(&label.id).copied().unwrap_or(0);
            ClassMetrics {
                label_id: label.id.clone(),
                display_name: label.display_name.clone(),
                image_count,
                predicted_count,
                true_positive_count,
                precision: ratio(true_positive_count, predicted_count),
                recall: ratio(true_positive_count, image_count),
            }
        })
        .collect::<Vec<_>>();
    let evaluated_classes = per_class
        .iter()
        .filter(|metrics| metrics.label_id != "unknown" && metrics.image_count > 0)
        .collect::<Vec<_>>();
    let macro_precision = average(
        evaluated_classes
            .iter()
            .map(|metrics| metrics.precision.unwrap_or(0.0)),
    );
    let macro_recall = average(
        evaluated_classes
            .iter()
            .map(|metrics| metrics.recall.unwrap_or(0.0)),
    );
    let confusion = confusion
        .into_iter()
        .map(|((expected_label, predicted_top1), count)| ConfusionCount {
            expected_label,
            predicted_top1,
            count,
        })
        .collect();
    let process_cpu_seconds = process_stats.cpu_seconds;
    let average_cpu_core_percent = process_cpu_seconds.and_then(|seconds| {
        (end_to_end_ms > 0.0).then_some(seconds / (end_to_end_ms / 1000.0) * 100.0)
    });

    EvaluationReport {
        schema_version: 2,
        generated_at: Utc::now().to_rfc3339(),
        status: if failure_count == 0 {
            "completed"
        } else {
            "completed_with_errors"
        }
        .into(),
        dataset_root: dataset_root.to_string_lossy().replace('\\', "/"),
        label_convention: "top-level folder uses stable label IDs joined with '+'".into(),
        model,
        backend,
        sample_count,
        successful_count,
        failure_count,
        class_counts,
        top1_accuracy: ratio(top1_hits, sample_count),
        top3_accuracy: ratio(top3_hits, sample_count),
        multilabel_micro_precision: ratio(
            micro_true_positive,
            micro_true_positive + micro_false_positive,
        ),
        multilabel_micro_recall: ratio(
            micro_true_positive,
            micro_true_positive + micro_false_negative,
        ),
        multilabel_macro_precision: macro_precision,
        multilabel_macro_recall: macro_recall,
        unknown_ratio: ratio(unknown_count, sample_count),
        per_class,
        confusion,
        model_load_ms,
        inference_ms,
        end_to_end_ms,
        peak_memory_bytes: process_stats.peak_memory_bytes,
        process_cpu_seconds,
        average_cpu_core_percent,
        calibration: None,
        samples,
    }
}

fn build_calibration_report(
    samples: &[EvaluatedSample],
    target_precision: f64,
    minimum_samples_per_class: usize,
    score_semantics: &str,
) -> CalibrationReport {
    const MARGIN_SWEEP: &[f32] = &[0.0, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.08, 0.10];
    let successful_samples = samples
        .iter()
        .filter(|sample| sample.error.is_none() && !sample.raw_similarities.is_empty())
        .collect::<Vec<_>>();
    let catalog = semantic_catalog();
    let mut labels = Vec::new();
    let mut eligible_label_count = 0;
    for label in catalog
        .iter()
        .filter(|label| label.category_group == "scene")
    {
        let positives = successful_samples
            .iter()
            .filter(|sample| sample.expected_labels.contains(&label.id))
            .count();
        let sample_count = successful_samples.len();
        let eligible = positives >= minimum_samples_per_class
            && sample_count.saturating_sub(positives) >= minimum_samples_per_class;
        if eligible {
            eligible_label_count += 1;
        }
        let candidates = successful_samples
            .iter()
            .filter_map(|sample| candidate_score(sample, &label.id))
            .collect::<Vec<_>>();
        let recommended = if eligible {
            recommend_label_threshold(
                &successful_samples,
                &label.id,
                target_precision,
                label.threshold,
            )
        } else {
            None
        };
        let threshold = recommended.unwrap_or(label.threshold);
        let accepted_count = candidates
            .iter()
            .filter(|score| **score >= threshold)
            .count();
        let true_positive = successful_samples
            .iter()
            .filter(|sample| {
                sample.expected_labels.contains(&label.id)
                    && candidate_score(sample, &label.id).is_some_and(|score| score >= threshold)
            })
            .count();
        labels.push(CalibrationLabel {
            label_id: label.id.clone(),
            display_name: label.display_name.clone(),
            sample_count,
            positive_count: positives,
            current_threshold: label.threshold,
            recommended_threshold: recommended,
            accepted_count,
            true_positive_count: true_positive,
            false_positive_count: accepted_count.saturating_sub(true_positive),
            precision: ratio(true_positive, accepted_count),
            recall: ratio(true_positive, positives),
            eligible,
            status: if eligible {
                "calibrated_candidate"
            } else {
                "insufficient_labeled_samples"
            }
            .into(),
        });
    }

    let margin_sweep = MARGIN_SWEEP
        .iter()
        .map(|margin| {
            let accepted = successful_samples
                .iter()
                .filter(|sample| accepted_by_margin(sample, *margin, &labels))
                .collect::<Vec<_>>();
            let true_positive = accepted
                .iter()
                .filter(|sample| {
                    sample
                        .top1
                        .as_ref()
                        .is_some_and(|label| sample.expected_labels.contains(label))
                })
                .count();
            let macro_recall = if eligible_label_count == 0 {
                None
            } else {
                average(
                    labels
                        .iter()
                        .filter(|label| label.eligible && label.recommended_threshold.is_some())
                        .map(|label| {
                            accepted
                                .iter()
                                .filter(|sample| {
                                    sample.top1.as_deref() == Some(label.label_id.as_str())
                                        && sample.expected_labels.contains(&label.label_id)
                                        && candidate_score(sample, &label.label_id).is_some_and(
                                            |score| score >= label.recommended_threshold.unwrap(),
                                        )
                                })
                                .count() as f64
                                / label.positive_count.max(1) as f64
                        }),
                )
            };
            CalibrationMarginSummary {
                margin: *margin,
                accepted_count: accepted.len(),
                true_positive_count: true_positive,
                precision: ratio(true_positive, accepted.len()),
                macro_recall,
                coverage: ratio(accepted.len(), successful_samples.len()),
                meets_target_precision: ratio(true_positive, accepted.len())
                    .is_some_and(|precision| precision >= target_precision),
            }
        })
        .collect::<Vec<_>>();
    let recommended_margin = (eligible_label_count > 0)
        .then(|| {
            margin_sweep
                .iter()
                .filter(|summary| summary.meets_target_precision)
                .max_by(|left, right| {
                    left.macro_recall
                        .unwrap_or(0.0)
                        .total_cmp(&right.macro_recall.unwrap_or(0.0))
                        .then(right.margin.total_cmp(&left.margin))
                })
                .map(|summary| summary.margin)
        })
        .flatten();

    let mut notes = vec![
        format!("本次校准使用 {score_semantics}；不同模型不得共用阈值。"),
        "每类阈值只在正负样本数达到最低要求时给出建议，不会用小样本覆盖运行时默认值。".into(),
        "margin 只用于互斥主题拒识；它不把多标签主体或环境属性变成互斥类别。".into(),
    ];
    if successful_samples.is_empty() {
        notes.push("没有可用于校准的成功推理样本。".into());
    }
    CalibrationReport {
        status: if eligible_label_count > 0 {
            "candidate_thresholds_ready"
        } else {
            "insufficient_labeled_samples"
        }
        .into(),
        score_semantics: score_semantics.into(),
        target_precision,
        minimum_samples_per_class,
        evaluated_sample_count: successful_samples.len(),
        eligible_label_count,
        recommended_margin,
        labels,
        margin_sweep,
        notes,
    }
}

fn score_semantics(topic_model: TopicModelKind) -> &'static str {
    match topic_model {
        TopicModelKind::Siglip2Base => {
            "SigLIP 2 sigmoid(logits_per_image), per-label independent score"
        }
        _ => unreachable!("only the bundled SigLIP 2 topic model is accepted"),
    }
}

fn candidate_score(sample: &EvaluatedSample, label_id: &str) -> Option<f32> {
    sample
        .raw_similarities
        .iter()
        .find(|candidate| candidate.label_id == label_id)
        .map(|candidate| candidate.similarity)
}

fn recommend_label_threshold(
    samples: &[&EvaluatedSample],
    label_id: &str,
    target_precision: f64,
    current_threshold: f32,
) -> Option<f32> {
    let mut candidates = samples
        .iter()
        .filter_map(|sample| candidate_score(sample, label_id))
        .collect::<Vec<_>>();
    candidates.push(current_threshold);
    candidates.sort_by(|left, right| left.total_cmp(right));
    candidates.dedup_by(|left, right| (*left - *right).abs() < f32::EPSILON);
    candidates
        .into_iter()
        .filter_map(|threshold| {
            let accepted = samples
                .iter()
                .filter(|sample| {
                    candidate_score(sample, label_id).is_some_and(|score| score >= threshold)
                })
                .collect::<Vec<_>>();
            let true_positive = accepted
                .iter()
                .filter(|sample| {
                    sample
                        .expected_labels
                        .iter()
                        .any(|expected| expected == label_id)
                })
                .count();
            let precision = ratio(true_positive, accepted.len())?;
            (precision >= target_precision).then_some((threshold, accepted.len(), true_positive))
        })
        .max_by(|left, right| {
            let left_recall = left.2 as f64
                / samples
                    .iter()
                    .filter(|sample| {
                        sample
                            .expected_labels
                            .iter()
                            .any(|expected| expected == label_id)
                    })
                    .count()
                    .max(1) as f64;
            let right_recall = right.2 as f64
                / samples
                    .iter()
                    .filter(|sample| {
                        sample
                            .expected_labels
                            .iter()
                            .any(|expected| expected == label_id)
                    })
                    .count()
                    .max(1) as f64;
            left_recall
                .total_cmp(&right_recall)
                .then(right.0.total_cmp(&left.0))
        })
        .map(|(threshold, _, _)| threshold)
}

fn accepted_by_margin(sample: &EvaluatedSample, margin: f32, labels: &[CalibrationLabel]) -> bool {
    let Some(top1) = sample.raw_similarities.first() else {
        return false;
    };
    let threshold = labels
        .iter()
        .find(|label| label.label_id == top1.label_id)
        .map(|label| {
            label
                .recommended_threshold
                .unwrap_or(label.current_threshold)
        })
        .unwrap_or(top1.threshold);
    let second = sample
        .raw_similarities
        .get(1)
        .map(|candidate| candidate.similarity)
        .unwrap_or(0.0);
    top1.similarity >= threshold && top1.similarity - second >= margin
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then_some(numerator as f64 / denominator as f64)
}

fn average(values: impl Iterator<Item = f64>) -> Option<f64> {
    let values = values.collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn process_stats_delta(start: ProcessStats, end: ProcessStats) -> ProcessStats {
    ProcessStats {
        peak_memory_bytes: end.peak_memory_bytes,
        cpu_seconds: match (start.cpu_seconds, end.cpu_seconds) {
            (Some(start), Some(end)) => Some((end - start).max(0.0)),
            _ => None,
        },
    }
}

fn write_report(path: &Path, report: &EvaluationReport) -> Result<(), Box<dyn std::error::Error>> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
        return Err("--output must use a .json extension".into());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        create_dir_all(parent)?;
    }
    let content = serde_json::to_vec_pretty(report)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(&content)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn collect_process_stats() -> ProcessStats {
    let command = format!(
        "$p=Get-Process -Id {}; [pscustomobject]@{{PeakMemoryBytes=[uint64]$p.PeakWorkingSet64;CpuSeconds=[double]$p.CPU}} | ConvertTo-Json -Compress",
        std::process::id()
    );
    Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &command,
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice(&output.stdout).ok())
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn collect_process_stats() -> ProcessStats {
    let _ = Command::new("true");
    ProcessStats::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use photo_organizer_lib::semantic::MODEL_NAME;
    use tempfile::tempdir;

    #[test]
    fn discovers_single_and_multi_label_directories() {
        let temporary = tempdir().expect("temp dir");
        let portrait = temporary.path().join("photo_portrait");
        let landscape_outdoor = temporary.path().join("photo_landscape+outdoor");
        std::fs::create_dir_all(&portrait).expect("portrait dir");
        std::fs::create_dir_all(&landscape_outdoor).expect("landscape outdoor dir");
        std::fs::write(portrait.join("one.jpg"), b"fixture").expect("portrait fixture");
        std::fs::write(landscape_outdoor.join("two.webp"), b"fixture").expect("landscape fixture");

        let discovered = discover_evaluation_inputs(temporary.path()).expect("discover inputs");
        assert_eq!(discovered.len(), 2);
        assert!(
            discovered
                .iter()
                .any(|input| input.expected_labels == ["outdoor", "photo_landscape"])
        );
        assert!(
            discovered
                .iter()
                .any(|input| input.expected_labels == ["photo_portrait"])
        );
    }

    #[test]
    fn computes_accuracy_multilabel_unknown_and_confusion() {
        let samples = vec![
            evaluated_sample("portrait/one.jpg", &["portrait"], &["portrait"], "portrait"),
            evaluated_sample("group/two.jpg", &["group"], &["unknown"], "unknown"),
        ];
        let report = build_report(
            Path::new("evaluation-data"),
            ModelMetadata {
                name: MODEL_NAME.into(),
                version: "test".into(),
                analysis_version: "test".into(),
                license: Some("MIT".into()),
                installed: true,
                model_size_bytes: Some(1),
                model_sha256: None,
                supported_backends: vec![ExecutionBackend::Cpu],
            },
            ExecutionBackend::Cpu,
            samples,
            1.0,
            2.0,
            3.0,
            ProcessStats::default(),
        );
        assert_eq!(report.top1_accuracy, Some(0.5));
        assert_eq!(report.top3_accuracy, Some(0.5));
        assert_eq!(report.multilabel_micro_precision, Some(1.0));
        assert_eq!(report.multilabel_micro_recall, Some(0.5));
        assert_eq!(report.unknown_ratio, Some(0.5));
        assert!(report.confusion.iter().any(|item| {
            item.expected_label == "group" && item.predicted_top1 == "unknown" && item.count == 1
        }));
    }

    #[test]
    fn calibration_applies_per_label_threshold_before_margin() {
        let samples = vec![
            calibration_sample(
                "portrait/positive.jpg",
                "photo_portrait",
                0.40,
                "photo_landscape",
                0.20,
            ),
            calibration_sample(
                "landscape/negative-for-portrait.jpg",
                "photo_landscape",
                0.10,
                "photo_landscape",
                0.35,
            ),
            calibration_sample(
                "landscape/low-score-negative.jpg",
                "photo_landscape",
                0.10,
                "photo_landscape",
                0.00,
            ),
            calibration_sample(
                "landscape/false-positive-for-portrait.jpg",
                "photo_landscape",
                0.25,
                "photo_landscape",
                0.35,
            ),
        ];
        let report = build_calibration_report(&samples, 0.85, 1, "test score");
        let portrait = report
            .labels
            .iter()
            .find(|label| label.label_id == "photo_portrait")
            .expect("portrait calibration");
        assert_eq!(portrait.recommended_threshold, Some(0.40));
        assert_eq!(portrait.accepted_count, 1);
        assert_eq!(portrait.false_positive_count, 0);
        assert!(report.recommended_margin.is_some());
        assert_eq!(report.margin_sweep[0].accepted_count, 3);
    }

    fn calibration_sample(
        path: &str,
        expected_label: &str,
        portrait_score: f32,
        second_label: &str,
        second_score: f32,
    ) -> EvaluatedSample {
        let mut raw_similarities = vec![
            SemanticSimilarity {
                label_id: "photo_portrait".into(),
                display_name: "人像".into(),
                category_group: "topic_candidate".into(),
                similarity: portrait_score,
                threshold: 0.22,
            },
            SemanticSimilarity {
                label_id: second_label.into(),
                display_name: "第二候选".into(),
                category_group: "topic_candidate".into(),
                similarity: second_score,
                threshold: 0.18,
            },
        ];
        raw_similarities.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        let top1 = raw_similarities[0].label_id.clone();
        EvaluatedSample {
            path: path.into(),
            expected_labels: vec![expected_label.into()],
            predicted_labels: vec![top1.clone()],
            top1: Some(top1),
            top3: raw_similarities
                .iter()
                .map(|candidate| candidate.label_id.clone())
                .collect(),
            raw_similarities,
            latency_ms: 1.0,
            error: None,
        }
    }

    fn evaluated_sample(
        path: &str,
        expected: &[&str],
        predicted: &[&str],
        top1: &str,
    ) -> EvaluatedSample {
        EvaluatedSample {
            path: path.into(),
            expected_labels: expected.iter().map(|label| (*label).into()).collect(),
            predicted_labels: predicted.iter().map(|label| (*label).into()).collect(),
            top1: Some(top1.into()),
            top3: vec![top1.into()],
            raw_similarities: vec![SemanticSimilarity {
                label_id: top1.into(),
                display_name: top1.into(),
                category_group: "scene".into(),
                similarity: 0.3,
                threshold: 0.16,
            }],
            latency_ms: 1.0,
            error: None,
        }
    }
}
