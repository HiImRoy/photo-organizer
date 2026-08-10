use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use clap::Parser;
use image::codecs::jpeg::JpegEncoder;
use image::{Rgb, RgbImage};
use photo_organizer_lib::db::Repository;
use photo_organizer_lib::paths::AppPaths;
use photo_organizer_lib::scanner::scan_library;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(
    name = "import-benchmark",
    about = "Measure cold and warm PhotoOrganizer import on an isolated source"
)]
struct Arguments {
    #[arg(long, value_name = "DIR", conflicts_with = "generate")]
    images: Option<PathBuf>,

    #[arg(
        long,
        value_name = "DIR",
        default_value = "benchmark-output/import-benchmark"
    )]
    data_dir: PathBuf,

    #[arg(long, default_value_t = 0, value_name = "COUNT")]
    generate: usize,

    #[arg(long, default_value_t = 4000)]
    width: u32,

    #[arg(long, default_value_t = 3000)]
    height: u32,

    #[arg(long, default_value_t = 90)]
    quality: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkReport {
    source_dir: String,
    data_dir: String,
    generated_files: usize,
    cold_wall_ms: f64,
    cold: photo_organizer_lib::models::ScanSummary,
    cache_reuse_wall_ms: f64,
    cache_reuse: photo_organizer_lib::models::ScanSummary,
    warm_wall_ms: f64,
    warm: photo_organizer_lib::models::ScanSummary,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("import-benchmark: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    if arguments.generate == 0 && arguments.images.is_none() {
        return Err("provide --images or set --generate to a positive count".into());
    }
    if arguments.generate > 0 && (arguments.width == 0 || arguments.height == 0) {
        return Err("generated image dimensions must be positive".into());
    }
    if arguments.quality == 0 || arguments.quality > 100 {
        return Err("JPEG quality must be between 1 and 100".into());
    }

    let benchmark_root = arguments.data_dir.canonicalize().unwrap_or_else(|_| {
        std::env::current_dir()
            .expect("current directory")
            .join(&arguments.data_dir)
    });
    fs::create_dir_all(&benchmark_root)?;
    let source_dir = match arguments.images {
        Some(path) => path.canonicalize()?,
        None => {
            let generated = benchmark_root.join("source-fixtures");
            generate_fixtures(
                &generated,
                arguments.generate,
                arguments.width,
                arguments.height,
                arguments.quality,
            )?;
            generated
        }
    };
    if !source_dir.is_dir() {
        return Err(format!("source directory does not exist: {}", source_dir.display()).into());
    }

    let paths = AppPaths::initialize(benchmark_root.join("app-data"))?;
    let repository = Repository::new(&paths.database_path);
    repository.initialize()?;
    let cancelled = AtomicBool::new(false);

    let cold_started = Instant::now();
    let cold = scan_library(
        &repository,
        &paths.thumbnail_dir,
        &source_dir,
        "import-benchmark-cold",
        &cancelled,
        |_| {},
    )?;
    let cold_wall_ms = cold_started.elapsed().as_secs_f64() * 1000.0;

    let connection = Connection::open(&paths.database_path)?;
    connection.execute("DELETE FROM tone_features", [])?;
    connection.execute("DELETE FROM color_features", [])?;
    drop(connection);

    let cache_reuse_started = Instant::now();
    let cache_reuse = scan_library(
        &repository,
        &paths.thumbnail_dir,
        &source_dir,
        "import-benchmark-cache-reuse",
        &cancelled,
        |_| {},
    )?;
    let cache_reuse_wall_ms = cache_reuse_started.elapsed().as_secs_f64() * 1000.0;

    let warm_started = Instant::now();
    let warm = scan_library(
        &repository,
        &paths.thumbnail_dir,
        &source_dir,
        "import-benchmark-warm",
        &cancelled,
        |_| {},
    )?;
    let warm_wall_ms = warm_started.elapsed().as_secs_f64() * 1000.0;

    let report = BenchmarkReport {
        source_dir: source_dir.to_string_lossy().into_owned(),
        data_dir: benchmark_root.to_string_lossy().into_owned(),
        generated_files: arguments.generate,
        cold_wall_ms,
        cold,
        cache_reuse_wall_ms,
        cache_reuse,
        warm_wall_ms,
        warm,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn generate_fixtures(
    source_dir: &Path,
    count: usize,
    width: u32,
    height: u32,
    quality: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(source_dir)?;
    for index in 0..count {
        let path = source_dir.join(format!("generated-{index:03}.jpg"));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        write_fixture(file, index, width, height, quality)?;
    }
    Ok(())
}

fn write_fixture(
    file: File,
    seed: usize,
    width: u32,
    height: u32,
    quality: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut image = RgbImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let x_value = x.wrapping_add((seed as u32).wrapping_mul(37));
        let y_value = y.wrapping_add((seed as u32).wrapping_mul(19));
        *pixel = Rgb([
            (x_value % 256) as u8,
            (y_value % 256) as u8,
            (x_value.wrapping_add(y_value) % 256) as u8,
        ]);
    }
    let mut file = file;
    JpegEncoder::new_with_quality(&mut file, quality).encode_image(&image)?;
    Ok(())
}
