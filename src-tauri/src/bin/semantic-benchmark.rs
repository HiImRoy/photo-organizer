use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use photo_organizer_lib::semantic::{
    BenchmarkReport, ExecutionBackend, SemanticClassifier, TinyClipClassifier,
    UnavailableClassifier, benchmark_classifier, discover_benchmark_images,
};

#[derive(Debug, Parser)]
#[command(
    name = "semantic-benchmark",
    about = "Benchmark a PhotoOrganizer semantic classifier adapter",
    long_about = "Runs the bundled TinyCLIP ONNX adapter against an explicit fixture directory and records real CPU predictions and timing. The unavailable adapter remains available to verify the no-fake-label fallback."
)]
struct Arguments {
    #[arg(long, value_name = "DIR")]
    images: PathBuf,

    #[arg(long, default_value = "tinyclip")]
    model: String,

    #[arg(long, value_name = "DIR")]
    model_dir: Option<PathBuf>,

    #[arg(long, value_name = "onnxruntime.dll")]
    runtime: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = BackendArgument::Auto)]
    backend: BackendArgument,

    #[arg(long, default_value_t = 1)]
    batch_size: usize,

    #[arg(long, value_name = "REPORT.json|REPORT.csv")]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum BackendArgument {
    Auto,
    Cpu,
    Directml,
    Cuda,
    Coreml,
    Npu,
}

impl From<BackendArgument> for ExecutionBackend {
    fn from(value: BackendArgument) -> Self {
        match value {
            BackendArgument::Auto => Self::Auto,
            BackendArgument::Cpu => Self::Cpu,
            BackendArgument::Directml => Self::DirectMl,
            BackendArgument::Cuda => Self::Cuda,
            BackendArgument::Coreml => Self::CoreMl,
            BackendArgument::Npu => Self::Npu,
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("semantic-benchmark: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    if !arguments.images.is_dir() {
        return Err(format!(
            "--images must name a readable directory: {}",
            arguments.images.display()
        )
        .into());
    }
    if !(1..=1024).contains(&arguments.batch_size) {
        return Err("--batch-size must be between 1 and 1024".into());
    }
    let images = discover_benchmark_images(&arguments.images);
    let classifier: Box<dyn SemanticClassifier> = if arguments.model == "unavailable" {
        Box::new(UnavailableClassifier::default())
    } else if arguments.model == "tinyclip" {
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
        Box::new(TinyClipClassifier::load(&model_dir, &runtime)?)
    } else {
        return Err("--model must be tinyclip or unavailable".into());
    };
    let report = benchmark_classifier(
        classifier.as_ref(),
        &arguments.model,
        &images,
        arguments.backend.into(),
        arguments.batch_size,
    );

    if let Some(output) = arguments.output {
        write_report(&output, &report)?;
        println!("wrote {}", output.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }

    Ok(if report.status == "model_unavailable" {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    })
}

fn write_report(
    path: &PathBuf,
    report: &BenchmarkReport,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json")
        .to_ascii_lowercase();
    let content = match extension.as_str() {
        "json" => serde_json::to_string_pretty(report)?,
        "csv" => report_as_csv(report),
        _ => return Err("output extension must be .json or .csv".into()),
    };
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn report_as_csv(report: &BenchmarkReport) -> String {
    let header = "schema_version,requested_model,model_name,model_version,backend,batch_size,sample_count,failure_count,mean_latency_ms,p50_latency_ms,p95_latency_ms,throughput_per_second,peak_memory_bytes,status,error\n";
    let row = format!(
        "{},{},{},{},{:?},{},{},{},{},{},{},{},{},{},{}\n",
        report.schema_version,
        csv_cell(&report.requested_model),
        csv_cell(&report.model.name),
        csv_cell(&report.model.version),
        report.backend,
        report.batch_size,
        report.sample_count,
        report.failure_count,
        optional_number(report.mean_latency_ms),
        optional_number(report.p50_latency_ms),
        optional_number(report.p95_latency_ms),
        optional_number(report.throughput_per_second),
        report
            .peak_memory_bytes
            .map(|value| value.to_string())
            .unwrap_or_default(),
        csv_cell(&report.status),
        csv_cell(report.error.as_deref().unwrap_or_default()),
    );
    format!("{header}{row}")
}

fn optional_number(value: Option<f64>) -> String {
    value
        .map(|number| format!("{number:.6}"))
        .unwrap_or_default()
}

fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use photo_organizer_lib::semantic::ModelMetadata;

    #[test]
    fn csv_report_has_header_and_unavailable_status() {
        let report = BenchmarkReport {
            schema_version: 1,
            requested_model: "none".into(),
            model: ModelMetadata {
                name: "none".into(),
                version: "0".into(),
                analysis_version: "1".into(),
                license: None,
                installed: false,
                model_size_bytes: None,
                model_sha256: None,
                supported_backends: vec![ExecutionBackend::Cpu],
            },
            backend: ExecutionBackend::Cpu,
            batch_size: 1,
            sample_count: 2,
            failure_count: 2,
            mean_latency_ms: None,
            p50_latency_ms: None,
            p95_latency_ms: None,
            throughput_per_second: None,
            peak_memory_bytes: None,
            sample_predictions: Vec::new(),
            status: "model_unavailable".into(),
            error: Some("not installed".into()),
        };
        let csv = report_as_csv(&report);
        assert!(csv.starts_with("schema_version,"));
        assert!(csv.contains("model_unavailable"));
    }
}
