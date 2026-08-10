use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use chrono::Utc;
use clap::{Parser, ValueEnum};
use photo_organizer_lib::semantic::{
    ExecutionBackend, ModelMetadata, SemanticClassifier, SemanticSimilarity, TinyClipClassifier,
    semantic_catalog,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    name = "semantic-evaluate",
    about = "Evaluate bundled TinyCLIP against an explicit, locally licensed photography set",
    long_about = "Reads only the supplied evaluation directory. Each top-level folder is a stable label ID or a '+'-joined multi-label set such as portrait+night. Reports are created outside the dataset and never overwrite an existing file."
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

    #[arg(long, value_name = "DIR")]
    model_dir: Option<PathBuf>,

    #[arg(long, value_name = "onnxruntime.dll")]
    runtime: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = BackendArgument::Cpu)]
    backend: BackendArgument,
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

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_dir = arguments.model_dir.unwrap_or_else(|| {
        manifest_dir
            .join("resources")
            .join("models")
            .join("tinyclip-vit-8m-16-text-3m-yfcc15m")
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
    let classifier = TinyClipClassifier::load(&model_dir, &runtime)?;
    let model_load_ms = load_start.elapsed().as_secs_f64() * 1000.0;
    let metadata = classifier.metadata();
    let backend: ExecutionBackend = arguments.backend.into();
    let actual_backend = classifier.status().selected_backend.unwrap_or(backend);

    let inference_start = Instant::now();
    let mut samples = Vec::with_capacity(inputs.len());
    for input in &inputs {
        let sample_start = Instant::now();
        let result = classifier.classify_batch(std::slice::from_ref(&input.path), backend);
        let latency_ms = sample_start.elapsed().as_secs_f64() * 1000.0;
        let relative_path = input
            .path
            .strip_prefix(&arguments.data)
            .unwrap_or(&input.path)
            .to_string_lossy()
            .replace('\\', "/");
        match result {
            Ok(mut outputs) if outputs.len() == 1 => {
                let output = outputs.remove(0);
                let top3 = output
                    .raw_similarities
                    .iter()
                    .take(3)
                    .map(|score| score.label_id.clone())
                    .collect::<Vec<_>>();
                let top1 = top3.first().cloned();
                samples.push(EvaluatedSample {
                    path: relative_path,
                    expected_labels: input.expected_labels.clone(),
                    predicted_labels: output
                        .predictions
                        .iter()
                        .map(|prediction| prediction.label_id.clone())
                        .collect(),
                    top1,
                    top3,
                    raw_similarities: output.raw_similarities,
                    latency_ms,
                    error: None,
                });
            }
            Ok(_) => samples.push(failed_sample(
                input,
                relative_path,
                latency_ms,
                "classifier returned an unexpected result count".into(),
            )),
            Err(error) => samples.push(failed_sample(
                input,
                relative_path,
                latency_ms,
                error.to_string(),
            )),
        }
    }
    let inference_ms = inference_start.elapsed().as_secs_f64() * 1000.0;
    let end_to_end_ms = overall_start.elapsed().as_secs_f64() * 1000.0;
    let process_stats = process_stats_delta(process_start, collect_process_stats());
    let report = build_report(
        &arguments.data,
        metadata,
        actual_backend,
        samples,
        model_load_ms,
        inference_ms,
        end_to_end_ms,
        process_stats,
    );
    write_report(&arguments.output, &report)?;
    println!("wrote {}", arguments.output.display());
    Ok(())
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
        schema_version: 1,
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
        samples,
    }
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
        let portrait = temporary.path().join("portrait");
        let night_group = temporary.path().join("group+night");
        std::fs::create_dir_all(&portrait).expect("portrait dir");
        std::fs::create_dir_all(&night_group).expect("night group dir");
        std::fs::write(portrait.join("one.jpg"), b"fixture").expect("portrait fixture");
        std::fs::write(night_group.join("two.webp"), b"fixture").expect("group fixture");

        let discovered = discover_evaluation_inputs(temporary.path()).expect("discover inputs");
        assert_eq!(discovered.len(), 2);
        assert!(
            discovered
                .iter()
                .any(|input| input.expected_labels == ["group", "night"])
        );
        assert!(
            discovered
                .iter()
                .any(|input| input.expected_labels == ["portrait"])
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
