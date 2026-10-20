//! Handle-bound file operations for export and rollback.
//!
//! Windows writes and deletes are tied to the exact handles that were checked.
//! The directory leases block ordinary rename/delete operations on leased
//! directories, but do not claim to stop an adversarial process from moving an
//! unleased ancestor directory while an operation is in progress.

use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use crate::source_identity::{identity_key, paths_overlap};

const HASH_BUFFER_SIZE: usize = 64 * 1024;

/// Stable Windows file identity. It comes from the opened handle's filesystem
/// metadata, never from a path, timestamp, or content hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableFileIdentity {
    #[serde(alias = "volume_serial")]
    pub volume_serial: u32,
    #[serde(alias = "file_index_high")]
    pub file_index_high: u32,
    #[serde(alias = "file_index_low")]
    pub file_index_low: u32,
    /// Raw FILETIME creation timestamp.
    #[serde(
        alias = "creation_filetime",
        serialize_with = "serialize_u64_as_decimal_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub creation_filetime: u64,
}

/// Persistable file facts. Identity, size, modified time, and BLAKE3 are kept
/// as independent checks so callers can retain the complete preview snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSnapshot {
    /// `None` only on platforms where this helper cannot obtain the Windows
    /// durable file ID. Such a snapshot cannot authorize a write or delete.
    pub identity: Option<DurableFileIdentity>,
    #[serde(alias = "size_bytes")]
    pub size_bytes: u64,
    /// Raw FILETIME on Windows; nanoseconds since Unix epoch elsewhere.
    #[serde(
        alias = "modified_time",
        serialize_with = "serialize_u64_as_decimal_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub modified_time: u64,
    /// Lowercase hexadecimal BLAKE3 digest of the complete byte stream.
    pub blake3: String,
}

/// Metadata-only source information for plan construction. The full source
/// fingerprint comes from the existing catalog scan and is checked later by
/// `open_source_read` during execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFileMetadata {
    pub identity: Option<DurableFileIdentity>,
    #[serde(alias = "size_bytes")]
    pub size_bytes: u64,
    #[serde(
        alias = "modified_time",
        serialize_with = "serialize_u64_as_decimal_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub modified_time: u64,
}

/// An existing protected source or application-data directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedRoot {
    pub path: PathBuf,
    pub identity: Option<DurableFileIdentity>,
}

/// Failure while creating a new output directory. `may_have_created` is true
/// once the exclusive directory creation succeeded and any later handle or
/// boundary check failed; callers must leave the prepared root intent for
/// reconciliation in that case.
#[derive(Debug)]
pub struct OutputRootCreateError {
    pub error: io::Error,
    pub may_have_created: bool,
}

impl From<io::Error> for OutputRootCreateError {
    fn from(error: io::Error) -> Self {
        Self {
            error,
            may_have_created: false,
        }
    }
}

impl ProtectedRoot {
    /// Resolve and record a non-reparse directory using an open directory
    /// handle on Windows.
    pub fn from_existing(path: &Path) -> io::Result<Self> {
        #[cfg(windows)]
        {
            let file = open_directory_lease(path)?;
            let metadata = file.metadata()?;
            ensure_directory_metadata(path, &metadata)?;
            let identity = durable_identity_from_handle(&file)?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Windows directory has no durable file ID",
                )
            })?;
            Ok(Self {
                path: final_path_by_handle(&file)?,
                identity: Some(identity),
            })
        }
        #[cfg(not(windows))]
        {
            let metadata = fs::symlink_metadata(path)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(unsafe_path("protected root must be a real directory"));
            }
            Ok(Self {
                path: fs::canonicalize(path)?,
                identity: None,
            })
        }
    }
}

/// Stateless entry point. It intentionally exposes no path-based delete API.
#[derive(Debug, Default, Clone, Copy)]
pub struct FileOperationService;

impl FileOperationService {
    /// Freeze source bytes and metadata from one read-only handle. This only
    /// reads bytes for a streaming BLAKE3 fingerprint; it does not decode them.
    pub fn snapshot_source(&self, path: &Path) -> io::Result<FileSnapshot> {
        let mut file = open_source_file(path)?;
        snapshot_from_handle(&mut file)
    }

    /// Read only filesystem metadata from the same kind of read-only handle;
    /// this is the low-cost path for immutable plan construction.
    pub fn inspect_source_metadata(&self, path: &Path) -> io::Result<SourceFileMetadata> {
        let file = open_source_file(path)?;
        source_metadata_from_handle(&file)
    }

    /// Read only a file's durable identity using a metadata-only handle that
    /// shares read, write, and delete access. This is intended for the live
    /// SQLite catalog, whose ordinary connections may remain open while an
    /// authorization is checked; source-copy verification still uses the
    /// stricter read lease and complete same-handle BLAKE3 snapshot.
    pub fn inspect_shared_file_identity(
        &self,
        path: &Path,
    ) -> io::Result<Option<DurableFileIdentity>> {
        let file = open_metadata_file_shared(path)?;
        let metadata = file.metadata()?;
        ensure_regular_file_metadata(path, &metadata)?;
        durable_identity_from_handle(&file)
    }

    /// Cancellable form of `snapshot_source`. The callback runs before reading
    /// and after each 64 KiB buffer with the number of bytes processed.
    pub fn snapshot_source_with_cancel(
        &self,
        path: &Path,
        cancel_check: &mut impl FnMut(u64) -> io::Result<()>,
    ) -> io::Result<FileSnapshot> {
        let mut file = open_source_file(path)?;
        snapshot_from_handle_with_cancel(&mut file, cancel_check)
    }

    /// Open one read-only source handle, verify its entire frozen snapshot, and
    /// rewind that same handle for the eventual copy stream.
    pub fn open_source_read(
        &self,
        path: &Path,
        expected: &FileSnapshot,
    ) -> io::Result<SourceReadLease> {
        self.open_source_read_with_cancel(path, expected, &mut |_| Ok(()))
    }

    /// Cancellable form of `open_source_read`, with the same expected-snapshot
    /// contract and same-handle verification.
    pub fn open_source_read_with_cancel(
        &self,
        path: &Path,
        expected: &FileSnapshot,
        cancel_check: &mut impl FnMut(u64) -> io::Result<()>,
    ) -> io::Result<SourceReadLease> {
        let mut file = open_source_file(path)?;
        let actual = snapshot_from_handle_with_cancel(&mut file, cancel_check)?;
        verify_snapshot(expected, &actual)?;
        file.seek(SeekFrom::Start(0))?;
        Ok(SourceReadLease { file })
    }

    /// Create a new output child below a previously granted parent directory.
    /// The child must be one plain path component and must not exist.
    pub fn create_new_root(
        &self,
        parent: &GrantedParent,
        child_name: &OsStr,
        protected_roots: &[ProtectedRoot],
    ) -> io::Result<OwnedOutputRoot> {
        self.create_new_root_detailed(parent, child_name, protected_roots)
            .map_err(|failure| failure.error)
    }

    /// Detailed form for the journal runner, which distinguishes a proven
    /// pre-create failure from a failure after CREATE_NEW may have succeeded.
    pub fn create_new_root_detailed(
        &self,
        parent: &GrantedParent,
        child_name: &OsStr,
        protected_roots: &[ProtectedRoot],
    ) -> Result<OwnedOutputRoot, OutputRootCreateError> {
        #[cfg(windows)]
        {
            validate_component(child_name)?;
            verify_parent(parent)?;
            let protected = merge_protected(&parent.protected_roots, protected_roots)?;
            validate_protected_roots(&protected)?;

            let candidate = parent.final_path.join(child_name);
            reject_protected_overlap(&candidate, &protected)?;
            // create_dir is an exclusive CREATE_NEW equivalent for directories.
            // If validation after this point fails, leave the directory isolated;
            // never attempt a path-based cleanup.
            fs::create_dir(&candidate)?;
            let post_create = |error| OutputRootCreateError {
                error,
                may_have_created: true,
            };
            let file = open_directory_lease(&candidate).map_err(post_create)?;
            let metadata = file.metadata().map_err(post_create)?;
            ensure_directory_metadata(&candidate, &metadata).map_err(post_create)?;
            let identity = durable_identity_from_handle(&file)
                .map_err(post_create)?
                .ok_or_else(|| post_create(unsupported("output root has no durable file ID")))?;
            let final_path = final_path_by_handle(&file).map_err(post_create)?;
            verify_direct_child(&parent.final_path, child_name, &final_path)
                .map_err(post_create)?;
            verify_parent(parent).map_err(post_create)?;
            reject_protected_overlap(&final_path, &protected).map_err(post_create)?;
            Ok(OwnedOutputRoot {
                file,
                identity,
                final_path,
                protected_roots: protected,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (parent, child_name, protected_roots);
            Err(unsupported("safe output-root creation requires Windows handle semantics").into())
        }
    }

    /// Open an existing job root only when its durable physical identity still
    /// matches the recorded identity. This does not claim or repair unknown roots.
    pub fn open_existing_output_root(
        &self,
        parent: &GrantedParent,
        child_name: &OsStr,
        expected_root_identity: &DurableFileIdentity,
        protected_roots: &[ProtectedRoot],
    ) -> io::Result<OwnedOutputRoot> {
        #[cfg(windows)]
        {
            validate_component(child_name)?;
            verify_parent(parent)?;
            let protected = merge_protected(&parent.protected_roots, protected_roots)?;
            validate_protected_roots(&protected)?;
            let candidate = parent.final_path.join(child_name);
            let file = open_directory_lease(&candidate)?;
            let metadata = file.metadata()?;
            ensure_directory_metadata(&candidate, &metadata)?;
            let identity = durable_identity_from_handle(&file)?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "output root has no durable file ID",
                )
            })?;
            if &identity != expected_root_identity {
                return Err(unsafe_path("existing output root identity changed"));
            }
            let final_path = final_path_by_handle(&file)?;
            verify_direct_child(&parent.final_path, child_name, &final_path)?;
            verify_parent(parent)?;
            reject_protected_overlap(&final_path, &protected)?;
            Ok(OwnedOutputRoot {
                file,
                identity,
                final_path,
                protected_roots: protected,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (parent, child_name, expected_root_identity, protected_roots);
            Err(unsupported(
                "safe output-root leases require Windows handle semantics",
            ))
        }
    }

    /// Read a target through a fresh read-only handle and return its current
    /// snapshot. The handle path must still be below the verified output root.
    pub fn inspect_target(
        &self,
        root: &OwnedOutputRoot,
        relative_path: &Path,
    ) -> io::Result<FileSnapshot> {
        self.inspect_target_with_protected(root, relative_path, &[])
    }

    pub fn inspect_target_with_protected(
        &self,
        root: &OwnedOutputRoot,
        relative_path: &Path,
        latest_protected_roots: &[ProtectedRoot],
    ) -> io::Result<FileSnapshot> {
        #[cfg(windows)]
        {
            let protected = merge_protected(&root.protected_roots, latest_protected_roots)?;
            verify_output_root(root, &protected)?;
            let path = target_path(root, relative_path)?;
            let _directory_leases = open_existing_parent_chain(root, relative_path)?;
            let mut file = open_source_file(&path)?;
            let snapshot = snapshot_from_handle(&mut file)?;
            let final_path = final_path_by_handle(&file)?;
            verify_target_final_path(root, &final_path, &protected)?;
            Ok(snapshot)
        }
        #[cfg(not(windows))]
        {
            let _ = (root, relative_path, latest_protected_roots);
            Err(unsupported(
                "safe output inspection requires a Windows root lease",
            ))
        }
    }

    /// Delete only a previously committed export whose identity, BLAKE3, final
    /// path, and output root still match, using the exact verified file handle.
    pub fn delete_if_unchanged_same_handle(
        &self,
        root: &OwnedOutputRoot,
        relative_path: &Path,
        expected: &CommittedTargetIdentity,
        latest_protected_roots: &[ProtectedRoot],
    ) -> io::Result<()> {
        self.delete_if_unchanged_same_handle_with_cancel(
            root,
            relative_path,
            expected,
            latest_protected_roots,
            &mut |_| Ok(()),
        )
    }

    /// Cancellable rollback form. Cancellation is checked before hashing and
    /// after every 64 KiB read; the disposition is set only after a complete
    /// successful verification.
    pub fn delete_if_unchanged_same_handle_with_cancel(
        &self,
        root: &OwnedOutputRoot,
        relative_path: &Path,
        expected: &CommittedTargetIdentity,
        latest_protected_roots: &[ProtectedRoot],
        cancel_check: &mut impl FnMut(u64) -> io::Result<()>,
    ) -> io::Result<()> {
        #[cfg(windows)]
        {
            let protected = merge_protected(&root.protected_roots, latest_protected_roots)?;
            verify_output_root(root, &protected)?;
            let path = target_path(root, relative_path)?;
            let _directory_leases = open_existing_parent_chain(root, relative_path)?;
            let mut file = open_target_for_delete(&path)?;
            let snapshot = snapshot_from_handle_with_cancel(&mut file, cancel_check)?;
            let identity = snapshot.identity.as_ref().ok_or_else(|| {
                io::Error::new(io::ErrorKind::Unsupported, "target has no durable file ID")
            })?;
            let final_path = final_path_by_handle(&file)?;
            verify_target_final_path(root, &final_path, &protected)?;
            if identity != &expected.identity
                || snapshot.size_bytes != expected.size_bytes
                || snapshot.blake3 != expected.blake3
                || identity_key(&final_path) != identity_key(&expected.final_path)
            {
                return Err(unsafe_path("target identity, bytes, or final path changed"));
            }
            cancel_check(snapshot.size_bytes)?;
            set_delete_disposition(&file)?;
            drop(file);
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = (
                root,
                relative_path,
                expected,
                latest_protected_roots,
                cancel_check,
            );
            Err(unsupported(
                "handle-bound deletion is unavailable on this platform",
            ))
        }
    }
}

/// A source file opened once and verified from that exact handle. Implements
/// `Read`/`Seek` so the copy pipeline can stream bytes without reopening a path.
pub struct SourceReadLease {
    file: File,
}

impl Read for SourceReadLease {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl Seek for SourceReadLease {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

/// Handle lease for an existing export parent directory.
pub struct GrantedParent {
    file: File,
    identity: Option<DurableFileIdentity>,
    final_path: PathBuf,
    protected_roots: Vec<ProtectedRoot>,
}

impl GrantedParent {
    /// Final path resolved from the granted directory handle.
    pub fn path(&self) -> &Path {
        &self.final_path
    }

    /// Durable physical identity captured from the granted handle. This is
    /// absent only on non-Windows builds, where it cannot authorize writes.
    pub fn identity(&self) -> io::Result<&DurableFileIdentity> {
        self.identity.as_ref().ok_or_else(|| {
            unsupported("durable directory identity is unavailable on this platform")
        })
    }

    /// Open and freeze a directory grant. The handle is kept open to prevent
    /// ordinary rename/delete of this parent while a job uses it.
    pub fn grant_existing(
        path: &Path,
        protected_roots: &[ProtectedRoot],
    ) -> io::Result<GrantedParent> {
        #[cfg(windows)]
        {
            let file = open_directory_lease(path)?;
            let metadata = file.metadata()?;
            ensure_directory_metadata(path, &metadata)?;
            let identity = durable_identity_from_handle(&file)?;
            let final_path = final_path_by_handle(&file)?;
            let protected = merge_protected(&[], protected_roots)?;
            validate_protected_roots(&protected)?;
            Ok(GrantedParent {
                file,
                identity,
                final_path,
                protected_roots: protected,
            })
        }
        #[cfg(not(windows))]
        {
            let metadata = fs::symlink_metadata(path)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(unsafe_path("target parent must be a real directory"));
            }
            let file = File::open(path)?;
            let final_path = fs::canonicalize(path)?;
            Ok(GrantedParent {
                file,
                identity: None,
                final_path,
                protected_roots: protected_roots.to_vec(),
            })
        }
    }
}

/// Verified output root lease, bound to the directory's durable handle identity.
pub struct OwnedOutputRoot {
    file: File,
    identity: DurableFileIdentity,
    final_path: PathBuf,
    protected_roots: Vec<ProtectedRoot>,
}

impl OwnedOutputRoot {
    pub fn path(&self) -> &Path {
        &self.final_path
    }

    pub fn identity(&self) -> &DurableFileIdentity {
        &self.identity
    }

    /// Create a new target file and hold its exclusive writer handle plus every
    /// parent directory handle in the relative path.
    pub fn create_new_file(&self, relative_path: &Path) -> io::Result<NewTargetWriter> {
        self.create_new_file_with_protected(relative_path, &[])
    }

    pub fn create_new_file_with_protected(
        &self,
        relative_path: &Path,
        latest_protected_roots: &[ProtectedRoot],
    ) -> io::Result<NewTargetWriter> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;

            let protected = merge_protected(&self.protected_roots, latest_protected_roots)?;
            verify_output_root(self, &protected)?;
            let components = validate_relative_path(relative_path)?;
            let target = self.final_path.join(relative_path);
            reject_protected_overlap(&target, &protected)?;

            let mut directory_leases = vec![self.file.try_clone()?];
            let mut directory_path = self.final_path.clone();
            for component in &components[..components.len() - 1] {
                directory_path.push(component);
                let directory = match open_directory_lease(&directory_path) {
                    Ok(directory) => directory,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        match fs::create_dir(&directory_path) {
                            Ok(()) => open_directory_lease(&directory_path)?,
                            Err(create_error)
                                if create_error.kind() == io::ErrorKind::AlreadyExists =>
                            {
                                open_directory_lease(&directory_path)?
                            }
                            Err(create_error) => return Err(create_error),
                        }
                    }
                    Err(error) => return Err(error),
                };
                let metadata = directory.metadata()?;
                ensure_directory_metadata(&directory_path, &metadata)?;
                let final_path = final_path_by_handle(&directory)?;
                verify_target_final_path(self, &final_path, &protected)?;
                directory_leases.push(directory);
            }

            let mut options = OpenOptions::new();
            options
                .read(true)
                .write(true)
                .create_new(true)
                .share_mode(0x0000_0001)
                .access_mode(0x8000_0000 | 0x4000_0000 | 0x0001_0000 | 0x0010_0000)
                .custom_flags(0x0020_0000 | 0x0800_0000);
            let file = options.open(&target)?;
            let metadata = file.metadata()?;
            ensure_regular_file_metadata(&target, &metadata)?;
            let created_identity = durable_identity_from_handle(&file)?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "new target has no durable file ID",
                )
            })?;
            let final_path = final_path_by_handle(&file)?;
            verify_target_final_path(self, &final_path, &protected)?;
            let root_lease = self.file.try_clone()?;
            Ok(NewTargetWriter {
                file: Some(file),
                root_lease,
                directory_leases,
                created_identity,
                root_identity: self.identity.clone(),
                root_final_path: self.final_path.clone(),
                protected_roots: protected,
                aborted: false,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (relative_path, latest_protected_roots);
            Err(unsupported(
                "safe target creation requires Windows handle semantics",
            ))
        }
    }
}

/// Exclusive writer for a newly created target. A failed abort keeps the same
/// handle alive so the caller can report the partial and retry while held.
pub struct NewTargetWriter {
    file: Option<File>,
    #[allow(dead_code)]
    root_lease: File,
    #[allow(dead_code)]
    directory_leases: Vec<File>,
    created_identity: DurableFileIdentity,
    root_identity: DurableFileIdentity,
    root_final_path: PathBuf,
    protected_roots: Vec<ProtectedRoot>,
    aborted: bool,
}

impl NewTargetWriter {
    /// Durable identity captured from the newly created writer handle. This
    /// must be persisted by the journal before the first content byte is
    /// written.
    pub fn created_identity(&self) -> &DurableFileIdentity {
        &self.created_identity
    }

    pub fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("writer is live until abort succeeds")
    }

    /// Write one bounded chunk; callers should submit at most 64 KiB per call
    /// to keep cancellation checks bounded.
    pub fn write_chunk(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > HASH_BUFFER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "write_chunk is limited to 64 KiB",
            ));
        }
        self.file_mut().write_all(bytes)
    }

    /// Flush and durably sync, then hash and identify the target from its same
    /// writer handle. The root path/identity and final path are rechecked.
    pub fn finish_and_sync(self) -> io::Result<CommittedTargetIdentity> {
        self.finish_and_sync_with_protected(&[])
    }

    pub fn finish_and_sync_with_protected(
        mut self,
        latest_protected_roots: &[ProtectedRoot],
    ) -> io::Result<CommittedTargetIdentity> {
        self.sync_and_snapshot_with_protected(latest_protected_roots)
    }

    /// Flush, sync, and snapshot from this still-held writer handle. Unlike
    /// `finish_and_sync`, this leaves the handle available for a same-handle
    /// abort if the caller's post-copy validation or journal commit fails.
    pub fn sync_and_snapshot_with_protected(
        &mut self,
        latest_protected_roots: &[ProtectedRoot],
    ) -> io::Result<CommittedTargetIdentity> {
        #[cfg(windows)]
        {
            if self.aborted {
                return Err(unsafe_path("aborted writer cannot be committed"));
            }
            let protected = merge_protected(&self.protected_roots, latest_protected_roots)?;
            if durable_identity_from_handle(&self.root_lease)?.as_ref() != Some(&self.root_identity)
                || final_path_by_handle(&self.root_lease)? != self.root_final_path
            {
                return Err(unsafe_path("output root changed while writing"));
            }
            validate_protected_roots(&protected)?;
            let file = self
                .file
                .as_mut()
                .ok_or_else(|| unsafe_path("writer handle is unavailable"))?;
            file.flush()?;
            file.sync_all()?;
            let snapshot = snapshot_from_handle(file)?;
            let identity = snapshot.identity.clone().ok_or_else(|| {
                io::Error::new(io::ErrorKind::Unsupported, "target has no durable file ID")
            })?;
            let final_path = final_path_by_handle(file)?;
            verify_final_path_below(&self.root_final_path, &final_path)?;
            reject_protected_overlap(&final_path, &protected)?;
            if identity != self.created_identity {
                return Err(unsafe_path(
                    "writer handle no longer identifies its created file",
                ));
            }
            Ok(CommittedTargetIdentity {
                identity,
                size_bytes: snapshot.size_bytes,
                blake3: snapshot.blake3,
                final_path,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = latest_protected_roots;
            Err(unsupported(
                "safe writer commit requires Windows handle semantics",
            ))
        }
    }

    /// Persist a best-effort, same-handle snapshot of a partial target before
    /// abort/reconciliation. The writer remains held so a later abort attempt
    /// still targets exactly this object.
    pub fn snapshot_partial_with_protected(
        &mut self,
        latest_protected_roots: &[ProtectedRoot],
    ) -> io::Result<FileSnapshot> {
        #[cfg(windows)]
        {
            if self.aborted {
                return Err(unsafe_path("aborted writer has no partial snapshot"));
            }
            let protected = merge_protected(&self.protected_roots, latest_protected_roots)?;
            validate_protected_roots(&protected)?;
            if durable_identity_from_handle(&self.root_lease)?.as_ref() != Some(&self.root_identity)
                || final_path_by_handle(&self.root_lease)? != self.root_final_path
            {
                return Err(unsafe_path(
                    "output root changed while snapshotting partial",
                ));
            }
            let file = self
                .file
                .as_mut()
                .ok_or_else(|| unsafe_path("writer handle is unavailable"))?;
            file.flush()?;
            file.sync_all()?;
            let snapshot = snapshot_from_handle(file)?;
            if snapshot.identity.as_ref() != Some(&self.created_identity) {
                return Err(unsafe_path("partial target identity changed"));
            }
            let final_path = final_path_by_handle(file)?;
            verify_final_path_below(&self.root_final_path, &final_path)?;
            reject_protected_overlap(&final_path, &protected)?;
            Ok(snapshot)
        }
        #[cfg(not(windows))]
        {
            let _ = latest_protected_roots;
            Err(unsupported(
                "safe partial snapshots require Windows handle semantics",
            ))
        }
    }

    /// Mark this writer's own still-held file for deletion. There is no path
    /// fallback; on failure the writer and partial remain available to caller.
    pub fn abort_same_handle(&mut self) -> io::Result<()> {
        self.abort_same_handle_with_protected(&[])
    }

    /// Same-handle abort with the latest protection snapshot. If protection
    /// changed or cannot be verified, the partial remains on disk.
    pub fn abort_same_handle_with_protected(
        &mut self,
        latest_protected_roots: &[ProtectedRoot],
    ) -> io::Result<()> {
        #[cfg(windows)]
        {
            if self.aborted {
                return Ok(());
            }
            let protected = merge_protected(&self.protected_roots, latest_protected_roots)?;
            let file = self
                .file
                .as_ref()
                .ok_or_else(|| unsafe_path("writer handle is unavailable"))?;
            let metadata = file.metadata()?;
            ensure_regular_file_metadata(Path::new("<held writer handle>"), &metadata)?;
            let identity = durable_identity_from_handle(file)?.ok_or_else(|| {
                io::Error::new(io::ErrorKind::Unsupported, "partial has no durable file ID")
            })?;
            if identity != self.created_identity {
                return Err(unsafe_path("partial target identity changed"));
            }
            let final_path = final_path_by_handle(file)?;
            verify_final_path_below(&self.root_final_path, &final_path)?;
            if durable_identity_from_handle(&self.root_lease)?.as_ref() != Some(&self.root_identity)
                || final_path_by_handle(&self.root_lease)? != self.root_final_path
            {
                return Err(unsafe_path("output root changed while aborting partial"));
            }
            validate_protected_roots(&protected)?;
            reject_protected_overlap(&final_path, &protected)?;
            set_delete_disposition(file)?;
            self.file.take();
            self.aborted = true;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = latest_protected_roots;
            Err(unsupported(
                "same-handle abort is unavailable on this platform",
            ))
        }
    }
}

impl Write for NewTargetWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file_mut().flush()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommittedTargetIdentity {
    pub identity: DurableFileIdentity,
    #[serde(alias = "size_bytes")]
    pub size_bytes: u64,
    pub blake3: String,
    #[serde(alias = "final_path")]
    pub final_path: PathBuf,
}

fn serialize_u64_as_decimal_string<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&value.to_string())
}

fn deserialize_u64_from_string_or_number<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct U64Visitor;

    impl serde::de::Visitor<'_> for U64Visitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an unsigned integer or its decimal string")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            u64::try_from(value).map_err(E::custom)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            value.parse().map_err(E::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(U64Visitor)
}

#[cfg(windows)]
fn open_source_file(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .share_mode(0x0000_0001)
        .access_mode(0x8000_0000 | 0x0010_0000)
        .custom_flags(0x0020_0000 | 0x0800_0000);
    options.open(path)
}

#[cfg(windows)]
fn open_metadata_file_shared(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    let mut options = OpenOptions::new();
    options
        .read(true)
        .share_mode(0x0000_0001 | 0x0000_0002 | 0x0000_0004)
        .access_mode(0x8000_0000 | 0x0000_0080 | 0x0010_0000)
        .custom_flags(0x0020_0000);
    options.open(path)
}

#[cfg(not(windows))]
fn open_metadata_file_shared(path: &Path) -> io::Result<File> {
    File::open(path)
}

#[cfg(not(windows))]
fn open_source_file(path: &Path) -> io::Result<File> {
    File::open(path)
}

#[cfg(windows)]
fn open_target_for_delete(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut options = OpenOptions::new();
    options
        .read(true)
        // READ|DELETE access is required for handle-bound disposition. Sharing
        // only READ prevents ordinary writes, renames, and replacement while
        // the verified handle is held.
        .share_mode(0x0000_0001)
        .access_mode(0x8000_0000 | 0x0001_0000 | 0x0010_0000)
        .custom_flags(0x0020_0000);
    options.open(path)
}

fn snapshot_from_handle(file: &mut File) -> io::Result<FileSnapshot> {
    snapshot_from_handle_with_cancel(file, &mut |_| Ok(()))
}

fn snapshot_from_handle_with_cancel(
    file: &mut File,
    cancel_check: &mut impl FnMut(u64) -> io::Result<()>,
) -> io::Result<FileSnapshot> {
    let before = file.metadata()?;
    ensure_regular_file_metadata(Path::new("<open file handle>"), &before)?;
    let identity_before = durable_identity_from_handle(file)?;
    let modified_before = modified_time(&before)?;
    let (size_bytes, hash) = hash_handle(file, cancel_check)?;
    let after = file.metadata()?;
    let identity_after = durable_identity_from_handle(file)?;
    let modified_after = modified_time(&after)?;
    if before.len() != after.len()
        || identity_before != identity_after
        || modified_before != modified_after
        || size_bytes != after.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file changed while its snapshot was being computed",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(FileSnapshot {
        identity: identity_after,
        size_bytes,
        modified_time: modified_after,
        blake3: hash,
    })
}

fn source_metadata_from_handle(file: &File) -> io::Result<SourceFileMetadata> {
    let metadata = file.metadata()?;
    ensure_regular_file_metadata(Path::new("<open source handle>"), &metadata)?;
    Ok(SourceFileMetadata {
        identity: durable_identity_from_handle(file)?,
        size_bytes: metadata.len(),
        modified_time: modified_time(&metadata)?,
    })
}

fn hash_handle(
    file: &mut File,
    cancel_check: &mut impl FnMut(u64) -> io::Result<()>,
) -> io::Result<(u64, String)> {
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; HASH_BUFFER_SIZE];
    let mut total = 0u64;
    cancel_check(total)?;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "file is too large"))?;
        hasher.update(&buffer[..read]);
        cancel_check(total)?;
    }
    use std::fmt::Write as FmtWrite;
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize().as_bytes() {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    Ok((total, hex))
}

fn verify_snapshot(expected: &FileSnapshot, actual: &FileSnapshot) -> io::Result<()> {
    if expected != actual {
        return Err(unsafe_path("source identity or fingerprint changed"));
    }
    #[cfg(windows)]
    if actual.identity.is_none() {
        return Err(unsupported("source has no durable Windows file identity"));
    }
    #[cfg(not(windows))]
    if expected.identity.is_some() {
        return Err(unsupported(
            "this platform cannot verify a Windows file identity",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn durable_identity_from_handle(file: &File) -> io::Result<Option<DurableFileIdentity>> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    let handle = HANDLE(file.as_raw_handle() as *mut _);
    unsafe { GetFileInformationByHandle(handle, &mut information) }
        .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(Some(DurableFileIdentity {
        volume_serial: information.dwVolumeSerialNumber,
        file_index_high: information.nFileIndexHigh,
        file_index_low: information.nFileIndexLow,
        creation_filetime: ((information.ftCreationTime.dwHighDateTime as u64) << 32)
            | information.ftCreationTime.dwLowDateTime as u64,
    }))
}

#[cfg(not(windows))]
fn durable_identity_from_handle(_file: &File) -> io::Result<Option<DurableFileIdentity>> {
    Ok(None)
}

#[cfg(windows)]
fn modified_time(metadata: &Metadata) -> io::Result<u64> {
    use std::os::windows::fs::MetadataExt;
    Ok(metadata.last_write_time())
}

#[cfg(not(windows))]
fn modified_time(metadata: &Metadata) -> io::Result<u64> {
    use std::time::UNIX_EPOCH;
    let duration = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(duration.as_nanos().min(u64::MAX as u128) as u64)
}

#[cfg(windows)]
fn open_directory_lease(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut options = OpenOptions::new();
    options
        .read(true)
        // Share READ|WRITE, deliberately excluding DELETE so normal rename and
        // deletion requests fail while this lease is held.
        .share_mode(0x0000_0001 | 0x0000_0002)
        .access_mode(0x0000_0080 | 0x0010_0000)
        .custom_flags(0x0200_0000 | 0x0020_0000);
    options.open(path)
}

#[cfg(windows)]
fn ensure_directory_metadata(path: &Path, metadata: &Metadata) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    if !metadata.is_dir() || metadata.file_attributes() & 0x0000_0400 != 0 {
        return Err(unsafe_path(format!(
            "reparse point or non-directory rejected: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
fn ensure_directory_metadata(path: &Path, metadata: &Metadata) -> io::Result<()> {
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(unsafe_path(format!(
            "reparse point or non-directory rejected: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn ensure_regular_file_metadata(path: &Path, metadata: &Metadata) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    if !metadata.is_file() || metadata.file_attributes() & 0x0000_0400 != 0 {
        return Err(unsafe_path(format!(
            "reparse point or non-file rejected: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
fn ensure_regular_file_metadata(path: &Path, metadata: &Metadata) -> io::Result<()> {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(unsafe_path(format!(
            "reparse point or non-file rejected: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn final_path_by_handle(file: &File) -> io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW};

    let handle = HANDLE(file.as_raw_handle() as *mut _);
    let mut buffer = vec![0u16; 512];
    loop {
        let length =
            unsafe { GetFinalPathNameByHandleW(handle, &mut buffer, FILE_NAME_NORMALIZED) };
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if (length as usize) < buffer.len() {
            return Ok(PathBuf::from(std::ffi::OsString::from_wide(
                &buffer[..length as usize],
            )));
        }
        buffer.resize(length as usize + 1, 0);
    }
}

#[cfg(windows)]
fn set_delete_disposition(file: &File) -> io::Result<()> {
    use std::mem::size_of;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
    };

    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    let handle = HANDLE(file.as_raw_handle() as *mut _);
    unsafe {
        SetFileInformationByHandle(
            handle,
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    }
    .map_err(|error| io::Error::other(error.to_string()))
}

#[cfg(windows)]
fn verify_parent(parent: &GrantedParent) -> io::Result<()> {
    let metadata = parent.file.metadata()?;
    ensure_directory_metadata(&parent.final_path, &metadata)?;
    if durable_identity_from_handle(&parent.file)? != parent.identity
        || final_path_by_handle(&parent.file)? != parent.final_path
    {
        return Err(unsafe_path("granted target parent changed"));
    }
    Ok(())
}

#[cfg(windows)]
fn verify_output_root(root: &OwnedOutputRoot, protected: &[ProtectedRoot]) -> io::Result<()> {
    let metadata = root.file.metadata()?;
    ensure_directory_metadata(&root.final_path, &metadata)?;
    if durable_identity_from_handle(&root.file)?.as_ref() != Some(&root.identity)
        || final_path_by_handle(&root.file)? != root.final_path
    {
        return Err(unsafe_path("output root identity or final path changed"));
    }
    validate_protected_roots(protected)?;
    reject_protected_overlap(&root.final_path, protected)
}

#[cfg(windows)]
fn validate_protected_roots(protected: &[ProtectedRoot]) -> io::Result<()> {
    for root in protected {
        let expected = root.identity.as_ref().ok_or_else(|| {
            unsupported("protected root is missing its durable Windows file identity")
        })?;
        let file = open_directory_lease(&root.path)?;
        let metadata = file.metadata()?;
        ensure_directory_metadata(&root.path, &metadata)?;
        if durable_identity_from_handle(&file)?.as_ref() != Some(expected)
            || identity_key(&final_path_by_handle(&file)?) != identity_key(&root.path)
        {
            return Err(unsafe_path("protected root identity or final path changed"));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn reject_protected_overlap(path: &Path, protected: &[ProtectedRoot]) -> io::Result<()> {
    let path_key = identity_key(path);
    for root in protected {
        if paths_overlap(&path_key, &identity_key(&root.path)) {
            return Err(unsafe_path(format!(
                "target overlaps a protected root: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn merge_protected(
    saved: &[ProtectedRoot],
    latest: &[ProtectedRoot],
) -> io::Result<Vec<ProtectedRoot>> {
    let mut merged = saved.to_vec();
    for candidate in latest {
        if let Some(existing) = merged
            .iter()
            .find(|item| identity_key(&item.path) == identity_key(&candidate.path))
        {
            if existing.identity != candidate.identity {
                return Err(unsafe_path("protected root identity snapshot conflicts"));
            }
        } else {
            merged.push(candidate.clone());
        }
    }
    validate_protected_roots(&merged)?;
    Ok(merged)
}

#[cfg(not(windows))]
fn merge_protected(
    saved: &[ProtectedRoot],
    latest: &[ProtectedRoot],
) -> io::Result<Vec<ProtectedRoot>> {
    let mut merged = saved.to_vec();
    merged.extend_from_slice(latest);
    Ok(merged)
}

#[cfg(windows)]
fn verify_direct_child(parent: &Path, child_name: &OsStr, actual: &Path) -> io::Result<()> {
    let expected = parent.join(child_name);
    if identity_key(&expected) != identity_key(actual) {
        return Err(unsafe_path(
            "opened directory is not the expected direct child",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn verify_target_final_path(
    root: &OwnedOutputRoot,
    final_path: &Path,
    protected: &[ProtectedRoot],
) -> io::Result<()> {
    verify_final_path_below(&root.final_path, final_path)?;
    reject_protected_overlap(final_path, protected)
}

#[cfg(windows)]
fn verify_final_path_below(root: &Path, final_path: &Path) -> io::Result<()> {
    let root_key = identity_key(root);
    let final_key = identity_key(final_path);
    if !crate::source_identity::is_same_or_descendant(&root_key, &final_key)
        || root_key == final_key
    {
        return Err(unsafe_path("handle final path is outside the output root"));
    }
    Ok(())
}

#[cfg(windows)]
fn target_path(root: &OwnedOutputRoot, relative_path: &Path) -> io::Result<PathBuf> {
    validate_relative_path(relative_path)?;
    Ok(root.final_path.join(relative_path))
}

#[cfg(windows)]
fn open_existing_parent_chain(
    root: &OwnedOutputRoot,
    relative_path: &Path,
) -> io::Result<Vec<File>> {
    let components = validate_relative_path(relative_path)?;
    let mut leases = vec![root.file.try_clone()?];
    let mut path = root.final_path.clone();
    for component in &components[..components.len() - 1] {
        path.push(component);
        let file = open_directory_lease(&path)?;
        let metadata = file.metadata()?;
        ensure_directory_metadata(&path, &metadata)?;
        let final_path = final_path_by_handle(&file)?;
        verify_final_path_below(&root.final_path, &final_path)?;
        leases.push(file);
    }
    Ok(leases)
}

fn validate_relative_path(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(unsafe_path(
                "target path must be relative and cannot contain dot segments",
            ));
        };
        validate_component(part)?;
        components.push(PathBuf::from(part));
    }
    if components.is_empty() {
        return Err(unsafe_path("target relative path is empty"));
    }
    Ok(components)
}

fn validate_component(component: &OsStr) -> io::Result<()> {
    if component.is_empty() || component == OsStr::new(".") || component == OsStr::new("..") {
        return Err(unsafe_path("empty and dot path components are forbidden"));
    }
    let value = component.to_string_lossy();
    if value.ends_with(' ')
        || value.ends_with('.')
        || value.chars().any(|ch| {
            ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
    {
        return Err(unsafe_path("invalid Windows path component"));
    }
    let base = value.split('.').next().unwrap_or_default().trim_end();
    let upper = base.to_ascii_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit()
            && upper.as_bytes()[3] != b'0');
    if reserved {
        return Err(unsafe_path("reserved Windows device name is forbidden"));
    }
    Ok(())
}

fn unsafe_path(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message.into())
}

fn unsupported(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> tempfile::TempDir {
        let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-data");
        tempfile::Builder::new()
            .prefix("photo organizer 中文 Пример café ")
            .tempdir_in(scratch)
            .expect("test-owned temporary directory")
    }

    #[test]
    fn source_snapshot_and_read_lease_stream_same_unchanged_bytes() {
        let temp = fixture_dir();
        let source = temp.path().join("旅行 файл space.webp");
        let bytes = (0..190_000)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        fs::write(&source, &bytes).expect("fixture source");

        let service = FileOperationService;
        let snapshot = service.snapshot_source(&source).expect("snapshot");
        let mut lease = service
            .open_source_read(&source, &snapshot)
            .expect("same-handle source lease");
        let mut copied = Vec::new();
        lease.read_to_end(&mut copied).expect("read source lease");
        assert_eq!(copied, bytes);
        assert_eq!(fs::read(&source).expect("source still exists"), bytes);
        assert_eq!(snapshot.size_bytes, copied.len() as u64);
        assert_eq!(snapshot.blake3, blake3::hash(&copied).to_hex().to_string());
    }

    #[test]
    fn changed_source_is_rejected_before_copy() {
        let temp = fixture_dir();
        let source = temp.path().join("source.jpg");
        fs::write(&source, b"first bytes").expect("fixture source");
        let service = FileOperationService;
        let snapshot = service.snapshot_source(&source).expect("snapshot");
        fs::write(&source, b"changed bytes").expect("change fixture only");
        assert!(service.open_source_read(&source, &snapshot).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn metadata_only_identity_can_be_read_while_database_style_handle_is_open() {
        use std::os::windows::fs::OpenOptionsExt;

        let temp = fixture_dir();
        let database = temp.path().join("catalog.sqlite3");
        fs::write(&database, b"sqlite fixture bytes").expect("fixture catalog file");
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .share_mode(0x0000_0001 | 0x0000_0002);
        let _database_connection = options.open(&database).expect("open shared catalog handle");

        let identity = FileOperationService
            .inspect_shared_file_identity(&database)
            .expect("metadata-only identity with another catalog handle open")
            .expect("Windows durable identity");
        let handle_identity = durable_identity_from_handle(&_database_connection)
            .expect("read identity from held catalog handle")
            .expect("Windows durable identity");
        assert_eq!(identity, handle_identity);
    }

    #[test]
    fn source_fingerprint_hash_checks_cancellation_at_buffer_boundaries() {
        let temp = fixture_dir();
        let source = temp.path().join("large source.bin");
        fs::write(&source, vec![7u8; HASH_BUFFER_SIZE * 3]).expect("fixture source");
        let mut last_checked = 0;
        let error = FileOperationService
            .snapshot_source_with_cancel(&source, &mut |bytes| {
                last_checked = bytes;
                if bytes >= HASH_BUFFER_SIZE as u64 {
                    Err(io::Error::new(io::ErrorKind::Interrupted, "fixture cancel"))
                } else {
                    Ok(())
                }
            })
            .expect_err("cancel after the first bounded hash chunk");
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(last_checked, HASH_BUFFER_SIZE as u64);
    }

    #[cfg(windows)]
    fn roots(temp: &Path) -> (PathBuf, PathBuf, Vec<ProtectedRoot>) {
        let source_root = temp.join("源 Photos");
        let target_parent = temp.join("Target parent");
        fs::create_dir(&source_root).expect("fixture source root");
        fs::create_dir(&target_parent).expect("fixture target parent");
        let protected = vec![ProtectedRoot::from_existing(&source_root).expect("protected source")];
        (source_root, target_parent, protected)
    }

    #[cfg(windows)]
    fn create_root(
        service: &FileOperationService,
        parent_path: &Path,
        protected: &[ProtectedRoot],
        name: &OsStr,
    ) -> (GrantedParent, OwnedOutputRoot) {
        let parent = GrantedParent::grant_existing(parent_path, protected).expect("parent grant");
        let root = service
            .create_new_root(&parent, name, protected)
            .expect("exclusive output root");
        (parent, root)
    }

    #[cfg(windows)]
    #[test]
    fn create_new_refuses_existing_target_without_changing_it() {
        let temp = fixture_dir();
        let (_, parent_path, protected) = roots(temp.path());
        let service = FileOperationService;
        let parent = GrantedParent::grant_existing(&parent_path, &protected).expect("parent");
        let existing = parent_path.join("already exists");
        fs::write(&existing, b"keep me").expect("existing fixture target");
        let failure = match service.create_new_root_detailed(
            &parent,
            OsStr::new("already exists"),
            &protected,
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("exclusive root creation unexpectedly claimed an existing entry"),
        };
        assert!(!failure.may_have_created);
        assert_eq!(fs::read(&existing).expect("unchanged target"), b"keep me");
        drop(parent);
    }

    #[cfg(windows)]
    #[test]
    fn writer_finish_hash_matches_bytes_and_same_handle_rollback_succeeds() {
        let temp = fixture_dir();
        let (_, parent_path, protected) = roots(temp.path());
        let service = FileOperationService;
        let (parent, root) = create_root(
            &service,
            &parent_path,
            &protected,
            OsStr::new("归档 export"),
        );
        let bytes = b"bounded writer bytes with unicode path";
        let mut writer = root
            .create_new_file(Path::new("2026/旅行 file.jpg"))
            .expect("new target");
        writer.write_chunk(bytes).expect("write");
        let committed = writer.finish_and_sync().expect("sync and commit snapshot");
        assert_eq!(committed.size_bytes, bytes.len() as u64);
        assert_eq!(committed.blake3, blake3::hash(bytes).to_hex().to_string());
        assert_eq!(
            fs::read(&committed.final_path).expect("target bytes"),
            bytes
        );

        service
            .delete_if_unchanged_same_handle(
                &root,
                Path::new("2026/旅行 file.jpg"),
                &committed,
                &protected,
            )
            .expect("same-identity rollback");
        assert!(!committed.final_path.exists());
        drop(root);
        drop(parent);
    }

    #[cfg(windows)]
    #[test]
    fn cancelled_rollback_hash_never_sets_delete_disposition() {
        let temp = fixture_dir();
        let (_, parent_path, protected) = roots(temp.path());
        let service = FileOperationService;
        let (parent, root) = create_root(
            &service,
            &parent_path,
            &protected,
            OsStr::new("cancel rollback"),
        );
        let relative = Path::new("2026/cancelled.bin");
        let bytes = vec![19u8; HASH_BUFFER_SIZE * 3];
        let mut writer = root.create_new_file(relative).expect("new target");
        writer
            .write_chunk(&bytes[..HASH_BUFFER_SIZE])
            .expect("chunk 1");
        writer
            .write_chunk(&bytes[HASH_BUFFER_SIZE..HASH_BUFFER_SIZE * 2])
            .expect("chunk 2");
        writer
            .write_chunk(&bytes[HASH_BUFFER_SIZE * 2..])
            .expect("chunk 3");
        let committed = writer.finish_and_sync().expect("commit target");

        let error = service
            .delete_if_unchanged_same_handle_with_cancel(
                &root,
                relative,
                &committed,
                &protected,
                &mut |_| Err(io::Error::new(io::ErrorKind::Interrupted, "fixture cancel")),
            )
            .expect_err("cancel before rollback disposition");
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(
            fs::read(&committed.final_path).expect("target remains"),
            bytes
        );
        drop(root);
        drop(parent);
    }

    #[cfg(windows)]
    #[test]
    fn wrong_root_modified_and_replaced_targets_are_never_deleted() {
        let temp = fixture_dir();
        let (_, parent_path, protected) = roots(temp.path());
        let service = FileOperationService;
        let parent = GrantedParent::grant_existing(&parent_path, &protected).expect("parent");
        let root_a = service
            .create_new_root(&parent, OsStr::new("root A"), &protected)
            .expect("root A");
        let root_b = service
            .create_new_root(&parent, OsStr::new("root B"), &protected)
            .expect("root B");
        let relative = Path::new("same.bin");
        let mut writer = root_a.create_new_file(relative).expect("new target");
        writer.write_chunk(b"original").expect("write target");
        let original = writer.finish_and_sync().expect("finish");
        assert!(
            service
                .delete_if_unchanged_same_handle(&root_b, relative, &original, &protected)
                .is_err()
        );
        assert!(original.final_path.exists());

        fs::write(&original.final_path, b"modified").expect("modify test fixture target");
        assert!(
            service
                .delete_if_unchanged_same_handle(&root_a, relative, &original, &protected)
                .is_err()
        );
        assert_eq!(
            fs::read(&original.final_path).expect("preserved target"),
            b"modified"
        );

        fs::remove_file(&original.final_path).expect("replace only test fixture target");
        fs::write(&original.final_path, b"replacement").expect("replacement fixture target");
        assert!(
            service
                .delete_if_unchanged_same_handle(&root_a, relative, &original, &protected)
                .is_err()
        );
        assert_eq!(
            fs::read(&original.final_path).expect("replacement preserved"),
            b"replacement"
        );
        drop(root_b);
        drop(root_a);
        drop(parent);
    }

    #[cfg(windows)]
    #[test]
    fn partial_abort_succeeds_and_failed_abort_preserves_partial() {
        let temp = fixture_dir();
        let (_, parent_path, protected) = roots(temp.path());
        let service = FileOperationService;
        let (parent, root) = create_root(
            &service,
            &parent_path,
            &protected,
            OsStr::new("abort success"),
        );
        let path = root.final_path.join("partial.bin");
        let mut writer = root
            .create_new_file(Path::new("partial.bin"))
            .expect("partial target");
        writer.write_chunk(b"partial bytes").expect("partial write");
        writer.abort_same_handle().expect("same-handle abort");
        assert!(!path.exists());
        drop(writer);
        drop(root);
        drop(parent);

        let (parent, root) = create_root(
            &service,
            &parent_path,
            &protected,
            OsStr::new("abort retry"),
        );
        let path = root.final_path.join("partial.bin");
        let mut writer = root
            .create_new_file(Path::new("partial.bin"))
            .expect("partial target");
        writer.write_chunk(b"partial bytes").expect("partial write");
        let original_permissions = fs::metadata(&path).expect("metadata").permissions();
        let mut read_only_permissions = original_permissions.clone();
        read_only_permissions.set_readonly(true);
        fs::set_permissions(&path, read_only_permissions).expect("mark fixture target read-only");
        assert!(writer.abort_same_handle().is_err());
        assert_eq!(
            fs::read(&path).expect("partial remains after failure"),
            b"partial bytes"
        );
        fs::set_permissions(&path, original_permissions)
            .expect("restore fixture target permissions");
        writer.abort_same_handle().expect("retry same-handle abort");
        assert!(!path.exists());
        drop(writer);
        drop(root);
        drop(parent);
    }

    #[cfg(windows)]
    #[test]
    fn traversal_and_protected_overlap_are_rejected() {
        let temp = fixture_dir();
        let (source_root, parent_path, protected) = roots(temp.path());
        let service = FileOperationService;
        let parent = GrantedParent::grant_existing(&parent_path, &protected).expect("parent");
        assert!(
            service
                .create_new_root(&parent, OsStr::new(".."), &protected)
                .is_err()
        );
        assert!(
            service
                .create_new_root(
                    &parent,
                    OsStr::new("source child"),
                    &[ProtectedRoot::from_existing(&parent_path).expect("protect parent")]
                )
                .is_err()
        );
        let root = service
            .create_new_root(&parent, OsStr::new("safe root"), &protected)
            .expect("safe root");
        assert!(root.create_new_file(Path::new("../escape.bin")).is_err());
        assert!(
            root.create_new_file(Path::new("a/../../escape.bin"))
                .is_err()
        );
        assert!(root.create_new_file(Path::new("CON.txt")).is_err());
        assert!(root.create_new_file(Path::new("safe.bin")).is_ok());
        assert!(source_root.exists());
        drop(root);
        drop(parent);
    }

    #[cfg(windows)]
    #[test]
    fn directory_junction_is_rejected() {
        let temp = fixture_dir();
        let outside = temp.path().join("outside");
        let parent_path = temp.path().join("parent");
        fs::create_dir(&outside).expect("outside fixture");
        fs::create_dir(&parent_path).expect("parent fixture");
        let link = parent_path.join("junction-like-link");
        create_junction(&link, &outside);
        let junction_handle = open_directory_lease(&link).expect("open fixture junction handle");
        let junction_identity = durable_identity_from_handle(&junction_handle)
            .expect("read fixture junction identity")
            .expect("Windows fixture junction identity");
        drop(junction_handle);

        assert!(
            GrantedParent::grant_existing(&link, &[]).is_err(),
            "a junction cannot be granted as a real directory"
        );
        let service = FileOperationService;
        let parent = GrantedParent::grant_existing(&parent_path, &[]).expect("parent grant");
        assert!(
            service
                .open_existing_output_root(
                    &parent,
                    OsStr::new("junction-like-link"),
                    &junction_identity,
                    &[]
                )
                .is_err()
        );
        drop(parent);
    }

    #[cfg(windows)]
    fn create_junction(link: &Path, target: &Path) {
        let script = concat!(
            "$ErrorActionPreference = 'Stop'; ",
            "New-Item -ItemType Junction ",
            "-Path $env:PHOTO_ORGANIZER_JUNCTION_LINK ",
            "-Target $env:PHOTO_ORGANIZER_JUNCTION_TARGET ",
            "-ErrorAction Stop | Out-Null"
        );
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .env("PHOTO_ORGANIZER_JUNCTION_LINK", link)
            .env("PHOTO_ORGANIZER_JUNCTION_TARGET", target)
            .output()
            .expect("start PowerShell to create fixture junction");
        assert!(
            output.status.success(),
            "PowerShell junction creation failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn relative_paths_reject_parent_and_absolute_components() {
        assert!(validate_relative_path(Path::new("../escape")).is_err());
        assert!(validate_relative_path(Path::new("/absolute")).is_err());
        assert!(validate_relative_path(Path::new("folder/file.jpg")).is_ok());
    }
}
