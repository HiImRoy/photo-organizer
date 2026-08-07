use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use crate::db::Repository;
use crate::error::{AppError, AppResult};
use crate::imaging::process_image;
use crate::models::{FileSnapshot, ScanProgress, ScanSummary};

pub fn validate_scan_root(root: &Path) -> AppResult<PathBuf> {
    if !root.is_dir() {
        return Err(AppError::InvalidRoot(root.to_path_buf()));
    }
    root.canonicalize().map_err(AppError::from)
}

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp"
            )
        })
}

pub fn scan_library<F>(
    repository: &Repository,
    thumbnail_dir: &Path,
    root: &Path,
    task_id: &str,
    cancelled: &AtomicBool,
    emit: F,
) -> AppResult<ScanSummary>
where
    F: Fn(ScanProgress),
{
    let root = validate_scan_root(root)?;
    let root_string = path_to_string(&root);
    let (library_id, generation) = repository.begin_scan(&root_string, task_id)?;
    let mut progress = ScanProgress::starting(task_id);
    progress.library_id = Some(library_id);
    progress.stage = "discovering".into();
    emit(progress.clone());

    let mut candidates = Vec::new();
    for entry in walkdir::WalkDir::new(&root).follow_links(false) {
        match entry {
            Ok(entry) if entry.file_type().is_file() => {
                if is_supported_image(entry.path()) {
                    candidates.push(entry.into_path());
                } else {
                    progress.skipped += 1;
                }
            }
            Ok(_) => {}
            Err(error) => {
                progress.failed += 1;
                progress.error = Some(error.to_string());
                emit(progress.clone());
            }
        }
    }
    candidates.sort_by_key(|path| path_to_string(path));
    progress.discovered = candidates.len() as u64;
    progress.stage = "processing".into();
    progress.error = None;
    emit(progress.clone());
    repository.update_job_progress(task_id, 0, progress.discovered)?;

    for path in candidates {
        if cancelled.load(Ordering::Relaxed) {
            progress.status = "cancelled".into();
            progress.stage = "cancelled".into();
            progress.current_path = None;
            repository.cancel_scan(task_id, library_id)?;
            emit(progress.clone());
            return Ok(summary_from_progress(&progress, library_id));
        }

        progress.current_path = Some(path_to_string(&path));
        let snapshot = match snapshot_file(&root, &path) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let fallback = fallback_snapshot(&root, &path);
                let fingerprint = fallback_fingerprint(&fallback);
                repository.upsert_failed_asset(
                    library_id,
                    generation,
                    &fallback,
                    &fingerprint,
                    &error.to_string(),
                )?;
                progress.processed += 1;
                progress.failed += 1;
                progress.error = Some(error.to_string());
                repository.update_job_progress(task_id, progress.processed, progress.discovered)?;
                emit(progress.clone());
                continue;
            }
        };

        if let Some(existing) =
            repository.find_existing_asset(library_id, &snapshot.absolute_path)?
        {
            let cache_ready = existing.thumbnail_status.as_deref() == Some("ready")
                && existing
                    .cache_path
                    .as_deref()
                    .is_some_and(|cache| Path::new(cache).is_file());
            if existing.file_size == snapshot.file_size
                && existing.modified_at == snapshot.modified_at
                && existing.analysis_status == "completed"
                && existing.analysis_algorithm_version.as_deref()
                    == Some(crate::imaging::ANALYSIS_VERSION)
                && cache_ready
            {
                repository.touch_asset_seen(existing.id, generation)?;
                progress.processed += 1;
                progress.skipped += 1;
                progress.error = None;
                repository.update_job_progress(task_id, progress.processed, progress.discovered)?;
                emit(progress.clone());
                continue;
            }
        }

        let fingerprint = match hash_file(&path) {
            Ok(value) => value,
            Err(error) => {
                let fallback = fallback_fingerprint(&snapshot);
                repository.upsert_failed_asset(
                    library_id,
                    generation,
                    &snapshot,
                    &fallback,
                    &error.to_string(),
                )?;
                progress.processed += 1;
                progress.failed += 1;
                progress.error = Some(error.to_string());
                repository.update_job_progress(task_id, progress.processed, progress.discovered)?;
                emit(progress.clone());
                continue;
            }
        };

        match process_image(&path, thumbnail_dir, &fingerprint) {
            Ok(processed) => {
                repository.upsert_processed_asset(
                    library_id,
                    generation,
                    &snapshot,
                    &fingerprint,
                    &processed,
                )?;
                progress.succeeded += 1;
                progress.error = None;
            }
            Err(error) => {
                repository.upsert_failed_asset(
                    library_id,
                    generation,
                    &snapshot,
                    &fingerprint,
                    &error.to_string(),
                )?;
                progress.failed += 1;
                progress.error = Some(error.to_string());
            }
        }
        progress.processed += 1;
        repository.update_job_progress(task_id, progress.processed, progress.discovered)?;
        emit(progress.clone());
    }

    progress.missing = repository.complete_scan(task_id, library_id, generation)?;
    progress.status = "completed".into();
    progress.stage = "completed".into();
    progress.current_path = None;
    progress.error = None;
    emit(progress.clone());
    Ok(summary_from_progress(&progress, library_id))
}

fn snapshot_file(root: &Path, path: &Path) -> AppResult<FileSnapshot> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(AppError::InvalidArgument(format!(
            "not a file: {}",
            path.display()
        )));
    }
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0);
    let relative = path.strip_prefix(root).unwrap_or(path);
    Ok(FileSnapshot {
        absolute_path: path_to_string(path),
        relative_path: path_to_string(relative),
        file_name: path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default(),
        extension: path
            .extension()
            .map(|value| value.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default(),
        file_size: metadata.len().min(i64::MAX as u64) as i64,
        modified_at,
    })
}

fn fallback_snapshot(root: &Path, path: &Path) -> FileSnapshot {
    let relative = path.strip_prefix(root).unwrap_or(path);
    FileSnapshot {
        absolute_path: path_to_string(path),
        relative_path: path_to_string(relative),
        file_name: path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default(),
        extension: path
            .extension()
            .map(|value| value.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default(),
        file_size: 0,
        modified_at: 0,
    }
}

pub fn hash_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn fallback_fingerprint(snapshot: &FileSnapshot) -> String {
    let value = format!(
        "unreadable\0{}\0{}\0{}",
        snapshot.absolute_path, snapshot.file_size, snapshot.modified_at
    );
    blake3::hash(value.as_bytes()).to_hex().to_string()
}

fn summary_from_progress(progress: &ScanProgress, library_id: i64) -> ScanSummary {
    ScanSummary {
        task_id: progress.task_id.clone(),
        library_id,
        status: progress.status.clone(),
        discovered: progress.discovered,
        processed: progress.processed,
        succeeded: progress.succeeded,
        failed: progress.failed,
        skipped: progress.skipped,
        missing: progress.missing,
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use image::{DynamicImage, Rgba, RgbaImage};

    use super::*;
    use crate::models::{AssetSortField, SortDirection};
    use crate::paths::AppPaths;

    fn setup() -> (tempfile::TempDir, AppPaths, Repository, PathBuf) {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = AppPaths::initialize(temp.path().join("app-data")).expect("app paths");
        let repository = Repository::new(&paths.database_path);
        repository.initialize().expect("database");
        let source = temp.path().join("fixture-library");
        fs::create_dir_all(&source).expect("source");
        (temp, paths, repository, source)
    }

    fn save_pixel(path: &Path, pixel: Rgba<u8>, dimensions: (u32, u32)) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(dimensions.0, dimensions.1, pixel))
            .save(path)
            .expect("save fixture");
    }

    #[test]
    fn empty_directory_scans_successfully() {
        let (_temp, paths, repository, source) = setup();
        let events = Mutex::new(Vec::new());
        let summary = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "empty-task",
            &AtomicBool::new(false),
            |progress| events.lock().expect("events").push(progress),
        )
        .expect("scan");
        assert_eq!(summary.status, "completed");
        assert_eq!(summary.discovered, 0);
        assert_eq!(
            events.lock().expect("events").last().expect("last").stage,
            "completed"
        );
    }

    #[test]
    fn nested_unicode_supported_formats_and_corruption_are_isolated() {
        let (_temp, paths, repository, source) = setup();
        let images = [
            source.join("中文 路径").join("红色.JPG"),
            source.join("русский").join("синий.png"),
            source.join("emoji 😀").join("green.WebP"),
        ];
        save_pixel(&images[0], Rgba([255, 0, 0, 255]), (10, 8));
        save_pixel(&images[1], Rgba([0, 0, 255, 255]), (7, 9));
        save_pixel(&images[2], Rgba([0, 255, 0, 255]), (6, 6));
        fs::write(source.join("broken.jpeg"), b"not an image").expect("broken fixture");
        fs::write(source.join("notes.txt"), b"unsupported").expect("unsupported fixture");
        let original_hashes: Vec<String> = images
            .iter()
            .map(|path| hash_file(path).expect("hash before"))
            .collect();

        let summary = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "unicode-task",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("scan");
        assert_eq!(summary.discovered, 4);
        assert_eq!(summary.succeeded, 3);
        assert_eq!(summary.failed, 1);
        assert!(summary.skipped >= 1);

        let library = repository.list_libraries().expect("libraries").remove(0);
        let page = repository
            .list_assets(
                library.id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &crate::models::AssetFilter::default(),
            )
            .expect("assets");
        assert_eq!(page.total, 4);
        assert_eq!(
            page.items
                .iter()
                .filter(|asset| asset.thumbnail_available)
                .count(),
            3
        );
        let folders = repository
            .list_library_folders(library.id)
            .expect("folder tree");
        assert!(
            folders
                .iter()
                .any(|folder| folder.relative_path == "中文 路径")
        );
        let nested = repository
            .list_assets(
                library.id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &crate::models::AssetFilter {
                    folder_prefix: Some("中文 路径".into()),
                    ..crate::models::AssetFilter::default()
                },
            )
            .expect("nested folder filter");
        assert_eq!(nested.total, 1);
        assert_eq!(nested.items[0].relative_path, "中文 路径\\红色.JPG");
        assert!(
            page.items
                .iter()
                .any(|asset| asset.analysis_status == "failed")
        );

        let semantic_candidates = repository
            .create_semantic_job("semantic-filter-task", library.id, false, None)
            .expect("semantic candidates");
        let candidate = semantic_candidates.first().expect("semantic candidate");
        let semantic_asset = page
            .items
            .iter()
            .find(|asset| asset.id == candidate.id)
            .expect("semantic asset");
        repository
            .save_semantic_result(
                "semantic-filter-task",
                candidate,
                &crate::semantic::SemanticAnalysisOutput {
                    predictions: vec![
                        crate::semantic::SemanticPrediction {
                            label_id: "portrait".into(),
                            display_name: "人像".into(),
                            similarity: 0.31,
                            threshold: 0.16,
                            is_primary: true,
                        },
                        crate::semantic::SemanticPrediction {
                            label_id: "night".into(),
                            display_name: "夜景".into(),
                            similarity: 0.27,
                            threshold: 0.16,
                            is_primary: false,
                        },
                    ],
                    embedding: vec![0.01; crate::semantic::EMBEDDING_DIMENSIONS],
                    raw_similarities: Vec::new(),
                },
            )
            .expect("save semantic result");
        let combined_filter = crate::models::AssetFilter {
            semantic_labels: vec!["portrait".into(), "night".into()],
            semantic_match: crate::models::SemanticMatchMode::All,
            tone_labels: semantic_asset.tone_label.clone().into_iter().collect(),
            color_categories: semantic_asset
                .dominant_color_category
                .clone()
                .into_iter()
                .collect(),
            brightness_min: semantic_asset.brightness.map(|value| value - 0.001),
            brightness_max: semantic_asset.brightness.map(|value| value + 0.001),
            saturation_min: semantic_asset.saturation.map(|value| value - 0.001),
            saturation_max: semantic_asset.saturation.map(|value| value + 0.001),
            ..crate::models::AssetFilter::default()
        };
        let filtered = repository
            .list_assets(
                library.id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &combined_filter,
            )
            .expect("combined filtered assets");
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.items[0].id, candidate.id);
        assert_eq!(filtered.items[0].semantic_labels.len(), 2);
        for (path, before) in images.iter().zip(original_hashes) {
            assert_eq!(hash_file(path).expect("hash after"), before);
        }
        assert!(!source.join("thumbnails").exists());
        assert!(
            paths
                .thumbnail_dir
                .read_dir()
                .expect("thumbs")
                .next()
                .is_some()
        );
    }

    #[test]
    fn repeated_added_modified_and_missing_files_are_detected() {
        let (_temp, paths, repository, source) = setup();
        let first = source.join("first.png");
        let second = source.join("sub").join("second.jpg");
        save_pixel(&first, Rgba([20, 20, 20, 255]), (4, 4));

        let initial = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "initial",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("initial scan");
        assert_eq!(initial.succeeded, 1);

        let repeated = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "repeated",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("repeat scan");
        assert_eq!(repeated.succeeded, 0);
        assert_eq!(repeated.skipped, 1);

        save_pixel(&first, Rgba([250, 250, 250, 255]), (8, 8));
        save_pixel(&second, Rgba([0, 200, 90, 255]), (5, 5));
        let changed = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "changed",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("changed scan");
        assert_eq!(changed.succeeded, 2);

        fs::remove_file(&first).expect("remove temporary fixture to simulate missing");
        let missing = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "missing",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("missing scan");
        assert_eq!(missing.missing, 1);
        let library = repository.list_libraries().expect("libraries").remove(0);
        assert_eq!(library.asset_count, 2);
        assert_eq!(library.present_count, 1);
        assert_eq!(library.missing_count, 1);
    }

    #[test]
    fn cancellation_does_not_mark_unvisited_assets_missing() {
        let (_temp, paths, repository, source) = setup();
        save_pixel(&source.join("one.png"), Rgba([10, 20, 30, 255]), (4, 4));
        scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "before-cancel",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("initial scan");

        let cancellation = AtomicBool::new(true);
        let summary = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "cancelled-task",
            &cancellation,
            |_| {},
        )
        .expect("cancel scan");
        assert_eq!(summary.status, "cancelled");
        assert_eq!(
            repository.list_libraries().expect("libraries")[0].missing_count,
            0
        );
    }

    #[cfg(windows)]
    #[test]
    fn unreadable_locked_file_does_not_abort_scan() {
        use std::os::windows::fs::OpenOptionsExt;

        let (_temp, paths, repository, source) = setup();
        let locked_path = source.join("locked.png");
        save_pixel(&locked_path, Rgba([10, 20, 30, 255]), (4, 4));
        let _lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&locked_path)
            .expect("exclusive lock");
        let summary = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "locked-task",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("scan continues");
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.status, "completed");
    }
}
