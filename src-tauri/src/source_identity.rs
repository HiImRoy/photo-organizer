use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use unicode_normalization_alignments::UnicodeNormalization;

use crate::error::{AppError, AppResult};

/// The path used for display/opening and the stable key used for comparisons.
///
/// source_path is resolved for an existing source root so that junctions and
/// symlinks cannot create two logical roots for the same directory. The
/// identity key is deliberately stored separately from the user-facing path:
/// SQLite TEXT comparison is not a substitute for Windows path semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub source_path: PathBuf,
    pub identity_key: String,
}

pub fn validate_source_root(root: &Path, app_data_root: &Path) -> AppResult<SourceIdentity> {
    if !root.is_dir() {
        return Err(AppError::InvalidRoot(root.to_path_buf()));
    }
    if !app_data_root.is_dir() {
        return Err(AppError::UnsafePath(app_data_root.to_path_buf()));
    }

    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(AppError::UnsafePath(root.to_path_buf()));
    }

    let source_path = fs::canonicalize(root)?;
    let app_data_path = fs::canonicalize(app_data_root)?;
    let source_key = identity_key(&source_path);
    let app_data_key = identity_key(&app_data_path);
    if paths_overlap(&source_key, &app_data_key) {
        return Err(AppError::UnsafePath(root.to_path_buf()));
    }

    Ok(SourceIdentity {
        source_path,
        identity_key: source_key,
    })
}

pub fn existing_identity(path: &Path) -> AppResult<SourceIdentity> {
    if !path.is_dir() {
        return Err(AppError::InvalidRoot(path.to_path_buf()));
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(AppError::UnsafePath(path.to_path_buf()));
    }
    let source_path = fs::canonicalize(path)?;
    Ok(SourceIdentity {
        identity_key: identity_key(&source_path),
        source_path,
    })
}

/// Build a comparison key for an existing or lexical path.
///
/// On Windows the key follows the product's default filesystem policy:
/// separators, dot segments, long-path prefixes and Unicode representation
/// are normalized, and path components are case-folded. On other platforms
/// case is preserved so the tests do not silently impose Windows semantics on
/// a case-sensitive filesystem.
pub fn identity_key(path: &Path) -> String {
    normalize_path_string(&path.to_string_lossy())
}

pub fn paths_overlap(left: &str, right: &str) -> bool {
    is_same_or_descendant(left, right) || is_same_or_descendant(right, left)
}

pub fn is_same_or_descendant(root: &str, candidate: &str) -> bool {
    if root == candidate {
        return true;
    }
    if root == "/" {
        return candidate.starts_with('/');
    }
    let root_without_trailing = root.trim_end_matches('/');
    candidate
        .strip_prefix(root_without_trailing)
        .is_some_and(|rest| rest.starts_with('/'))
}

pub fn normalize_path_string(value: &str) -> String {
    let mut value = value.replace('\\', "/");
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("//?/unc/") {
        value = format!("//{}", &value[8..]);
    } else if value.starts_with("//?/") {
        value = value[4..].to_owned();
    }

    let is_unc = value.starts_with("//");
    let is_absolute = value.starts_with('/');
    let drive = value
        .as_bytes()
        .get(1)
        .is_some_and(|character| *character == b':')
        && value.as_bytes().first().is_some_and(u8::is_ascii_alphabetic);

    let mut components = Vec::new();
    for component in value.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            if drive || is_unc || is_absolute {
                components.pop();
            } else {
                components.push(component.to_owned());
            }
            continue;
        }
        components.push(normalize_component(component));
    }

    if drive {
        let prefix = normalize_component(&value[..2]);
        if components
            .first()
            .is_some_and(|component| component == &prefix)
        {
            components.remove(0);
        }
        if components.is_empty() {
            return format!("{prefix}/");
        }
        return format!("{prefix}/{}", components.join("/"));
    }
    if is_unc {
        return format!("//{}", components.join("/"));
    }
    if is_absolute {
        if components.is_empty() {
            return "/".into();
        }
        return format!("/{}", components.join("/"));
    }
    components.join("/")
}

fn normalize_component(value: &str) -> String {
    let normalized = value
        .nfc()
        .map(|(character, _)| character)
        .collect::<String>();
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    let _ = metadata;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_separators_dot_segments_and_unicode() {
        let decomposed = "e\u{301}";
        let expected = if cfg!(windows) {
            "c:/photos/café"
        } else {
            "C:/Photos/Café"
        };
        assert_eq!(
            normalize_path_string(r"C:\Photos\.\旅行\..\Café"),
            expected
        );
        assert_eq!(normalize_path_string(decomposed), "é");
    }

    #[test]
    fn normalizes_long_path_forms() {
        let drive_expected = if cfg!(windows) {
            "c:/photos/travel"
        } else {
            "C:/Photos/Travel"
        };
        let unc_expected = if cfg!(windows) {
            "//server/share/photos"
        } else {
            "//server/share/Photos"
        };
        assert_eq!(
            normalize_path_string(r"\\?\C:\Photos\Travel"),
            drive_expected
        );
        assert_eq!(
            normalize_path_string(r"\\?\UNC\server\share\Photos"),
            unc_expected
        );
    }

    #[test]
    fn containment_uses_path_components() {
        assert!(is_same_or_descendant("c:/photos", "c:/photos/child"));
        assert!(is_same_or_descendant("c:/photos", "c:/photos/child/image.jpg"));
        assert!(!is_same_or_descendant("c:/photos", "c:/photos-old"));
        assert!(!is_same_or_descendant("c:/photos", "c:/photography"));
    }

    #[test]
    fn source_and_app_data_overlap_is_rejected() {
        assert!(paths_overlap("c:/app-data", "c:/app-data/cache"));
        assert!(paths_overlap("c:/photos", "c:/photos/app-data"));
        assert!(!paths_overlap("c:/photos", "c:/app-data"));
    }

    #[test]
    fn existing_identity_resolves_a_fixture_directory() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("中文 Photos");
        fs::create_dir_all(&source).expect("source");
        let identity = existing_identity(&source).expect("identity");
        assert_eq!(identity.source_path, fs::canonicalize(source).expect("canonical"));
        assert!(
            identity
                .identity_key
                .to_ascii_lowercase()
                .contains("photos")
        );
    }
}
