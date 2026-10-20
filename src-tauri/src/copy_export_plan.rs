//! Immutable v2 original-file copy plans.
//!
//! Planning reads the complete query scope from one SQLite snapshot, freezes
//! source metadata and content fingerprints, and never writes to source files.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime};

use crate::db::Repository;
use crate::error::{AppError, AppResult};
use crate::export_file_safety::{DurableFileIdentity, FileOperationService, ProtectedRoot};
use crate::models::{
    CopyExportConflictPolicy, CopyExportLayout, CopyPlanIssue, CopyPlanItemStatus, CopyPlanStatus,
    CreateOrganizationPlanV2Request, MissingCaptureTimePolicy, OrganizationPlanItemV2,
    OrganizationPlanScopeV2, OrganizationPlanSummaryV2, OrganizationPlanV2, OrganizationTreeNode,
};
use crate::source_identity::{identity_key, is_same_or_descendant, paths_overlap};

const ITEM_PAGE_SIZE: u32 = 120;
const MAX_SEGMENT_UTF16_UNITS: usize = 255;
const MAX_PATH_UTF16_UNITS: usize = 260;

pub fn build_plan_v2(
    repository: &Repository,
    request: &CreateOrganizationPlanV2Request,
    target_parent_path: &Path,
    target_parent_identity: &DurableFileIdentity,
    protected_roots: &[ProtectedRoot],
) -> AppResult<OrganizationPlanV2> {
    if request.rules.version != "copy-export-rules-v1" {
        return Err(AppError::InvalidArgument(format!(
            "unsupported copy export rules version: {}",
            request.rules.version
        )));
    }
    if !target_parent_path.is_absolute() || !target_parent_path.is_dir() {
        return Err(AppError::InvalidRoot(target_parent_path.to_path_buf()));
    }

    let mut scope = request.scope.clone();
    if let OrganizationPlanScopeV2::Query { query } = &mut scope {
        query.page = 1;
        query.page_size = ITEM_PAGE_SIZE;
    }
    let snapshot = repository.snapshot_organization_scope_v2(&scope)?;
    let source_root_count = snapshot
        .assets
        .iter()
        .map(|item| identity_key(&item.source_root_path))
        .collect::<HashSet<_>>()
        .len();
    let target_root = target_parent_path.join(&request.target.child_name);
    let target_root_text = path_to_string(&target_root);
    let target_parent_text = path_to_string(target_parent_path);
    let mut global_issues = Vec::new();
    let child_valid = valid_segment(&request.target.child_name);
    if !child_valid {
        global_issues.push(issue(
            "invalid_target_child",
            None,
            None,
            Some(target_root_text.clone()),
            "目标文件夹名称不符合 Windows 路径规则。",
        ));
    }
    match fs::symlink_metadata(&target_root) {
        Ok(_) => global_issues.push(issue(
            "target_exists",
            None,
            None,
            Some(target_root_text.clone()),
            "目标文件夹已存在；复制计划只允许创建全新的目录。",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => global_issues.push(issue(
            "target_unavailable",
            None,
            None,
            Some(target_root_text.clone()),
            &format!("无法检查目标文件夹：{error}"),
        )),
    }
    let target_key = identity_key(&target_root);
    if let Some(root) = protected_roots
        .iter()
        .find(|root| paths_overlap(&identity_key(&root.path), &target_key))
    {
        global_issues.push(issue(
            "target_overlaps_protected_root",
            None,
            Some(path_to_string(&root.path)),
            Some(target_root_text.clone()),
            "目标文件夹与当前 Source 或应用数据目录重叠。",
        ));
    }

    let file_service = FileOperationService;
    let mut assigned_targets = HashSet::new();
    let mut items = Vec::with_capacity(snapshot.assets.len());
    for (index, source) in snapshot.assets.into_iter().enumerate() {
        let source_path = PathBuf::from(&source.asset.absolute_path);
        let source_root_path = source.source_root_path;
        let normalized_owner_relative = normalize_owner_relative(&source.asset.relative_path);
        let source_relative_path = normalized_owner_relative
            .clone()
            .unwrap_or_else(|| normalized_relative(&source.asset.relative_path));
        let root_key = identity_key(&source_root_path);
        let root = protected_roots
            .iter()
            .find(|candidate| identity_key(&candidate.path) == root_key);
        let root_identity = root.and_then(|candidate| candidate.identity.clone());
        let expected_source_path =
            source_root_path.join(source_relative_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let source_path_key = identity_key(&source_path);
        let owner_path_valid = normalized_owner_relative.is_some()
            && is_same_or_descendant(&root_key, &source_path_key)
            && identity_key(&expected_source_path) == source_path_key;
        let source_metadata = owner_path_valid
            .then(|| file_service.inspect_source_metadata(&source_path).ok())
            .flatten();
        let mut issues = Vec::new();

        if source.asset.file_status != "present" || source.asset.scan_status == "failed" {
            issues.push(issue(
                "source_not_copyable",
                Some(source.asset.id),
                Some(source.asset.absolute_path.clone()),
                None,
                "来源文件没有可用的成功扫描快照。",
            ));
        }
        if root.is_none() {
            issues.push(issue(
                "source_root_unavailable",
                Some(source.asset.id),
                Some(source.asset.absolute_path.clone()),
                None,
                "来源目录未能通过当前安全边界核验。",
            ));
        }
        if !owner_path_valid {
            issues.push(issue(
                "source_path_outside_owner_root",
                Some(source.asset.id),
                Some(source.asset.absolute_path.clone()),
                None,
                "来源绝对路径与所属Source根目录和相对路径不一致。",
            ));
        }
        if !valid_blake3(&source.fingerprint_blake3) {
            issues.push(issue(
                "source_fingerprint_unavailable",
                Some(source.asset.id),
                Some(source.asset.absolute_path.clone()),
                None,
                "目录中没有有效的完整文件指纹；请重新扫描来源。",
            ));
        }
        let source_file_identity = source_metadata
            .as_ref()
            .and_then(|metadata| metadata.identity.clone());
        if source_file_identity.is_none() {
            issues.push(issue(
                "durable_source_identity_unavailable",
                Some(source.asset.id),
                Some(source.asset.absolute_path.clone()),
                None,
                "当前平台无法取得可持久核验的来源文件身份。",
            ));
        }
        if let Some(metadata) = &source_metadata {
            let stored_size = source.asset.file_size.max(0) as u64;
            let stored_modified = source.asset.modified_at;
            if metadata.size_bytes != stored_size
                || modified_time_unix_millis(metadata.modified_time) != Some(stored_modified)
            {
                issues.push(issue(
                    "source_changed_since_scan",
                    Some(source.asset.id),
                    Some(source.asset.absolute_path.clone()),
                    None,
                    "来源文件大小或修改时间已变化；请重新扫描后再创建计划。",
                ));
            }
        } else {
            issues.push(issue(
                "source_unavailable",
                Some(source.asset.id),
                Some(source.asset.absolute_path.clone()),
                None,
                "无法读取来源文件元数据。",
            ));
        }

        let mut status = if issues.is_empty() {
            CopyPlanItemStatus::Ready
        } else {
            CopyPlanItemStatus::Blocked
        };
        if source_relative_path.is_empty() {
            issues.push(issue(
                "source_relative_path_invalid",
                Some(source.asset.id),
                Some(source.asset.absolute_path.clone()),
                None,
                "数据库中的来源相对路径无效。",
            ));
            status = CopyPlanItemStatus::Blocked;
        }

        let basename = Path::new(&source_relative_path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let source_namespace = if source_root_count > 1 {
            let namespace = source_root_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Source".into());
            Some(format!("{namespace} ({})", source.asset.library_id))
        } else {
            None
        };

        let relative = match request.rules.layout {
            CopyExportLayout::PreserveOriginal => {
                join_namespace(source_namespace.as_deref(), &source_relative_path)
            }
            CopyExportLayout::CaptureYearMonth => {
                match capture_year_month(source.asset.capture_time.as_deref()) {
                    Some((year, month)) => join_namespace(
                        source_namespace.as_deref(),
                        &format!("{year:04}/{month:02}/{basename}"),
                    ),
                    None => match request.rules.missing_capture_time_policy {
                        MissingCaptureTimePolicy::Block => {
                            issues.push(issue(
                                "capture_time_missing",
                                Some(source.asset.id),
                                Some(source.asset.absolute_path.clone()),
                                None,
                                "拍摄时间缺失，无法按年月组织。",
                            ));
                            status = CopyPlanItemStatus::Blocked;
                            join_namespace(source_namespace.as_deref(), &source_relative_path)
                        }
                        MissingCaptureTimePolicy::Skip => {
                            issues.push(issue(
                                "capture_time_missing_skipped",
                                Some(source.asset.id),
                                Some(source.asset.absolute_path.clone()),
                                None,
                                "拍摄时间缺失，按规则跳过此文件。",
                            ));
                            status = CopyPlanItemStatus::Skipped;
                            join_namespace(
                                source_namespace.as_deref(),
                                &format!("Unknown/{basename}"),
                            )
                        }
                        MissingCaptureTimePolicy::UnknownFolder => join_namespace(
                            source_namespace.as_deref(),
                            &format!("Unknown/{basename}"),
                        ),
                        MissingCaptureTimePolicy::PreserveOriginalFolders => {
                            join_namespace(source_namespace.as_deref(), &source_relative_path)
                        }
                    },
                }
            }
        };

        for global_issue in &global_issues {
            issues.push(global_issue.clone());
            status = CopyPlanItemStatus::Blocked;
        }
        let target_relative_path = if status == CopyPlanItemStatus::Skipped {
            relative.clone()
        } else {
            resolve_collisions(
                &relative,
                source.asset.id,
                &source.fingerprint_blake3,
                request.rules.conflict_policy,
                &mut assigned_targets,
                &mut issues,
                &mut status,
            )
        };
        if !target_relative_path.is_empty()
            && !valid_relative_path(&target_relative_path, &target_root_text)
        {
            issues.push(issue(
                "invalid_target_path",
                Some(source.asset.id),
                Some(source.asset.absolute_path.clone()),
                Some(path_to_string(&target_root.join(&target_relative_path))),
                "生成的目标路径包含非法、保留或过长的 Windows 路径段。",
            ));
            status = CopyPlanItemStatus::Blocked;
        }
        let target_path = if target_relative_path.is_empty() {
            String::new()
        } else {
            path_to_string(&target_root.join(&target_relative_path))
        };
        let source_size = source.asset.file_size.max(0) as u64;
        items.push(OrganizationPlanItemV2 {
            ordinal: index as u32,
            asset_id: source.asset.id,
            library_id: source.asset.library_id,
            source_root_path: path_to_string(&source_root_path),
            source_root_identity: root_identity,
            source_path: source.asset.absolute_path,
            source_relative_path,
            source_file_identity,
            source_fingerprint_blake3: source.fingerprint_blake3,
            source_size,
            source_modified_at: source.asset.modified_at,
            source_modified_filetime: source_metadata
                .as_ref()
                .map(|metadata| metadata.modified_time)
                .unwrap_or_default(),
            capture_time: source.asset.capture_time,
            classification_revision: source.classification_revision,
            target_relative_path,
            target_path,
            status,
            issues,
        });
    }

    let mut summary_issues = global_issues;
    for missing_id in snapshot.missing_explicit_asset_ids {
        summary_issues.push(issue(
            "explicit_asset_missing",
            Some(missing_id),
            None,
            None,
            "所选文件已不在目录中；计划被阻止以免静默漏项。",
        ));
    }
    let matched_count = items.len() as u64
        + summary_issues
            .iter()
            .filter(|issue| issue.code == "explicit_asset_missing")
            .count() as u64;
    let eligible_count = items
        .iter()
        .filter(|item| item.status == CopyPlanItemStatus::Ready)
        .count() as u64;
    let skipped_count = items
        .iter()
        .filter(|item| item.status == CopyPlanItemStatus::Skipped)
        .count() as u64;
    let blocked_count = items
        .iter()
        .filter(|item| item.status == CopyPlanItemStatus::Blocked)
        .count() as u64
        + summary_issues
            .iter()
            .filter(|issue| issue.code == "explicit_asset_missing")
            .count() as u64;
    let estimated_bytes = items
        .iter()
        .filter(|item| item.status == CopyPlanItemStatus::Ready)
        .map(|item| item.source_size)
        .fold(0_u64, u64::saturating_add);
    let status = if blocked_count > 0 {
        CopyPlanStatus::Blocked
    } else {
        CopyPlanStatus::Ready
    };
    let tree = build_tree(&request.target.child_name, &items);
    let plan_id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let snapshot_blake3 = plan_snapshot_hash(
        &scope,
        &request.rules,
        &target_parent_text,
        target_parent_identity,
        protected_roots,
        &items,
    )?;
    let summary = OrganizationPlanSummaryV2 {
        plan_id,
        revision: "copy-export-plan-v2".into(),
        status,
        scope,
        target_root: target_root_text,
        matched_count,
        eligible_count,
        skipped_count,
        blocked_count,
        estimated_bytes,
        issues: summary_issues,
        snapshot_blake3,
        created_at,
        rules: request.rules.clone(),
        tree,
    };
    let item_total = items.len() as u64;
    Ok(OrganizationPlanV2 {
        summary,
        items,
        item_total,
        item_page: 1,
        item_page_size: ITEM_PAGE_SIZE,
    })
}

fn resolve_collisions(
    original: &str,
    asset_id: i64,
    fingerprint: &str,
    policy: CopyExportConflictPolicy,
    assigned: &mut HashSet<String>,
    issues: &mut Vec<CopyPlanIssue>,
    status: &mut CopyPlanItemStatus,
) -> String {
    let key = identity_key(Path::new(original));
    if assigned.insert(key) {
        return original.to_owned();
    }
    match policy {
        CopyExportConflictPolicy::Block => {
            issues.push(issue(
                "target_conflict",
                Some(asset_id),
                None,
                Some(original.into()),
                "两个来源文件映射到同一目标路径。",
            ));
            *status = CopyPlanItemStatus::Blocked;
            original.to_owned()
        }
        CopyExportConflictPolicy::Skip => {
            issues.push(issue(
                "target_conflict_skipped",
                Some(asset_id),
                None,
                Some(original.into()),
                "目标路径已被前一个文件占用，按规则跳过此文件。",
            ));
            *status = CopyPlanItemStatus::Skipped;
            original.to_owned()
        }
        CopyExportConflictPolicy::Sequence => {
            for number in 2_u32..=100_000 {
                let candidate = insert_suffix(original, &format!(" ({number})"));
                if assigned.insert(identity_key(Path::new(&candidate))) {
                    return candidate;
                }
            }
            *status = CopyPlanItemStatus::Blocked;
            original.to_owned()
        }
        CopyExportConflictPolicy::ShortHash => {
            let digest = blake3::hash(format!("{asset_id}:{fingerprint}").as_bytes())
                .to_hex()
                .to_string();
            let candidate = insert_suffix(original, &format!("~{}", &digest[..8]));
            if assigned.insert(identity_key(Path::new(&candidate))) {
                candidate
            } else {
                *status = CopyPlanItemStatus::Blocked;
                original.to_owned()
            }
        }
    }
}

fn build_tree(root_name: &str, items: &[OrganizationPlanItemV2]) -> OrganizationTreeNode {
    let mut root = OrganizationTreeNode {
        name: root_name.into(),
        relative_path: String::new(),
        file_count: 0,
        byte_count: 0,
        children: Vec::new(),
    };
    for item in items
        .iter()
        .filter(|item| item.status == CopyPlanItemStatus::Ready)
    {
        root.file_count += 1;
        root.byte_count = root.byte_count.saturating_add(item.source_size);
        let mut node = &mut root;
        let mut relative = String::new();
        for segment in item
            .target_relative_path
            .split('/')
            .filter(|part| !part.is_empty())
        {
            if !relative.is_empty() {
                relative.push('/');
            }
            relative.push_str(segment);
            let index = node
                .children
                .iter()
                .position(|child| child.name == segment)
                .unwrap_or_else(|| {
                    node.children.push(OrganizationTreeNode {
                        name: segment.into(),
                        relative_path: relative.clone(),
                        file_count: 0,
                        byte_count: 0,
                        children: Vec::new(),
                    });
                    node.children.len() - 1
                });
            node = &mut node.children[index];
            node.file_count += 1;
            node.byte_count = node.byte_count.saturating_add(item.source_size);
        }
    }
    sort_tree(&mut root);
    root
}

fn sort_tree(node: &mut OrganizationTreeNode) {
    node.children.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });
    for child in &mut node.children {
        sort_tree(child);
    }
}

fn plan_snapshot_hash(
    scope: &OrganizationPlanScopeV2,
    rules: &crate::models::CopyExportRules,
    parent_path: &str,
    parent_identity: &DurableFileIdentity,
    protected_roots: &[ProtectedRoot],
    items: &[OrganizationPlanItemV2],
) -> AppResult<String> {
    let snapshot = serde_json::json!({
        "scope": scope,
        "rules": rules,
        "parentPath": parent_path,
        "parentIdentity": parent_identity,
        "protectedRoots": protected_roots,
        "items": items.iter().map(|item| serde_json::json!({
            "item": item,
            "sourceRootIdentity": &item.source_root_identity,
            "sourceFileIdentity": &item.source_file_identity,
            "sourceModifiedFiletime": item.source_modified_filetime,
        })).collect::<Vec<_>>(),
    });
    Ok(blake3::hash(&serde_json::to_vec(&snapshot)?)
        .to_hex()
        .to_string())
}

fn capture_year_month(value: Option<&str>) -> Option<(i32, u32)> {
    let value = value?;
    let date_text = value.split(['T', ' ']).next().unwrap_or(value);
    if let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y-%m-%d") {
        return Some((date.year(), date.month()));
    }
    if let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y:%m:%d") {
        return Some((date.year(), date.month()));
    }
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| (date.year(), date.month()))
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|date| (date.year(), date.month()))
        })
}

fn modified_time_unix_millis(modified: u64) -> Option<i64> {
    #[cfg(windows)]
    {
        const WINDOWS_TO_UNIX_EPOCH_100NS: u64 = 116_444_736_000_000_000;
        modified
            .checked_sub(WINDOWS_TO_UNIX_EPOCH_100NS)
            .map(|ticks| (ticks / 10_000).min(i64::MAX as u64) as i64)
    }
    #[cfg(not(windows))]
    {
        Some((modified / 1_000_000).min(i64::MAX as u64) as i64)
    }
}

fn valid_blake3(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_segment(value: &str) -> bool {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.encode_utf16().count() > MAX_SEGMENT_UTF16_UNITS
        || value.ends_with(' ')
        || value.ends_with('.')
        || value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
    {
        return false;
    }
    let base = value
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return false;
    }
    !(base.len() == 4
        && (base.starts_with("COM") || base.starts_with("LPT"))
        && base.as_bytes()[3].is_ascii_digit()
        && base.as_bytes()[3] != b'0')
}

fn valid_relative_path(relative: &str, root: &str) -> bool {
    if relative.is_empty() || relative.starts_with('/') || relative.starts_with('\\') {
        return false;
    }
    if !relative.split('/').all(valid_segment) {
        return false;
    }
    format!(
        "{root}{}{}",
        std::path::MAIN_SEPARATOR,
        relative.replace('/', std::path::MAIN_SEPARATOR_STR)
    )
    .encode_utf16()
    .count()
        <= MAX_PATH_UTF16_UNITS
}

fn insert_suffix(relative: &str, suffix: &str) -> String {
    let path = Path::new(relative);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let (stem, extension) = filename
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(stem, extension)| (stem, format!(".{extension}")))
        .unwrap_or((filename.as_ref(), String::new()));
    let next = format!("{stem}{suffix}{extension}");
    parent
        .map(|parent| format!("{}/{next}", parent.to_string_lossy().replace('\\', "/")))
        .unwrap_or(next)
}

fn join_namespace(namespace: Option<&str>, relative: &str) -> String {
    match namespace {
        Some(namespace) if !relative.is_empty() => format!("{namespace}/{relative}"),
        Some(namespace) => namespace.to_owned(),
        None => relative.to_owned(),
    }
}

fn normalized_relative(value: &str) -> String {
    value.replace('\\', "/")
}

/// Normalize only a safe owner-relative path. Separators from the catalog are
/// canonicalized, while absolute paths, drive prefixes, empty segments and
/// traversal components remain invalid and are never silently repaired.
pub fn normalize_owner_relative(value: &str) -> Option<String> {
    let normalized = normalized_relative(value);
    if normalized.is_empty() || normalized.starts_with('/') {
        return None;
    }
    let mut components = normalized.split('/');
    let first = components.next()?;
    if first.is_empty() || first.contains(':') || first == "." || first == ".." {
        return None;
    }
    if components.any(|component| component.is_empty() || component == "." || component == "..") {
        return None;
    }
    Some(normalized)
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', std::path::MAIN_SEPARATOR_STR)
}

fn issue(
    code: &str,
    asset_id: Option<i64>,
    source_path: Option<String>,
    target_path: Option<String>,
    detail: &str,
) -> CopyPlanIssue {
    CopyPlanIssue {
        code: code.into(),
        detail: detail.into(),
        asset_id,
        source_path,
        target_path,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_owner_relative;

    #[test]
    fn owner_relative_paths_normalize_separators_without_permitting_traversal() {
        assert_eq!(
            normalize_owner_relative(r"旅行\Фото 01.jpg"),
            Some("旅行/Фото 01.jpg".into())
        );
        assert_eq!(normalize_owner_relative("../outside.jpg"), None);
        assert_eq!(normalize_owner_relative(r"旅行\..\outside.jpg"), None);
        assert_eq!(normalize_owner_relative(r"C:\outside.jpg"), None);
        assert_eq!(normalize_owner_relative(r"\rooted.jpg"), None);
        assert_eq!(normalize_owner_relative("folder//photo.jpg"), None);
    }
}
