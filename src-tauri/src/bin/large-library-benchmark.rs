use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use clap::Parser;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use photo_organizer_lib::db::Repository;
use photo_organizer_lib::models::{
    ASSET_QUERY_VERSION, AssetFilter, AssetQuery, AssetQueryRoot, AssetSortField, ScanSummary,
    SortDirection,
};
use photo_organizer_lib::paths::AppPaths;
use photo_organizer_lib::scanner::scan_library;
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

const FILE_COUNT: usize = 3_000;
const SOURCE_WIDTH: u32 = 320;
const SOURCE_HEIGHT: u32 = 240;
const COMMIT_BATCH: u64 = 16;
const UNIQUE_SEED_CAPACITY: usize = 13 * 256;
const HIGH_SEED_STEP: u8 = 20;
const DEFAULT_OUTPUT_ROOT: &str =
    "test-data/.tmp/large-library-stress-mixed-retry-20260927-c1726f2e";

#[derive(Debug, Parser)]
#[command(name = "large-library-benchmark")]
struct Arguments {
    #[arg(long, default_value = DEFAULT_OUTPUT_ROOT, value_name = "DIR")]
    output_root: PathBuf,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct FileHash {
    relative_path: String,
    sha256: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct FixtureManifest {
    file_count: usize,
    total_bytes: u64,
    sha256: String,
    files: Vec<FileHash>,
}

#[derive(Debug, Serialize)]
struct TimedScan {
    wall_ms: f64,
    summary: ScanSummary,
}

#[derive(Debug, Serialize)]
struct CommitBoundary {
    database_visible_assets: Option<i64>,
    query_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct QueryMeasurement {
    wall_ms: f64,
    total: i64,
    returned: usize,
    page_size: u32,
}

#[derive(Debug, Serialize)]
struct RemovalMeasurement {
    database_index_reconciliation_wall_ms: f64,
    removed: bool,
    removed_asset_count: usize,
    thumbnail_cache_candidates: usize,
    library_remains_indexed: bool,
    cache_cleanup: &'static str,
}

#[derive(Debug, Serialize)]
struct Report {
    file_count: usize,
    source_dimensions: (u32, u32),
    format_counts: (usize, usize, usize),
    thumbnail_count: usize,
    max_thumbnail_dimension: u32,
    fixture_unchanged: bool,
    manifest_before: FixtureManifest,
    manifest_after: FixtureManifest,
    cold_import: TimedScan,
    cancelled_scan: TimedScan,
    cancel_boundary: CommitBoundary,
    resumed_scan: TimedScan,
    warm_rescan: TimedScan,
    filtered_query: QueryMeasurement,
    remove_index: RemovalMeasurement,
    ui_responsiveness: &'static str,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("large-library-benchmark: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    let output_root = prepare_output_root(&arguments.output_root)?;
    let fixture_root = output_root.join("source-fixtures");
    fs::create_dir(&fixture_root)?;

    let format_counts = generate_fixtures(&fixture_root)?;
    let manifest_before = fixture_manifest(&fixture_root)?;
    ensure(
        manifest_before.file_count == FILE_COUNT,
        "generated fixture count did not match the requested count",
    )?;

    let cold_paths = AppPaths::initialize(output_root.join("cold-app-data"))?;
    let cold_repository = Repository::new(&cold_paths.database_path);
    cold_repository.initialize()?;
    let cold_import = timed_scan(
        &cold_repository,
        &cold_paths.thumbnail_dir,
        &fixture_root,
        "large-library-cold",
    )?;
    ensure_completed(&cold_import.summary, FILE_COUNT, "cold import")?;

    let (thumbnail_count, max_thumbnail_dimension) = inspect_thumbnails(&cold_paths.thumbnail_dir)?;
    ensure(
        thumbnail_count == FILE_COUNT && max_thumbnail_dimension <= 640,
        &format!(
            "thumbnail output count/dimensions unexpected: found {thumbnail_count} thumbnails, maximum dimension {max_thumbnail_dimension}; expected {FILE_COUNT} and <= 640"
        ),
    )?;

    let warm_rescan = timed_scan(
        &cold_repository,
        &cold_paths.thumbnail_dir,
        &fixture_root,
        "large-library-warm",
    )?;
    ensure(
        warm_rescan.summary.status == "completed"
            && warm_rescan.summary.skipped == FILE_COUNT as u64
            && warm_rescan.summary.failed == 0,
        "warm rescan did not skip the complete unchanged library",
    )?;

    let filtered_query = timed_filtered_query(&cold_repository, cold_import.summary.library_id)?;
    ensure(
        filtered_query.total > 0 && filtered_query.total < FILE_COUNT as i64,
        "representative filtered query returned an unexpected total",
    )?;

    let removal_started = Instant::now();
    let removal =
        cold_repository.remove_library_with_reconciliation(cold_import.summary.library_id)?;
    let removal_wall_ms = elapsed_ms(removal_started);
    let library_remains_indexed = cold_repository
        .list_libraries()?
        .iter()
        .any(|library| library.id == cold_import.summary.library_id);
    ensure(
        removal.removed
            && removal.removed_preview_asset_ids.len() == FILE_COUNT
            && !library_remains_indexed,
        "library index removal did not reconcile the expected asset count",
    )?;

    let resume_paths = AppPaths::initialize(output_root.join("resume-app-data"))?;
    let resume_repository = Repository::new(&resume_paths.database_path);
    resume_repository.initialize()?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancellation_library_id = Arc::new(AtomicI64::new(0));
    let commit_boundary = Arc::new(Mutex::new(None::<CommitBoundary>));
    let monitor_cancelled = Arc::clone(&cancelled);
    let monitor_library_id = Arc::clone(&cancellation_library_id);
    let monitor_boundary = Arc::clone(&commit_boundary);
    let monitor_repository = resume_repository.clone();
    let monitor = std::thread::spawn(move || {
        loop {
            if monitor_cancelled.load(Ordering::Relaxed) {
                return;
            }

            let library_id = monitor_library_id.load(Ordering::Relaxed);
            if library_id != 0 {
                match monitor_repository.query_assets(&source_query(library_id, 1)) {
                    Ok(page) if page.total >= COMMIT_BATCH as i64 => {
                        *monitor_boundary.lock().expect("commit boundary lock") =
                            Some(CommitBoundary {
                                database_visible_assets: Some(page.total),
                                query_error: None,
                            });
                        monitor_cancelled.store(true, Ordering::Relaxed);
                        return;
                    }
                    Err(error) => {
                        *monitor_boundary.lock().expect("commit boundary lock") =
                            Some(CommitBoundary {
                                database_visible_assets: None,
                                query_error: Some(error.to_string()),
                            });
                        monitor_cancelled.store(true, Ordering::Relaxed);
                        return;
                    }
                    Ok(_) => {}
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });
    let cancel_started = Instant::now();
    let cancelled_result = scan_library(
        &resume_repository,
        &resume_paths.thumbnail_dir,
        &fixture_root,
        "large-library-cancel",
        &cancelled,
        |progress| {
            if let Some(library_id) = progress.library_id {
                cancellation_library_id.store(library_id, Ordering::Relaxed);
            }
        },
    );
    cancelled.store(true, Ordering::Relaxed);
    monitor
        .join()
        .map_err(|_| "cancellation monitor thread panicked")?;
    let cancelled_summary = cancelled_result?;
    let cancelled_scan = TimedScan {
        wall_ms: elapsed_ms(cancel_started),
        summary: cancelled_summary,
    };
    let commit_boundary = Arc::try_unwrap(commit_boundary)
        .map_err(|_| "commit boundary monitor still owns the result")?
        .into_inner()
        .map_err(|_| "commit boundary lock poisoned")?
        .ok_or("cancellation monitor did not observe a committed batch")?;
    ensure(
        cancelled_scan.summary.status == "cancelled"
            && cancelled_scan.summary.processed == COMMIT_BATCH
            && cancelled_scan.summary.succeeded == COMMIT_BATCH
            && cancelled_scan.summary.failed == 0
            && commit_boundary.database_visible_assets == Some(COMMIT_BATCH as i64),
        "cancellation was not confirmed after exactly one committed batch",
    )?;

    let resumed_scan = timed_scan(
        &resume_repository,
        &resume_paths.thumbnail_dir,
        &fixture_root,
        "large-library-resume",
    )?;
    ensure(
        resumed_scan.summary.status == "completed"
            && resumed_scan.summary.succeeded == (FILE_COUNT as u64 - COMMIT_BATCH)
            && resumed_scan.summary.skipped == COMMIT_BATCH
            && resumed_scan.summary.failed == 0,
        "resume did not skip the committed batch and import the remaining files",
    )?;

    let manifest_after = fixture_manifest(&fixture_root)?;
    let fixture_unchanged = manifest_before == manifest_after;
    ensure(fixture_unchanged, "source fixture SHA-256 manifest changed")?;

    let report = Report {
        file_count: FILE_COUNT,
        source_dimensions: (SOURCE_WIDTH, SOURCE_HEIGHT),
        format_counts,
        thumbnail_count,
        max_thumbnail_dimension,
        fixture_unchanged,
        manifest_before,
        manifest_after,
        cold_import,
        cancelled_scan,
        cancel_boundary: commit_boundary,
        resumed_scan,
        warm_rescan,
        filtered_query,
        remove_index: RemovalMeasurement {
            database_index_reconciliation_wall_ms: removal_wall_ms,
            removed: removal.removed,
            removed_asset_count: removal.removed_preview_asset_ids.len(),
            thumbnail_cache_candidates: removal.removed_thumbnail_cache_paths.len(),
            library_remains_indexed,
            cache_cleanup: "unmeasured; performed separately by the IPC path",
        },
        ui_responsiveness: "backend latency proxy only; manual desktop acceptance is required",
    };
    let report_path = output_root.join("report.json");
    let report_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&report_path)?;
    serde_json::to_writer_pretty(report_file, &report)?;
    println!("report: {}", report_path.display());
    println!("fixture files: {FILE_COUNT}; SHA-256 manifests match");
    println!("cold import: {:.1} ms", report.cold_import.wall_ms);
    println!(
        "cancel/resume: {:.1} / {:.1} ms",
        report.cancelled_scan.wall_ms, report.resumed_scan.wall_ms
    );
    println!("warm rescan: {:.1} ms", report.warm_rescan.wall_ms);
    println!(
        "filtered query: {:.1} ms ({} results)",
        report.filtered_query.wall_ms, report.filtered_query.total
    );
    println!(
        "index reconciliation: {:.1} ms",
        report.remove_index.database_index_reconciliation_wall_ms
    );
    Ok(())
}

fn timed_scan(
    repository: &Repository,
    thumbnail_dir: &Path,
    fixture_root: &Path,
    task_id: &str,
) -> Result<TimedScan, Box<dyn std::error::Error>> {
    let cancelled = AtomicBool::new(false);
    let started = Instant::now();
    let summary = scan_library(
        repository,
        thumbnail_dir,
        fixture_root,
        task_id,
        &cancelled,
        |_| {},
    )?;
    Ok(TimedScan {
        wall_ms: elapsed_ms(started),
        summary,
    })
}

fn timed_filtered_query(
    repository: &Repository,
    library_id: i64,
) -> Result<QueryMeasurement, Box<dyn std::error::Error>> {
    let mut query = source_query(library_id, 50);
    query.filter = AssetFilter {
        search: Some("photo-01".into()),
        ..AssetFilter::default()
    };
    query.sort = AssetSortField::Brightness;
    let started = Instant::now();
    let page = repository.query_assets(&query)?;
    Ok(QueryMeasurement {
        wall_ms: elapsed_ms(started),
        total: page.total,
        returned: page.items.len(),
        page_size: query.page_size,
    })
}

fn source_query(library_id: i64, page_size: u32) -> AssetQuery {
    AssetQuery {
        version: ASSET_QUERY_VERSION,
        root: AssetQueryRoot::Source { library_id },
        include_descendants: true,
        filter: AssetFilter::default(),
        sort: AssetSortField::FileName,
        direction: SortDirection::Asc,
        page: 1,
        page_size,
    }
}

fn ensure_completed(
    summary: &ScanSummary,
    expected: usize,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    ensure(
        summary.status == "completed"
            && summary.discovered == expected as u64
            && summary.succeeded == expected as u64
            && summary.failed == 0,
        &format!("{label} counts/status did not match the fixture"),
    )
}

fn ensure(condition: bool, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned().into())
    }
}

fn prepare_output_root(candidate: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("crate directory has no workspace parent")?;
    let allowed_parent = workspace.join("test-data").join(".tmp").canonicalize()?;
    let candidate = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        workspace.join(candidate)
    };
    let parent = candidate
        .parent()
        .ok_or("output root has no parent")?
        .canonicalize()?;
    ensure(
        parent == allowed_parent,
        "output root must be directly under test-data/.tmp",
    )?;
    let name = candidate.file_name().ok_or("output root has no name")?;
    ensure(
        name.to_string_lossy().starts_with("large-library-stress-"),
        "output root must use the large-library-stress- prefix",
    )?;
    let root = parent.join(name);
    if !root.exists() {
        fs::create_dir(&root)?;
    } else {
        ensure(
            !fs::symlink_metadata(&root)?.file_type().is_symlink(),
            "output root must not be a symlink",
        )?;
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            ensure(
                entry.file_name() == "cargo-target" && entry.file_type()?.is_dir(),
                "output root is not empty except for its owned Cargo target",
            )?;
        }
    }
    Ok(root)
}

fn generate_fixtures(root: &Path) -> Result<(usize, usize, usize), Box<dyn std::error::Error>> {
    ensure(
        FILE_COUNT <= UNIQUE_SEED_CAPACITY,
        "fixture count exceeds the unique seed pattern capacity",
    )?;
    let mut counts = (0, 0, 0);
    for index in 0..FILE_COUNT {
        let (format, extension) = fixture_format(index);
        match index % 3 {
            0 => counts.0 += 1,
            1 => counts.1 += 1,
            _ => counts.2 += 1,
        }
        let path = fixture_path(root, index, extension);
        fs::create_dir_all(path.parent().ok_or("fixture path has no parent")?)?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        DynamicImage::ImageRgb8(synthetic_image(index)).write_to(&mut file, format)?;
    }
    Ok(counts)
}

fn fixture_format(index: usize) -> (ImageFormat, &'static str) {
    match index % 3 {
        0 => (ImageFormat::Jpeg, "jpg"),
        1 => (ImageFormat::Png, "png"),
        _ => (ImageFormat::WebP, "webp"),
    }
}

fn fixture_path(root: &Path, index: usize, extension: &str) -> PathBuf {
    let batch = index / 120;
    let album = (index / 30) % 4;
    root.join(format!("批次 {batch:02} - 旅行"))
        .join(format!("альбом {album:02}"))
        .join(format!("photo-{index:05}.{extension}"))
}

fn synthetic_image(seed: usize) -> RgbImage {
    let seed = seed as u32;
    let low_seed = (seed & 0xff) as u8;
    let high_marker = ((seed >> 8) as u8) * HIGH_SEED_STEP;
    RgbImage::from_fn(SOURCE_WIDTH, SOURCE_HEIGHT, |x, y| {
        if x < 64 && y < 64 {
            return Rgb([high_marker, low_seed, high_marker]);
        }
        Rgb([
            ((x + seed * 17) % 256) as u8,
            ((y * 3 + seed * 29) % 256) as u8,
            ((x + y + seed * 43) % 256) as u8,
        ])
    })
}

fn fixture_manifest(root: &Path) -> Result<FixtureManifest, Box<dyn std::error::Error>> {
    let mut paths = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    paths.sort();

    let mut files = Vec::with_capacity(paths.len());
    let mut total_bytes = 0_u64;
    let mut manifest_hash = Sha256::new();
    for path in paths {
        let relative_path = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path)?;
        let digest = Sha256::digest(&bytes);
        let hash = hex(&digest);
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        manifest_hash.update(relative_path.as_bytes());
        manifest_hash.update([0]);
        manifest_hash.update(digest);
        manifest_hash.update([0xff]);
        files.push(FileHash {
            relative_path,
            sha256: hash,
        });
    }
    Ok(FixtureManifest {
        file_count: files.len(),
        total_bytes,
        sha256: hex(&manifest_hash.finalize()),
        files,
    })
}

fn inspect_thumbnails(root: &Path) -> Result<(usize, u32), Box<dyn std::error::Error>> {
    let mut count = 0;
    let mut max_dimension = 0;
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if !path.is_file()
            || !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("-grid-640-v1.jpg"))
        {
            continue;
        }
        let (width, height) = image::image_dimensions(&path)?;
        max_dimension = max_dimension.max(width).max(height);
        count += 1;
    }
    Ok((count, max_dimension))
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000.0
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{FILE_COUNT, fixture_format, fixture_path, synthetic_image};
    use std::path::Path;

    #[test]
    fn fixture_is_evenly_mixed_and_uses_unicode_nested_paths() {
        let counts = (0..FILE_COUNT).fold((0, 0, 0), |mut counts, index| {
            match index % 3 {
                0 => counts.0 += 1,
                1 => counts.1 += 1,
                _ => counts.2 += 1,
            }
            counts
        });
        assert_eq!(counts, (1_000, 1_000, 1_000));
        assert_eq!(fixture_format(0).1, "jpg");
        assert_eq!(fixture_format(1).1, "png");
        assert_eq!(fixture_format(2).1, "webp");

        let path = fixture_path(Path::new("fixture"), 0, "jpg");
        let path = path.to_string_lossy();
        assert!(path.contains("批次 00 - 旅行"));
        assert!(path.contains("альбом 00"));
        assert!(path.contains(" "));
    }

    #[test]
    fn seed_marker_distinguishes_seeds_that_previously_repeated() {
        let first = synthetic_image(0);
        let repeated_low_bits = synthetic_image(256);
        assert_ne!(first.get_pixel(32, 32), repeated_low_bits.get_pixel(32, 32));
    }
}
