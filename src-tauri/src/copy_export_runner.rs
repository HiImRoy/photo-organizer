//! Durable, handle-bound execution for prepared copy-export jobs.
//!
//! The repository owns authorization, journal state, and short SQLite
//! transactions. This module owns the serial filesystem work and never uses a
//! path-based delete fallback.

use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::db::{CopyExportJobContext, Repository};
use crate::error::{AppError, AppResult};
use crate::export_file_safety::{
    FileOperationService, FileSnapshot, GrantedParent, NewTargetWriter, OwnedOutputRoot,
    ProtectedRoot,
};
use crate::source_identity::identity_key;

const COPY_BUFFER_SIZE: usize = 64 * 1024;
const DATABASE_CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyExportProgressPhase {
    Preparing,
    VerifyingSource,
    Copying,
    RollingBack,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyExportProgress {
    pub job_id: String,
    pub phase: CopyExportProgressPhase,
    pub completed_items: usize,
    pub total_items: usize,
    pub current_ordinal: Option<u32>,
    pub current_asset_id: Option<i64>,
    pub bytes_processed: u64,
    pub total_bytes: u64,
}

fn dispatch_progress(progress: &mut impl FnMut(CopyExportProgress), update: CopyExportProgress) {
    progress(update);
}

macro_rules! emit_job_progress {
    (
        $progress:expr,
        $context:expr,
        $phase:expr,
        $completed_items:expr,
        $total_items:expr,
        $current_ordinal:expr,
        $current_asset_id:expr,
        $bytes_processed:expr,
        $total_bytes:expr $(,)?
    ) => {{
        let total_bytes = $total_bytes;
        dispatch_progress(
            $progress,
            CopyExportProgress {
                job_id: $context.job_id.clone(),
                phase: $phase,
                completed_items: $completed_items,
                total_items: $total_items,
                current_ordinal: $current_ordinal,
                current_asset_id: $current_asset_id,
                bytes_processed: ($bytes_processed).min(total_bytes),
                total_bytes,
            },
        )
    }};
}

macro_rules! emit_item_progress {
    (
        $progress:expr,
        $context:expr,
        $item:expr,
        $phase:expr,
        $completed_items:expr,
        $total_items:expr,
        $bytes_processed:expr,
        $total_bytes:expr $(,)?
    ) => {{
        let item = $item;
        emit_job_progress!(
            $progress,
            $context,
            $phase,
            $completed_items,
            $total_items,
            Some(item.ordinal),
            Some(item.asset_id),
            $bytes_processed,
            $total_bytes,
        )
    }};
}

/// Execute an already-authorized queued/reconciled job serially.
///
/// Cancellation is checked from the shared in-process atomic for every 64 KiB
/// source/copy chunk. The durable database cancel flag is refreshed at item
/// boundaries and at most once every 250 ms, avoiding a SQLite open per chunk.
pub fn run_copy_export_job<F>(
    repository: &Repository,
    job_id: &str,
    cancel: &AtomicBool,
    progress: F,
) -> AppResult<()>
where
    F: FnMut(CopyExportProgress),
{
    repository.ensure_copy_recovery_gate()?;
    run_copy_export_job_after_recovery_gate(repository, job_id, cancel, progress)
}

pub(crate) fn run_copy_export_job_after_recovery_gate<F>(
    repository: &Repository,
    job_id: &str,
    cancel: &AtomicBool,
    mut progress: F,
) -> AppResult<()>
where
    F: FnMut(CopyExportProgress),
{
    let context = repository.load_copy_export_context(job_id)?;
    validate_job_target_paths(&context)?;

    let total_bytes = context.items.iter().fold(0u64, |total, item| {
        total.saturating_add(item.source_snapshot.size_bytes)
    });
    let total_items = context.items.len();
    let mut completed_items = 0usize;
    let mut bytes_processed = 0u64;
    emit_job_progress!(
        &mut progress,
        &context,
        CopyExportProgressPhase::Preparing,
        completed_items,
        total_items,
        None,
        None,
        bytes_processed,
        total_bytes,
    );

    let mut cancellation = CopyCancellation::new(repository, job_id, cancel);
    if cancellation.check(true)? {
        repository.finish_copy_export_job(job_id)?;
        emit_job_progress!(
            &mut progress,
            &context,
            CopyExportProgressPhase::Completed,
            completed_items,
            total_items,
            None,
            None,
            bytes_processed,
            total_bytes,
        );
        return Ok(());
    }

    // The parent path and physical ID are frozen in the plan. The live lease
    // proves both still refer to that same directory before the output root is
    // created or opened.
    let parent =
        GrantedParent::grant_existing(&context.target_parent_path, &context.protected_roots)?;
    if parent.identity()? != &context.target_parent_identity
        || identity_key(parent.path()) != identity_key(&context.target_parent_path)
    {
        return Err(unsafe_job("target parent identity or final path changed"));
    }

    let latest_roots = current_protected_roots(repository)?;
    let service = FileOperationService;
    let (root, created_now) = match &context.target_root_identity {
        Some(expected) => (
            service.open_existing_output_root(
                &parent,
                std::ffi::OsStr::new(&context.target_child_name),
                expected,
                &latest_roots,
            )?,
            false,
        ),
        None => match service.create_new_root_detailed(
            &parent,
            std::ffi::OsStr::new(&context.target_child_name),
            &latest_roots,
        ) {
            Ok(root) => (root, true),
            Err(failure) => {
                if !failure.may_have_created {
                    repository
                        .mark_copy_export_root_create_failed(job_id, &failure.error.to_string())?;
                }
                // If the directory may exist, preserve the prepared root
                // intent for reconciliation. It is never removed by path.
                return Err(AppError::Io(failure.error));
            }
        },
    };
    if created_now {
        // Record the identity as soon as the helper returns a held handle. Any
        // subsequent path mismatch then remains tied to this specific object.
        repository.record_copy_export_root_created(job_id, root.identity())?;
    }
    if identity_key(root.path()) != identity_key(&context.target_root_path) {
        return Err(unsafe_job(
            "output root final path differs from frozen job path",
        ));
    }

    for item in &context.items {
        if item.partial_target_identity.is_some() {
            return Err(unsafe_job(format!(
                "copy item {} still has a journaled partial target and requires reconciliation",
                item.ordinal
            )));
        }
        match item.state.as_str() {
            "copied" => {
                completed_items += 1;
                bytes_processed = bytes_processed.saturating_add(item.source_snapshot.size_bytes);
                emit_job_progress!(
                    &mut progress,
                    &context,
                    CopyExportProgressPhase::Copying,
                    completed_items,
                    total_items,
                    Some(item.ordinal),
                    Some(item.asset_id),
                    bytes_processed,
                    total_bytes,
                );
                continue;
            }
            "pending" | "failed" | "cancelled" => {}
            state => {
                return Err(unsafe_job(format!(
                    "copy item {} requires explicit reconciliation from state {state}",
                    item.ordinal
                )));
            }
        }

        if cancellation.check(false)? {
            break;
        }

        // This transaction records the operation intent before source
        // verification and, critically, before CREATE_NEW touches the target.
        repository.mark_copy_export_item_started(job_id, item.ordinal)?;
        emit_job_progress!(
            &mut progress,
            &context,
            CopyExportProgressPhase::VerifyingSource,
            completed_items,
            total_items,
            Some(item.ordinal),
            Some(item.asset_id),
            bytes_processed,
            total_bytes,
        );

        let source_result = service.open_source_read_with_cancel(
            &item.source_path,
            &item.source_snapshot,
            &mut |_| cancellation_check_as_io(&mut cancellation),
        );
        let mut source = match source_result {
            Ok(source) => source,
            Err(error) => {
                if cancellation.check(false)? {
                    repository.mark_copy_export_item_cancelled(job_id, item.ordinal)?;
                    break;
                }
                repository.mark_copy_export_item_failed(
                    job_id,
                    item.ordinal,
                    &error.to_string(),
                    None,
                )?;
                emit_item_progress!(
                    &mut progress,
                    &context,
                    item,
                    CopyExportProgressPhase::Copying,
                    completed_items,
                    total_items,
                    bytes_processed,
                    total_bytes,
                );
                continue;
            }
        };

        if cancellation.check(false)? {
            repository.mark_copy_export_item_cancelled(job_id, item.ordinal)?;
            break;
        }
        let latest_roots = current_protected_roots(repository)?;
        let mut writer =
            match root.create_new_file_with_protected(&item.target_relative_path, &latest_roots) {
                Ok(writer) => writer,
                Err(error) => {
                    repository.mark_copy_export_item_failed(
                        job_id,
                        item.ordinal,
                        &error.to_string(),
                        None,
                    )?;
                    emit_item_progress!(
                        &mut progress,
                        &context,
                        item,
                        CopyExportProgressPhase::Copying,
                        completed_items,
                        total_items,
                        bytes_processed,
                        total_bytes,
                    );
                    continue;
                }
            };

        if let Err(journal_error) = repository.record_copy_export_item_target_created(
            job_id,
            item.ordinal,
            writer.created_identity(),
        ) {
            let latest_roots = current_protected_roots(repository)?;
            let cleanup = abort_created_writer(&mut writer, &latest_roots);
            match cleanup {
                PartialCleanup::Aborted => {
                    let _ = repository.mark_copy_export_item_aborted_failed(
                        job_id,
                        item.ordinal,
                        &journal_error.to_string(),
                    );
                }
                PartialCleanup::Preserved { error, snapshot } => {
                    if let Some(snapshot) = snapshot {
                        let _ = repository.mark_copy_export_item_failed(
                            job_id,
                            item.ordinal,
                            &format!("{journal_error}; same-handle abort failed: {error}"),
                            Some(&snapshot),
                        );
                    }
                }
            }
            return Err(journal_error);
        }

        let mut hasher = blake3::Hasher::new();
        let mut copied_bytes = 0u64;
        let copy_result = copy_source_stream(
            &mut source,
            &mut writer,
            &mut cancellation,
            &mut hasher,
            &mut copied_bytes,
            &mut |chunk_bytes, _chunk_length| {
                emit_job_progress!(
                    &mut progress,
                    &context,
                    CopyExportProgressPhase::Copying,
                    completed_items,
                    total_items,
                    Some(item.ordinal),
                    Some(item.asset_id),
                    bytes_processed.saturating_add(chunk_bytes),
                    total_bytes,
                );
            },
        );

        match copy_result {
            Ok(true) => {
                bytes_processed = bytes_processed.saturating_add(copied_bytes);
                let latest_roots = current_protected_roots(repository)?;
                match writer.abort_same_handle_with_protected(&latest_roots) {
                    Ok(()) => repository.mark_copy_export_item_cancelled(job_id, item.ordinal)?,
                    Err(abort_error) => {
                        let snapshot = writer.snapshot_partial_with_protected(&latest_roots).ok();
                        if let Some(snapshot) = snapshot {
                            repository.mark_copy_export_item_failed(
                                job_id,
                                item.ordinal,
                                &format!("cancelled; same-handle abort failed: {abort_error}"),
                                Some(&snapshot),
                            )?;
                        } else {
                            return Err(AppError::Io(abort_error));
                        }
                    }
                }
                emit_item_progress!(
                    &mut progress,
                    &context,
                    item,
                    CopyExportProgressPhase::Copying,
                    completed_items,
                    total_items,
                    bytes_processed,
                    total_bytes,
                );
                break;
            }
            Ok(false) => {}
            Err(error) => {
                bytes_processed = bytes_processed.saturating_add(copied_bytes);
                let latest_roots = current_protected_roots(repository)?;
                match abort_created_writer(&mut writer, &latest_roots) {
                    PartialCleanup::Aborted => repository.mark_copy_export_item_aborted_failed(
                        job_id,
                        item.ordinal,
                        &error.to_string(),
                    )?,
                    PartialCleanup::Preserved {
                        error: abort_error,
                        snapshot,
                    } => {
                        if let Some(snapshot) = snapshot {
                            repository.mark_copy_export_item_failed(
                                job_id,
                                item.ordinal,
                                &format!("{error}; same-handle abort failed: {abort_error}"),
                                Some(&snapshot),
                            )?;
                        } else {
                            return Err(error);
                        }
                    }
                }
                emit_item_progress!(
                    &mut progress,
                    &context,
                    item,
                    CopyExportProgressPhase::Copying,
                    completed_items,
                    total_items,
                    bytes_processed,
                    total_bytes,
                );
                continue;
            }
        }

        if cancellation.check(false)? {
            bytes_processed = bytes_processed.saturating_add(copied_bytes);
            let latest_roots = current_protected_roots(repository)?;
            match writer.abort_same_handle_with_protected(&latest_roots) {
                Ok(()) => repository.mark_copy_export_item_cancelled(job_id, item.ordinal)?,
                Err(abort_error) => {
                    let snapshot = writer.snapshot_partial_with_protected(&latest_roots).ok();
                    if let Some(snapshot) = snapshot {
                        repository.mark_copy_export_item_failed(
                            job_id,
                            item.ordinal,
                            &format!(
                                "cancelled before commit; same-handle abort failed: {abort_error}"
                            ),
                            Some(&snapshot),
                        )?;
                    } else {
                        return Err(AppError::Io(abort_error));
                    }
                }
            }
            emit_item_progress!(
                &mut progress,
                &context,
                item,
                CopyExportProgressPhase::Copying,
                completed_items,
                total_items,
                bytes_processed,
                total_bytes,
            );
            break;
        }

        bytes_processed = bytes_processed.saturating_add(copied_bytes);
        let stream_hash = hasher.finalize().to_hex().to_string();
        if copied_bytes != item.source_snapshot.size_bytes
            || stream_hash != item.source_snapshot.blake3
        {
            let latest_roots = current_protected_roots(repository)?;
            let failure = format!(
                "copied stream differs from frozen source snapshot ({} bytes, BLAKE3 {})",
                copied_bytes, stream_hash
            );
            match abort_created_writer(&mut writer, &latest_roots) {
                PartialCleanup::Aborted => repository.mark_copy_export_item_aborted_failed(
                    job_id,
                    item.ordinal,
                    &failure,
                )?,
                PartialCleanup::Preserved { error, snapshot } => {
                    if let Some(snapshot) = snapshot {
                        repository.mark_copy_export_item_failed(
                            job_id,
                            item.ordinal,
                            &format!("{failure}; same-handle abort failed: {error}"),
                            Some(&snapshot),
                        )?;
                    } else {
                        return Err(unsafe_job(format!("{failure}; abort failed: {error}")));
                    }
                }
            }
            emit_item_progress!(
                &mut progress,
                &context,
                item,
                CopyExportProgressPhase::Copying,
                completed_items,
                total_items,
                bytes_processed,
                total_bytes,
            );
            continue;
        }

        let latest_roots = current_protected_roots(repository)?;
        let committed = match writer.sync_and_snapshot_with_protected(&latest_roots) {
            Ok(committed) => committed,
            Err(error) => {
                match abort_created_writer(&mut writer, &latest_roots) {
                    PartialCleanup::Aborted => repository.mark_copy_export_item_aborted_failed(
                        job_id,
                        item.ordinal,
                        &error.to_string(),
                    )?,
                    PartialCleanup::Preserved {
                        error: abort_error,
                        snapshot,
                    } => {
                        if let Some(snapshot) = snapshot {
                            repository.mark_copy_export_item_failed(
                                job_id,
                                item.ordinal,
                                &format!("{error}; same-handle abort failed: {abort_error}"),
                                Some(&snapshot),
                            )?;
                        } else {
                            return Err(AppError::Io(error));
                        }
                    }
                }
                emit_item_progress!(
                    &mut progress,
                    &context,
                    item,
                    CopyExportProgressPhase::Copying,
                    completed_items,
                    total_items,
                    bytes_processed,
                    total_bytes,
                );
                continue;
            }
        };
        if committed.size_bytes != copied_bytes || committed.blake3 != stream_hash {
            let latest_roots = current_protected_roots(repository)?;
            let failure = "same-handle target snapshot differs from copied source stream";
            match abort_created_writer(&mut writer, &latest_roots) {
                PartialCleanup::Aborted => repository.mark_copy_export_item_aborted_failed(
                    job_id,
                    item.ordinal,
                    failure,
                )?,
                PartialCleanup::Preserved { error, snapshot } => {
                    if let Some(snapshot) = snapshot {
                        repository.mark_copy_export_item_failed(
                            job_id,
                            item.ordinal,
                            &format!("{failure}; same-handle abort failed: {error}"),
                            Some(&snapshot),
                        )?;
                    } else {
                        return Err(unsafe_job(format!("{failure}; abort failed: {error}")));
                    }
                }
            }
            emit_item_progress!(
                &mut progress,
                &context,
                item,
                CopyExportProgressPhase::Copying,
                completed_items,
                total_items,
                bytes_processed,
                total_bytes,
            );
            continue;
        }

        // A failed transaction after the durable file is complete must not
        // trigger deletion: the journal may have committed despite an
        // ambiguous I/O error. Recovery will compare the recorded target ID.
        repository.mark_copy_export_item_copied(job_id, item.ordinal, &committed)?;
        completed_items += 1;
        emit_item_progress!(
            &mut progress,
            &context,
            item,
            CopyExportProgressPhase::Copying,
            completed_items,
            total_items,
            bytes_processed,
            total_bytes,
        );
    }

    repository.finish_copy_export_job(job_id)?;
    emit_job_progress!(
        &mut progress,
        &context,
        CopyExportProgressPhase::Completed,
        completed_items,
        total_items,
        None,
        None,
        bytes_processed,
        total_bytes,
    );
    Ok(())
}

/// Roll back only committed job outputs, after the repository has claimed the
/// rollback state and each item has a durable prepared intent.
pub fn run_copy_export_rollback_job<F>(
    repository: &Repository,
    job_id: &str,
    cancel: &AtomicBool,
    progress: F,
) -> AppResult<()>
where
    F: FnMut(CopyExportProgress),
{
    repository.ensure_copy_recovery_gate()?;
    run_copy_export_rollback_job_after_recovery_gate(repository, job_id, cancel, progress)
}

pub(crate) fn run_copy_export_rollback_job_after_recovery_gate<F>(
    repository: &Repository,
    job_id: &str,
    cancel: &AtomicBool,
    mut progress: F,
) -> AppResult<()>
where
    F: FnMut(CopyExportProgress),
{
    repository.begin_copy_export_rollback(job_id)?;
    let context = repository.load_copy_export_context(job_id)?;
    validate_job_target_paths(&context)?;
    let rollback_items = context
        .items
        .iter()
        .filter(|item| item.state == "copied")
        .collect::<Vec<_>>();
    let total_bytes = rollback_items.iter().fold(0u64, |total, item| {
        total.saturating_add(
            item.output_identity
                .as_ref()
                .map_or(0, |output| output.size_bytes),
        )
    });
    let total_items = rollback_items.len();
    let mut completed_items = 0usize;
    let mut bytes_processed = 0u64;
    emit_job_progress!(
        &mut progress,
        &context,
        CopyExportProgressPhase::RollingBack,
        completed_items,
        total_items,
        None,
        None,
        bytes_processed,
        total_bytes,
    );

    let setup = open_rollback_root(repository, &context);
    let (parent, root) = match setup {
        Ok(root) => root,
        Err(error) => {
            for item in rollback_items {
                repository.mark_copy_export_rollback_item_started(job_id, item.ordinal)?;
                repository.mark_copy_export_rollback_item(
                    job_id,
                    item.ordinal,
                    false,
                    Some(&error.to_string()),
                )?;
                emit_item_progress!(
                    &mut progress,
                    &context,
                    item,
                    CopyExportProgressPhase::RollingBack,
                    completed_items,
                    total_items,
                    bytes_processed,
                    total_bytes,
                );
            }
            repository.finish_copy_export_rollback(job_id)?;
            return Err(error);
        }
    };

    let service = FileOperationService;
    for item in rollback_items {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        repository.mark_copy_export_rollback_item_started(job_id, item.ordinal)?;

        let latest_roots = match current_protected_roots(repository) {
            Ok(roots) => roots,
            Err(error) => {
                repository.mark_copy_export_rollback_item(
                    job_id,
                    item.ordinal,
                    false,
                    Some(&error.to_string()),
                )?;
                break;
            }
        };
        let Some(expected) = &item.output_identity else {
            repository.mark_copy_export_rollback_item(
                job_id,
                item.ordinal,
                false,
                Some("committed output identity is missing"),
            )?;
            emit_item_progress!(
                &mut progress,
                &context,
                item,
                CopyExportProgressPhase::RollingBack,
                completed_items,
                total_items,
                bytes_processed,
                total_bytes,
            );
            continue;
        };

        let mut bytes_hashed = 0u64;
        let delete_result = service.delete_if_unchanged_same_handle_with_cancel(
            &root,
            &item.target_relative_path,
            expected,
            &latest_roots,
            &mut |bytes| {
                bytes_hashed = bytes;
                if cancel.load(Ordering::Acquire) {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "rollback cancelled",
                    ));
                }
                emit_job_progress!(
                    &mut progress,
                    &context,
                    CopyExportProgressPhase::RollingBack,
                    completed_items,
                    total_items,
                    Some(item.ordinal),
                    Some(item.asset_id),
                    bytes_processed.saturating_add(bytes),
                    total_bytes,
                );
                Ok(())
            },
        );
        let (deleted, error) = match delete_result {
            Ok(()) => (true, None),
            Err(error) => (false, Some(error.to_string())),
        };
        repository.mark_copy_export_rollback_item(
            job_id,
            item.ordinal,
            deleted,
            error.as_deref(),
        )?;
        if deleted {
            bytes_processed = bytes_processed.saturating_add(expected.size_bytes);
        } else {
            bytes_processed = bytes_processed.saturating_add(bytes_hashed.min(expected.size_bytes));
        }
        if deleted {
            completed_items += 1;
        }
        emit_item_progress!(
            &mut progress,
            &context,
            item,
            CopyExportProgressPhase::RollingBack,
            completed_items,
            total_items,
            bytes_processed,
            total_bytes,
        );
        if cancel.load(Ordering::Acquire) {
            break;
        }
    }

    repository.finish_copy_export_rollback(job_id)?;
    emit_job_progress!(
        &mut progress,
        &context,
        CopyExportProgressPhase::Completed,
        completed_items,
        total_items,
        None,
        None,
        bytes_processed,
        total_bytes,
    );
    drop(root);
    drop(parent);
    Ok(())
}

struct CopyCancellation<'a> {
    repository: &'a Repository,
    job_id: &'a str,
    process_flag: &'a AtomicBool,
    last_database_check: Option<Instant>,
    database_cancelled: bool,
}

impl<'a> CopyCancellation<'a> {
    fn new(repository: &'a Repository, job_id: &'a str, process_flag: &'a AtomicBool) -> Self {
        Self {
            repository,
            job_id,
            process_flag,
            last_database_check: None,
            database_cancelled: false,
        }
    }

    fn check(&mut self, force_database_check: bool) -> AppResult<bool> {
        if self.process_flag.load(Ordering::Acquire) || self.database_cancelled {
            return Ok(true);
        }
        let now = Instant::now();
        if force_database_check
            || self
                .last_database_check
                .is_none_or(|last| now.duration_since(last) >= DATABASE_CANCEL_POLL_INTERVAL)
        {
            self.database_cancelled = self
                .repository
                .is_copy_export_cancel_requested(self.job_id)?;
            self.last_database_check = Some(now);
        }
        Ok(self.database_cancelled)
    }
}

fn cancellation_check_as_io(cancellation: &mut CopyCancellation<'_>) -> io::Result<()> {
    match cancellation.check(false) {
        Ok(true) => Err(io::Error::new(io::ErrorKind::Interrupted, "copy cancelled")),
        Ok(false) => Ok(()),
        Err(error) => Err(io::Error::other(error.to_string())),
    }
}

fn copy_source_stream(
    source: &mut impl Read,
    writer: &mut NewTargetWriter,
    cancellation: &mut CopyCancellation<'_>,
    hasher: &mut blake3::Hasher,
    total: &mut u64,
    mut on_progress: impl FnMut(u64, usize),
) -> AppResult<bool> {
    let mut buffer = [0u8; COPY_BUFFER_SIZE];
    loop {
        if cancellation.check(false)? {
            return Ok(true);
        }
        let read = source.read(&mut buffer)?;
        if read == 0 {
            return Ok(false);
        }
        let next_total = total
            .checked_add(read as u64)
            .ok_or_else(|| AppError::InvalidArgument("source file is too large".into()))?;
        writer.write_chunk(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        *total = next_total;
        on_progress(*total, read);
    }
}

enum PartialCleanup {
    Aborted,
    Preserved {
        error: io::Error,
        snapshot: Option<FileSnapshot>,
    },
}

fn abort_created_writer(
    writer: &mut NewTargetWriter,
    latest_roots: &[ProtectedRoot],
) -> PartialCleanup {
    match writer.abort_same_handle_with_protected(latest_roots) {
        Ok(()) => PartialCleanup::Aborted,
        Err(error) => {
            let snapshot = writer.snapshot_partial_with_protected(latest_roots).ok();
            PartialCleanup::Preserved { error, snapshot }
        }
    }
}

fn current_protected_roots(repository: &Repository) -> AppResult<Vec<ProtectedRoot>> {
    let app_data_path = repository
        .database_path()
        .parent()
        .ok_or_else(|| unsafe_job("database path has no application-data parent"))?;
    repository.current_protected_roots(app_data_path)
}

fn open_rollback_root(
    repository: &Repository,
    context: &CopyExportJobContext,
) -> AppResult<(GrantedParent, OwnedOutputRoot)> {
    validate_job_target_paths(context)?;
    let parent =
        GrantedParent::grant_existing(&context.target_parent_path, &context.protected_roots)?;
    if parent.identity()? != &context.target_parent_identity
        || identity_key(parent.path()) != identity_key(&context.target_parent_path)
    {
        return Err(unsafe_job("target parent identity or final path changed"));
    }
    let expected_root = context
        .target_root_identity
        .as_ref()
        .ok_or_else(|| unsafe_job("output root identity was never recorded"))?;
    let latest_roots = current_protected_roots(repository)?;
    let root = FileOperationService.open_existing_output_root(
        &parent,
        std::ffi::OsStr::new(&context.target_child_name),
        expected_root,
        &latest_roots,
    )?;
    if identity_key(root.path()) != identity_key(&context.target_root_path) {
        return Err(unsafe_job(
            "output root final path differs from frozen job path",
        ));
    }
    Ok((parent, root))
}

fn validate_job_target_paths(context: &CopyExportJobContext) -> AppResult<()> {
    let child = Path::new(&context.target_child_name);
    let mut components = child.components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
        || context
            .target_child_name
            .chars()
            .any(|character| matches!(character, '/' | '\\' | ':'))
    {
        return Err(unsafe_job(
            "target child name must be one plain path component",
        ));
    }
    let expected_root = context.target_parent_path.join(&context.target_child_name);
    if identity_key(&expected_root) != identity_key(&context.target_root_path) {
        return Err(unsafe_job(
            "target root is inconsistent with its frozen parent and child",
        ));
    }
    Ok(())
}

fn unsafe_job(message: impl Into<String>) -> AppError {
    AppError::InvalidArgument(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::fs;
    #[cfg(windows)]
    use std::io::Cursor;
    use std::path::PathBuf;
    #[cfg(windows)]
    use std::sync::atomic::AtomicBool;
    #[cfg(windows)]
    use tempfile::TempDir;

    fn job_context(parent: &Path, child: &str, root: &Path) -> CopyExportJobContext {
        CopyExportJobContext {
            job_id: "job".into(),
            plan_id: "plan".into(),
            target_parent_path: parent.to_path_buf(),
            target_parent_identity: crate::export_file_safety::DurableFileIdentity {
                volume_serial: 0,
                file_index_high: 0,
                file_index_low: 0,
                creation_filetime: 0,
            },
            target_child_name: child.into(),
            target_root_path: root.to_path_buf(),
            target_root_identity: None,
            protected_roots: Vec::new(),
            items: Vec::new(),
        }
    }

    #[cfg(windows)]
    fn fixture_dir() -> TempDir {
        let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-data");
        tempfile::Builder::new()
            .prefix("copy runner 中文 Пример café ")
            .tempdir_in(scratch)
            .expect("test-owned temporary directory")
    }

    #[test]
    fn progress_never_reports_more_than_total_bytes() {
        let context = CopyExportJobContext {
            job_id: "job".into(),
            plan_id: "plan".into(),
            target_parent_path: PathBuf::from("parent"),
            target_parent_identity: crate::export_file_safety::DurableFileIdentity {
                volume_serial: 0,
                file_index_high: 0,
                file_index_low: 0,
                creation_filetime: 0,
            },
            target_child_name: "child".into(),
            target_root_path: PathBuf::from("parent/child"),
            target_root_identity: None,
            protected_roots: Vec::new(),
            items: Vec::new(),
        };
        let mut last = None;
        emit_job_progress!(
            &mut |update| last = Some(update),
            &context,
            CopyExportProgressPhase::Copying,
            0,
            0,
            None,
            None,
            12,
            7,
        );
        assert_eq!(last.expect("progress").bytes_processed, 7);
    }

    #[test]
    fn target_root_contract_accepts_unicode_and_rejects_child_traversal() {
        let parent = PathBuf::from(r"E:\fixtures\父 folder");
        let good = job_context(&parent, "导出 Пример", &parent.join("导出 Пример"));
        assert!(validate_job_target_paths(&good).is_ok());

        let traversal = job_context(&parent, "..\\outside", &parent.join("outside"));
        assert!(validate_job_target_paths(&traversal).is_err());
        let nested = job_context(&parent, "folder\\child", &parent.join("folder/child"));
        assert!(validate_job_target_paths(&nested).is_err());
    }

    #[cfg(windows)]
    fn create_test_writer(
        temp: &TempDir,
    ) -> (
        Vec<ProtectedRoot>,
        GrantedParent,
        OwnedOutputRoot,
        NewTargetWriter,
    ) {
        let source_root = temp.path().join("图库 source");
        let target_parent = temp.path().join("target parent");
        fs::create_dir(&source_root).expect("fixture source root");
        fs::create_dir(&target_parent).expect("fixture output parent");
        let protected = vec![ProtectedRoot::from_existing(&source_root).expect("source root")];
        let parent = GrantedParent::grant_existing(&target_parent, &protected).expect("parent");
        let root = FileOperationService
            .create_new_root(&parent, std::ffi::OsStr::new("导出 Пример"), &protected)
            .expect("output root");
        let writer = root
            .create_new_file(Path::new("2026/旅行 café.webp"))
            .expect("output writer");
        (protected, parent, root, writer)
    }

    #[cfg(windows)]
    #[test]
    fn copy_stream_writes_real_target_in_bounded_chunks_with_frozen_hash() {
        let temp = fixture_dir();
        let (protected, parent, root, mut writer) = create_test_writer(&temp);
        let bytes = (0..COPY_BUFFER_SIZE * 3 + 9)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let repository = Repository::new(temp.path().join("unused-catalog.sqlite3"));
        let cancel = AtomicBool::new(false);
        let mut cancellation = CopyCancellation {
            repository: &repository,
            job_id: "unused",
            process_flag: &cancel,
            last_database_check: Some(Instant::now()),
            database_cancelled: false,
        };
        let mut source = Cursor::new(bytes.clone());
        let mut hasher = blake3::Hasher::new();
        let mut total = 0;
        let mut largest_chunk = 0usize;
        let cancelled = copy_source_stream(
            &mut source,
            &mut writer,
            &mut cancellation,
            &mut hasher,
            &mut total,
            |_processed, chunk_length| largest_chunk = largest_chunk.max(chunk_length),
        )
        .expect("bounded copy stream");
        assert!(!cancelled);
        assert_eq!(total, bytes.len() as u64);
        assert!(largest_chunk <= COPY_BUFFER_SIZE);
        assert_eq!(
            hasher.finalize().to_hex().to_string(),
            blake3::hash(&bytes).to_hex().to_string()
        );

        let committed = writer
            .sync_and_snapshot_with_protected(&protected)
            .expect("same-handle output snapshot");
        assert_eq!(committed.size_bytes, bytes.len() as u64);
        assert_eq!(committed.blake3, blake3::hash(&bytes).to_hex().to_string());
        assert_eq!(
            fs::read(&committed.final_path).expect("copied bytes"),
            bytes
        );
        drop(writer);
        drop(root);
        drop(parent);
    }

    #[cfg(windows)]
    #[test]
    fn atomic_cancel_before_chunk_aborts_only_held_writer() {
        let temp = fixture_dir();
        let (protected, parent, root, mut writer) = create_test_writer(&temp);
        let relative = Path::new("2026/旅行 café.webp");
        let target_path = root.path().join(relative);
        let repository = Repository::new(temp.path().join("unused-catalog.sqlite3"));
        let cancel = AtomicBool::new(true);
        let mut cancellation = CopyCancellation {
            repository: &repository,
            job_id: "unused",
            process_flag: &cancel,
            last_database_check: Some(Instant::now()),
            database_cancelled: false,
        };
        let mut source = Cursor::new(&b"must not be copied"[..]);
        let mut hasher = blake3::Hasher::new();
        let mut total = 0;
        assert!(
            copy_source_stream(
                &mut source,
                &mut writer,
                &mut cancellation,
                &mut hasher,
                &mut total,
                |_, _| {},
            )
            .expect("cancellation boundary")
        );
        writer
            .abort_same_handle_with_protected(&protected)
            .expect("same-handle cancel abort");
        drop(writer);
        assert!(!target_path.exists());
        drop(root);
        drop(parent);
    }
}
