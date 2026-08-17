use std::collections::{BTreeSet, HashMap};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use chrono::{SecondsFormat, Utc};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat, Rgba};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::Repository;
use crate::error::{AppError, AppResult};
use crate::semantic::{SemanticClassifier, SemanticError, default_topic_model_metadata};
use crate::source_identity::{identity_key, is_same_or_descendant};

const ASSET_PROJECTION: &str = "
    a.id, a.library_id, a.file_name, a.extension, a.file_size,
    a.width, a.height, a.capture_time, a.rating, a.color_label,
    a.is_favorite,
    EXISTS(
        SELECT 1 FROM thumbnails t
        WHERE t.asset_id=a.id AND t.status='ready'
    )";

const LIBRARY_SCOPE_FILTER: &str = "COALESCE(
    (SELECT assignment.library_id
     FROM asset_library_assignments assignment
     WHERE assignment.asset_id=a.id),
    a.library_id
) IN (
    WITH RECURSIVE library_scope(library_id) AS (
        SELECT id FROM libraries WHERE id=?1
        UNION
        SELECT child.id
        FROM libraries child
        JOIN library_scope scope ON child.parent_library_id=scope.library_id
    )
    SELECT library_id FROM library_scope
)";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAsset {
    pub id: i64,
    pub library_id: i64,
    pub file_name: String,
    pub extension: String,
    pub file_size: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub capture_time: Option<String>,
    pub rating: i64,
    pub color_label: Option<String>,
    pub is_favorite: bool,
    pub thumbnail_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub asset_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDetail {
    #[serde(flatten)]
    pub summary: CollectionSummary,
    pub assets: Vec<WorkflowAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub fingerprint: String,
    pub assets: Vec<WorkflowAsset>,
    pub total_bytes: i64,
    pub reclaimable_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SimilarAsset {
    #[serde(flatten)]
    pub asset: WorkflowAsset,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSearchResponse {
    pub query: String,
    pub normalized_query: String,
    pub embedded_asset_count: usize,
    pub items: Vec<SimilarAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SimilarityCluster {
    pub id: String,
    pub assets: Vec<SimilarAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SimilarityClusterResponse {
    pub clusters: Vec<SimilarityCluster>,
    pub embedded_asset_count: usize,
    pub candidate_pair_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FaceFeatureStatus {
    pub status: String,
    pub message: String,
    pub enabled: bool,
    pub model_installed: bool,
    pub detection_count: i64,
    pub cluster_count: i64,
    pub privacy_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CropRecipe {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditRecipe {
    #[serde(default)]
    pub rotate_degrees: i32,
    #[serde(default)]
    pub flip_horizontal: bool,
    #[serde(default)]
    pub flip_vertical: bool,
    #[serde(default)]
    pub crop: Option<CropRecipe>,
    #[serde(default)]
    pub exposure: f32,
    #[serde(default)]
    pub contrast: f32,
    #[serde(default)]
    pub saturation: f32,
}

impl Default for EditRecipe {
    fn default() -> Self {
        Self {
            rotate_degrees: 0,
            flip_horizontal: false,
            flip_vertical: false,
            crop: None,
            exposure: 0.0,
            contrast: 0.0,
            saturation: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditExportPlan {
    pub plan_id: String,
    pub asset_id: i64,
    pub source_path: String,
    pub target_path: String,
    pub source_fingerprint: String,
    pub recipe: EditRecipe,
    pub status: String,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EditExportResult {
    pub plan_id: String,
    pub target_path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EditRollbackPlan {
    pub plan_id: String,
    pub target_path: String,
    pub target_hash: String,
    pub status: String,
    pub issues: Vec<String>,
}

#[derive(Debug)]
struct EmbeddedAsset {
    asset: WorkflowAsset,
    vector: Vec<f32>,
}

pub fn list_favorite_asset_ids(repository: &Repository, library_id: i64) -> AppResult<Vec<i64>> {
    let connection = open(repository)?;
    let mut statement = connection.prepare(&format!(
        "SELECT a.id FROM assets a
         WHERE {LIBRARY_SCOPE_FILTER}
           AND a.file_status='present' AND a.is_favorite=1
         ORDER BY a.id"
    ))?;
    Ok(statement
        .query_map([library_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

pub fn list_favorite_assets(
    repository: &Repository,
    library_id: i64,
) -> AppResult<Vec<WorkflowAsset>> {
    let connection = open(repository)?;
    query_assets(
        &connection,
        &format!(
            "SELECT {ASSET_PROJECTION} FROM assets a
             WHERE {LIBRARY_SCOPE_FILTER}
               AND a.file_status='present' AND a.is_favorite=1
             ORDER BY COALESCE(a.capture_time, ''), a.id DESC"
        ),
        [library_id],
    )
}

pub fn set_favorite(repository: &Repository, asset_id: i64, favorite: bool) -> AppResult<bool> {
    let connection = open(repository)?;
    let changed = connection.execute(
        "UPDATE assets SET is_favorite=?2 WHERE id=?1",
        params![asset_id, favorite],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(format!("asset {asset_id}")));
    }
    Ok(favorite)
}

pub fn list_collections(repository: &Repository) -> AppResult<Vec<CollectionSummary>> {
    let connection = open(repository)?;
    let mut statement = connection.prepare(
        "SELECT c.id, c.name, c.description, c.created_at, c.updated_at, COUNT(ca.asset_id)
         FROM collections c
         LEFT JOIN collection_assets ca ON ca.collection_id=c.id
         GROUP BY c.id
         ORDER BY c.name COLLATE NOCASE, c.id",
    )?;
    Ok(statement
        .query_map([], map_collection)?
        .collect::<Result<Vec<_>, _>>()?)
}

pub fn create_collection(
    repository: &Repository,
    name: &str,
    description: &str,
) -> AppResult<CollectionSummary> {
    let name = validate_collection_name(name)?;
    let description = description.trim();
    if description.chars().count() > 500 {
        return Err(AppError::InvalidArgument(
            "collection description must be 500 characters or fewer".into(),
        ));
    }
    let connection = open(repository)?;
    let timestamp = now();
    connection.execute(
        "INSERT INTO collections(name, description, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?3)",
        params![name, description, timestamp],
    )?;
    collection_summary(&connection, connection.last_insert_rowid())
}

pub fn delete_collection(repository: &Repository, collection_id: i64) -> AppResult<bool> {
    let connection = open(repository)?;
    Ok(connection.execute("DELETE FROM collections WHERE id=?1", [collection_id])? > 0)
}

pub fn add_assets_to_collection(
    repository: &Repository,
    collection_id: i64,
    asset_ids: &[i64],
) -> AppResult<CollectionSummary> {
    let mut connection = open(repository)?;
    let transaction = connection.transaction()?;
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM collections WHERE id=?1)",
        [collection_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::NotFound(format!("collection {collection_id}")));
    }
    let timestamp = now();
    for asset_id in asset_ids.iter().copied().collect::<BTreeSet<_>>() {
        transaction.execute(
            "INSERT OR IGNORE INTO collection_assets(collection_id, asset_id, added_at)
             SELECT ?1, id, ?3 FROM assets WHERE id=?2",
            params![collection_id, asset_id, timestamp],
        )?;
    }
    transaction.execute(
        "UPDATE collections SET updated_at=?2 WHERE id=?1",
        params![collection_id, timestamp],
    )?;
    transaction.commit()?;
    let connection = open(repository)?;
    collection_summary(&connection, collection_id)
}

pub fn remove_assets_from_collection(
    repository: &Repository,
    collection_id: i64,
    asset_ids: &[i64],
) -> AppResult<CollectionSummary> {
    let mut connection = open(repository)?;
    let transaction = connection.transaction()?;
    for asset_id in asset_ids.iter().copied().collect::<BTreeSet<_>>() {
        transaction.execute(
            "DELETE FROM collection_assets WHERE collection_id=?1 AND asset_id=?2",
            params![collection_id, asset_id],
        )?;
    }
    transaction.execute(
        "UPDATE collections SET updated_at=?2 WHERE id=?1",
        params![collection_id, now()],
    )?;
    transaction.commit()?;
    let connection = open(repository)?;
    collection_summary(&connection, collection_id)
}

pub fn get_collection(repository: &Repository, collection_id: i64) -> AppResult<CollectionDetail> {
    let connection = open(repository)?;
    let summary = collection_summary(&connection, collection_id)?;
    let assets = query_assets(
        &connection,
        &format!(
            "SELECT {ASSET_PROJECTION} FROM assets a
             JOIN collection_assets ca ON ca.asset_id=a.id
             WHERE ca.collection_id=?1 AND a.file_status='present'
             ORDER BY ca.added_at DESC, a.id DESC"
        ),
        [collection_id],
    )?;
    Ok(CollectionDetail { summary, assets })
}

pub fn list_duplicate_groups(
    repository: &Repository,
    library_id: i64,
    limit: u32,
) -> AppResult<Vec<DuplicateGroup>> {
    let connection = open(repository)?;
    let limit = i64::from(limit.clamp(1, 200));
    let mut statement = connection.prepare(&format!(
        "SELECT a.fingerprint, SUM(a.file_size), MAX(a.file_size)
         FROM assets a
         WHERE {LIBRARY_SCOPE_FILTER}
           AND a.file_status='present' AND a.fingerprint<>''
         GROUP BY fingerprint
         HAVING COUNT(*) > 1
         ORDER BY (SUM(file_size) - MAX(file_size)) DESC, fingerprint
         LIMIT ?2"
    ))?;
    let summaries = statement
        .query_map(params![library_id, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut groups = Vec::with_capacity(summaries.len());
    for (fingerprint, total_bytes, largest_file) in summaries {
        let assets = query_assets(
            &connection,
            &format!(
                "SELECT {ASSET_PROJECTION} FROM assets a
                 WHERE {LIBRARY_SCOPE_FILTER}
                   AND a.fingerprint=?2 AND a.file_status='present'
                 ORDER BY a.is_favorite DESC, a.rating DESC, a.capture_time, a.id"
            ),
            params![library_id, fingerprint],
        )?;
        groups.push(DuplicateGroup {
            fingerprint,
            assets,
            total_bytes,
            reclaimable_bytes: total_bytes.saturating_sub(largest_file),
        });
    }
    Ok(groups)
}

pub fn search_by_text(
    repository: &Repository,
    classifier: &Arc<dyn SemanticClassifier>,
    library_id: i64,
    query: &str,
    limit: u32,
    minimum_similarity: f32,
) -> AppResult<LocalSearchResponse> {
    let query = query.trim();
    if query.is_empty() {
        return Err(AppError::InvalidArgument(
            "search query cannot be empty".into(),
        ));
    }
    if !(-1.0..=1.0).contains(&minimum_similarity) {
        return Err(AppError::InvalidArgument(
            "minimum similarity must be between -1 and 1".into(),
        ));
    }
    let normalized_query = normalize_local_query(query);
    let query_vector = classifier
        .encode_text(std::slice::from_ref(&normalized_query))
        .map_err(semantic_app_error)?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::InvalidArgument("model returned no text embedding".into()))?;
    let embedded = list_embedded_assets(repository, library_id, 10_001)?;
    let mut items = embedded
        .iter()
        .map(|candidate| SimilarAsset {
            asset: candidate.asset.clone(),
            similarity: cosine_similarity(&query_vector, &candidate.vector),
        })
        .filter(|candidate| candidate.similarity >= minimum_similarity)
        .collect::<Vec<_>>();
    items.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
    items.truncate(limit.clamp(1, 200) as usize);
    Ok(LocalSearchResponse {
        query: query.into(),
        normalized_query,
        embedded_asset_count: embedded.len(),
        items,
    })
}

pub fn find_similar_assets(
    repository: &Repository,
    library_id: i64,
    asset_id: i64,
    limit: u32,
    minimum_similarity: f32,
) -> AppResult<Vec<SimilarAsset>> {
    if !(0.0..=1.0).contains(&minimum_similarity) {
        return Err(AppError::InvalidArgument(
            "minimum similarity must be between 0 and 1".into(),
        ));
    }
    let embedded = list_embedded_assets(repository, library_id, 10_001)?;
    let reference = embedded
        .iter()
        .find(|candidate| candidate.asset.id == asset_id)
        .ok_or_else(|| AppError::NotFound(format!("embedding for asset {asset_id}")))?;
    let mut items = embedded
        .iter()
        .filter(|candidate| candidate.asset.id != asset_id)
        .map(|candidate| SimilarAsset {
            asset: candidate.asset.clone(),
            similarity: cosine_similarity(&reference.vector, &candidate.vector),
        })
        .filter(|candidate| candidate.similarity >= minimum_similarity)
        .collect::<Vec<_>>();
    items.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
    items.truncate(limit.clamp(1, 200) as usize);
    Ok(items)
}

pub fn build_similarity_clusters(
    repository: &Repository,
    library_id: i64,
    threshold: f32,
) -> AppResult<SimilarityClusterResponse> {
    if !(0.75..=0.999).contains(&threshold) {
        return Err(AppError::InvalidArgument(
            "cluster threshold must be between 0.75 and 0.999".into(),
        ));
    }
    const MAX_EMBEDDINGS: usize = 5_000;
    let mut embedded = list_embedded_assets(repository, library_id, MAX_EMBEDDINGS + 1)?;
    let truncated = embedded.len() > MAX_EMBEDDINGS;
    embedded.truncate(MAX_EMBEDDINGS);
    for item in &mut embedded {
        normalize_vector(&mut item.vector);
    }

    let mut buckets: HashMap<(usize, bool), Vec<usize>> = HashMap::new();
    for (index, item) in embedded.iter().enumerate() {
        for dimension in top_dimensions(&item.vector, 4) {
            buckets
                .entry((dimension, item.vector[dimension].is_sign_positive()))
                .or_default()
                .push(index);
        }
    }
    let mut candidate_pairs = BTreeSet::new();
    for ((dimension, _), indices) in &mut buckets {
        indices.sort_by(|left, right| {
            embedded[*left].vector[*dimension]
                .abs()
                .total_cmp(&embedded[*right].vector[*dimension].abs())
        });
        for left_index in 0..indices.len() {
            for right_index in left_index + 1..(left_index + 97).min(indices.len()) {
                let left = indices[left_index];
                let right = indices[right_index];
                candidate_pairs.insert((left.min(right), left.max(right)));
            }
        }
    }

    let mut parents = (0..embedded.len()).collect::<Vec<_>>();
    for &(left, right) in &candidate_pairs {
        let similarity = dot_product(&embedded[left].vector, &embedded[right].vector);
        if similarity >= threshold {
            union(&mut parents, left, right);
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for index in 0..embedded.len() {
        let root = find_root(&mut parents, index);
        groups.entry(root).or_default().push(index);
    }
    let connected_groups = groups
        .into_values()
        .filter(|indices| indices.len() > 1)
        .collect::<Vec<_>>();
    // Split single-linkage components into complete-linkage groups so one
    // bridging image cannot merge two visually distinct bursts.
    let mut groups = Vec::new();
    for indices in connected_groups {
        let mut tight_groups: Vec<Vec<usize>> = Vec::new();
        for index in indices {
            if let Some(group) = tight_groups.iter_mut().find(|group| {
                group.iter().all(|member| {
                    dot_product(&embedded[index].vector, &embedded[*member].vector) >= threshold
                })
            }) {
                group.push(index);
            } else {
                tight_groups.push(vec![index]);
            }
        }
        groups.extend(tight_groups.into_iter().filter(|group| group.len() > 1));
    }
    groups.sort_by_key(|indices| std::cmp::Reverse(indices.len()));
    let clusters = groups
        .into_iter()
        .enumerate()
        .map(|(cluster_index, indices)| {
            let representative = indices[0];
            let mut assets = indices
                .into_iter()
                .map(|index| SimilarAsset {
                    asset: embedded[index].asset.clone(),
                    similarity: if index == representative {
                        1.0
                    } else {
                        dot_product(&embedded[representative].vector, &embedded[index].vector)
                    },
                })
                .collect::<Vec<_>>();
            assets.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
            SimilarityCluster {
                id: format!("similar-{}", cluster_index + 1),
                assets,
            }
        })
        .collect();
    Ok(SimilarityClusterResponse {
        clusters,
        embedded_asset_count: embedded.len(),
        candidate_pair_count: candidate_pairs.len(),
        truncated,
    })
}

pub fn face_feature_status(repository: &Repository) -> AppResult<FaceFeatureStatus> {
    let connection = open(repository)?;
    let enabled = connection
        .query_row(
            "SELECT value_json FROM workflow_preferences WHERE key='face_analysis_enabled'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .as_deref()
        == Some("true");
    let detection_count =
        connection.query_row("SELECT COUNT(*) FROM face_detections", [], |row| row.get(0))?;
    let cluster_count =
        connection.query_row("SELECT COUNT(*) FROM face_clusters", [], |row| row.get(0))?;
    Ok(FaceFeatureStatus {
        status: "model_unavailable".into(),
        message: "人脸功能的数据边界已就绪，但当前安装包未包含经产品许可审核的检测与特征模型，因此不会执行人脸分析。".into(),
        enabled,
        model_installed: false,
        detection_count,
        cluster_count,
        privacy_note: "人脸框、向量和聚类只允许保存在本地数据库，并可一键清空；不会写回原图。".into(),
    })
}

pub fn clear_face_data(repository: &Repository) -> AppResult<FaceFeatureStatus> {
    let mut connection = open(repository)?;
    let transaction = connection.transaction()?;
    transaction.execute("DELETE FROM face_cluster_members", [])?;
    transaction.execute("DELETE FROM face_clusters", [])?;
    transaction.execute("DELETE FROM face_detections", [])?;
    transaction.execute(
        "INSERT INTO workflow_preferences(key, value_json, updated_at)
         VALUES('face_analysis_enabled', 'false', ?1)
         ON CONFLICT(key) DO UPDATE SET value_json='false', updated_at=excluded.updated_at",
        [now()],
    )?;
    transaction.commit()?;
    face_feature_status(repository)
}

pub fn render_edit_preview(
    repository: &Repository,
    asset_id: i64,
    recipe: &EditRecipe,
    max_width: u32,
    max_height: u32,
) -> AppResult<String> {
    validate_recipe(recipe)?;
    let (source, _) = repository.asset_source(asset_id)?;
    let mut image = apply_recipe(image::open(source)?, recipe)?;
    image = image.resize(
        max_width.clamp(64, 2_560),
        max_height.clamp(64, 2_560),
        FilterType::Lanczos3,
    );
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 88).encode_image(&image)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub fn preview_edit_export(
    repository: &Repository,
    asset_id: i64,
    target_path: &Path,
    recipe: &EditRecipe,
) -> AppResult<EditExportPlan> {
    validate_recipe(recipe)?;
    validate_export_target(repository, target_path)?;
    let (source_path, source_fingerprint) = repository.asset_source(asset_id)?;
    if !source_path.is_file() {
        return Err(AppError::NotFound(source_path.display().to_string()));
    }
    let plan = EditExportPlan {
        plan_id: Uuid::new_v4().to_string(),
        asset_id,
        source_path: source_path.to_string_lossy().into_owned(),
        target_path: target_path.to_string_lossy().into_owned(),
        source_fingerprint,
        recipe: recipe.clone(),
        status: "ready".into(),
        issues: Vec::new(),
    };
    let connection = open(repository)?;
    connection.execute(
        "INSERT INTO edit_export_plans(
            id, asset_id, source_fingerprint, target_path, recipe_json, status, created_at
         ) VALUES(?1, ?2, ?3, ?4, ?5, 'ready', ?6)",
        params![
            plan.plan_id,
            plan.asset_id,
            plan.source_fingerprint,
            plan.target_path,
            serde_json::to_string(&plan.recipe)?,
            now(),
        ],
    )?;
    Ok(plan)
}

pub fn execute_edit_export(repository: &Repository, plan_id: &str) -> AppResult<EditExportResult> {
    let connection = open(repository)?;
    let stored = connection
        .query_row(
            "SELECT asset_id, source_fingerprint, target_path, recipe_json, status
             FROM edit_export_plans WHERE id=?1",
            [plan_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("edit export plan {plan_id}")))?;
    let (asset_id, planned_fingerprint, target_path, recipe_json, status) = stored;
    if status != "ready" {
        return Err(AppError::InvalidArgument(format!(
            "edit export plan is not executable: {status}"
        )));
    }
    let recipe: EditRecipe = serde_json::from_str(&recipe_json)?;
    validate_recipe(&recipe)?;
    let target = PathBuf::from(&target_path);
    validate_export_target(repository, &target)?;
    let (source, current_fingerprint) = repository.asset_source(asset_id)?;
    if current_fingerprint != planned_fingerprint || fingerprint(&source)? != planned_fingerprint {
        return Err(AppError::InvalidArgument(
            "source changed after the export preview was created".into(),
        ));
    }

    let job_id = Uuid::new_v4().to_string();
    let timestamp = now();
    connection.execute(
        "INSERT INTO file_operation_jobs(
            id, library_id, operation_type, status, dry_run, conflict_strategy, created_at, updated_at
         ) SELECT ?1, library_id, 'edit_copy', 'running', 0, 'skip', ?3, ?3
           FROM assets WHERE id=?2",
        params![job_id, asset_id, timestamp],
    )?;
    connection.execute(
        "INSERT INTO file_operations(
            job_id, source_path, target_path, operation_type, plan_status, execution_status,
            conflict_strategy, source_hash
         ) VALUES(?1, ?2, ?3, 'edit_copy', 'ready', 'running', 'skip', ?4)",
        params![
            job_id,
            source.to_string_lossy(),
            target_path,
            planned_fingerprint
        ],
    )?;

    let result = write_edited_copy(&source, &target, &recipe);
    match result {
        Ok(()) => {
            let target_hash = fingerprint(&target)?;
            let completed_at = now();
            connection.execute(
                "UPDATE file_operations
                 SET execution_status='completed', target_hash=?2
                 WHERE job_id=?1",
                params![job_id, target_hash],
            )?;
            connection.execute(
                "UPDATE file_operation_jobs SET status='completed', updated_at=?2 WHERE id=?1",
                params![job_id, completed_at],
            )?;
            connection.execute(
                "UPDATE edit_export_plans
                 SET status='completed', executed_at=?2, error_message=NULL WHERE id=?1",
                params![plan_id, completed_at],
            )?;
            Ok(EditExportResult {
                plan_id: plan_id.into(),
                target_path,
                status: "completed".into(),
            })
        }
        Err(error) => {
            let message = error.to_string();
            let _ = connection.execute(
                "UPDATE file_operations SET execution_status='failed', error_message=?2
                 WHERE job_id=?1",
                params![job_id, message],
            );
            let _ = connection.execute(
                "UPDATE file_operation_jobs SET status='failed', updated_at=?2, error_message=?3
                 WHERE id=?1",
                params![job_id, now(), message],
            );
            let _ = connection.execute(
                "UPDATE edit_export_plans SET status='failed', error_message=?2 WHERE id=?1",
                params![plan_id, message],
            );
            Err(error)
        }
    }
}

pub fn preview_edit_rollback(
    repository: &Repository,
    plan_id: &str,
) -> AppResult<EditRollbackPlan> {
    let (target_path, target_hash) = rollback_target(repository, plan_id)?;
    let target = PathBuf::from(&target_path);
    if !target.is_file() {
        return Err(AppError::NotFound(target_path));
    }
    if fingerprint(&target)? != target_hash {
        return Err(AppError::InvalidArgument(
            "generated copy changed after export and cannot be rolled back safely".into(),
        ));
    }
    Ok(EditRollbackPlan {
        plan_id: plan_id.into(),
        target_path,
        target_hash,
        status: "ready".into(),
        issues: Vec::new(),
    })
}

pub fn execute_edit_rollback(
    repository: &Repository,
    plan_id: &str,
) -> AppResult<EditExportResult> {
    let preview = preview_edit_rollback(repository, plan_id)?;
    let target = PathBuf::from(&preview.target_path);
    let connection = open(repository)?;
    connection.execute(
        "UPDATE file_operations
         SET rollback_status='running'
         WHERE operation_type='edit_copy' AND target_path=?1 AND target_hash=?2",
        params![preview.target_path, preview.target_hash],
    )?;
    match std::fs::remove_file(&target) {
        Ok(()) => {
            let timestamp = now();
            connection.execute(
                "UPDATE file_operations
                 SET rollback_status='completed'
                 WHERE operation_type='edit_copy' AND target_path=?1 AND target_hash=?2",
                params![preview.target_path, preview.target_hash],
            )?;
            connection.execute(
                "UPDATE file_operation_jobs
                 SET status='rolled_back', updated_at=?2
                 WHERE id=(
                    SELECT job_id FROM file_operations
                    WHERE operation_type='edit_copy' AND target_path=?1 AND target_hash=?3
                    ORDER BY id DESC LIMIT 1
                 )",
                params![preview.target_path, timestamp, preview.target_hash],
            )?;
            connection.execute(
                "UPDATE edit_export_plans
                 SET status='rolled_back', executed_at=?2, error_message=NULL
                 WHERE id=?1",
                params![plan_id, timestamp],
            )?;
            Ok(EditExportResult {
                plan_id: plan_id.into(),
                target_path: preview.target_path,
                status: "rolled_back".into(),
            })
        }
        Err(error) => {
            let _ = connection.execute(
                "UPDATE file_operations
                 SET rollback_status='failed', error_message=?3
                 WHERE operation_type='edit_copy' AND target_path=?1 AND target_hash=?2",
                params![preview.target_path, preview.target_hash, error.to_string()],
            );
            Err(AppError::Io(error))
        }
    }
}

fn open(repository: &Repository) -> AppResult<Connection> {
    let connection = Connection::open(repository.database_path())?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.busy_timeout(Duration::from_secs(5))?;
    Ok(connection)
}

fn map_asset(row: &Row<'_>) -> rusqlite::Result<WorkflowAsset> {
    Ok(WorkflowAsset {
        id: row.get(0)?,
        library_id: row.get(1)?,
        file_name: row.get(2)?,
        extension: row.get(3)?,
        file_size: row.get(4)?,
        width: row.get(5)?,
        height: row.get(6)?,
        capture_time: row.get(7)?,
        rating: row.get(8)?,
        color_label: row.get(9)?,
        is_favorite: row.get(10)?,
        thumbnail_available: row.get(11)?,
    })
}

fn query_assets<P: rusqlite::Params>(
    connection: &Connection,
    sql: &str,
    params: P,
) -> AppResult<Vec<WorkflowAsset>> {
    let mut statement = connection.prepare(sql)?;
    Ok(statement
        .query_map(params, map_asset)?
        .collect::<Result<Vec<_>, _>>()?)
}

fn map_collection(row: &Row<'_>) -> rusqlite::Result<CollectionSummary> {
    Ok(CollectionSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        asset_count: row.get(5)?,
    })
}

fn collection_summary(connection: &Connection, collection_id: i64) -> AppResult<CollectionSummary> {
    connection
        .query_row(
            "SELECT c.id, c.name, c.description, c.created_at, c.updated_at, COUNT(ca.asset_id)
             FROM collections c
             LEFT JOIN collection_assets ca ON ca.collection_id=c.id
             WHERE c.id=?1
             GROUP BY c.id",
            [collection_id],
            map_collection,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("collection {collection_id}")))
}

fn validate_collection_name(name: &str) -> AppResult<&str> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(AppError::InvalidArgument(
            "collection name must contain 1 to 100 characters".into(),
        ));
    }
    Ok(name)
}

fn rollback_target(repository: &Repository, plan_id: &str) -> AppResult<(String, String)> {
    let connection = open(repository)?;
    let (target_path, status) = connection
        .query_row(
            "SELECT target_path, status FROM edit_export_plans WHERE id=?1",
            [plan_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("edit export plan {plan_id}")))?;
    if status != "completed" {
        return Err(AppError::InvalidArgument(format!(
            "only completed edit exports can be rolled back: {status}"
        )));
    }
    let target_hash = connection
        .query_row(
            "SELECT target_hash FROM file_operations
             WHERE operation_type='edit_copy' AND target_path=?1
               AND execution_status='completed' AND target_hash IS NOT NULL
             ORDER BY id DESC LIMIT 1",
            [&target_path],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("operation log for {target_path}")))?;
    Ok((target_path, target_hash))
}

fn list_embedded_assets(
    repository: &Repository,
    library_id: i64,
    limit: usize,
) -> AppResult<Vec<EmbeddedAsset>> {
    let connection = open(repository)?;
    let active_model = connection
        .query_row(
            "SELECT name, version, analysis_version
             FROM semantic_models
             WHERE is_active=1
             ORDER BY id DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .unwrap_or_else(|| {
            let metadata = default_topic_model_metadata();
            (metadata.name, metadata.version, metadata.analysis_version)
        });
    let sql = format!(
        "SELECT {ASSET_PROJECTION}, se.dimensions, se.vector_blob
         FROM assets a
         JOIN semantic_embeddings se ON se.asset_id=a.id AND se.source_fingerprint=a.fingerprint
         WHERE {LIBRARY_SCOPE_FILTER}
           AND a.file_status='present'
           AND se.model_name=?2 AND se.model_version=?3 AND se.analysis_version=?4
         ORDER BY a.id
         LIMIT ?5"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(
        params![
            library_id,
            active_model.0,
            active_model.1,
            active_model.2,
            limit as i64
        ],
        |row| {
            let dimensions = row.get::<_, i64>(12)?;
            let blob = row.get::<_, Vec<u8>>(13)?;
            Ok((map_asset(row)?, dimensions, blob))
        },
    )?;
    let mut result = Vec::new();
    for row in rows {
        let (asset, dimensions, blob) = row?;
        if dimensions <= 0 || blob.len() != dimensions as usize * 4 {
            continue;
        }
        let vector = blob
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            .collect();
        result.push(EmbeddedAsset { asset, vector });
    }
    Ok(result)
}

fn normalize_local_query(query: &str) -> String {
    let mappings = [
        ("人像", "portrait photo of a person"),
        ("人物", "photo of a person"),
        ("风景", "landscape scenery"),
        ("建筑", "architecture building"),
        ("动物", "animal"),
        ("食物", "food"),
        ("文档", "document page"),
        ("截图", "screenshot"),
        ("夜景", "night scene"),
        ("花", "flower"),
        ("产品", "product photo"),
    ];
    let mut normalized = query.to_string();
    for (source, replacement) in mappings {
        normalized = normalized.replace(source, replacement);
    }
    normalized
}

fn semantic_app_error(error: SemanticError) -> AppError {
    AppError::InvalidArgument(error.to_string())
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return -1.0;
    }
    let dot = dot_product(left, right);
    let left_norm = dot_product(left, left).sqrt();
    let right_norm = dot_product(right, right).sqrt();
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        -1.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn dot_product(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn normalize_vector(vector: &mut [f32]) {
    let norm = dot_product(vector, vector).sqrt();
    if norm > f32::EPSILON {
        for value in vector {
            *value /= norm;
        }
    }
}

fn top_dimensions(vector: &[f32], count: usize) -> Vec<usize> {
    let mut dimensions = vector
        .iter()
        .enumerate()
        .map(|(index, value)| (index, value.abs()))
        .collect::<Vec<_>>();
    dimensions.sort_by(|left, right| right.1.total_cmp(&left.1));
    dimensions
        .into_iter()
        .take(count)
        .map(|(index, _)| index)
        .collect()
}

fn find_root(parents: &mut [usize], value: usize) -> usize {
    if parents[value] != value {
        parents[value] = find_root(parents, parents[value]);
    }
    parents[value]
}

fn union(parents: &mut [usize], left: usize, right: usize) {
    let left_root = find_root(parents, left);
    let right_root = find_root(parents, right);
    if left_root != right_root {
        parents[right_root] = left_root;
    }
}

fn validate_recipe(recipe: &EditRecipe) -> AppResult<()> {
    if ![0, 90, 180, 270].contains(&recipe.rotate_degrees.rem_euclid(360)) {
        return Err(AppError::InvalidArgument(
            "rotation must be 0, 90, 180, or 270 degrees".into(),
        ));
    }
    if !(-2.0..=2.0).contains(&recipe.exposure)
        || !(-1.0..=1.0).contains(&recipe.contrast)
        || !(-1.0..=1.0).contains(&recipe.saturation)
    {
        return Err(AppError::InvalidArgument(
            "edit adjustment is outside the supported range".into(),
        ));
    }
    if let Some(crop) = &recipe.crop
        && (crop.x < 0.0
            || crop.y < 0.0
            || crop.width <= 0.0
            || crop.height <= 0.0
            || crop.x + crop.width > 1.0
            || crop.y + crop.height > 1.0)
    {
        return Err(AppError::InvalidArgument(
            "crop must be a non-empty normalized rectangle inside the image".into(),
        ));
    }
    Ok(())
}

fn apply_recipe(mut image: DynamicImage, recipe: &EditRecipe) -> AppResult<DynamicImage> {
    image = match recipe.rotate_degrees.rem_euclid(360) {
        0 => image,
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => unreachable!(),
    };
    if recipe.flip_horizontal {
        image = image.fliph();
    }
    if recipe.flip_vertical {
        image = image.flipv();
    }
    if let Some(crop) = &recipe.crop {
        let (width, height) = image.dimensions();
        let x = (crop.x * width as f32).floor() as u32;
        let y = (crop.y * height as f32).floor() as u32;
        let crop_width = (crop.width * width as f32)
            .round()
            .max(1.0)
            .min((width - x) as f32) as u32;
        let crop_height = (crop.height * height as f32)
            .round()
            .max(1.0)
            .min((height - y) as f32) as u32;
        image = image.crop_imm(x, y, crop_width, crop_height);
    }
    if recipe.exposure.abs() > f32::EPSILON {
        let delta = ((2_f32.powf(recipe.exposure) - 1.0) * 96.0).round() as i32;
        image = image.brighten(delta);
    }
    if recipe.contrast.abs() > f32::EPSILON {
        image = image.adjust_contrast(recipe.contrast * 100.0);
    }
    if recipe.saturation.abs() > f32::EPSILON {
        let factor = 1.0 + recipe.saturation;
        let mut pixels = image.to_rgba8();
        for pixel in pixels.pixels_mut() {
            let [red, green, blue, alpha] = pixel.0;
            let luminance =
                0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue);
            let adjust = |value: u8| {
                (luminance + (f32::from(value) - luminance) * factor)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            *pixel = Rgba([adjust(red), adjust(green), adjust(blue), alpha]);
        }
        image = DynamicImage::ImageRgba8(pixels);
    }
    Ok(image)
}

fn validate_export_target(repository: &Repository, target: &Path) -> AppResult<()> {
    if target.exists() {
        return Err(AppError::InvalidArgument(format!(
            "target already exists and will not be overwritten: {}",
            target.display()
        )));
    }
    let parent = target
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or_else(|| {
            AppError::InvalidArgument("target parent directory does not exist".into())
        })?;
    let canonical_parent = parent.canonicalize().map_err(AppError::from)?;
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !["jpg", "jpeg", "png", "webp"].contains(&extension.as_str()) {
        return Err(AppError::InvalidArgument(
            "edited copies support JPEG, PNG, and WebP targets".into(),
        ));
    }
    let target_identity =
        identity_key(&canonical_parent.join(target.file_name().unwrap_or_default()));
    let connection = open(repository)?;
    let mut statement = connection.prepare("SELECT source_identity_key FROM libraries")?;
    let roots = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if roots
        .iter()
        .any(|root| is_same_or_descendant(root, &target_identity))
    {
        return Err(AppError::UnsafePath(target.to_path_buf()));
    }
    Ok(())
}

fn write_edited_copy(source: &Path, target: &Path, recipe: &EditRecipe) -> AppResult<()> {
    let image = apply_recipe(image::open(source)?, recipe)?;
    let format = ImageFormat::from_extension(
        target
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
    )
    .ok_or_else(|| AppError::InvalidArgument("unsupported export image format".into()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    let result = image.write_to(&mut file, format).map_err(AppError::from);
    drop(file);
    if result.is_err() {
        let _ = std::fs::remove_file(target);
    }
    result
}

fn fingerprint(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes = file.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        hasher.update(&buffer[..bytes]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture_repository() -> (TempDir, Repository, i64, i64) {
        let temporary = TempDir::new().expect("temporary directory");
        let repository = Repository::new(temporary.path().join("workflow.sqlite3"));
        repository.initialize().expect("repository initialization");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../test-data/manual-verification-20260808/Parent 库/parent-landscape.jpg");
        let source = source.canonicalize().expect("fixture image");
        let source_root = source.parent().expect("fixture parent");
        let source_hash = fingerprint(&source).expect("fixture hash");
        let connection = open(&repository).expect("database");
        connection
            .execute(
                "INSERT INTO libraries(
                    root_path, created_at, name, source_path, source_identity_key
                 ) VALUES(?1, ?2, 'Fixture', ?1, ?3)",
                params![
                    source_root.to_string_lossy(),
                    now(),
                    identity_key(source_root)
                ],
            )
            .expect("library");
        let library_id = connection.last_insert_rowid();
        for index in 0..2 {
            let absolute_path = if index == 0 {
                source.to_string_lossy().into_owned()
            } else {
                source_root
                    .join("duplicate-placeholder.jpg")
                    .to_string_lossy()
                    .into_owned()
            };
            connection
                .execute(
                    "INSERT INTO assets(
                        library_id, absolute_path, relative_path, file_name, extension,
                        file_size, modified_at, fingerprint, first_seen_at, last_seen_at,
                        asset_identity_key, is_favorite
                     ) VALUES(?1, ?2, ?3, ?4, 'jpg', 100, 1, ?5, ?6, ?6, ?7, 0)",
                    params![
                        library_id,
                        absolute_path,
                        format!("fixture-{index}.jpg"),
                        format!("fixture-{index}.jpg"),
                        source_hash,
                        now(),
                        format!("fixture-{index}")
                    ],
                )
                .expect("asset");
        }
        (temporary, repository, library_id, 1)
    }

    #[test]
    fn favorites_collections_and_duplicates_are_virtual() {
        let (_temporary, repository, library_id, asset_id) = fixture_repository();
        assert!(set_favorite(&repository, asset_id, true).expect("favorite"));
        assert_eq!(
            list_favorite_asset_ids(&repository, library_id).expect("favorites"),
            vec![asset_id]
        );
        let collection =
            create_collection(&repository, "精选", "本地虚拟集合").expect("collection");
        let collection = add_assets_to_collection(&repository, collection.id, &[asset_id])
            .expect("collection membership");
        assert_eq!(collection.asset_count, 1);
        assert_eq!(
            get_collection(&repository, collection.id)
                .expect("collection detail")
                .assets
                .len(),
            1
        );
        let groups = list_duplicate_groups(&repository, library_id, 20).expect("duplicates");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].assets.len(), 2);
        assert_eq!(groups[0].reclaimable_bytes, 100);
    }

    #[test]
    fn workflow_queries_include_descendant_and_virtual_assets() {
        let (_temporary, repository, parent_library_id, asset_id) = fixture_repository();
        let connection = open(&repository).expect("database");
        let fingerprint: String = connection
            .query_row(
                "SELECT fingerprint FROM assets WHERE id=?1",
                [asset_id],
                |row| row.get(0),
            )
            .expect("fixture fingerprint");
        connection
            .execute(
                "INSERT INTO libraries(
                    root_path, created_at, name, source_path, source_identity_key,
                    parent_library_id, parent_relation
                 ) VALUES('C:\\workflow-child', ?1, 'Child', 'C:\\workflow-child',
                          'c:/workflow-child', ?2, 'manual')",
                params![now(), parent_library_id],
            )
            .expect("child library");
        let child_library_id = connection.last_insert_rowid();
        let child_asset_path = "C:\\workflow-child\\child.jpg";
        connection
            .execute(
                "INSERT INTO assets(
                    library_id, asset_identity_key, absolute_path, relative_path,
                    file_name, extension, file_size, modified_at, fingerprint,
                    file_status, scan_status, analysis_status, first_seen_at,
                    last_seen_at, last_seen_scan
                 ) VALUES(?1, ?2, ?3, 'child.jpg', 'child.jpg', 'jpg', 100, 1,
                          ?4, 'present', 'indexed', 'completed', ?5, ?5, 1)",
                params![
                    child_library_id,
                    identity_key(Path::new(child_asset_path)),
                    child_asset_path,
                    fingerprint,
                    now()
                ],
            )
            .expect("child asset");
        let child_asset_id = connection.last_insert_rowid();
        drop(connection);

        assert!(set_favorite(&repository, asset_id, true).expect("parent favorite"));
        assert!(set_favorite(&repository, child_asset_id, true).expect("child favorite"));
        assert!(
            repository
                .assign_asset_to_library(asset_id, child_library_id)
                .expect("virtual assignment")
        );

        assert_eq!(
            list_favorite_asset_ids(&repository, parent_library_id).expect("parent favorites"),
            vec![asset_id, child_asset_id]
        );
        assert_eq!(
            list_favorite_asset_ids(&repository, child_library_id).expect("child favorites"),
            vec![asset_id, child_asset_id]
        );
        let parent_groups =
            list_duplicate_groups(&repository, parent_library_id, 20).expect("parent duplicates");
        assert_eq!(parent_groups.len(), 1);
        assert_eq!(parent_groups[0].assets.len(), 3);
        let child_groups =
            list_duplicate_groups(&repository, child_library_id, 20).expect("child duplicates");
        assert_eq!(child_groups.len(), 1);
        assert_eq!(child_groups[0].assets.len(), 2);
    }

    #[test]
    fn edit_export_requires_preview_and_never_overwrites() {
        let (temporary, repository, _library_id, asset_id) = fixture_repository();
        let target = temporary.path().join("编辑副本 中文.jpg");
        let source_before = repository.asset_source(asset_id).expect("source").0;
        let before_hash = fingerprint(&source_before).expect("before hash");
        let recipe = EditRecipe {
            rotate_degrees: 90,
            exposure: 0.1,
            ..EditRecipe::default()
        };
        let plan = preview_edit_export(&repository, asset_id, &target, &recipe).expect("plan");
        let result = execute_edit_export(&repository, &plan.plan_id).expect("export");
        assert_eq!(result.status, "completed");
        assert!(target.is_file());
        assert_eq!(
            fingerprint(&source_before).expect("after hash"),
            before_hash
        );
        assert!(preview_edit_export(&repository, asset_id, &target, &recipe).is_err());
        let exported_bytes = std::fs::read(&target).expect("exported bytes");
        {
            use std::io::Write;
            let mut changed = OpenOptions::new()
                .append(true)
                .open(&target)
                .expect("open generated copy");
            changed
                .write_all(b"changed")
                .expect("change generated copy");
        }
        assert!(preview_edit_rollback(&repository, &plan.plan_id).is_err());
        std::fs::write(&target, exported_bytes).expect("restore generated test copy");
        let rollback = preview_edit_rollback(&repository, &plan.plan_id).expect("rollback plan");
        assert_eq!(rollback.target_path, target.to_string_lossy());
        let rollback = execute_edit_rollback(&repository, &plan.plan_id).expect("rollback");
        assert_eq!(rollback.status, "rolled_back");
        assert!(!target.exists());
        assert_eq!(
            fingerprint(&source_before).expect("post-rollback source hash"),
            before_hash
        );
    }
}
