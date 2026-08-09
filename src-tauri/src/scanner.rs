use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use crate::db::{LibrarySourceRoot, Repository};
use crate::error::{AppError, AppResult};
use crate::imaging::process_image_with_source_bytes;
use crate::models::{FileSnapshot, ProcessedImage, ScanPerformance, ScanProgress, ScanSummary};
use crate::source_identity::{
    SourceIdentity, existing_identity, identity_key, is_same_or_descendant,
};

const IMPORT_IMAGE_WORKERS: usize = 2;

struct PendingImageWork {
    path: PathBuf,
    snapshot: FileSnapshot,
    library_id: i64,
    generation: i64,
}

struct ImageWorkResult {
    path: PathBuf,
    snapshot: FileSnapshot,
    library_id: i64,
    generation: i64,
    fingerprint: Option<String>,
    processed: Option<ProcessedImage>,
    error: Option<String>,
    fingerprint_us: u64,
    image_processing_us: u64,
}

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
const SCAN_PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(120);
const SCAN_PROGRESS_DB_INTERVAL: Duration = Duration::from_millis(300);
const SCAN_PROGRESS_DB_BATCH: u64 = 32;

struct FingerprintedSource {
    fingerprint: String,
    bytes: Option<Vec<u8>>,
}

struct ScanProgressReporter<'a, F> {
    repository: &'a Repository,
    task_id: &'a str,
    emit: F,
    last_event_at: Instant,
    last_persist_at: Instant,
    last_persisted: u64,
}

impl<'a, F> ScanProgressReporter<'a, F>
where
    F: Fn(ScanProgress),
{
    fn new(repository: &'a Repository, task_id: &'a str, emit: F) -> Self {
        let now = Instant::now();
        Self {
            repository,
            task_id,
            emit,
            last_event_at: now,
            last_persist_at: now,
            last_persisted: 0,
        }
    }

    fn force(
        &mut self,
        progress: &mut ScanProgress,
        performance: &ScanPerformance,
    ) -> AppResult<()> {
        progress.performance = performance.clone();
        let now = Instant::now();
        self.repository.update_job_progress(
            self.task_id,
            progress.processed,
            progress.discovered,
        )?;
        self.last_persisted = progress.processed;
        self.last_persist_at = now;
        (self.emit)(progress.clone());
        self.last_event_at = now;
        Ok(())
    }

    fn report(
        &mut self,
        progress: &mut ScanProgress,
        performance: &ScanPerformance,
    ) -> AppResult<()> {
        progress.performance = performance.clone();
        let now = Instant::now();
        let should_persist = progress.processed.saturating_sub(self.last_persisted)
            >= SCAN_PROGRESS_DB_BATCH
            || now.duration_since(self.last_persist_at) >= SCAN_PROGRESS_DB_INTERVAL;
        if should_persist {
            self.repository.update_job_progress(
                self.task_id,
                progress.processed,
                progress.discovered,
            )?;
            self.last_persisted = progress.processed;
            self.last_persist_at = now;
        }

        if now.duration_since(self.last_event_at) >= SCAN_PROGRESS_EVENT_INTERVAL {
            (self.emit)(progress.clone());
            self.last_event_at = now;
        }
        Ok(())
    }
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
    scan_library_scope(
        repository,
        thumbnail_dir,
        &root,
        task_id,
        cancelled,
        library_id,
        generation,
        true,
        emit,
    )
}

/// Discover one Library source root for every directory that directly contains
/// at least one supported image. The selected root is always included, even if
/// it contains no images. Empty/intermediate directories remain invisible.
pub fn discover_import_source_roots(root: &Path) -> AppResult<Vec<SourceIdentity>> {
    let root_identity = existing_identity(root)?;
    let mut roots = HashMap::<String, SourceIdentity>::new();
    roots.insert(root_identity.identity_key.clone(), root_identity.clone());

    for entry in walkdir::WalkDir::new(&root_identity.source_path)
        .follow_links(false)
        .into_iter()
    {
        let entry = entry?;
        if !entry.file_type().is_file() || !is_supported_image(entry.path()) {
            continue;
        }
        let Some(parent) = entry.path().parent() else {
            continue;
        };
        let parent_identity = existing_identity(parent)?;
        roots
            .entry(parent_identity.identity_key.clone())
            .or_insert(parent_identity);
    }

    let mut roots = roots.into_values().collect::<Vec<_>>();
    roots.sort_by_key(|root| {
        (
            root.identity_key.matches('/').count(),
            root.identity_key.clone(),
        )
    });
    Ok(roots)
}

/// Scan each explicitly imported source root as an independent ownership
/// scope. The root scan owns only files not covered by an explicitly imported
/// child root; each child scope is scanned separately without creating another
/// analysis job or rescanning sibling/parent libraries.
pub fn scan_library_tree<F>(
    repository: &Repository,
    thumbnail_dir: &Path,
    root: &Path,
    task_id: &str,
    cancelled: &AtomicBool,
    targets: Vec<LibrarySourceRoot>,
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
        .unwrap_or_else(|| "Library".into());
    let (root_library_id, root_generation) = repository.begin_scan_with_identity(
        &root_string,
        &root_identity.identity_key,
        &name,
        task_id,
    )?;

    let mut ordered_targets = Vec::with_capacity(targets.len() + 1);
    ordered_targets.push((root.clone(), root_library_id, root_generation));
    let mut child_targets = targets
        .into_iter()
        .filter(|target| target.identity_key != root_identity.identity_key)
        .filter(|target| is_same_or_descendant(&root_identity.identity_key, &target.identity_key))
        .collect::<Vec<_>>();
    child_targets.sort_by_key(|target| {
        (
            target.identity_key.matches('/').count(),
            target.identity_key.clone(),
        )
    });
    for target in child_targets {
        let generation = repository.begin_existing_library_scan(target.library_id)?;
        ordered_targets.push((target.source_path, target.library_id, generation));
    }

    let mut aggregate = ScanProgress::starting(task_id);
    aggregate.library_id = Some(root_library_id);
    aggregate.stage = "discovering".into();
    emit(aggregate.clone());

    for (scope_root, library_id, generation) in ordered_targets {
        if cancelled.load(Ordering::Relaxed) {
            aggregate.status = "cancelled".into();
            aggregate.stage = "cancelled".into();
            repository.cancel_scan(task_id, root_library_id)?;
            emit(aggregate.clone());
            return Ok(summary_from_progress(&aggregate, root_library_id));
        }

        let base_progress = aggregate.clone();
        let scope_summary = scan_library_scope(
            repository,
            thumbnail_dir,
            &scope_root,
            task_id,
            cancelled,
            library_id,
            generation,
            false,
            |local| emit(aggregate_scope_progress(&base_progress, &local)),
        )?;
        add_summary_to_progress(&mut aggregate, &scope_summary);
        if scope_summary.status == "cancelled" {
            aggregate.status = "cancelled".into();
            aggregate.stage = "cancelled".into();
            repository.cancel_scan(task_id, root_library_id)?;
            emit(aggregate.clone());
            return Ok(summary_from_progress(&aggregate, root_library_id));
        }

        aggregate.stage = "processing".into();
        aggregate.status = "running".into();
        repository.update_job_progress(task_id, aggregate.processed, aggregate.discovered)?;
        emit(aggregate.clone());
    }

    // Each scope was completed independently above. This final call closes the
    // single user-visible root job without triggering any child rescan.
    repository.complete_scan(task_id, root_library_id, root_generation)?;
    aggregate.status = "completed".into();
    aggregate.stage = "completed".into();
    aggregate.current_path = None;
    aggregate.error = None;
    emit(aggregate.clone());
    Ok(summary_from_progress(&aggregate, root_library_id))
}

#[allow(clippy::too_many_arguments)]
fn scan_library_scope<F>(
    repository: &Repository,
    thumbnail_dir: &Path,
    root: &Path,
    task_id: &str,
    cancelled: &AtomicBool,
    library_id: i64,
    generation: i64,
    complete_job: bool,
    emit: F,
) -> AppResult<ScanSummary>
where
    F: Fn(ScanProgress),
{
    let descendant_roots = repository.nested_source_roots(library_id)?;
    let mut performance = ScanPerformance::default();
    let mut progress = ScanProgress::starting(task_id);
    progress.library_id = Some(library_id);
    progress.stage = "discovering".into();
    let mut reporter = ScanProgressReporter::new(repository, task_id, emit);
    reporter.force(&mut progress, &performance)?;

    let discovery_started = Instant::now();
    let mut candidates = Vec::new();
    for entry in walkdir::WalkDir::new(root)
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
                reporter.report(&mut progress, &performance)?;
            }
        }
    }
    add_elapsed(&mut performance.discovery_us, discovery_started);
    candidates.sort_by_key(|path| path_to_string(path));
    progress.discovered = candidates.len() as u64;
    progress.stage = "processing".into();
    progress.error = None;
    reporter.force(&mut progress, &performance)?;

    let mut pending_work = Vec::with_capacity(IMPORT_IMAGE_WORKERS);
    for path in candidates {
        if cancelled.load(Ordering::Relaxed) {
            progress.status = "cancelled".into();
            progress.stage = "cancelled".into();
            progress.current_path = None;
            if complete_job {
                repository.cancel_scan(task_id, library_id)?;
            } else {
                repository.cancel_library_scope(library_id)?;
            }
            reporter.force(&mut progress, &performance)?;
            return Ok(summary_from_progress(&progress, library_id));
        }

        let ownership_started = Instant::now();
        let Some(owner) = repository.resolve_library_owner(&path)? else {
            add_elapsed(&mut performance.ownership_lookup_us, ownership_started);
            continue;
        };
        add_elapsed(&mut performance.ownership_lookup_us, ownership_started);
        if owner.library_id != library_id {
            continue;
        }
        progress.current_path = Some(path_to_string(&path));

        let metadata_started = Instant::now();
        let snapshot = match snapshot_file(&owner.source_path, &path) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                add_elapsed(&mut performance.metadata_lookup_us, metadata_started);
                let fallback = fallback_snapshot(&owner.source_path, &path);
                let fingerprint = fallback_fingerprint(&fallback);
                let database_started = Instant::now();
                let database_result = repository.upsert_failed_asset(
                    library_id,
                    generation,
                    &fallback,
                    &fingerprint,
                    &error.to_string(),
                );
                add_elapsed(&mut performance.database_write_us, database_started);
                database_result?;
                progress.processed += 1;
                progress.failed += 1;
                performance.failed_files += 1;
                progress.error = Some(error.to_string());
                reporter.report(&mut progress, &performance)?;
                continue;
            }
        };

        let asset_identity_key = identity_key(Path::new(&snapshot.absolute_path));
        let existing = repository.find_existing_asset(&asset_identity_key)?;
        add_elapsed(&mut performance.metadata_lookup_us, metadata_started);
        if let Some(existing) = existing {
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
                let database_started = Instant::now();
                let database_result = repository.touch_asset_seen(
                    existing.id,
                    owner.library_id,
                    &snapshot.relative_path,
                    generation,
                );
                add_elapsed(&mut performance.database_write_us, database_started);
                database_result?;
                progress.processed += 1;
                progress.skipped += 1;
                performance.skipped_files += 1;
                progress.error = None;
                reporter.report(&mut progress, &performance)?;
                continue;
            }
        }

        pending_work.push(PendingImageWork {
            path,
            snapshot,
            library_id,
            generation,
        });
        if pending_work.len() < IMPORT_IMAGE_WORKERS {
            continue;
        }

        if let Some(next) = pending_work.last() {
            progress.current_path = Some(path_to_string(&next.path));
            reporter.force(&mut progress, &performance)?;
        }
        for result in process_image_work_batch(std::mem::take(&mut pending_work), thumbnail_dir) {
            progress.current_path = Some(path_to_string(&result.path));
            apply_image_work_result(repository, result, &mut progress, &mut performance)?;
            reporter.report(&mut progress, &performance)?;
        }
    }

    if !pending_work.is_empty() {
        if let Some(next) = pending_work.last() {
            progress.current_path = Some(path_to_string(&next.path));
            reporter.force(&mut progress, &performance)?;
        }
        for result in process_image_work_batch(pending_work, thumbnail_dir) {
            progress.current_path = Some(path_to_string(&result.path));
            apply_image_work_result(repository, result, &mut progress, &mut performance)?;
            reporter.report(&mut progress, &performance)?;
        }
    }

    progress.missing = if complete_job {
        repository.complete_scan(task_id, library_id, generation)?
    } else {
        repository.complete_library_scope(library_id, generation)?
    };
    progress.status = "completed".into();
    progress.stage = "completed".into();
    progress.current_path = None;
    progress.error = None;
    reporter.force(&mut progress, &performance)?;
    Ok(summary_from_progress(&progress, library_id))
}

fn process_image_work_batch(
    work: Vec<PendingImageWork>,
    thumbnail_dir: &Path,
) -> Vec<ImageWorkResult> {
    std::thread::scope(|scope| {
        let handles = work
            .into_iter()
            .map(|work| {
                let thumbnail_dir = thumbnail_dir.to_path_buf();
                scope.spawn(move || process_image_work(work, thumbnail_dir))
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("image worker panicked"))
            .collect()
    })
}

fn process_image_work(work: PendingImageWork, thumbnail_dir: PathBuf) -> ImageWorkResult {
    let fingerprint_started = Instant::now();
    let fingerprinted_source = match read_fingerprinted_source(&work.path) {
        Ok(value) => value,
        Err(error) => {
            return ImageWorkResult {
                path: work.path,
                snapshot: work.snapshot,
                library_id: work.library_id,
                generation: work.generation,
                fingerprint: None,
                processed: None,
                error: Some(error.to_string()),
                fingerprint_us: elapsed_us(fingerprint_started),
                image_processing_us: 0,
            };
        }
    };
    let fingerprint_us = elapsed_us(fingerprint_started);
    let image_started = Instant::now();
    let fingerprint = fingerprinted_source.fingerprint;
    let processed = process_image_with_source_bytes(
        &work.path,
        &thumbnail_dir,
        &fingerprint,
        fingerprinted_source.bytes.as_deref(),
    );
    let image_processing_us = elapsed_us(image_started);

    match processed {
        Ok(processed) => ImageWorkResult {
            path: work.path,
            snapshot: work.snapshot,
            library_id: work.library_id,
            generation: work.generation,
            fingerprint: Some(fingerprint),
            processed: Some(processed),
            error: None,
            fingerprint_us,
            image_processing_us,
        },
        Err(error) => ImageWorkResult {
            path: work.path,
            snapshot: work.snapshot,
            library_id: work.library_id,
            generation: work.generation,
            fingerprint: Some(fingerprint),
            processed: None,
            error: Some(error.to_string()),
            fingerprint_us,
            image_processing_us,
        },
    }
}

fn apply_image_work_result(
    repository: &Repository,
    result: ImageWorkResult,
    progress: &mut ScanProgress,
    performance: &mut ScanPerformance,
) -> AppResult<()> {
    performance.fingerprint_us = performance
        .fingerprint_us
        .saturating_add(result.fingerprint_us);
    performance.image_processing_us = performance
        .image_processing_us
        .saturating_add(result.image_processing_us);

    if let Some(processed) = result.processed {
        performance.exif_us = performance
            .exif_us
            .saturating_add(processed.timings.exif_us);
        performance.decode_us = performance
            .decode_us
            .saturating_add(processed.timings.decode_us);
        performance.resize_us = performance
            .resize_us
            .saturating_add(processed.timings.resize_us);
        performance.feature_analysis_us = performance
            .feature_analysis_us
            .saturating_add(processed.timings.feature_analysis_us);
        performance.thumbnail_write_us = performance
            .thumbnail_write_us
            .saturating_add(processed.timings.thumbnail_write_us);
        let fingerprint = result.fingerprint.as_deref().unwrap_or("unreadable");
        let database_started = Instant::now();
        let database_result = repository.upsert_processed_asset(
            result.library_id,
            result.generation,
            &result.snapshot,
            fingerprint,
            &processed,
        );
        add_elapsed(&mut performance.database_write_us, database_started);
        database_result?;
        progress.succeeded += 1;
        performance.processed_files += 1;
        progress.error = None;
    } else {
        let fingerprint = result
            .fingerprint
            .unwrap_or_else(|| fallback_fingerprint(&result.snapshot));
        let error = result
            .error
            .unwrap_or_else(|| "image processing failed".into());
        let database_started = Instant::now();
        let database_result = repository.upsert_failed_asset(
            result.library_id,
            result.generation,
            &result.snapshot,
            &fingerprint,
            &error,
        );
        add_elapsed(&mut performance.database_write_us, database_started);
        database_result?;
        progress.failed += 1;
        performance.failed_files += 1;
        progress.error = Some(error);
    }
    progress.processed += 1;
    Ok(())
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
        performance: progress.performance.clone(),
    }
}

fn add_summary_to_progress(progress: &mut ScanProgress, summary: &ScanSummary) {
    progress.discovered = progress.discovered.saturating_add(summary.discovered);
    progress.processed = progress.processed.saturating_add(summary.processed);
    progress.succeeded = progress.succeeded.saturating_add(summary.succeeded);
    progress.failed = progress.failed.saturating_add(summary.failed);
    progress.skipped = progress.skipped.saturating_add(summary.skipped);
    progress.missing = progress.missing.saturating_add(summary.missing);
    add_performance(&mut progress.performance, &summary.performance);
}

fn aggregate_scope_progress(base: &ScanProgress, local: &ScanProgress) -> ScanProgress {
    let mut progress = base.clone();
    progress.status = if local.status == "cancelled" {
        "cancelled".into()
    } else {
        "running".into()
    };
    progress.stage = local.stage.clone();
    progress.discovered = progress.discovered.saturating_add(local.discovered);
    progress.processed = progress.processed.saturating_add(local.processed);
    progress.succeeded = progress.succeeded.saturating_add(local.succeeded);
    progress.failed = progress.failed.saturating_add(local.failed);
    progress.skipped = progress.skipped.saturating_add(local.skipped);
    progress.missing = progress.missing.saturating_add(local.missing);
    progress.current_path = local.current_path.clone();
    progress.error = local.error.clone();
    add_performance(&mut progress.performance, &local.performance);
    progress
}

fn add_performance(total: &mut ScanPerformance, value: &ScanPerformance) {
    total.discovery_us = total.discovery_us.saturating_add(value.discovery_us);
    total.ownership_lookup_us = total
        .ownership_lookup_us
        .saturating_add(value.ownership_lookup_us);
    total.metadata_lookup_us = total
        .metadata_lookup_us
        .saturating_add(value.metadata_lookup_us);
    total.fingerprint_us = total.fingerprint_us.saturating_add(value.fingerprint_us);
    total.image_processing_us = total
        .image_processing_us
        .saturating_add(value.image_processing_us);
    total.exif_us = total.exif_us.saturating_add(value.exif_us);
    total.decode_us = total.decode_us.saturating_add(value.decode_us);
    total.resize_us = total.resize_us.saturating_add(value.resize_us);
    total.feature_analysis_us = total
        .feature_analysis_us
        .saturating_add(value.feature_analysis_us);
    total.thumbnail_write_us = total
        .thumbnail_write_us
        .saturating_add(value.thumbnail_write_us);
    total.database_write_us = total
        .database_write_us
        .saturating_add(value.database_write_us);
    total.processed_files = total.processed_files.saturating_add(value.processed_files);
    total.skipped_files = total.skipped_files.saturating_add(value.skipped_files);
    total.failed_files = total.failed_files.saturating_add(value.failed_files);
}

fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

fn add_elapsed(total: &mut u64, started: Instant) {
    *total = total.saturating_add(elapsed_us(started));
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
    fn completed_scan_reports_stage_timing_snapshot() {
        let (_temp, paths, repository, source) = setup();
        for index in 0..3 {
            save_pixel(
                &source.join(format!("image-{index}.png")),
                Rgba([20 + index as u8, 80, 160, 255]),
                (24, 16),
            );
        }
        let events = Mutex::new(Vec::new());
        let summary = scan_library(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "timing-task",
            &AtomicBool::new(false),
            |progress| events.lock().expect("events").push(progress),
        )
        .expect("scan");
        let final_progress = events
            .lock()
            .expect("events")
            .last()
            .expect("final progress")
            .clone();

        assert_eq!(summary.processed, 3);
        assert_eq!(final_progress.performance.processed_files, 3);
        assert!(final_progress.performance.discovery_us > 0);
        assert!(final_progress.performance.fingerprint_us > 0);
        assert!(final_progress.performance.image_processing_us > 0);
        assert!(final_progress.performance.database_write_us > 0);
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
        let full_assets = repository
            .list_assets_for_organization(library.id, &crate::models::AssetFilter::default(), None)
            .expect("full assets");
        assert!(
            full_assets
                .iter()
                .any(|asset| asset.relative_path == "中文 路径\\红色.JPG")
        );
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
            primary_categories: vec!["portrait".into()],
            auxiliary_tags: vec!["night".into()],
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
        assert_eq!(repeated.performance.processed_files, 0);
        assert_eq!(repeated.performance.skipped_files, 1);
        assert_eq!(repeated.performance.fingerprint_us, 0);
        assert_eq!(repeated.performance.image_processing_us, 0);

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
    fn structured_import_creates_one_library_per_image_directory_and_scans_scopes() {
        let (_temp, paths, repository, source) = setup();
        save_pixel(&source.join("root.png"), Rgba([220, 80, 30, 255]), (8, 8));
        save_pixel(
            &source.join("旅行").join("child.png"),
            Rgba([30, 80, 220, 255]),
            (8, 8),
        );
        fs::create_dir_all(source.join("空文件夹")).expect("empty directory");

        let discovered = discover_import_source_roots(&source).expect("discover roots");
        assert_eq!(discovered.len(), 2);
        let targets = repository
            .ensure_library_source_roots(&discovered)
            .expect("register roots");
        let summary = scan_library_tree(
            &repository,
            &paths.thumbnail_dir,
            &source,
            "structured-import",
            &AtomicBool::new(false),
            targets,
            |_| {},
        )
        .expect("structured scan");

        assert_eq!(summary.status, "completed");
        assert_eq!(summary.discovered, 2);
        assert_eq!(summary.processed, 2);

        let libraries = repository.list_libraries().expect("libraries");
        assert_eq!(libraries.len(), 2);
        let parent = libraries
            .iter()
            .find(|library| library.source_identity_key == identity_key(&source))
            .expect("root library");
        let child = libraries
            .iter()
            .find(|library| library.source_path.ends_with("旅行"))
            .expect("child library");
        assert_eq!(child.parent_library_id, Some(parent.id));

        let child_assets = repository
            .list_assets(
                child.id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &crate::models::AssetFilter::default(),
            )
            .expect("child assets");
        assert_eq!(child_assets.total, 1);
        assert_eq!(child_assets.items[0].library_id, child.id);
        let child_full_assets = repository
            .list_assets_for_organization(child.id, &crate::models::AssetFilter::default(), None)
            .expect("child full assets");
        assert!(child_full_assets[0].relative_path.ends_with("child.png"));
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
