//! Read-only organization planning.
//!
//! This module intentionally has no file operation executor. It reads the
//! SQLite asset snapshot and (optionally) source/target metadata, then builds
//! deterministic strings and diagnostics in memory. The only write path in
//! this module is the explicit manifest export, which creates a new JSON/CSV
//! file selected by the user.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, Utc};
use unicode_normalization_alignments::UnicodeNormalization;

use crate::error::{AppError, AppResult};
use crate::models::{
    AssetListItem, OrganizationConflictStrategy, OrganizationIssue, OrganizationIssueSeverity,
    OrganizationItemStatus, OrganizationLevelKind, OrganizationMissingFallback, OrganizationPlan,
    OrganizationPlanItem, OrganizationPlanRequest, OrganizationPlanSummary, OrganizationRules,
    OrganizationTreeNode,
};

const MAX_SEGMENT_UNITS: usize = 255;
const MAX_PATH_UNITS: usize = 260;

#[derive(Debug, Clone)]
enum TemplateToken {
    Literal(String),
    Variable {
        name: String,
        format: Option<String>,
    },
}

#[derive(Debug, Clone, Default)]
struct RenderContext {
    values: BTreeMap<String, String>,
    capture_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct DraftItem {
    item: OrganizationPlanItem,
    valid_target: bool,
}

pub fn validate_rules(rules: &OrganizationRules) -> Vec<OrganizationIssue> {
    let mut issues = Vec::new();
    if rules.sequence_width == 0 || rules.sequence_width > 12 {
        issues.push(rule_issue(
            "sequence_width",
            "序号宽度必须在 1 到 12 之间。",
        ));
    }
    if rules.sequence_start == 0 {
        issues.push(rule_issue("sequence_start", "序号起点必须大于 0。"));
    }
    if rules.levels.len() > 8 {
        issues.push(rule_issue("levels", "目录维度最多支持 8 层。"));
    }
    let mut seen = HashSet::new();
    for level in &rules.levels {
        if !seen.insert(level.kind.as_str()) {
            issues.push(rule_issue(
                "duplicate_level",
                &format!("目录维度 {} 重复。", level.kind.as_str()),
            ));
        }
        if matches!(
            &level.fallback,
            OrganizationMissingFallback::ModificationTime
        ) && !matches!(
            &level.kind,
            OrganizationLevelKind::Year | OrganizationLevelKind::Month | OrganizationLevelKind::Day
        ) {
            issues.push(rule_issue(
                "invalid_level_fallback",
                &format!(
                    "目录维度 {} 不能使用文件修改时间回退；该回退仅适用于拍摄年份、月份或日期。",
                    level.kind.as_str()
                ),
            ));
        }
    }
    match parse_template(&rules.template) {
        Ok(tokens) => {
            const ALLOWED: &[&str] = &[
                "capture_time",
                "capture_date",
                "captured_date",
                "captured_time",
                "camera",
                "camera_make",
                "camera_model",
                "lens",
                "original_name",
                "original_stem",
                "extension",
                "semantic",
                "primary_label",
                "tone",
                "dominant_color",
                "saturation",
                "sequence",
                "short_hash",
            ];
            for token in tokens {
                if let TemplateToken::Variable { name, .. } = token
                    && !ALLOWED.contains(&name.as_str())
                {
                    issues.push(rule_issue(
                        "unknown_template_variable",
                        &format!("未知命名变量 {{{name}}}。"),
                    ));
                }
            }
        }
        Err(message) => issues.push(rule_issue("invalid_template", &message)),
    }
    issues
}

pub fn build_plan(
    request: &OrganizationPlanRequest,
    source_root: &str,
    mut assets: Vec<AssetListItem>,
) -> AppResult<OrganizationPlan> {
    let rule_issues = validate_rules(&request.rules);
    if let Some(error) = rule_issues
        .iter()
        .find(|issue| issue.severity == OrganizationIssueSeverity::Error)
    {
        return Err(AppError::InvalidArgument(error.detail.clone()));
    }
    validate_target_boundary(source_root, &request.target_root)?;
    let tokens = parse_template(&request.rules.template).map_err(AppError::InvalidArgument)?;

    assets.sort_by(|left, right| {
        normalize_for_compare(&left.relative_path)
            .cmp(&normalize_for_compare(&right.relative_path))
            .then(left.id.cmp(&right.id))
    });
    let source_snapshot = source_snapshot(&assets);
    let mut drafts = Vec::with_capacity(assets.len());
    for (index, asset) in assets.iter().enumerate() {
        let ordinal = u32::try_from(index).unwrap_or(u32::MAX).saturating_add(1);
        let (context, mut issues) = render_context(asset, &request.rules, ordinal);
        let levels = render_levels(asset, &request.rules, &context, &mut issues);
        let rendered_name =
            render_template(&tokens, &context, ordinal).unwrap_or_else(|_| asset.file_name.clone());
        if rendered_name.contains('/') || rendered_name.contains('\\') {
            issues.push(issue(
                "target_escape",
                OrganizationIssueSeverity::Error,
                Some(asset.absolute_path.clone()),
                Some(rendered_name.clone()),
                "文件名模板不能注入目录分隔符。",
            ));
        }
        let mut file_name = rendered_name;
        if !file_name
            .to_ascii_lowercase()
            .ends_with(&format!(".{}", asset.extension))
        {
            file_name.push('.');
            file_name.push_str(&asset.extension);
        }
        let mut relative = levels;
        relative.push(file_name);
        let relative_target = relative.join("/");
        let mut item_issues = issues;
        let valid_target = validate_relative_target(
            &relative_target,
            &request.target_root,
            &asset.absolute_path,
            &mut item_issues,
        );
        if asset.file_status != "present" || !Path::new(&asset.absolute_path).is_file() {
            item_issues.push(issue(
                "source_missing",
                OrganizationIssueSeverity::Error,
                Some(asset.absolute_path.clone()),
                None,
                "源文件当前不可读取，预览不会替代缺失文件。",
            ));
        } else if let Ok(metadata) = fs::metadata(&asset.absolute_path)
            && (i64::try_from(metadata.len()).unwrap_or(i64::MAX) != asset.file_size
                || metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs() as i64)
                    .is_some_and(|modified| modified != asset.modified_at))
        {
            item_issues.push(issue(
                "source_changed",
                OrganizationIssueSeverity::Warning,
                Some(asset.absolute_path.clone()),
                None,
                "源文件大小或修改时间已变化；生成计划前请重新扫描。",
            ));
        }
        let status = status_for_issues(&item_issues);
        drafts.push(DraftItem {
            item: OrganizationPlanItem {
                ordinal,
                asset_id: asset.id,
                source_path: asset.absolute_path.clone(),
                source_relative_path: asset.relative_path.clone(),
                source_fingerprint: asset_fingerprint(asset),
                target_relative_path: relative_target,
                target_path: String::new(),
                file_size: u64::try_from(asset.file_size.max(0)).unwrap_or_default(),
                status,
                variables: context.values,
                issues: item_issues,
            },
            valid_target,
        });
    }

    apply_conflicts(
        &mut drafts,
        &request.target_root,
        &request.rules.conflict_strategy,
        request.rules.sequence_start,
        request.rules.sequence_width,
    );
    for draft in &mut drafts {
        draft.item.target_path = path_to_string(
            Path::new(&request.target_root).join(
                draft
                    .item
                    .target_relative_path
                    .replace('/', std::path::MAIN_SEPARATOR_STR),
            ),
        );
        if draft.item.issues.iter().any(|issue| {
            issue.code == "invalid_segment"
                || issue.code == "reserved_name"
                || issue.code == "path_too_long"
                || issue.code == "target_escape"
        }) {
            draft.item.status = OrganizationItemStatus::Error;
        }
    }

    let mut items: Vec<OrganizationPlanItem> = drafts.into_iter().map(|draft| draft.item).collect();
    let mut tree = OrganizationTreeNode {
        name: Path::new(&request.target_root)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&request.target_root)
            .to_string(),
        relative_path: String::new(),
        file_count: 0,
        byte_count: 0,
        children: Vec::new(),
    };
    for item in &items {
        insert_tree_item(&mut tree, &item.target_relative_path, item.file_size);
    }
    let conflict_count = items
        .iter()
        .flat_map(|item| item.issues.iter())
        .filter(|issue| issue.code.contains("conflict") || issue.code == "existing_target")
        .count() as u64;
    let error_count = rule_issues
        .iter()
        .filter(|issue| issue.severity == OrganizationIssueSeverity::Error)
        .count() as u64
        + items
            .iter()
            .flat_map(|item| item.issues.iter())
            .filter(|issue| issue.severity == OrganizationIssueSeverity::Error)
            .count() as u64;
    let warning_count = rule_issues
        .iter()
        .filter(|issue| issue.severity == OrganizationIssueSeverity::Warning)
        .count() as u64
        + items
            .iter()
            .flat_map(|item| item.issues.iter())
            .filter(|issue| issue.severity == OrganizationIssueSeverity::Warning)
            .count() as u64;
    let estimated_bytes = items.iter().map(|item| item.file_size).sum();
    let status = if error_count > 0 {
        "has_errors"
    } else if warning_count > 0 {
        "has_warnings"
    } else {
        "ready"
    };
    let summary = OrganizationPlanSummary {
        plan_id: uuid::Uuid::new_v4().to_string(),
        library_id: request.library_id,
        source_root: source_root.into(),
        target_root: request.target_root.clone(),
        scope: request.scope.clone(),
        item_count: items.len() as u64,
        conflict_count,
        error_count,
        warning_count,
        estimated_bytes,
        target_available_bytes: None,
        generated_at: Utc::now().to_rfc3339(),
        status: status.into(),
        source_snapshot,
        rules: request.rules.clone(),
    };
    // Keep the item order explicit even if the conflict strategy inserted a
    // suffix. This makes regenerated plans stable for the same SQLite state.
    for (index, item) in items.iter_mut().enumerate() {
        item.ordinal = index as u32 + 1;
    }
    Ok(OrganizationPlan {
        summary,
        items,
        tree,
    })
}

pub fn export_manifest(plan: &OrganizationPlan, output_path: &Path, format: &str) -> AppResult<()> {
    let format = format.to_ascii_lowercase();
    if format != "json" && format != "csv" {
        return Err(AppError::InvalidArgument(
            "导出格式必须是 json 或 csv。".into(),
        ));
    }
    let output_normalized = normalize_for_compare(&path_to_string(output_path.to_path_buf()));
    let source_normalized = normalize_for_compare(&plan.summary.source_root);
    if output_normalized == source_normalized
        || output_normalized.starts_with(&format!("{source_normalized}/"))
        || plan
            .items
            .iter()
            .any(|item| normalize_for_compare(&item.source_path) == output_normalized)
    {
        return Err(AppError::UnsafePath(output_path.to_path_buf()));
    }
    if output_path.exists() {
        return Err(AppError::InvalidArgument(format!(
            "导出文件已存在，为避免覆盖请另选路径: {}",
            output_path.display()
        )));
    }
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.is_dir()
    {
        return Err(AppError::InvalidArgument(
            "导出目录不存在；dry-run 不会创建目录。".into(),
        ));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)?;
    match format.as_str() {
        "json" => serde_json::to_writer_pretty(&mut file, plan)?,
        "csv" => write_csv(&mut file, plan)?,
        _ => unreachable!("validated export format"),
    }
    file.flush()?;
    Ok(())
}

fn write_csv(file: &mut impl Write, plan: &OrganizationPlan) -> AppResult<()> {
    writeln!(
        file,
        "ordinal,asset_id,source_path,target_path,status,file_size,issues"
    )?;
    for item in &plan.items {
        let issues = item
            .issues
            .iter()
            .map(|issue| format!("{}: {}", issue.code, issue.detail))
            .collect::<Vec<_>>()
            .join(" | ");
        writeln!(
            file,
            "{},{},{},{},{},{},{}",
            item.ordinal,
            item.asset_id,
            csv_field(&item.source_path),
            csv_field(&item.target_path),
            csv_field(&format!("{:?}", item.status)),
            item.file_size,
            csv_field(&issues),
        )?;
    }
    Ok(())
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn render_context(
    asset: &AssetListItem,
    rules: &OrganizationRules,
    ordinal: u32,
) -> (RenderContext, Vec<OrganizationIssue>) {
    let mut issues = Vec::new();
    let (capture_time, capture_value) =
        resolve_capture_time(asset, &rules.missing_fallback, &mut issues);
    let semantic = asset.classification.primary_category.effective.clone();
    let semantic_value = resolve_value(
        semantic,
        &rules.missing_fallback,
        "semantic_missing",
        &asset.absolute_path,
        &mut issues,
    );
    let camera_model = asset.camera_model.clone().or(asset.camera_make.clone());
    let camera = camera_model.clone().or_else(|| {
        resolve_optional_string(
            None,
            &rules.missing_fallback,
            "camera_missing",
            &asset.absolute_path,
            &mut issues,
        )
    });
    let lens = resolve_optional_string(
        asset.lens_model.clone(),
        &rules.missing_fallback,
        "lens_missing",
        &asset.absolute_path,
        &mut issues,
    );
    let tone = resolve_optional_string(
        asset.tone_label.clone(),
        &rules.missing_fallback,
        "tone_missing",
        &asset.absolute_path,
        &mut issues,
    );
    let color = resolve_optional_string(
        asset.dominant_color_category.clone(),
        &rules.missing_fallback,
        "color_missing",
        &asset.absolute_path,
        &mut issues,
    );
    let saturation = resolve_optional_string(
        asset.saturation_label.clone(),
        &rules.missing_fallback,
        "saturation_missing",
        &asset.absolute_path,
        &mut issues,
    );
    let path = Path::new(&asset.file_name);
    let original_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(&asset.file_name)
        .to_string();
    let extension = asset.extension.trim_start_matches('.').to_string();
    let short_hash = short_hash(&asset_fingerprint(asset));
    let mut values = BTreeMap::new();
    values.insert(
        "capture_time".into(),
        capture_value.clone().unwrap_or_default(),
    );
    values.insert(
        "capture_date".into(),
        capture_time
            .map(|value| value.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
    );
    values.insert(
        "captured_date".into(),
        capture_time
            .map(|value| value.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
    );
    values.insert(
        "captured_time".into(),
        capture_time
            .map(|value| value.format("%H-%M-%S").to_string())
            .unwrap_or_default(),
    );
    values.insert("camera".into(), camera.clone().unwrap_or_default());
    values.insert(
        "camera_make".into(),
        asset.camera_make.clone().unwrap_or_default(),
    );
    values.insert(
        "camera_model".into(),
        asset.camera_model.clone().unwrap_or_default(),
    );
    values.insert("lens".into(), lens.clone().unwrap_or_default());
    values.insert("original_name".into(), asset.file_name.clone());
    values.insert("original_stem".into(), original_stem);
    values.insert("extension".into(), extension);
    values.insert("semantic".into(), semantic_value.clone());
    values.insert("primary_label".into(), semantic_value);
    values.insert("tone".into(), tone.unwrap_or_default());
    values.insert("dominant_color".into(), color.unwrap_or_default());
    values.insert("saturation".into(), saturation.unwrap_or_default());
    values.insert("sequence".into(), ordinal.to_string());
    values.insert("short_hash".into(), short_hash);
    (
        RenderContext {
            values,
            capture_time,
        },
        issues,
    )
}

fn render_levels(
    asset: &AssetListItem,
    rules: &OrganizationRules,
    context: &RenderContext,
    issues: &mut Vec<OrganizationIssue>,
) -> Vec<String> {
    let mut levels = Vec::new();
    for level in &rules.levels {
        let value = match level.kind {
            OrganizationLevelKind::Year => context
                .capture_time
                .map(|time| time.format("%Y").to_string()),
            OrganizationLevelKind::Month => context
                .capture_time
                .map(|time| time.format("%m").to_string()),
            OrganizationLevelKind::Day => context
                .capture_time
                .map(|time| time.format("%d").to_string()),
            OrganizationLevelKind::OriginalDirectory => Path::new(&asset.relative_path)
                .parent()
                .filter(|parent| *parent != Path::new("."))
                .map(|parent| parent.to_string_lossy().replace('\\', "/")),
            OrganizationLevelKind::PrimarySemantic => context.values.get("semantic").cloned(),
            OrganizationLevelKind::Tone => context.values.get("tone").cloned(),
            OrganizationLevelKind::DominantColor => context.values.get("dominant_color").cloned(),
            OrganizationLevelKind::Saturation => context.values.get("saturation").cloned(),
        };
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            for segment in value.split('/') {
                if !segment.is_empty() {
                    levels.push(segment.to_string());
                }
            }
            continue;
        }
        match level.fallback {
            OrganizationMissingFallback::ModificationTime => {
                if matches!(
                    level.kind,
                    OrganizationLevelKind::Year
                        | OrganizationLevelKind::Month
                        | OrganizationLevelKind::Day
                ) && let Some(time) = modified_time(asset.modified_at)
                {
                    let value = match level.kind {
                        OrganizationLevelKind::Year => time.format("%Y").to_string(),
                        OrganizationLevelKind::Month => time.format("%m").to_string(),
                        OrganizationLevelKind::Day => time.format("%d").to_string(),
                        _ => String::new(),
                    };
                    levels.push(value);
                    continue;
                }
                issues.push(issue(
                    "missing_metadata",
                    OrganizationIssueSeverity::Warning,
                    Some(asset.absolute_path.clone()),
                    None,
                    &format!(
                        "目录维度 {} 缺少元数据且无法使用修改时间回退。",
                        level.kind.as_str()
                    ),
                ));
            }
            OrganizationMissingFallback::Unknown => levels.push("unknown".into()),
            OrganizationMissingFallback::Skip => {}
            OrganizationMissingFallback::Block => issues.push(issue(
                "missing_metadata",
                OrganizationIssueSeverity::Error,
                Some(asset.absolute_path.clone()),
                None,
                &format!("目录维度 {} 缺少必需元数据。", level.kind.as_str()),
            )),
        }
    }
    levels
}

fn resolve_capture_time(
    asset: &AssetListItem,
    fallback: &OrganizationMissingFallback,
    issues: &mut Vec<OrganizationIssue>,
) -> (Option<DateTime<Utc>>, Option<String>) {
    if let Some(value) = asset.capture_time.as_deref().and_then(parse_capture_time) {
        return (Some(value), asset.capture_time.clone());
    }
    match fallback {
        OrganizationMissingFallback::ModificationTime => {
            let value = modified_time(asset.modified_at);
            if value.is_none() {
                issues.push(issue(
                    "capture_time_missing",
                    OrganizationIssueSeverity::Warning,
                    Some(asset.absolute_path.clone()),
                    None,
                    "缺少拍摄时间，修改时间也不可用。",
                ));
            }
            (value, value.as_ref().map(|time| time.to_rfc3339()))
        }
        OrganizationMissingFallback::Unknown | OrganizationMissingFallback::Skip => {
            issues.push(issue(
                "capture_time_missing",
                OrganizationIssueSeverity::Warning,
                Some(asset.absolute_path.clone()),
                None,
                "缺少拍摄时间，已按规则回退。",
            ));
            (None, Some("unknown".into()))
        }
        OrganizationMissingFallback::Block => {
            issues.push(issue(
                "capture_time_missing",
                OrganizationIssueSeverity::Error,
                Some(asset.absolute_path.clone()),
                None,
                "缺少拍摄时间，当前回退策略要求阻止此项。",
            ));
            (None, None)
        }
    }
}

fn resolve_value(
    value: Option<String>,
    fallback: &OrganizationMissingFallback,
    code: &str,
    source_path: &str,
    issues: &mut Vec<OrganizationIssue>,
) -> String {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        return value;
    }
    match fallback {
        OrganizationMissingFallback::Unknown | OrganizationMissingFallback::ModificationTime => {
            "unknown".into()
        }
        OrganizationMissingFallback::Skip => String::new(),
        OrganizationMissingFallback::Block => {
            issues.push(issue(
                code,
                OrganizationIssueSeverity::Error,
                Some(source_path.into()),
                None,
                "缺少命名所需元数据。",
            ));
            String::new()
        }
    }
}

fn resolve_optional_string(
    value: Option<String>,
    fallback: &OrganizationMissingFallback,
    code: &str,
    source_path: &str,
    issues: &mut Vec<OrganizationIssue>,
) -> Option<String> {
    let value = resolve_value(value, fallback, code, source_path, issues);
    (!value.is_empty()).then_some(value)
}

fn apply_conflicts(
    drafts: &mut [DraftItem],
    target_root: &str,
    strategy: &OrganizationConflictStrategy,
    sequence_start: u32,
    sequence_width: u8,
) {
    let mut assigned: HashSet<String> = HashSet::new();
    let mut collision_sequences: HashMap<String, u32> = HashMap::new();
    for draft in drafts {
        let original = draft.item.target_relative_path.clone();
        let mut normalized = normalize_for_compare(&original);
        let existing = Path::new(target_root)
            .join(original.replace('/', std::path::MAIN_SEPARATOR_STR))
            .exists();
        if !draft.valid_target {
            assigned.insert(normalized);
            continue;
        }
        if !existing && !assigned.contains(&normalized) {
            assigned.insert(normalized);
            continue;
        }
        draft.item.issues.push(issue(
            if existing {
                "existing_target"
            } else {
                "duplicate_target"
            },
            if *strategy == OrganizationConflictStrategy::Skip {
                OrganizationIssueSeverity::Error
            } else {
                OrganizationIssueSeverity::Warning
            },
            Some(draft.item.source_path.clone()),
            Some(path_to_string(
                Path::new(target_root).join(original.replace('/', std::path::MAIN_SEPARATOR_STR)),
            )),
            if existing {
                "目标文件已经存在；当前策略将生成预览解决方案。"
            } else {
                "多个源文件映射到同一目标路径。"
            },
        ));
        match strategy {
            OrganizationConflictStrategy::Skip => {
                draft.item.status = OrganizationItemStatus::SkippedConflict;
            }
            OrganizationConflictStrategy::Sequence => {
                let counter = collision_sequences
                    .entry(normalized.clone())
                    .or_insert(sequence_start);
                let extension = Path::new(&original)
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| format!(".{value}"))
                    .unwrap_or_default();
                let stem = original
                    .strip_suffix(&extension)
                    .unwrap_or(&original)
                    .to_string();
                loop {
                    let suffix = format!("_{:0width$}", *counter, width = sequence_width as usize);
                    *counter = counter.saturating_add(1);
                    let candidate = format!("{stem}{suffix}{extension}");
                    let candidate_normalized = normalize_for_compare(&candidate);
                    let candidate_exists = Path::new(target_root)
                        .join(candidate.replace('/', std::path::MAIN_SEPARATOR_STR))
                        .exists();
                    if !assigned.contains(&candidate_normalized) && !candidate_exists {
                        draft.item.target_relative_path = candidate;
                        normalized = candidate_normalized;
                        draft.item.status = OrganizationItemStatus::Warning;
                        break;
                    }
                }
                assigned.insert(normalized);
            }
            OrganizationConflictStrategy::ShortHash => {
                let extension = Path::new(&original)
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| format!(".{value}"))
                    .unwrap_or_default();
                let stem = original
                    .strip_suffix(&extension)
                    .unwrap_or(&original)
                    .to_string();
                let candidate = format!(
                    "{stem}_{}{}",
                    short_hash(&draft.item.source_fingerprint),
                    extension
                );
                let candidate_normalized = normalize_for_compare(&candidate);
                draft.item.target_relative_path = candidate;
                draft.item.status = OrganizationItemStatus::Warning;
                assigned.insert(candidate_normalized);
            }
        }
    }
}

fn validate_target_boundary(source_root: &str, target_root: &str) -> AppResult<()> {
    let source = normalize_for_compare(source_root);
    let target = normalize_for_compare(target_root);
    if target.is_empty() || target == source || target.starts_with(&format!("{source}/")) {
        return Err(AppError::UnsafePath(PathBuf::from(target_root)));
    }
    Ok(())
}

fn validate_relative_target(
    relative: &str,
    target_root: &str,
    source_path: &str,
    issues: &mut Vec<OrganizationIssue>,
) -> bool {
    let mut valid = true;
    if relative.is_empty()
        || relative.starts_with('/')
        || relative.starts_with('\\')
        || relative.contains(':')
    {
        issues.push(issue(
            "target_escape",
            OrganizationIssueSeverity::Error,
            Some(source_path.into()),
            Some(relative.into()),
            "目标相对路径必须是相对路径，不能包含盘符、绝对路径或路径分隔符注入。",
        ));
        valid = false;
    }
    for segment in relative.split('/') {
        if segment == "." || segment == ".." || segment.contains('\\') {
            issues.push(issue(
                "target_escape",
                OrganizationIssueSeverity::Error,
                Some(source_path.into()),
                Some(relative.into()),
                "目标路径包含 .、.. 或反斜杠，已阻止。",
            ));
            valid = false;
        }
        if segment.encode_utf16().count() > MAX_SEGMENT_UNITS {
            issues.push(issue(
                "segment_too_long",
                OrganizationIssueSeverity::Error,
                Some(source_path.into()),
                Some(relative.into()),
                "目标路径段超过 Windows 255 个 UTF-16 单元。",
            ));
            valid = false;
        }
        if has_invalid_windows_segment(segment) {
            issues.push(issue(
                if is_reserved_name(segment) {
                    "reserved_name"
                } else {
                    "invalid_segment"
                },
                OrganizationIssueSeverity::Error,
                Some(source_path.into()),
                Some(relative.into()),
                "目标路径包含 Windows 非法字符、尾随空格/句点或保留名称。",
            ));
            valid = false;
        }
    }
    let full = path_to_string(
        Path::new(target_root).join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)),
    );
    if full.encode_utf16().count() > MAX_PATH_UNITS {
        issues.push(issue(
            "path_too_long",
            OrganizationIssueSeverity::Error,
            Some(source_path.into()),
            Some(full),
            "目标完整路径超过 Windows 260 个 UTF-16 单元。",
        ));
        valid = false;
    }
    valid
}

fn has_invalid_windows_segment(segment: &str) -> bool {
    segment.is_empty()
        || segment.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
        || segment.ends_with(' ')
        || segment.ends_with('.')
        || is_reserved_name(segment)
}

fn is_reserved_name(segment: &str) -> bool {
    let base = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && base.as_bytes()[3].is_ascii_digit()
            && base.as_bytes()[3] != b'0')
}

fn parse_template(template: &str) -> Result<Vec<TemplateToken>, String> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while let Some(start) = template[cursor..].find('{') {
        let start = cursor + start;
        if start > cursor {
            tokens.push(TemplateToken::Literal(template[cursor..start].into()));
        }
        let Some(end_offset) = template[start + 1..].find('}') else {
            return Err("命名模板缺少右花括号。".into());
        };
        let end = start + 1 + end_offset;
        let inner = &template[start + 1..end];
        if inner.trim().is_empty() {
            return Err("命名模板包含空变量。".into());
        }
        let mut parts = inner.splitn(2, ':');
        let name = parts.next().unwrap_or_default().trim();
        if name.is_empty() {
            return Err("命名模板变量名不能为空。".into());
        }
        tokens.push(TemplateToken::Variable {
            name: name.into(),
            format: parts
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        });
        cursor = end + 1;
    }
    if cursor < template.len() {
        tokens.push(TemplateToken::Literal(template[cursor..].into()));
    }
    Ok(tokens)
}

fn render_template(
    tokens: &[TemplateToken],
    context: &RenderContext,
    ordinal: u32,
) -> Result<String, String> {
    let mut rendered = String::new();
    for token in tokens {
        match token {
            TemplateToken::Literal(value) => rendered.push_str(value),
            TemplateToken::Variable { name, format } => {
                let value = if name == "sequence" {
                    format
                        .as_deref()
                        .map(|pattern| format_sequence(ordinal, pattern))
                        .unwrap_or_else(|| ordinal.to_string())
                } else if name == "capture_time" {
                    if let Some(time) = context.capture_time {
                        format
                            .as_deref()
                            .map(|pattern| format_capture_time(time, pattern))
                            .unwrap_or_else(|| time.to_rfc3339())
                    } else {
                        context.values.get(name).cloned().unwrap_or_default()
                    }
                } else {
                    context.values.get(name).cloned().unwrap_or_default()
                };
                rendered.push_str(&value);
            }
        }
    }
    if rendered.is_empty() {
        return Err("命名模板渲染结果为空。".into());
    }
    Ok(rendered)
}

fn format_sequence(value: u32, pattern: &str) -> String {
    let width = pattern
        .chars()
        .filter(|character| *character == '0')
        .count()
        .max(1);
    format!("{value:0width$}")
}

fn format_capture_time(value: DateTime<Utc>, pattern: &str) -> String {
    let mut format = pattern.to_string();
    for (token, replacement) in [
        ("yyyy", "%Y"),
        ("MM", "%m"),
        ("dd", "%d"),
        ("HH", "%H"),
        ("mm", "%M"),
        ("ss", "%S"),
    ] {
        format = format.replace(token, replacement);
    }
    value.format(&format).to_string()
}

fn parse_capture_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|value| DateTime::<Utc>::from_naive_utc_and_offset(value, Utc))
        })
}

fn modified_time(value: i64) -> Option<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(value, 0)
}

fn status_for_issues(issues: &[OrganizationIssue]) -> OrganizationItemStatus {
    if issues
        .iter()
        .any(|issue| issue.severity == OrganizationIssueSeverity::Error)
    {
        OrganizationItemStatus::Error
    } else if issues.is_empty() {
        OrganizationItemStatus::Ready
    } else {
        OrganizationItemStatus::Warning
    }
}

fn insert_tree_item(root: &mut OrganizationTreeNode, relative: &str, size: u64) {
    root.file_count += 1;
    root.byte_count += size;
    let mut node = root;
    let mut path = String::new();
    for segment in relative.split('/').filter(|value| !value.is_empty()) {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(segment);
        let index = node
            .children
            .iter()
            .position(|child| child.name == segment)
            .unwrap_or_else(|| {
                node.children.push(OrganizationTreeNode {
                    name: segment.into(),
                    relative_path: path.clone(),
                    file_count: 0,
                    byte_count: 0,
                    children: Vec::new(),
                });
                node.children.len() - 1
            });
        node = &mut node.children[index];
        node.file_count += 1;
        node.byte_count += size;
    }
}

fn source_snapshot(assets: &[AssetListItem]) -> String {
    let mut value = String::new();
    for asset in assets {
        value.push_str(&asset.id.to_string());
        value.push(':');
        value.push_str(&asset_fingerprint(asset));
        value.push(';');
    }
    blake3::hash(value.as_bytes()).to_hex().to_string()
}

fn asset_fingerprint(asset: &AssetListItem) -> String {
    // The list projection intentionally does not expose the private DB
    // fingerprint. A stable path/size/mtime digest is enough for a preview
    // audit and avoids pretending this is a fresh content hash.
    blake3::hash(
        format!(
            "{}:{}:{}",
            asset.absolute_path, asset.file_size, asset.modified_at
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string()
}

fn short_hash(value: &str) -> String {
    value.chars().take(8).collect()
}

fn normalize_for_compare(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_end_matches('/')
        .nfc()
        .map(|(character, _)| character)
        .collect::<String>()
        .to_lowercase()
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy()
        .replace('\\', std::path::MAIN_SEPARATOR_STR)
}

fn issue(
    code: &str,
    severity: OrganizationIssueSeverity,
    source_path: Option<String>,
    target_path: Option<String>,
    detail: &str,
) -> OrganizationIssue {
    OrganizationIssue {
        code: code.into(),
        severity,
        source_path,
        target_path,
        detail: detail.into(),
    }
}

fn rule_issue(code: &str, detail: &str) -> OrganizationIssue {
    issue(code, OrganizationIssueSeverity::Error, None, None, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AssetFilter, OrganizationLevel, OrganizationScope, SemanticMatchMode};

    fn asset(id: i64, root: &Path, name: &str, capture_time: Option<&str>) -> AssetListItem {
        let path = root.join(name);
        std::fs::write(&path, format!("fixture-{id}")).expect("fixture file");
        AssetListItem {
            id,
            library_id: 1,
            absolute_path: path.to_string_lossy().into_owned(),
            relative_path: name.into(),
            file_name: name.into(),
            extension: "jpg".into(),
            file_size: format!("fixture-{id}").len() as i64,
            modified_at: 1_754_500_000,
            width: None,
            height: None,
            orientation: None,
            capture_time: capture_time.map(str::to_string),
            camera_make: None,
            camera_model: None,
            lens_model: None,
            exposure_time: None,
            aperture: None,
            iso: None,
            focal_length: None,
            file_status: "present".into(),
            scan_status: "indexed".into(),
            analysis_status: "completed".into(),
            error_message: None,
            thumbnail_available: false,
            brightness: None,
            contrast: None,
            tone_label: Some("balanced".into()),
            saturation: None,
            chroma: None,
            saturation_label: Some("medium".into()),
            dominant_color: None,
            dominant_color_category: Some("blue".into()),
            color_palette: None,
            neutral_ratio: Some(0.2),
            dominant_color_coverage: Some(0.5),
            semantic_status: "completed".into(),
            semantic_error: None,
            semantic_analyzed_at: None,
            rating: 0,
            color_label: None,
            is_favorite: false,
            semantic_labels: Vec::new(),
            classification: crate::classification::EffectiveClassification::default(),
        }
    }

    fn request(target_root: &Path, rules: OrganizationRules) -> OrganizationPlanRequest {
        OrganizationPlanRequest {
            library_id: 1,
            target_root: target_root.to_string_lossy().into_owned(),
            scope: OrganizationScope::All,
            filter: AssetFilter {
                semantic_match: SemanticMatchMode::Any,
                ..AssetFilter::default()
            },
            selected_asset_ids: Vec::new(),
            rules,
        }
    }

    #[test]
    fn modification_time_fallback_is_only_valid_for_date_dimensions() {
        let rules = OrganizationRules {
            levels: vec![OrganizationLevel {
                kind: OrganizationLevelKind::PrimarySemantic,
                fallback: OrganizationMissingFallback::ModificationTime,
            }],
            ..OrganizationRules::default()
        };
        let issues = validate_rules(&rules);
        assert!(issues.iter().any(|issue| {
            issue.code == "invalid_level_fallback"
                && issue.detail.contains("仅适用于拍摄年份、月份或日期")
        }));
    }

    #[test]
    fn target_inside_source_is_rejected_without_creating_it() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("图库");
        std::fs::create_dir(&source).expect("source");
        let target = source.join("整理结果");
        let result = build_plan(
            &request(&target, OrganizationRules::default()),
            &source.to_string_lossy(),
            Vec::new(),
        );
        assert!(matches!(result, Err(AppError::UnsafePath(_))));
        assert!(!target.exists());
    }

    #[test]
    fn deterministic_sequence_strategy_does_not_create_target_files() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("源 中文 😀");
        std::fs::create_dir(&source).expect("source");
        let target = temp.path().join("目标 русский").join("not-created");
        let rules = OrganizationRules {
            levels: Vec::new(),
            template: "same".into(),
            ..OrganizationRules::default()
        };
        let assets = vec![
            asset(2, &source, "二.jpg", Some("2026-08-05T18:30:00")),
            asset(1, &source, "一.jpg", None),
        ];
        let source_before = std::fs::read(source.join("一.jpg")).expect("source bytes");
        let first = build_plan(
            &request(&target, rules.clone()),
            &source.to_string_lossy(),
            assets.clone(),
        )
        .expect("first plan");
        let second = build_plan(&request(&target, rules), &source.to_string_lossy(), assets)
            .expect("second plan");
        let first_targets: Vec<_> = first
            .items
            .iter()
            .map(|item| item.target_relative_path.clone())
            .collect();
        let second_targets: Vec<_> = second
            .items
            .iter()
            .map(|item| item.target_relative_path.clone())
            .collect();
        assert_eq!(first_targets, second_targets);
        assert_eq!(first_targets, vec!["same.jpg", "same_0001.jpg"]);
        assert!(!target.exists());
        assert_eq!(
            std::fs::read(source.join("一.jpg")).expect("source bytes"),
            source_before
        );
    }

    #[test]
    fn invalid_windows_name_and_missing_capture_are_reported() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        std::fs::create_dir(&source).expect("source");
        let target = temp.path().join("target");
        std::fs::create_dir(&target).expect("target");
        let rules = OrganizationRules {
            levels: vec![OrganizationLevel {
                kind: OrganizationLevelKind::Year,
                fallback: OrganizationMissingFallback::ModificationTime,
            }],
            template: "CON*".into(),
            ..OrganizationRules::default()
        };
        let plan = build_plan(
            &request(&target, rules),
            &source.to_string_lossy(),
            vec![asset(1, &source, "фото.jpg", None)],
        )
        .expect("plan");
        let codes: HashSet<_> = plan.items[0]
            .issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect();
        assert!(codes.contains("invalid_segment") || codes.contains("reserved_name"));
        assert!(codes.contains("capture_time_missing"));
    }

    #[test]
    fn existing_target_uses_selected_conflict_strategy_without_overwrite() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        std::fs::create_dir(&source).expect("source");
        std::fs::create_dir(&target).expect("target");
        std::fs::write(target.join("same.jpg"), b"existing").expect("existing target");
        let rules = OrganizationRules {
            levels: Vec::new(),
            template: "same".into(),
            conflict_strategy: OrganizationConflictStrategy::ShortHash,
            ..OrganizationRules::default()
        };
        let plan = build_plan(
            &request(&target, rules),
            &source.to_string_lossy(),
            vec![asset(1, &source, "source.jpg", None)],
        )
        .expect("plan");
        assert_ne!(plan.items[0].target_relative_path, "same.jpg");
        assert!(
            plan.items[0]
                .issues
                .iter()
                .any(|issue| issue.code == "existing_target")
        );
        assert_eq!(
            std::fs::read(target.join("same.jpg")).expect("existing target"),
            b"existing"
        );
    }

    #[test]
    fn manifest_export_is_new_file_only_and_never_targets_source_root() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        std::fs::create_dir(&source).expect("source");
        std::fs::create_dir(&target).expect("target");
        let plan = build_plan(
            &request(&target, OrganizationRules::default()),
            &source.to_string_lossy(),
            vec![asset(1, &source, "source.jpg", None)],
        )
        .expect("plan");
        let manifest = target.join("dry-run.json");
        export_manifest(&plan, &manifest, "json").expect("export");
        assert!(manifest.is_file());
        assert!(export_manifest(&plan, &manifest, "json").is_err());
        assert!(matches!(
            export_manifest(&plan, &source.join("manifest.json"), "json"),
            Err(AppError::UnsafePath(_))
        ));
    }

    #[test]
    fn comparison_normalizes_case_and_canonical_unicode() {
        assert_eq!(
            normalize_for_compare("Cafe\u{301}"),
            normalize_for_compare("Café")
        );
        assert_eq!(
            normalize_for_compare("照片\\A.JPG"),
            normalize_for_compare("照片/a.jpg")
        );
    }
}
