use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use crate::db::{LibrarySourceRoot, Repository};
use crate::error::{AppError, AppResult};
use crate::imaging::process_image_with_source_bytes;
use crate::models::{FileSnapshot, ScanProgress, ScanSummary};
use crate::source_identity::{
    SourceIdentity, existing_identity, identity_key, is_same_or_descendant,
};

pub fn validate_scan_root(root: &Path) -> AppResult<PathBuf> {
    Ok(existing_identity(root)?.source_path)
}

pub fn validate_scan_root_with_app_data(
    root: &Path,
    app_data_root: &Path,
) -> AppResult<SourceIdentity> {
    crate::source_identity::validate_source_root(root, app_data_root)
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

const MAX_IN_MEMORY_SOURCE_BYTES: u64 = 32 * 1024 * 1024;

struct FingerprintedSource {
    fingerprint: String,
    bytes: Option<Vec<u8>>,
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
    let root_identity = existing_identity(root)?;
    let root = root_identity.source_path;
    let root_string = path_to_string(&root);
    let name = root
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "图库".into());
    let (library_id, generation) = repository.begin_scan_with_identity(
        &root_string,
        &root_identity.identity_key,
        &name,
        task_id,
    )?;
    let descendant_roots = repository.descendant_source_roots(library_id)?;
    let mut progress = ScanProgress::starting(task_id);
    progress.library_id = Some(library_id);
    progress.stage = "discovering".into();
    emit(progress.clone());

    let mut candidates = Vec::new();
    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_pruned_source_root(entry.path(), &descendant_roots))
    {
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

        let Some(owner) = repository.resolve_library_owner(&path)? else {
            continue;
        };
        if owner.library_id != library_id {
            continue;
        }
        progress.current_path = Some(path_to_string(&path));
        let snapshot = match snapshot_file(&owner.source_path, &path) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let fallback = fallback_snapshot(&owner.source_path, &path);
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

        let asset_identity_key = identity_key(Path::new(&snapshot.absolute_path));
        if let Some(existing) = repository.find_existing_asset(&asset_identity_key)? {
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
                repository.touch_asset_seen(
                    existing.id,
                    owner.library_id,
                    &snapshot.relative_path,
                    generation,
                )?;
                progress.processed += 1;
                progress.skipped += 1;
                progress.error = None;
                repository.update_job_progress(task_id, progress.processed, progress.discovered)?;
                emit(progress.clone());
                continue;
            }
        }

        let fingerprinted_source = match read_fingerprinted_source(&path) {
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
        let fingerprint = &fingerprinted_source.fingerprint;

        match process_image_with_source_bytes(
            &path,
            thumbnail_dir,
            fingerprint,
            fingerprinted_source.bytes.as_deref(),
        ) {
            Ok(processed) => {
                repository.upsert_processed_asset(
                    library_id,
                    generation,
                    &snapshot,
                    fingerprint,
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
                    fingerprint,
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

fn read_fingerprinted_source(path: &Path) -> AppResult<FingerprintedSource> {
    let file_size = fs::metadata(path)?.len();
    if file_size <= MAX_IN_MEMORY_SOURCE_BYTES {
        let bytes = fs::read(path)?;
        let fingerprint = blake3::hash(&bytes).to_hex().to_string();
        return Ok(FingerprintedSource {
            fingerprint,
            bytes: Some(bytes),
        });
    }

    Ok(FingerprintedSource {
        fingerprint: hash_file(path)?,
        bytes: None,
    })
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

fn is_pruned_source_root(path: &Path, descendants: &[LibrarySourceRoot]) -> bool {
    let path_key = identity_key(path);
    descendants
        .iter()
        .any(|root| is_same_or_descendant(&root.identity_key, &path_key))
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
    fn small_source_fingerprint_reuses_the_read_bytes_for_decode() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("small-source.bin");
        let bytes = b"synthetic source bytes";
        fs::write(&source, bytes).expect("write source");

        let fingerprinted = read_fingerprinted_source(&source).expect("fingerprint source");

        assert_eq!(fingerprinted.bytes.as_deref(), Some(bytes.as_slice()));
        assert_eq!(
            fingerprinted.fingerprint,
            hash_file(&source).expect("hash source")
        );
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
    fn nested_library_scan_converges_owner_prunes_parent_and_reconciles_on_remove() {
        let (_temp, paths, repository, source) = setup();
        let child = source.join("显式子图库");
        let image = child.join("nested.png");
        save_pixel(&image, Rgba([120, 40, 220, 255]), (8, 8));
        let before = hash_file(&image).expect("source hash before");

        scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "parent-first",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("parent scan");
        let parent = repository
            .list_libraries()
            .expect("parent library")
            .into_iter()
            .find(|library| library.source_identity_key == identity_key(&source))
            .expect("parent library row");
        let before_child = repository
            .list_assets(
                parent.id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &crate::models::AssetFilter::default(),
            )
            .expect("parent assets before child import");
        assert_eq!(before_child.total, 1);
        let asset_id = before_child.items[0].id;

        scan_library(
            &repository,
            &paths.thumbnail_dir,
            &child,
            "child-import",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("child scan");
        let child_library = repository
            .list_libraries()
            .expect("libraries after child import")
            .into_iter()
            .find(|library| library.source_identity_key == identity_key(&child))
            .expect("child library row");
        assert_eq!(child_library.parent_library_id, Some(parent.id));

        let child_assets = repository
            .list_assets(
                child_library.id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &crate::models::AssetFilter::default(),
            )
            .expect("child assets");
        assert_eq!(child_assets.total, 1);
        assert_eq!(child_assets.items[0].id, asset_id);
        assert_eq!(child_assets.items[0].library_id, child_library.id);

        let parent_rescan = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "parent-rescan",
            &AtomicBool::new(false),
            |_| {},
        )
        .expect("parent rescan");
        assert_eq!(parent_rescan.discovered, 0);
        assert_eq!(parent_rescan.missing, 0);
        let parent_assets_after_prune = repository
            .list_assets(
                parent.id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &crate::models::AssetFilter::default(),
            )
            .expect("recursive parent assets");
        assert_eq!(parent_assets_after_prune.total, 1);

        let removal = repository
            .remove_library_with_reconciliation(child_library.id)
            .expect("remove child library");
        assert!(removal.removed);
        let after_child_remove = repository
            .list_assets(
                parent.id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &crate::models::AssetFilter::default(),
            )
            .expect("asset after child removal");
        assert_eq!(after_child_remove.total, 1);
        assert_eq!(after_child_remove.items[0].id, asset_id);
        assert_eq!(after_child_remove.items[0].library_id, parent.id);
        assert_eq!(
            hash_file(&image).expect("source hash after child removal"),
            before
        );

        repository
            .remove_library_with_reconciliation(parent.id)
            .expect("remove parent library");
        assert!(
            repository
                .list_libraries()
                .expect("libraries after parent removal")
                .is_empty()
        );
        assert!(image.is_file());
        assert_eq!(
            hash_file(&image).expect("source hash after parent removal"),
            before
        );
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
