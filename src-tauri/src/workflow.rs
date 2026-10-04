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
use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::Repository;
use crate::error::{AppError, AppResult};
use crate::models::{
    ASSET_QUERY_VERSION, AssetFilter, AssetListItem, AssetQuery, AssetQueryRoot, AssetSortField,
    LibrarySummary, SortDirection,
};
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

const LIBRARY_SCOPE_FILTER: &str = "a.library_id IN (
    WITH RECURSIVE library_scope(library_id) AS (
        SELECT id FROM libraries WHERE id=?1
        UNION
        SELECT child.id
        FROM libraries child
        JOIN library_scope scope ON child.parent_library_id=scope.library_id
            AND child.parent_relation='source'
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
    pub parent_collection_id: Option<i64>,
    pub collection_kind: String,
    pub system_key: Option<String>,
    pub display_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BrowseNode {
    Source {
        library: LibrarySummary,
        children: Vec<BrowseNode>,
    },
    Collection {
        collection: CollectionSummary,
        children: Vec<BrowseNode>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDetail {
    #[serde(flatten)]
    pub summary: CollectionSummary,
    pub assets: Vec<WorkflowAsset>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CollectionDeleteMode {
    DeleteSubtree,
    PromoteChildren,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionMembershipMutation {
    pub affected_asset_count: i64,
    pub skipped_asset_count: i64,
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
    let items = repository.list_assets_for_query(&favorite_query(library_id))?;
    let mut ids = items
        .into_iter()
        .filter(|asset| asset.file_status == "present")
        .map(|asset| asset.id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    Ok(ids)
}

pub fn list_favorite_assets(
    repository: &Repository,
    library_id: i64,
) -> AppResult<Vec<WorkflowAsset>> {
    let items = repository.list_assets_for_query(&favorite_query(library_id))?;
    Ok(items
        .into_iter()
        .filter(|asset| asset.file_status == "present")
        .map(|asset| workflow_asset_from_list_item(&asset))
        .collect())
}

fn favorite_query(library_id: i64) -> AssetQuery {
    AssetQuery {
        version: ASSET_QUERY_VERSION,
        root: AssetQueryRoot::Source { library_id },
        include_descendants: true,
        filter: AssetFilter {
            favorite_only: true,
            ..AssetFilter::default()
        },
        sort: AssetSortField::CaptureTime,
        direction: SortDirection::Desc,
        page: 1,
        page_size: 500,
    }
}

pub fn set_favorite(repository: &Repository, asset_id: i64, favorite: bool) -> AppResult<bool> {
    let mut connection = open(repository)?;
    let transaction = connection.transaction()?;
    let asset_exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM assets WHERE id=?1)",
        [asset_id],
        |row| row.get(0),
    )?;
    if !asset_exists {
        return Err(AppError::NotFound(format!("asset {asset_id}")));
    }
    let default_collection_id: i64 = transaction
        .query_row(
            "SELECT id FROM collections WHERE system_key='default_favorites'",
            [],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| {
            AppError::InvalidArgument("default favorites collection is missing".into())
        })?;
    if favorite {
        transaction.execute(
            "INSERT OR IGNORE INTO collection_assets(collection_id, asset_id, added_at)
             VALUES(?1, ?2, ?3)",
            params![default_collection_id, asset_id, now()],
        )?;
    } else {
        transaction.execute(
            "DELETE FROM collection_assets WHERE collection_id=?1 AND asset_id=?2",
            params![default_collection_id, asset_id],
        )?;
    }
    transaction.execute(
        "UPDATE assets SET is_favorite=?2 WHERE id=?1",
        params![asset_id, favorite],
    )?;
    transaction.execute(
        "UPDATE collections SET updated_at=?2 WHERE id=?1",
        params![default_collection_id, now()],
    )?;
    transaction.commit()?;
    Ok(favorite)
}

pub fn list_collections(repository: &Repository) -> AppResult<Vec<CollectionSummary>> {
    let connection = open(repository)?;
    let mut statement = connection.prepare(
        "SELECT c.id, c.name, c.description, c.created_at, c.updated_at, COUNT(ca.asset_id),
                c.parent_collection_id, c.collection_kind, c.system_key, c.display_order
         FROM collections c
         LEFT JOIN collection_assets ca ON ca.collection_id=c.id
         GROUP BY c.id
         ORDER BY CASE WHEN c.system_key='default_favorites' THEN 0 ELSE 1 END,
                  c.display_order, c.name COLLATE NOCASE, c.id",
    )?;
    Ok(statement
        .query_map([], map_collection)?
        .collect::<Result<Vec<_>, _>>()?)
}

pub fn list_browse_nodes(repository: &Repository) -> AppResult<Vec<BrowseNode>> {
    let libraries = repository.list_libraries()?;
    let collections = list_collections(repository)?;

    let mut roots = build_collection_nodes(&collections, None);
    roots.extend(build_source_nodes(&libraries, None));
    Ok(roots)
}

pub fn create_collection(
    repository: &Repository,
    name: &str,
    description: &str,
) -> AppResult<CollectionSummary> {
    create_collection_under(repository, name, description, None)
}

pub fn create_collection_under(
    repository: &Repository,
    name: &str,
    description: &str,
    parent_collection_id: Option<i64>,
) -> AppResult<CollectionSummary> {
    let name = validate_collection_name(name)?;
    let description = description.trim();
    if description.chars().count() > 500 {
        return Err(AppError::InvalidArgument(
            "collection description must be 500 characters or fewer".into(),
        ));
    }
    let connection = open(repository)?;
    if let Some(parent_collection_id) = parent_collection_id {
        let parent_kind: Option<String> = connection
            .query_row(
                "SELECT collection_kind FROM collections WHERE id=?1",
                [parent_collection_id],
                |row| row.get(0),
            )
            .optional()?;
        match parent_kind.as_deref() {
            None => {
                return Err(AppError::NotFound(format!(
                    "collection {parent_collection_id}"
                )));
            }
            Some("system_favorites") => {
                return Err(AppError::InvalidArgument(
                    "default favorites collection cannot have children".into(),
                ));
            }
            Some(_) => {}
        }
    }
    let timestamp = now();
    connection.execute(
        "INSERT INTO collections(
             name, description, created_at, updated_at, parent_collection_id,
             display_order
         ) VALUES(?1, ?2, ?3, ?3, ?4,
                  COALESCE((SELECT MAX(display_order) + 1
                            FROM collections WHERE parent_collection_id IS ?4), 0))",
        params![name, description, timestamp, parent_collection_id],
    )?;
    collection_summary(&connection, connection.last_insert_rowid())
}

pub fn rename_collection(
    repository: &Repository,
    collection_id: i64,
    name: &str,
) -> AppResult<CollectionSummary> {
    let name = validate_collection_name(name)?;
    let connection = open(repository)?;
    let (_, parent_collection_id) = require_manual_collection(&connection, collection_id)?;
    ensure_collection_name_available(&connection, name, parent_collection_id, Some(collection_id))?;
    connection.execute(
        "UPDATE collections SET name=?2, updated_at=?3 WHERE id=?1",
        params![collection_id, name, now()],
    )?;
    collection_summary(&connection, collection_id)
}

pub fn move_collection(
    repository: &Repository,
    collection_id: i64,
    parent_collection_id: Option<i64>,
) -> AppResult<CollectionSummary> {
    let connection = open(repository)?;
    let (name, current_parent_id) = require_manual_collection(&connection, collection_id)?;
    if current_parent_id == parent_collection_id {
        return collection_summary(&connection, collection_id);
    }
    if parent_collection_id == Some(collection_id) {
        return Err(AppError::InvalidArgument(
            "collection cannot be its own parent".into(),
        ));
    }
    if let Some(parent_collection_id) = parent_collection_id {
        require_manual_collection(&connection, parent_collection_id)?;
        let creates_cycle: bool = connection.query_row(
            "WITH RECURSIVE descendants(id) AS (
                 SELECT id FROM collections WHERE parent_collection_id=?1
                 UNION ALL
                 SELECT child.id
                 FROM collections child
                 JOIN descendants parent ON child.parent_collection_id=parent.id
             )
             SELECT EXISTS(SELECT 1 FROM descendants WHERE id=?2)",
            params![collection_id, parent_collection_id],
            |row| row.get(0),
        )?;
        if creates_cycle {
            return Err(AppError::InvalidArgument(
                "collection cannot be moved into one of its descendants".into(),
            ));
        }
    }
    ensure_collection_name_available(
        &connection,
        &name,
        parent_collection_id,
        Some(collection_id),
    )?;
    connection.execute(
        "UPDATE collections
         SET parent_collection_id=?2,
             display_order=COALESCE((
                 SELECT MAX(display_order) + 1
                 FROM collections
                 WHERE parent_collection_id IS ?2 AND id<>?1
             ), 0),
             updated_at=?3
         WHERE id=?1",
        params![collection_id, parent_collection_id, now()],
    )?;
    collection_summary(&connection, collection_id)
}

pub fn delete_collection(repository: &Repository, collection_id: i64) -> AppResult<bool> {
    delete_collection_with_mode(
        repository,
        collection_id,
        CollectionDeleteMode::DeleteSubtree,
    )
}

pub fn delete_collection_with_mode(
    repository: &Repository,
    collection_id: i64,
    mode: CollectionDeleteMode,
) -> AppResult<bool> {
    let mut connection = open(repository)?;
    let transaction = connection.transaction()?;
    let (_, parent_collection_id) = require_manual_collection(&transaction, collection_id)?;
    if mode == CollectionDeleteMode::PromoteChildren {
        let has_name_conflict: bool = transaction.query_row(
            "SELECT EXISTS(
                 SELECT 1
                 FROM collections child
                 JOIN collections sibling
                   ON sibling.parent_collection_id IS ?2
                  AND sibling.name=child.name COLLATE NOCASE
                  AND sibling.id<>?1
                 WHERE child.parent_collection_id=?1
             )",
            params![collection_id, parent_collection_id],
            |row| row.get(0),
        )?;
        if has_name_conflict {
            return Err(AppError::InvalidArgument(
                "a promoted child would conflict with an existing collection name".into(),
            ));
        }
        transaction.execute(
            "UPDATE collections
             SET parent_collection_id=?2, updated_at=?3
             WHERE parent_collection_id=?1",
            params![collection_id, parent_collection_id, now()],
        )?;
    }
    let deleted = transaction.execute("DELETE FROM collections WHERE id=?1", [collection_id])?;
    transaction.commit()?;
    Ok(deleted > 0)
}

pub fn add_assets_to_collection(
    repository: &Repository,
    collection_id: i64,
    asset_ids: &[i64],
) -> AppResult<CollectionSummary> {
    add_assets_to_collections(repository, &[collection_id], asset_ids)?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NotFound(format!("collection {collection_id}")))
}

pub fn add_assets_to_collections(
    repository: &Repository,
    collection_ids: &[i64],
    asset_ids: &[i64],
) -> AppResult<Vec<CollectionSummary>> {
    let mut connection = open(repository)?;
    let transaction = connection.transaction()?;
    let unique_collection_ids = collection_ids.iter().copied().collect::<BTreeSet<_>>();
    if unique_collection_ids.is_empty() {
        return Err(AppError::InvalidArgument(
            "at least one target collection is required".into(),
        ));
    }
    let timestamp = now();
    let unique_asset_ids = asset_ids.iter().copied().collect::<BTreeSet<_>>();
    for collection_id in &unique_collection_ids {
        let collection_kind: Option<String> = transaction
            .query_row(
                "SELECT collection_kind FROM collections WHERE id=?1",
                [collection_id],
                |row| row.get(0),
            )
            .optional()?;
        let collection_kind = collection_kind
            .ok_or_else(|| AppError::NotFound(format!("collection {collection_id}")))?;
        for asset_id in &unique_asset_ids {
            transaction.execute(
                "INSERT OR IGNORE INTO collection_assets(collection_id, asset_id, added_at)
                 SELECT ?1, id, ?3 FROM assets WHERE id=?2",
                params![collection_id, asset_id, timestamp],
            )?;
        }
        if collection_kind == "system_favorites" {
            for asset_id in &unique_asset_ids {
                transaction.execute("UPDATE assets SET is_favorite=1 WHERE id=?1", [asset_id])?;
            }
        }
        transaction.execute(
            "UPDATE collections SET updated_at=?2 WHERE id=?1",
            params![collection_id, timestamp],
        )?;
    }
    transaction.commit()?;
    let connection = open(repository)?;
    unique_collection_ids
        .into_iter()
        .map(|collection_id| collection_summary(&connection, collection_id))
        .collect()
}

pub fn move_assets_between_collections(
    repository: &Repository,
    source_collection_id: i64,
    target_collection_id: i64,
    asset_ids: &[i64],
) -> AppResult<CollectionMembershipMutation> {
    if source_collection_id == target_collection_id {
        return Err(AppError::InvalidArgument(
            "source and target collections must differ".into(),
        ));
    }
    let mut connection = open(repository)?;
    let transaction = connection.transaction()?;
    require_manual_collection(&transaction, source_collection_id)?;
    let target_kind: Option<String> = transaction
        .query_row(
            "SELECT collection_kind FROM collections WHERE id=?1",
            [target_collection_id],
            |row| row.get(0),
        )
        .optional()?;
    let target_kind = target_kind
        .ok_or_else(|| AppError::NotFound(format!("collection {target_collection_id}")))?;
    let unique_asset_ids = asset_ids.iter().copied().collect::<BTreeSet<_>>();
    let timestamp = now();
    let mut affected_asset_count = 0_i64;
    for asset_id in &unique_asset_ids {
        let is_direct_member: bool = transaction.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM collection_assets
                 WHERE collection_id=?1 AND asset_id=?2
             )",
            params![source_collection_id, asset_id],
            |row| row.get(0),
        )?;
        if !is_direct_member {
            continue;
        }
        transaction.execute(
            "INSERT OR IGNORE INTO collection_assets(collection_id, asset_id, added_at)
             VALUES(?1, ?2, ?3)",
            params![target_collection_id, asset_id, timestamp],
        )?;
        transaction.execute(
            "DELETE FROM collection_assets WHERE collection_id=?1 AND asset_id=?2",
            params![source_collection_id, asset_id],
        )?;
        if target_kind == "system_favorites" {
            transaction.execute("UPDATE assets SET is_favorite=1 WHERE id=?1", [asset_id])?;
        }
        affected_asset_count += 1;
    }
    transaction.execute(
        "UPDATE collections SET updated_at=?3 WHERE id IN (?1, ?2)",
        params![source_collection_id, target_collection_id, timestamp],
    )?;
    transaction.commit()?;
    Ok(CollectionMembershipMutation {
        affected_asset_count,
        skipped_asset_count: unique_asset_ids.len() as i64 - affected_asset_count,
    })
}

pub fn remove_assets_from_collection(
    repository: &Repository,
    collection_id: i64,
    asset_ids: &[i64],
) -> AppResult<CollectionSummary> {
    let mut connection = open(repository)?;
    let transaction = connection.transaction()?;
    let is_default_favorites: bool = transaction
        .query_row(
            "SELECT collection_kind='system_favorites'
             FROM collections WHERE id=?1",
            [collection_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("collection {collection_id}")))?;
    let unique_asset_ids = asset_ids.iter().copied().collect::<BTreeSet<_>>();
    for asset_id in &unique_asset_ids {
        transaction.execute(
            "DELETE FROM collection_assets WHERE collection_id=?1 AND asset_id=?2",
            params![collection_id, asset_id],
        )?;
        if is_default_favorites {
            transaction.execute("UPDATE assets SET is_favorite=0 WHERE id=?1", [asset_id])?;
        }
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
    drop(connection);
    let assets = repository
        .list_assets_for_query(&AssetQuery {
            version: ASSET_QUERY_VERSION,
            root: AssetQueryRoot::Collection { collection_id },
            include_descendants: false,
            filter: AssetFilter::default(),
            sort: AssetSortField::CaptureTime,
            direction: SortDirection::Desc,
            page: 1,
            page_size: 500,
        })?
        .into_iter()
        .filter(|asset| asset.file_status == "present")
        .map(|asset| workflow_asset_from_list_item(&asset))
        .collect();
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
    let target_path = validate_export_target(repository, target_path)?;
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
    let mut connection = open(repository)?;
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
    let target = validate_export_target(repository, Path::new(&target_path))?;
    let (source, current_fingerprint) = repository.asset_source(asset_id)?;
    if current_fingerprint != planned_fingerprint || fingerprint(&source)? != planned_fingerprint {
        return Err(AppError::InvalidArgument(
            "source changed after the export preview was created".into(),
        ));
    }

    // The plan ID is the durable ownership key for this export's operation log.
    let job_id = plan_id.to_owned();
    let timestamp = now();
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let rollback_in_progress: bool = transaction.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM file_operations
             WHERE operation_type='edit_copy'
               AND path_identity_key(target_path)=?1
               AND rollback_status='running'
         )",
        [identity_key(&target)],
        |row| row.get(0),
    )?;
    if rollback_in_progress {
        return Err(AppError::InvalidArgument(
            "an edit export rollback is already in progress for this target path".into(),
        ));
    }
    transaction.execute(
        "INSERT INTO file_operation_jobs(
            id, library_id, operation_type, status, dry_run, conflict_strategy, created_at, updated_at
         ) SELECT ?1, library_id, 'edit_copy', 'running', 0, 'skip', ?3, ?3
           FROM assets WHERE id=?2",
        params![job_id, asset_id, timestamp],
    )?;
    transaction.execute(
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
    transaction.commit()?;

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
    let canonical_target = target.canonicalize().map_err(AppError::from)?;
    ensure_target_outside_libraries(repository, &canonical_target)?;
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
    let mut connection = open(repository)?;
    let claim = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let claimed = claim.execute(
        "UPDATE file_operations
         SET rollback_status='running'
         WHERE job_id=?1 AND operation_type='edit_copy' AND target_path=?2
           AND execution_status='completed' AND target_hash=?3
           AND rollback_status IN ('not_requested', 'failed')
           AND EXISTS (
               SELECT 1 FROM edit_export_plans plan
               WHERE plan.id=?1 AND plan.target_path=?2 AND plan.status='completed'
           )
           AND EXISTS (
               SELECT 1 FROM file_operation_jobs job
               WHERE job.id=file_operations.job_id AND job.status='completed'
           )
           AND NOT EXISTS (
               SELECT 1 FROM file_operations newer
               WHERE newer.operation_type='edit_copy'
                 AND path_identity_key(newer.target_path)=
                     path_identity_key(file_operations.target_path)
                 AND newer.id>file_operations.id
                 AND newer.execution_status='completed'
           )
           AND NOT EXISTS (
               SELECT 1 FROM file_operations running_export
               WHERE running_export.operation_type='edit_copy'
                 AND path_identity_key(running_export.target_path)=
                     path_identity_key(file_operations.target_path)
                 AND running_export.id<>file_operations.id
                 AND running_export.execution_status='running'
           )
           AND NOT EXISTS (
               SELECT 1 FROM file_operations running_rollback
               WHERE running_rollback.operation_type='edit_copy'
                 AND path_identity_key(running_rollback.target_path)=
                     path_identity_key(file_operations.target_path)
                 AND running_rollback.job_id<>file_operations.job_id
                 AND running_rollback.rollback_status='running'
           )",
        params![plan_id, preview.target_path, preview.target_hash],
    )?;
    if claimed != 1 {
        return Err(AppError::InvalidArgument(
            "edit export rollback ownership changed, a later export is active, or rollback is already in progress".into(),
        ));
    }
    claim.commit()?;

    if !target.is_file() {
        mark_edit_rollback_failed(
            repository,
            plan_id,
            &preview.target_path,
            "generated copy disappeared before rollback deletion",
        )?;
        return Err(AppError::NotFound(preview.target_path));
    }
    let canonical_target = match target.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            mark_edit_rollback_failed(
                repository,
                plan_id,
                &preview.target_path,
                &error.to_string(),
            )?;
            return Err(AppError::Io(error));
        }
    };
    if let Err(error) = ensure_target_outside_libraries(repository, &canonical_target) {
        mark_edit_rollback_failed(
            repository,
            plan_id,
            &preview.target_path,
            &error.to_string(),
        )?;
        return Err(error);
    }
    let current_hash = match fingerprint(&target) {
        Ok(hash) => hash,
        Err(error) => {
            mark_edit_rollback_failed(
                repository,
                plan_id,
                &preview.target_path,
                &error.to_string(),
            )?;
            return Err(error);
        }
    };
    if current_hash != preview.target_hash {
        mark_edit_rollback_failed(
            repository,
            plan_id,
            &preview.target_path,
            "generated copy changed after rollback preview",
        )?;
        return Err(AppError::InvalidArgument(
            "generated copy changed after export and cannot be rolled back safely".into(),
        ));
    }

    if let Err(error) = std::fs::remove_file(&target) {
        mark_edit_rollback_failed(
            repository,
            plan_id,
            &preview.target_path,
            &error.to_string(),
        )?;
        return Err(AppError::Io(error));
    }

    let timestamp = now();
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let operation_rows = transaction.execute(
        "UPDATE file_operations
         SET rollback_status='completed', error_message=NULL
         WHERE job_id=?1 AND operation_type='edit_copy' AND target_path=?2
           AND target_hash=?3 AND rollback_status='running'",
        params![plan_id, preview.target_path, preview.target_hash],
    )?;
    let job_rows = transaction.execute(
        "UPDATE file_operation_jobs
         SET status='rolled_back', updated_at=?2
         WHERE id=?1 AND status='completed'",
        params![plan_id, timestamp],
    )?;
    let plan_rows = transaction.execute(
        "UPDATE edit_export_plans
         SET status='rolled_back', executed_at=?2, error_message=NULL
         WHERE id=?1 AND status='completed'",
        params![plan_id, timestamp],
    )?;
    if operation_rows != 1 || job_rows != 1 || plan_rows != 1 {
        return Err(AppError::InvalidArgument(
            "edit export rollback records changed unexpectedly after file removal".into(),
        ));
    }
    transaction.commit()?;
    Ok(EditExportResult {
        plan_id: plan_id.into(),
        target_path: preview.target_path,
        status: "rolled_back".into(),
    })
}

fn mark_edit_rollback_failed(
    repository: &Repository,
    plan_id: &str,
    target_path: &str,
    error_message: &str,
) -> AppResult<()> {
    let connection = open(repository)?;
    let updated = connection.execute(
        "UPDATE file_operations
         SET rollback_status='failed', error_message=?3
         WHERE job_id=?1 AND operation_type='edit_copy' AND target_path=?2
           AND rollback_status='running'",
        params![plan_id, target_path, error_message],
    )?;
    if updated != 1 {
        return Err(AppError::InvalidArgument(
            "edit export rollback claim could not be marked failed".into(),
        ));
    }
    Ok(())
}

fn open(repository: &Repository) -> AppResult<Connection> {
    let connection = Connection::open(repository.database_path())?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.create_scalar_function(
        "path_identity_key",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |context| {
            let path = context.get::<String>(0)?;
            Ok(identity_key(Path::new(&path)))
        },
    )?;
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

fn workflow_asset_from_list_item(asset: &AssetListItem) -> WorkflowAsset {
    WorkflowAsset {
        id: asset.id,
        library_id: asset.library_id,
        file_name: asset.file_name.clone(),
        extension: asset.extension.clone(),
        file_size: asset.file_size,
        width: asset.width,
        height: asset.height,
        capture_time: asset.capture_time.clone(),
        rating: asset.rating,
        color_label: asset.color_label.clone(),
        is_favorite: asset.is_favorite,
        thumbnail_available: asset.thumbnail_available,
    }
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
        parent_collection_id: row.get(6)?,
        collection_kind: row.get(7)?,
        system_key: row.get(8)?,
        display_order: row.get(9)?,
    })
}

fn collection_summary(connection: &Connection, collection_id: i64) -> AppResult<CollectionSummary> {
    connection
        .query_row(
            "SELECT c.id, c.name, c.description, c.created_at, c.updated_at, COUNT(ca.asset_id),
                    c.parent_collection_id, c.collection_kind, c.system_key, c.display_order
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

fn build_source_nodes(
    libraries: &[LibrarySummary],
    parent_library_id: Option<i64>,
) -> Vec<BrowseNode> {
    let mut children = libraries
        .iter()
        .filter(|library| library.parent_library_id == parent_library_id)
        .collect::<Vec<_>>();
    children.sort_by(|left, right| {
        left.display_order
            .cmp(&right.display_order)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });
    children
        .into_iter()
        .map(|library| BrowseNode::Source {
            library: library.clone(),
            children: build_source_nodes(libraries, Some(library.id)),
        })
        .collect()
}

fn build_collection_nodes(
    collections: &[CollectionSummary],
    parent_collection_id: Option<i64>,
) -> Vec<BrowseNode> {
    let mut children = collections
        .iter()
        .filter(|collection| collection.parent_collection_id == parent_collection_id)
        .collect::<Vec<_>>();
    children.sort_by(|left, right| {
        let left_system = left.system_key.as_deref() == Some("default_favorites");
        let right_system = right.system_key.as_deref() == Some("default_favorites");
        right_system
            .cmp(&left_system)
            .then_with(|| left.display_order.cmp(&right.display_order))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });
    children
        .into_iter()
        .map(|collection| BrowseNode::Collection {
            collection: collection.clone(),
            children: build_collection_nodes(collections, Some(collection.id)),
        })
        .collect()
}

fn require_manual_collection(
    connection: &Connection,
    collection_id: i64,
) -> AppResult<(String, Option<i64>)> {
    let record = connection
        .query_row(
            "SELECT name, parent_collection_id, collection_kind
             FROM collections WHERE id=?1",
            [collection_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("collection {collection_id}")))?;
    if record.2 != "manual" {
        return Err(AppError::InvalidArgument(
            "default favorites collection cannot be managed as a regular collection".into(),
        ));
    }
    Ok((record.0, record.1))
}

fn ensure_collection_name_available(
    connection: &Connection,
    name: &str,
    parent_collection_id: Option<i64>,
    excluded_collection_id: Option<i64>,
) -> AppResult<()> {
    let conflict: bool = connection.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM collections
             WHERE parent_collection_id IS ?1
               AND name=?2 COLLATE NOCASE
               AND (?3 IS NULL OR id<>?3)
         )",
        params![parent_collection_id, name, excluded_collection_id],
        |row| row.get(0),
    )?;
    if conflict {
        return Err(AppError::InvalidArgument(
            "a collection with the same name already exists at this level".into(),
        ));
    }
    Ok(())
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

    let operations = {
        let mut statement = connection.prepare(
            "SELECT id, target_path, execution_status, target_hash, rollback_status
             FROM file_operations
             WHERE job_id=?1 AND operation_type='edit_copy'",
        )?;
        statement
            .query_map([plan_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    if operations.is_empty() {
        return Err(AppError::InvalidArgument(format!(
            "cannot safely roll back edit export plan {plan_id}: no file operation is associated with this plan; legacy operation ownership cannot be proven"
        )));
    }
    if operations.len() != 1 {
        return Err(AppError::InvalidArgument(format!(
            "cannot safely roll back edit export plan {plan_id}: operation ownership is ambiguous"
        )));
    }
    let (operation_id, operation_path, execution_status, target_hash, rollback_status) = operations
        .into_iter()
        .next()
        .expect("one operation was checked");
    if operation_path != target_path {
        return Err(AppError::InvalidArgument(format!(
            "cannot safely roll back edit export plan {plan_id}: operation target does not match the plan"
        )));
    }
    if execution_status != "completed" {
        return Err(AppError::InvalidArgument(format!(
            "cannot safely roll back edit export plan {plan_id}: its operation is not completed"
        )));
    }
    let target_hash = target_hash.ok_or_else(|| {
        AppError::InvalidArgument(format!(
            "cannot safely roll back edit export plan {plan_id}: operation target hash is missing"
        ))
    })?;
    if !matches!(rollback_status.as_str(), "not_requested" | "failed") {
        return Err(AppError::InvalidArgument(format!(
            "edit export rollback is not available in its current state: {rollback_status}"
        )));
    }

    let target_identity = identity_key(Path::new(&target_path));
    let latest_successful_job = connection
        .query_row(
            "SELECT job_id FROM file_operations
             WHERE operation_type='edit_copy'
               AND path_identity_key(target_path)=?1
               AND execution_status='completed'
             ORDER BY id DESC LIMIT 1",
            [&target_identity],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if latest_successful_job.as_deref() != Some(plan_id) {
        return Err(AppError::InvalidArgument(format!(
            "cannot safely roll back edit export plan {plan_id}: it was superseded by a newer successful export at the same target path"
        )));
    }
    let (other_export_running, other_rollback_running): (bool, bool) = connection.query_row(
        "SELECT
             EXISTS(
                 SELECT 1 FROM file_operations
                 WHERE operation_type='edit_copy'
                   AND path_identity_key(target_path)=?1
                   AND id<>?2 AND execution_status='running'
             ),
             EXISTS(
                 SELECT 1 FROM file_operations
                 WHERE operation_type='edit_copy'
                   AND path_identity_key(target_path)=?1
                   AND id<>?2 AND rollback_status='running'
             )",
        params![target_identity, operation_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if other_export_running {
        return Err(AppError::InvalidArgument(format!(
            "cannot safely roll back edit export plan {plan_id} while another export for the same target path is running"
        )));
    }
    if other_rollback_running {
        return Err(AppError::InvalidArgument(format!(
            "cannot safely roll back edit export plan {plan_id} while another rollback for the same target path is running"
        )));
    }
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

fn validate_export_target(repository: &Repository, target: &Path) -> AppResult<PathBuf> {
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
    let file_name = target
        .file_name()
        .ok_or_else(|| AppError::InvalidArgument("target path must include a file name".into()))?;
    let canonical_target = canonical_parent.join(file_name);
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
    ensure_target_outside_libraries(repository, &canonical_target)?;
    Ok(canonical_target)
}

fn ensure_target_outside_libraries(repository: &Repository, target: &Path) -> AppResult<()> {
    let target_identity = identity_key(target);
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
        let connection = open(&repository).expect("favorite database");
        let default_collection_id: i64 = connection
            .query_row(
                "SELECT id FROM collections WHERE system_key='default_favorites'",
                [],
                |row| row.get(0),
            )
            .expect("default collection");
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM collection_assets
                     WHERE collection_id=?1 AND asset_id=?2",
                    params![default_collection_id, asset_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("default membership"),
            1
        );
        drop(connection);
        assert!(!set_favorite(&repository, asset_id, false).expect("unfavorite"));
        assert!(
            list_favorite_asset_ids(&repository, library_id)
                .expect("empty favorites")
                .is_empty()
        );
        assert!(set_favorite(&repository, asset_id, true).expect("favorite again"));
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
        assert!(delete_collection(&repository, default_collection_id).is_err());
        remove_assets_from_collection(&repository, collection.id, &[asset_id])
            .expect("remove ordinary membership");
        let connection = open(&repository).expect("post collection database");
        assert_eq!(
            connection
                .query_row(
                    "SELECT is_favorite FROM assets WHERE id=?1",
                    [asset_id],
                    |row| { row.get::<_, i64>(0) }
                )
                .expect("favorite mirror remains"),
            1
        );
        drop(connection);
        let groups = list_duplicate_groups(&repository, library_id, 20).expect("duplicates");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].assets.len(), 2);
        assert_eq!(groups[0].reclaimable_bytes, 100);
    }

    #[test]
    fn browse_nodes_merge_default_favorites_collections_and_sources() {
        let (_temporary, repository, _library_id, _asset_id) = fixture_repository();
        let parent = create_collection(&repository, "旅行", "").expect("parent collection");
        let child = create_collection_under(&repository, "海边", "", Some(parent.id))
            .expect("child collection");

        let nodes = list_browse_nodes(&repository).expect("browse nodes");
        assert!(matches!(
            nodes.first(),
            Some(BrowseNode::Collection { collection, .. })
                if collection.system_key.as_deref() == Some("default_favorites")
        ));
        let parent_node = nodes
            .iter()
            .find_map(|node| match node {
                BrowseNode::Collection {
                    collection,
                    children,
                } if collection.id == parent.id => Some((collection, children)),
                _ => None,
            })
            .expect("parent browse node");
        assert_eq!(parent_node.0.parent_collection_id, None);
        assert!(matches!(
            parent_node.1.first(),
            Some(BrowseNode::Collection { collection, .. }) if collection.id == child.id
        ));
        assert!(nodes.iter().any(|node| matches!(
            node,
            BrowseNode::Source { library, .. } if library.name == "Fixture"
        )));
        assert!(
            create_collection_under(
                &repository,
                "不应挂在默认收藏下",
                "",
                Some(
                    nodes
                        .first()
                        .and_then(|node| match node {
                            BrowseNode::Collection { collection, .. } => Some(collection.id),
                            _ => None,
                        })
                        .expect("default favorites"),
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn collection_management_preserves_hierarchy_rules() {
        let (_temporary, repository, _library_id, _asset_id) = fixture_repository();
        let parent = create_collection(&repository, "旅行", "").expect("parent");
        let child =
            create_collection_under(&repository, "海边", "", Some(parent.id)).expect("child");
        let target = create_collection(&repository, "归档", "").expect("target");

        let renamed = rename_collection(&repository, parent.id, "旅途").expect("rename");
        assert_eq!(renamed.name, "旅途");
        assert!(rename_collection(&repository, target.id, "旅途").is_err());

        let moved = move_collection(&repository, child.id, Some(target.id)).expect("move child");
        assert_eq!(moved.parent_collection_id, Some(target.id));
        assert!(move_collection(&repository, target.id, Some(child.id)).is_err());

        let promoted_parent =
            create_collection(&repository, "待删除父级", "").expect("promoted parent");
        let promoted_child =
            create_collection_under(&repository, "保留子级", "", Some(promoted_parent.id))
                .expect("promoted child");
        delete_collection_with_mode(
            &repository,
            promoted_parent.id,
            CollectionDeleteMode::PromoteChildren,
        )
        .expect("delete and promote");
        let promoted = list_collections(&repository)
            .expect("collections")
            .into_iter()
            .find(|collection| collection.id == promoted_child.id)
            .expect("promoted child remains");
        assert_eq!(promoted.parent_collection_id, None);

        let subtree_parent =
            create_collection(&repository, "整树删除", "").expect("subtree parent");
        let subtree_child =
            create_collection_under(&repository, "随父删除", "", Some(subtree_parent.id))
                .expect("subtree child");
        delete_collection_with_mode(
            &repository,
            subtree_parent.id,
            CollectionDeleteMode::DeleteSubtree,
        )
        .expect("delete subtree");
        assert!(
            list_collections(&repository)
                .expect("collections after subtree delete")
                .iter()
                .all(|collection| collection.id != subtree_child.id)
        );
    }

    #[test]
    fn collection_membership_supports_multi_target_add_and_direct_move() {
        let (_temporary, repository, _library_id, asset_id) = fixture_repository();
        let second_asset_id = asset_id + 1;
        let first = create_collection(&repository, "第一组", "").expect("first");
        let second = create_collection(&repository, "第二组", "").expect("second");

        let summaries = add_assets_to_collections(
            &repository,
            &[first.id, second.id, first.id],
            &[asset_id, second_asset_id, asset_id],
        )
        .expect("multi target add");
        assert_eq!(summaries.len(), 2);
        assert!(summaries.iter().all(|summary| summary.asset_count == 2));

        remove_assets_from_collection(&repository, first.id, &[second_asset_id])
            .expect("prepare skipped member");
        let mutation = move_assets_between_collections(
            &repository,
            first.id,
            second.id,
            &[asset_id, second_asset_id],
        )
        .expect("move direct members");
        assert_eq!(mutation.affected_asset_count, 1);
        assert_eq!(mutation.skipped_asset_count, 1);
        assert_eq!(
            collection_summary(&open(&repository).expect("database"), first.id)
                .expect("first summary")
                .asset_count,
            0
        );

        let default_collection_id = list_collections(&repository)
            .expect("collections")
            .into_iter()
            .find(|collection| collection.system_key.as_deref() == Some("default_favorites"))
            .expect("default favorites")
            .id;
        add_assets_to_collections(&repository, &[default_collection_id], &[asset_id])
            .expect("add favorites");
        let connection = open(&repository).expect("favorite mirror");
        assert_eq!(
            connection
                .query_row(
                    "SELECT is_favorite FROM assets WHERE id=?1",
                    [asset_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("favorite state"),
            1
        );
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
        let parent_source_path: String = connection
            .query_row(
                "SELECT source_path FROM libraries WHERE id=?1",
                [parent_library_id],
                |row| row.get(0),
            )
            .expect("parent source path");
        let child_source_path = PathBuf::from(parent_source_path).join("workflow-child");
        let child_source_path = child_source_path.to_string_lossy().into_owned();
        let child_source_key = identity_key(Path::new(&child_source_path));
        connection
            .execute(
                "INSERT INTO libraries(
                     root_path, created_at, name, source_path, source_identity_key,
                     parent_library_id, parent_relation
                 ) VALUES(?2, ?1, 'Child', ?2, ?3, ?4, 'source')",
                params![
                    now(),
                    child_source_path,
                    child_source_key,
                    parent_library_id
                ],
            )
            .expect("child library");
        let child_library_id = connection.last_insert_rowid();
        let child_asset_path = format!("{child_source_path}\\child.jpg");
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
                    identity_key(Path::new(&child_asset_path)),
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
            vec![child_asset_id]
        );
        let parent_groups =
            list_duplicate_groups(&repository, parent_library_id, 20).expect("parent duplicates");
        assert_eq!(parent_groups.len(), 1);
        assert_eq!(parent_groups[0].assets.len(), 3);
        let child_groups =
            list_duplicate_groups(&repository, child_library_id, 20).expect("child duplicates");
        assert!(child_groups.is_empty());
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
        assert_eq!(
            identity_key(Path::new(&rollback.target_path)),
            identity_key(&target)
        );
        let rollback = execute_edit_rollback(&repository, &plan.plan_id).expect("rollback");
        assert_eq!(rollback.status, "rolled_back");
        assert!(!target.exists());
        assert_eq!(
            fingerprint(&source_before).expect("post-rollback source hash"),
            before_hash
        );
    }

    #[test]
    fn edit_export_rollback_cannot_target_a_newer_export_at_the_same_path() {
        let (temporary, repository, _library_id, asset_id) = fixture_repository();
        let target = temporary.path().join("重复内容-export.jpg");
        let recipe = EditRecipe {
            rotate_degrees: 90,
            exposure: 0.1,
            ..EditRecipe::default()
        };

        let first_plan =
            preview_edit_export(&repository, asset_id, &target, &recipe).expect("first plan");
        execute_edit_export(&repository, &first_plan.plan_id).expect("first export");
        let first_hash = fingerprint(&target).expect("first export hash");
        std::fs::remove_file(&target).expect("simulate external removal of generated copy");

        let second_plan =
            preview_edit_export(&repository, asset_id, &target, &recipe).expect("second plan");
        execute_edit_export(&repository, &second_plan.plan_id).expect("second export");
        let second_hash = fingerprint(&target).expect("second export hash");
        assert_eq!(
            second_hash, first_hash,
            "fixture exports should be identical"
        );

        let first_preview = preview_edit_rollback(&repository, &first_plan.plan_id);
        let first_rollback = execute_edit_rollback(&repository, &first_plan.plan_id);
        assert!(
            first_preview.is_err(),
            "a superseded plan must not preview rollback of a later identical copy"
        );
        assert!(
            first_rollback.is_err(),
            "a superseded plan must not execute rollback of a later identical copy"
        );
        assert!(target.is_file(), "the second export must remain present");
        assert_eq!(
            fingerprint(&target).expect("preserved second export"),
            second_hash
        );

        preview_edit_rollback(&repository, &second_plan.plan_id)
            .expect("current export rollback preview");
        let rollback =
            execute_edit_rollback(&repository, &second_plan.plan_id).expect("current rollback");
        assert_eq!(rollback.status, "rolled_back");
        assert!(!target.exists());

        let connection = open(&repository).expect("database");
        let first_status: String = connection
            .query_row(
                "SELECT rollback_status FROM file_operations WHERE job_id=?1",
                [&first_plan.plan_id],
                |row| row.get(0),
            )
            .expect("first operation status");
        let second_status: String = connection
            .query_row(
                "SELECT rollback_status FROM file_operations WHERE job_id=?1",
                [&second_plan.plan_id],
                |row| row.get(0),
            )
            .expect("second operation status");
        let first_job_status: String = connection
            .query_row(
                "SELECT status FROM file_operation_jobs WHERE id=?1",
                [&first_plan.plan_id],
                |row| row.get(0),
            )
            .expect("first job status");
        let second_job_status: String = connection
            .query_row(
                "SELECT status FROM file_operation_jobs WHERE id=?1",
                [&second_plan.plan_id],
                |row| row.get(0),
            )
            .expect("second job status");
        assert_eq!(first_status, "not_requested");
        assert_eq!(second_status, "completed");
        assert_eq!(first_job_status, "completed");
        assert_eq!(second_job_status, "rolled_back");
    }

    #[test]
    fn edit_export_rollback_rejects_an_older_running_operation_for_same_path() {
        let (temporary, repository, library_id, asset_id) = fixture_repository();
        let target = temporary.path().join("running-export.jpg");
        let source = repository.asset_source(asset_id).expect("source").0;
        let source_hash = fingerprint(&source).expect("source hash");
        let older_job_id = Uuid::new_v4().to_string();
        let connection = open(&repository).expect("database");
        let timestamp = now();
        connection
            .execute(
                "INSERT INTO file_operation_jobs(
                    id, library_id, operation_type, status, dry_run,
                    conflict_strategy, created_at, updated_at
                 ) VALUES(?1, ?2, 'edit_copy', 'running', 0, 'skip', ?3, ?3)",
                params![older_job_id, library_id, timestamp],
            )
            .expect("older running job");
        connection
            .execute(
                "INSERT INTO file_operations(
                    job_id, source_path, target_path, operation_type, plan_status,
                    execution_status, conflict_strategy, source_hash
                 ) VALUES(?1, ?2, ?3, 'edit_copy', 'ready', 'running', 'skip', ?4)",
                params![
                    older_job_id,
                    source.to_string_lossy(),
                    target.to_string_lossy(),
                    source_hash
                ],
            )
            .expect("older running operation");
        drop(connection);

        let plan = preview_edit_export(&repository, asset_id, &target, &EditRecipe::default())
            .expect("completed export plan");
        execute_edit_export(&repository, &plan.plan_id).expect("completed export");
        let connection = open(&repository).expect("operation ordering database");
        let older_id: i64 = connection
            .query_row(
                "SELECT id FROM file_operations WHERE job_id=?1",
                [&older_job_id],
                |row| row.get(0),
            )
            .expect("older operation id");
        let completed_id: i64 = connection
            .query_row(
                "SELECT id FROM file_operations WHERE job_id=?1",
                [&plan.plan_id],
                |row| row.get(0),
            )
            .expect("completed operation id");
        assert!(older_id < completed_id);
        drop(connection);

        assert!(preview_edit_rollback(&repository, &plan.plan_id).is_err());
        assert!(execute_edit_rollback(&repository, &plan.plan_id).is_err());
        assert!(target.is_file(), "the completed export must remain present");
    }

    #[test]
    fn durable_rollback_claim_survives_finalization_failure_and_blocks_export() {
        let (temporary, repository, _library_id, asset_id) = fixture_repository();
        let target = temporary.path().join("durable-rollback-claim.jpg");
        let first_plan =
            preview_edit_export(&repository, asset_id, &target, &EditRecipe::default())
                .expect("first plan");
        execute_edit_export(&repository, &first_plan.plan_id).expect("first export");

        let connection = open(&repository).expect("trigger database");
        connection
            .execute_batch(
                "CREATE TRIGGER reject_rollback_finalization
                 BEFORE UPDATE OF rollback_status ON file_operations
                 WHEN NEW.rollback_status='completed'
                 BEGIN
                     SELECT RAISE(FAIL, 'injected rollback finalization failure');
                 END;",
            )
            .expect("finalization failure trigger");
        drop(connection);

        assert!(execute_edit_rollback(&repository, &first_plan.plan_id).is_err());
        assert!(
            !target.exists(),
            "the injected database failure happens after the generated copy is removed"
        );
        let connection = open(&repository).expect("durable claim database");
        let rollback_status: String = connection
            .query_row(
                "SELECT rollback_status FROM file_operations WHERE job_id=?1",
                [&first_plan.plan_id],
                |row| row.get(0),
            )
            .expect("durable rollback state");
        assert_eq!(rollback_status, "running");
        drop(connection);

        let next_plan = preview_edit_export(&repository, asset_id, &target, &EditRecipe::default())
            .expect("next export plan");
        assert!(execute_edit_export(&repository, &next_plan.plan_id).is_err());
        let connection = open(&repository).expect("blocked export database");
        let next_job_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM file_operation_jobs WHERE id=?1)",
                [&next_plan.plan_id],
                |row| row.get(0),
            )
            .expect("next job existence");
        assert!(!next_job_exists, "blocked export must not register a job");
    }

    #[test]
    fn edit_export_rollback_rejects_targets_inside_a_source_library() {
        let (temporary, repository, _library_id, asset_id) = fixture_repository();
        let generated_target = temporary.path().join("safe-export.jpg");
        let plan = preview_edit_export(
            &repository,
            asset_id,
            &generated_target,
            &EditRecipe::default(),
        )
        .expect("export plan");
        execute_edit_export(&repository, &plan.plan_id).expect("export");

        let source = repository.asset_source(asset_id).expect("source").0;
        let source_hash = fingerprint(&source).expect("source hash before tampering");
        let connection = open(&repository).expect("database");
        connection
            .execute(
                "UPDATE edit_export_plans SET target_path=?2 WHERE id=?1",
                params![plan.plan_id, source.to_string_lossy()],
            )
            .expect("redirect plan to original source");
        connection
            .execute(
                "UPDATE file_operations SET target_path=?2, target_hash=?3 WHERE job_id=?1",
                params![plan.plan_id, source.to_string_lossy(), source_hash],
            )
            .expect("redirect owned operation to original source");
        drop(connection);

        assert!(matches!(
            preview_edit_rollback(&repository, &plan.plan_id),
            Err(AppError::UnsafePath(_))
        ));
        assert!(matches!(
            execute_edit_rollback(&repository, &plan.plan_id),
            Err(AppError::UnsafePath(_))
        ));
        assert_eq!(
            fingerprint(&source).expect("source hash after rejected rollback"),
            source_hash
        );
    }

    #[cfg(windows)]
    #[test]
    fn edit_export_rollback_uses_windows_case_and_extended_path_identity() {
        let (temporary, repository, _library_id, asset_id) = fixture_repository();
        let first_target = temporary.path().join("Case-Alias.jpg");
        let recipe = EditRecipe::default();
        let first_plan =
            preview_edit_export(&repository, asset_id, &first_target, &recipe).expect("first plan");
        execute_edit_export(&repository, &first_plan.plan_id).expect("first export");
        let first_hash = fingerprint(&first_target).expect("first export hash");
        std::fs::remove_file(&first_target).expect("remove first generated copy");

        let case_alias = temporary.path().join("case-alias.jpg");
        let alias_text = case_alias.to_string_lossy();
        let extended_alias = if alias_text.starts_with("\\\\?\\") {
            alias_text.into_owned()
        } else {
            format!("\\\\?\\{alias_text}")
        };
        let second_target = PathBuf::from(extended_alias);
        let second_plan = preview_edit_export(&repository, asset_id, &second_target, &recipe)
            .expect("case and extended-prefix alias plan");

        let connection = open(&repository).expect("rollback claim database");
        connection
            .execute(
                "UPDATE file_operations SET rollback_status='running' WHERE job_id=?1",
                [&first_plan.plan_id],
            )
            .expect("simulate durable rollback claim");
        drop(connection);
        assert!(execute_edit_export(&repository, &second_plan.plan_id).is_err());
        let connection = open(&repository).expect("blocked alias export database");
        let second_job_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM file_operation_jobs WHERE id=?1)",
                [&second_plan.plan_id],
                |row| row.get(0),
            )
            .expect("second job existence");
        assert!(!second_job_exists);
        connection
            .execute(
                "UPDATE file_operations SET rollback_status='failed' WHERE job_id=?1",
                [&first_plan.plan_id],
            )
            .expect("resolve simulated claim");
        drop(connection);

        execute_edit_export(&repository, &second_plan.plan_id).expect("second export");
        assert_eq!(
            fingerprint(&second_target).expect("second export hash"),
            first_hash
        );
        assert_eq!(
            identity_key(Path::new(&first_plan.target_path)),
            identity_key(Path::new(&second_plan.target_path))
        );

        let source = repository.asset_source(asset_id).expect("source").0;
        let source_hash = fingerprint(&source).expect("source hash");
        let running_job_id = Uuid::new_v4().to_string();
        let connection = open(&repository).expect("running alias database");
        let timestamp = now();
        connection
            .execute(
                "INSERT INTO file_operation_jobs(
                    id, library_id, operation_type, status, dry_run,
                    conflict_strategy, created_at, updated_at
                 ) SELECT ?1, library_id, 'edit_copy', 'running', 0, 'skip', ?2, ?2
                   FROM assets WHERE id=?3",
                params![running_job_id, timestamp, asset_id],
            )
            .expect("running alias job");
        connection
            .execute(
                "INSERT INTO file_operations(
                    job_id, source_path, target_path, operation_type, plan_status,
                    execution_status, conflict_strategy, source_hash
                 ) VALUES(?1, ?2, ?3, 'edit_copy', 'ready', 'running', 'skip', ?4)",
                params![
                    running_job_id,
                    source.to_string_lossy(),
                    second_target.to_string_lossy(),
                    source_hash
                ],
            )
            .expect("running alias operation");
        drop(connection);
        assert!(preview_edit_rollback(&repository, &second_plan.plan_id).is_err());
        assert!(execute_edit_rollback(&repository, &second_plan.plan_id).is_err());
        let connection = open(&repository).expect("finish running alias database");
        connection
            .execute(
                "UPDATE file_operations SET execution_status='failed' WHERE job_id=?1",
                [&running_job_id],
            )
            .expect("finish running operation");
        connection
            .execute(
                "UPDATE file_operation_jobs SET status='failed' WHERE id=?1",
                [&running_job_id],
            )
            .expect("finish running job");
        drop(connection);

        assert!(preview_edit_rollback(&repository, &first_plan.plan_id).is_err());
        assert!(execute_edit_rollback(&repository, &first_plan.plan_id).is_err());
        preview_edit_rollback(&repository, &second_plan.plan_id)
            .expect("current alias export preview");
        execute_edit_rollback(&repository, &second_plan.plan_id).expect("current alias rollback");
        assert!(!first_target.exists());
    }

    #[test]
    fn legacy_edit_export_without_plan_owned_operation_is_rejected() {
        let (temporary, repository, _library_id, asset_id) = fixture_repository();
        let target = temporary.path().join("legacy-export.jpg");
        let source = repository.asset_source(asset_id).expect("source").0;
        let source_hash = fingerprint(&source).expect("source hash");
        let plan = preview_edit_export(&repository, asset_id, &target, &EditRecipe::default())
            .expect("legacy plan fixture");
        let legacy_job_id = Uuid::new_v4().to_string();
        let connection = open(&repository).expect("database");
        let timestamp = now();
        connection
            .execute(
                "INSERT INTO file_operation_jobs(
                    id, library_id, operation_type, status, dry_run,
                    conflict_strategy, created_at, updated_at
                 ) VALUES(?1, ?2, 'edit_copy', 'completed', 0, 'skip', ?3, ?3)",
                params![legacy_job_id, _library_id, timestamp],
            )
            .expect("legacy operation job");
        connection
            .execute(
                "INSERT INTO file_operations(
                    job_id, source_path, target_path, operation_type, plan_status,
                    execution_status, conflict_strategy, source_hash, target_hash
                 ) VALUES(?1, ?2, ?3, 'edit_copy', 'ready', 'completed', 'skip', ?4, ?5)",
                params![
                    legacy_job_id,
                    source.to_string_lossy(),
                    target.to_string_lossy(),
                    source_hash,
                    "legacy-target-hash"
                ],
            )
            .expect("legacy operation");
        connection
            .execute(
                "UPDATE edit_export_plans SET status='completed' WHERE id=?1",
                [&plan.plan_id],
            )
            .expect("complete legacy plan");

        let error = preview_edit_rollback(&repository, &plan.plan_id)
            .expect_err("legacy operation ownership must not be guessed");
        assert!(
            matches!(error, AppError::InvalidArgument(ref message) if message.contains("ownership")),
            "expected an explicit ownership error, got {error}"
        );
        assert!(!target.exists());
    }
}
