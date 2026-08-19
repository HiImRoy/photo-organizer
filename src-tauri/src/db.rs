use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};
use sha2::{Digest, Sha256};

use crate::classification::{
    AutoClassification, EffectiveClassification, FIELD_AUXILIARY_TAGS,
    FIELD_DOMINANT_COLOR_CATEGORY, FIELD_PRIMARY_CATEGORY, FIELD_SATURATION_LEVEL, FIELD_TONE,
    ManualClassification, normalize_override_value, normalize_tag_id, resolve_classification,
};
use crate::error::{AppError, AppResult};
use crate::models::{
    ASSET_QUERY_VERSION, AssetDetail, AssetFilter, AssetGridItem, AssetListItem, AssetPage,
    AssetQuery, AssetQueryRoot, AssetSortField, ColorPalette, ExistingAssetSnapshot, FileSnapshot,
    FolderSummary, LibrarySummary, OrganizationIssue, OrganizationIssueSeverity, OrganizationPlan,
    OrganizationPlanRecord, ProcessedImage, SemanticGroupSummary, SemanticLabelResult,
    SemanticMatchMode, SemanticProgress, SortDirection,
};
use crate::semantic::{
    ANALYSIS_VERSION as SEMANTIC_ANALYSIS_VERSION, MODEL_NAME, MODEL_VERSION, ModelMetadata,
    SIGLIP2_ANALYSIS_VERSION, SIGLIP2_MODEL_NAME, SIGLIP2_MODEL_VERSION, SemanticAnalysisOutput,
    TAXONOMY_VERSION, canonical_label_id, category_group_for_label_id,
    known_display_name_for_label_id,
};
use crate::source_identity::{SourceIdentity, identity_key, is_same_or_descendant};
use crate::subject::{
    ANALYSIS_VERSION as SUBJECT_ANALYSIS_VERSION, MODEL_NAME as SUBJECT_MODEL_NAME,
    MODEL_VERSION as SUBJECT_MODEL_VERSION, SubjectAnalysisOutput,
    TAXONOMY_VERSION as SUBJECT_TAXONOMY_VERSION,
};

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const SEMANTIC_MIGRATION: &str = include_str!("../migrations/0002_semantic_workspace.sql");
const ORGANIZATION_MIGRATION: &str = include_str!("../migrations/0003_organization_dry_run.sql");
const LIBRARY_UX_MIGRATION: &str = include_str!("../migrations/0004_library_ux_refinement.sql");
const LIBRARY_SOURCE_MIGRATION: &str =
    include_str!("../migrations/0005_library_source_hierarchy.sql");
const ASSET_IDENTITY_MIGRATION: &str = include_str!("../migrations/0006_asset_global_identity.sql");
const MANUAL_LIBRARY_HIERARCHY_MIGRATION: &str =
    include_str!("../migrations/0007_manual_library_hierarchy.sql");
const ASSET_LIBRARY_ASSIGNMENT_MIGRATION: &str =
    include_str!("../migrations/0008_asset_library_assignments.sql");
const MANUAL_CLASSIFICATION_MIGRATION: &str =
    include_str!("../migrations/0009_manual_classification_overrides.sql");
const MANUAL_ASSET_MARKS_MIGRATION: &str =
    include_str!("../migrations/0010_manual_asset_marks.sql");
const PHOTO_WORKFLOW_MVP_MIGRATION: &str =
    include_str!("../migrations/0011_photo_workflow_mvp.sql");
const SEMANTIC_TAXONOMY_MIGRATION: &str = include_str!("../migrations/0012_semantic_taxonomy.sql");
const PLACES365_EVIDENCE_MIGRATION: &str =
    include_str!("../migrations/0013_places365_evidence.sql");
const SUBJECT_ANALYSIS_MIGRATION: &str = include_str!("../migrations/0014_subject_analysis.sql");
const IMPORT_SUBFOLDER_SCOPE_MIGRATION: &str =
    include_str!("../migrations/0015_import_subfolder_scope.sql");
const UNIFIED_SOURCE_COLLECTION_MIGRATION: &str =
    include_str!("../migrations/0016_unified_source_collection.sql");

#[derive(Debug, Clone)]
pub struct Repository {
    database_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SemanticAssetCandidate {
    pub id: i64,
    pub absolute_path: PathBuf,
    pub analysis_path: PathBuf,
    pub fingerprint: String,
    pub model_name: String,
    pub model_version: String,
    pub analysis_version: String,
    pub taxonomy_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySourceRoot {
    pub library_id: i64,
    pub source_path: PathBuf,
    pub identity_key: String,
}

pub struct ProcessedAssetWrite<'a> {
    pub library_id: i64,
    pub generation: i64,
    pub snapshot: &'a FileSnapshot,
    pub fingerprint: &'a str,
    pub processed: &'a ProcessedImage,
}

pub struct SeenAssetWrite {
    pub asset_id: i64,
    pub library_id: i64,
    pub relative_path: String,
    pub generation: i64,
}

#[derive(Debug, Clone, Default)]
pub struct LibraryRemovalResult {
    pub removed: bool,
    pub removed_cache_entries: Vec<(i64, String, Option<PathBuf>)>,
}

struct FullAssetPage {
    items: Vec<AssetListItem>,
    total: i64,
    page: u32,
    page_size: u32,
}

impl Repository {
    pub fn new(database_path: impl AsRef<Path>) -> Self {
        Self {
            database_path: database_path.as_ref().to_path_buf(),
        }
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn initialize(&self) -> AppResult<()> {
        if let Some(parent) = self.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut connection = self.open()?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )?;
        let current: i64 = connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;
        for (version, sql) in [
            (1_i64, INITIAL_MIGRATION),
            (2_i64, SEMANTIC_MIGRATION),
            (3_i64, ORGANIZATION_MIGRATION),
            (4_i64, LIBRARY_UX_MIGRATION),
            (5_i64, LIBRARY_SOURCE_MIGRATION),
            (6_i64, ASSET_IDENTITY_MIGRATION),
            (7_i64, MANUAL_LIBRARY_HIERARCHY_MIGRATION),
            (8_i64, ASSET_LIBRARY_ASSIGNMENT_MIGRATION),
            (9_i64, MANUAL_CLASSIFICATION_MIGRATION),
            (10_i64, MANUAL_ASSET_MARKS_MIGRATION),
            (11_i64, PHOTO_WORKFLOW_MVP_MIGRATION),
            (12_i64, SEMANTIC_TAXONOMY_MIGRATION),
            (13_i64, PLACES365_EVIDENCE_MIGRATION),
            (14_i64, SUBJECT_ANALYSIS_MIGRATION),
            (15_i64, IMPORT_SUBFOLDER_SCOPE_MIGRATION),
            (16_i64, UNIFIED_SOURCE_COLLECTION_MIGRATION),
        ] {
            if current < version {
                let transaction = connection.transaction()?;
                transaction.execute_batch(sql)?;
                if version == 16 {
                    migrate_unified_source_collection(&transaction)?;
                }
                transaction.execute(
                    "INSERT INTO schema_migrations(version, applied_at) VALUES(?1, ?2)",
                    params![version, now()],
                )?;
                transaction.commit()?;
            }
        }
        self.backfill_path_identities()?;
        self.recover_interrupted_jobs()?;
        Ok(())
    }

    fn backfill_path_identities(&self) -> AppResult<()> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let library_needs_backfill: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM libraries
                WHERE source_identity_key = '' OR source_path = ''
            )",
            [],
            |row| row.get(0),
        )?;
        let asset_needs_backfill: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM assets
                WHERE asset_identity_key = ''
            )",
            [],
            |row| row.get(0),
        )?;

        if library_needs_backfill {
            let libraries = {
                let mut statement =
                    transaction.prepare("SELECT id, root_path FROM libraries ORDER BY id")?;
                statement
                    .query_map([], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?
            };
            for (library_id, root_path) in libraries {
                let source_path =
                    std::fs::canonicalize(&root_path).unwrap_or_else(|_| PathBuf::from(&root_path));
                let source_path_string = source_path.to_string_lossy().into_owned();
                let source_identity_key = identity_key(&source_path);
                let name = library_name(&source_path, &root_path);
                transaction.execute(
                    "UPDATE libraries
                     SET name=?2, source_path=?3, source_identity_key=?4
                     WHERE id=?1",
                    params![library_id, name, source_path_string, source_identity_key],
                )?;
            }
        }

        if asset_needs_backfill {
            let assets = {
                let mut statement =
                    transaction.prepare("SELECT id, absolute_path FROM assets ORDER BY id")?;
                statement
                    .query_map([], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?
            };
            for (asset_id, absolute_path) in assets {
                let canonical_path = std::fs::canonicalize(&absolute_path)
                    .unwrap_or_else(|_| PathBuf::from(&absolute_path));
                transaction.execute(
                    "UPDATE assets SET asset_identity_key=?2 WHERE id=?1",
                    params![asset_id, identity_key(&canonical_path)],
                )?;
            }
        }

        let libraries = {
            let mut statement = transaction.prepare(
                "SELECT id, source_identity_key
                 FROM libraries
                 WHERE source_identity_key <> ''
                 ORDER BY id",
            )?;
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut duplicate_libraries = HashMap::<String, Vec<i64>>::new();
        for (library_id, source_identity_key) in &libraries {
            duplicate_libraries
                .entry(source_identity_key.clone())
                .or_default()
                .push(*library_id);
        }

        let assets = {
            let mut statement = transaction.prepare(
                "SELECT id, asset_identity_key
                 FROM assets
                 WHERE asset_identity_key <> ''
                 ORDER BY id",
            )?;
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut duplicate_assets = HashMap::<String, Vec<i64>>::new();
        for (asset_id, asset_identity_key) in &assets {
            duplicate_assets
                .entry(asset_identity_key.clone())
                .or_default()
                .push(*asset_id);
        }

        merge_duplicate_assets(&transaction, &duplicate_assets)?;
        merge_duplicate_libraries(&transaction, &duplicate_libraries)?;

        rebuild_library_hierarchy(&transaction)?;

        transaction.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_libraries_source_identity
             ON libraries(source_identity_key)",
            [],
        )?;
        transaction.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_identity
             ON assets(asset_identity_key)",
            [],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn open(&self) -> AppResult<Connection> {
        let connection = Connection::open(&self.database_path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(connection)
    }

    pub fn recover_interrupted_jobs(&self) -> AppResult<()> {
        let connection = self.open()?;
        let timestamp = now();
        connection.execute(
            "UPDATE analysis_jobs
             SET status = 'interrupted', updated_at = ?1,
                 error_message = 'Application stopped before this task completed'
             WHERE status IN ('running', 'cancelling')
               AND job_type <> 'semantic_classification'",
            [&timestamp],
        )?;
        connection.execute(
            "UPDATE analysis_jobs
             SET status = 'queued', updated_at = ?1, error_message = NULL
             WHERE job_type = 'semantic_classification'
               AND status IN ('running', 'cancelling')",
            [&timestamp],
        )?;
        connection.execute(
            "UPDATE analysis_job_items SET status = 'queued', updated_at = ?1
             WHERE status = 'running'",
            [&timestamp],
        )?;
        connection.execute(
            "UPDATE assets SET semantic_status='queued'
             WHERE semantic_status='running'
               AND id IN(SELECT asset_id FROM analysis_job_items WHERE status='queued')",
            [],
        )?;
        connection.execute(
            "UPDATE libraries SET status = 'ready'
             WHERE status = 'scanning'",
            [],
        )?;
        Ok(())
    }

    pub fn begin_scan(&self, root_path: &str, task_id: &str) -> AppResult<(i64, i64)> {
        let source_path = Path::new(root_path);
        let source_identity_key = identity_key(source_path);
        let name = library_name(source_path, root_path);
        self.begin_scan_with_identity(root_path, &source_identity_key, &name, true, task_id)
    }

    pub fn begin_scan_with_identity(
        &self,
        root_path: &str,
        source_identity_key: &str,
        name: &str,
        include_subfolder_images: bool,
        task_id: &str,
    ) -> AppResult<(i64, i64)> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let timestamp = now();
        transaction.execute(
            "INSERT OR IGNORE INTO libraries(
                root_path, name, source_path, source_identity_key, created_at, status
             ) VALUES(?1, ?2, ?1, ?3, ?4, 'ready')",
            params![root_path, name, source_identity_key, timestamp],
        )?;
        transaction.execute(
            "UPDATE libraries
             SET root_path = ?2, source_path = ?2, name = ?3,
                 include_subfolder_images = ?4,
                 status = 'scanning', last_error = NULL, scan_generation = scan_generation + 1
             WHERE source_identity_key = ?1",
            params![
                source_identity_key,
                root_path,
                name,
                include_subfolder_images as i64
            ],
        )?;
        rebuild_library_hierarchy(&transaction)?;
        let (library_id, generation): (i64, i64) = transaction.query_row(
            "SELECT id, scan_generation
             FROM libraries WHERE source_identity_key = ?1",
            [source_identity_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        transaction.execute(
            "INSERT INTO analysis_jobs(
                id, library_id, job_type, status, progress_current, progress_total,
                execution_backend, analysis_version, created_at, updated_at
             ) VALUES(?1, ?2, 'scan_and_basic_analysis', 'running', 0, 0,
                      'cpu', ?3, ?4, ?4)",
            params![
                task_id,
                library_id,
                crate::imaging::ANALYSIS_VERSION,
                timestamp
            ],
        )?;
        transaction.commit()?;
        Ok((library_id, generation))
    }

    /// Register a set of explicitly imported source roots without starting a scan.
    ///
    /// The source roots are independent Library records. Their initial parent
    /// relationship is derived from the source paths only for records that do
    /// not already have a manual relationship. Existing manual relationships
    /// are deliberately preserved.
    pub fn ensure_library_source_roots(
        &self,
        roots: &[SourceIdentity],
        include_subfolder_images: bool,
    ) -> AppResult<Vec<LibrarySourceRoot>> {
        if roots.is_empty() {
            return Err(AppError::InvalidArgument(
                "at least one source root is required".into(),
            ));
        }

        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        for root in roots {
            let source_path = root.source_path.to_string_lossy().into_owned();
            let name = library_name(&root.source_path, &source_path);
            transaction.execute(
                "INSERT OR IGNORE INTO libraries(
                    root_path, name, source_path, source_identity_key, created_at, status
                 ) VALUES(?1, ?2, ?1, ?3, ?4, 'ready')",
                params![source_path, name, root.identity_key, now()],
            )?;
            transaction.execute(
                "UPDATE libraries
                 SET root_path=?2, source_path=?2, name=?3,
                     include_subfolder_images=?4
                 WHERE source_identity_key=?1",
                params![
                    root.identity_key,
                    source_path,
                    name,
                    include_subfolder_images as i64
                ],
            )?;
        }
        rebuild_library_hierarchy(&transaction)?;

        let mut registered = Vec::with_capacity(roots.len());
        for root in roots {
            let source_root = transaction.query_row(
                "SELECT id, source_path, source_identity_key
                 FROM libraries WHERE source_identity_key=?1",
                [&root.identity_key],
                |row| {
                    Ok(LibrarySourceRoot {
                        library_id: row.get(0)?,
                        source_path: PathBuf::from(row.get::<_, String>(1)?),
                        identity_key: row.get(2)?,
                    })
                },
            )?;
            registered.push(source_root);
        }
        transaction.commit()?;
        Ok(registered)
    }

    pub fn begin_existing_library_scan(&self, library_id: i64) -> AppResult<i64> {
        let connection = self.open()?;
        let changed = connection.execute(
            "UPDATE libraries
             SET status='scanning', last_error=NULL, scan_generation=scan_generation + 1
             WHERE id=?1",
            [library_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("library {library_id}")));
        }
        connection
            .query_row(
                "SELECT scan_generation FROM libraries WHERE id=?1",
                [library_id],
                |row| row.get(0),
            )
            .map_err(AppError::from)
    }

    pub fn library_include_subfolder_images(&self, library_id: i64) -> AppResult<bool> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT include_subfolder_images FROM libraries WHERE id=?1",
                [library_id],
                |row| Ok(row.get::<_, i64>(0)? != 0),
            )
            .map_err(AppError::from)
    }

    pub fn set_library_parent(
        &self,
        library_id: i64,
        parent_library_id: Option<i64>,
    ) -> AppResult<bool> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let child_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM libraries WHERE id=?1)",
            [library_id],
            |row| row.get(0),
        )?;
        if !child_exists {
            transaction.commit()?;
            return Ok(false);
        }

        if let Some(parent_id) = parent_library_id {
            if parent_id == library_id {
                return Err(AppError::InvalidArgument(
                    "a library cannot be its own parent".into(),
                ));
            }
            let parent_exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM libraries WHERE id=?1)",
                [parent_id],
                |row| row.get(0),
            )?;
            if !parent_exists {
                return Err(AppError::NotFound(format!("parent library {parent_id}")));
            }
            let parent_is_descendant: bool = transaction.query_row(
                "WITH RECURSIVE descendants(id) AS (
                     SELECT id FROM libraries WHERE parent_library_id=?1
                     UNION
                     SELECT child.id
                     FROM libraries child
                     JOIN descendants parent ON child.parent_library_id=parent.id
                 )
                 SELECT EXISTS(SELECT 1 FROM descendants WHERE id=?2)",
                params![library_id, parent_id],
                |row| row.get(0),
            )?;
            if parent_is_descendant {
                return Err(AppError::InvalidArgument(
                    "a library cannot be moved below one of its descendants".into(),
                ));
            }
        }

        transaction.execute(
            "UPDATE libraries
             SET parent_library_id=?2, parent_relation='manual'
             WHERE id=?1",
            params![library_id, parent_library_id],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    /// Assign an asset to a Library for browsing without changing its actual
    /// SourceRoot owner or any source-relative path data. Assigning it back to
    /// that owner removes the manual override and restores automatic browsing.
    pub fn assign_asset_to_library(
        &self,
        asset_id: i64,
        target_library_id: i64,
    ) -> AppResult<bool> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let Some(owner_library_id) = transaction
            .query_row(
                "SELECT library_id FROM assets WHERE id=?1",
                [asset_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        else {
            transaction.commit()?;
            return Ok(false);
        };
        let target_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM libraries WHERE id=?1)",
            [target_library_id],
            |row| row.get(0),
        )?;
        if !target_exists {
            transaction.commit()?;
            return Ok(false);
        }

        if owner_library_id == target_library_id {
            transaction.execute(
                "DELETE FROM asset_library_assignments WHERE asset_id=?1",
                [asset_id],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO asset_library_assignments(asset_id, library_id, assigned_at)
                 VALUES(?1, ?2, ?3)
                 ON CONFLICT(asset_id) DO UPDATE SET
                    library_id=excluded.library_id,
                    assigned_at=excluded.assigned_at",
                params![asset_id, target_library_id, now()],
            )?;
        }
        transaction.commit()?;
        Ok(true)
    }

    pub fn update_job_progress(&self, task_id: &str, processed: u64, total: u64) -> AppResult<()> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE analysis_jobs
             SET progress_current = ?2, progress_total = ?3, updated_at = ?4
             WHERE id = ?1",
            params![task_id, as_i64(processed), as_i64(total), now()],
        )?;
        Ok(())
    }

    pub fn library_source_roots(&self) -> AppResult<Vec<LibrarySourceRoot>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, source_path, source_identity_key
             FROM libraries
             WHERE source_identity_key <> ''
             ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(LibrarySourceRoot {
                library_id: row.get(0)?,
                source_path: PathBuf::from(row.get::<_, String>(1)?),
                identity_key: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn library_source_root(&self, library_id: i64) -> AppResult<Option<LibrarySourceRoot>> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT id, source_path, source_identity_key
                 FROM libraries
                 WHERE id=?1 AND source_identity_key <> ''",
                [library_id],
                |row| {
                    Ok(LibrarySourceRoot {
                        library_id: row.get(0)?,
                        source_path: PathBuf::from(row.get::<_, String>(1)?),
                        identity_key: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn resolve_library_owner(&self, path: &Path) -> AppResult<Option<LibrarySourceRoot>> {
        let path_key = identity_key(path);
        Ok(self
            .library_source_roots()?
            .into_iter()
            .filter(|root| is_same_or_descendant(&root.identity_key, &path_key))
            .max_by_key(|root| path_depth(&root.identity_key)))
    }

    pub fn descendant_source_roots(&self, library_id: i64) -> AppResult<Vec<LibrarySourceRoot>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "WITH RECURSIVE descendants(library_id) AS (
                 SELECT id FROM libraries
                  WHERE parent_library_id=?1 AND parent_relation='source'
                 UNION
                 SELECT child.id
                 FROM libraries child
             JOIN descendants parent ON child.parent_library_id=parent.library_id
                 AND child.parent_relation='source'
             )
             SELECT libraries.id, libraries.source_path, libraries.source_identity_key
             FROM libraries
             JOIN descendants ON descendants.library_id=libraries.id
             ORDER BY LENGTH(libraries.source_identity_key) DESC",
        )?;
        let rows = statement.query_map([library_id], |row| {
            Ok(LibrarySourceRoot {
                library_id: row.get(0)?,
                source_path: PathBuf::from(row.get::<_, String>(1)?),
                identity_key: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    /// Return every explicitly imported SourceRoot physically nested below a
    /// library's source path. This is intentionally independent of the
    /// PhotoOrganizer parent relation: ownership pruning must remain safe even
    /// after a user manually moves a Library to another branch.
    pub fn nested_source_roots(&self, library_id: i64) -> AppResult<Vec<LibrarySourceRoot>> {
        let Some(parent) = self.library_source_root(library_id)? else {
            return Ok(Vec::new());
        };
        Ok(self
            .library_source_roots()?
            .into_iter()
            .filter(|root| {
                root.library_id != library_id
                    && is_same_or_descendant(&parent.identity_key, &root.identity_key)
            })
            .collect())
    }

    pub fn complete_scan(&self, task_id: &str, library_id: i64, generation: i64) -> AppResult<u64> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let missing = transaction.execute(
            "UPDATE assets
             SET file_status = 'missing', scan_status = 'missing'
             WHERE library_id = ?1 AND last_seen_scan <> ?2 AND file_status <> 'missing'",
            params![library_id, generation],
        )? as u64;
        let timestamp = now();
        transaction.execute(
            "UPDATE libraries
             SET status = 'ready', last_scan_at = ?2, last_error = NULL
             WHERE id = ?1",
            params![library_id, timestamp],
        )?;
        transaction.execute(
            "UPDATE analysis_jobs
             SET status = 'completed', progress_current = progress_total,
                 updated_at = ?2, error_message = NULL
             WHERE id = ?1",
            params![task_id, timestamp],
        )?;
        transaction.commit()?;
        Ok(missing)
    }

    pub fn complete_library_scope(&self, library_id: i64, generation: i64) -> AppResult<u64> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let missing = transaction.execute(
            "UPDATE assets
             SET file_status = 'missing', scan_status = 'missing'
             WHERE library_id = ?1 AND last_seen_scan <> ?2 AND file_status <> 'missing'",
            params![library_id, generation],
        )? as u64;
        transaction.execute(
            "UPDATE libraries
             SET status='ready', last_scan_at=?2, last_error=NULL
             WHERE id=?1",
            params![library_id, now()],
        )?;
        transaction.commit()?;
        Ok(missing)
    }

    pub fn fail_library_scope(&self, library_id: i64, error: &str) -> AppResult<()> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE libraries SET status='error', last_error=?2 WHERE id=?1",
            params![library_id, error],
        )?;
        Ok(())
    }

    pub fn cancel_scan(&self, task_id: &str, library_id: i64) -> AppResult<()> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let timestamp = now();
        transaction.execute(
            "UPDATE libraries SET status = 'ready' WHERE id = ?1",
            [library_id],
        )?;
        transaction.execute(
            "UPDATE analysis_jobs SET status = 'cancelled', updated_at = ?2 WHERE id = ?1",
            params![task_id, timestamp],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn cancel_library_scope(&self, library_id: i64) -> AppResult<()> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE libraries SET status='ready' WHERE id=?1",
            [library_id],
        )?;
        Ok(())
    }

    pub fn fail_scan(&self, task_id: &str, library_id: Option<i64>, error: &str) -> AppResult<()> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let timestamp = now();
        if let Some(library_id) = library_id {
            transaction.execute(
                "UPDATE libraries SET status = 'error', last_error = ?2 WHERE id = ?1",
                params![library_id, error],
            )?;
        }
        transaction.execute(
            "UPDATE analysis_jobs
             SET status = 'failed', updated_at = ?2, error_message = ?3
             WHERE id = ?1",
            params![task_id, timestamp, error],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn find_existing_asset(
        &self,
        asset_identity_key: &str,
    ) -> AppResult<Option<ExistingAssetSnapshot>> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT a.id, a.file_size, a.modified_at, a.analysis_status,
                        t.status, t.cache_path, COALESCE(tf.algorithm_version, cf.algorithm_version)
                 FROM assets a
                 LEFT JOIN thumbnails t ON t.asset_id = a.id AND t.spec = ?2
                 LEFT JOIN tone_features tf ON tf.asset_id = a.id
                 LEFT JOIN color_features cf ON cf.asset_id = a.id
                 WHERE a.asset_identity_key = ?1",
                params![asset_identity_key, crate::imaging::THUMBNAIL_SPEC],
                |row| {
                    Ok(ExistingAssetSnapshot {
                        id: row.get(0)?,
                        file_size: row.get(1)?,
                        modified_at: row.get(2)?,
                        analysis_status: row.get(3)?,
                        thumbnail_status: row.get(4)?,
                        cache_path: row.get(5)?,
                        analysis_algorithm_version: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn touch_asset_seen(
        &self,
        asset_id: i64,
        library_id: i64,
        relative_path: &str,
        generation: i64,
    ) -> AppResult<()> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE assets
                 SET library_id = ?2, relative_path = ?3,
                 file_status = 'present', scan_status = 'indexed', error_message = NULL,
                 last_seen_at = ?4, last_seen_scan = ?5
             WHERE id = ?1",
            params![asset_id, library_id, relative_path, now(), generation],
        )?;
        Ok(())
    }

    pub fn touch_assets_seen_batch(&self, items: Vec<SeenAssetWrite>) -> AppResult<()> {
        if items.is_empty() {
            return Ok(());
        }
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let timestamp = now();
        for item in items {
            transaction.execute(
                "UPDATE assets
                     SET library_id = ?2, relative_path = ?3,
                     file_status = 'present', scan_status = 'indexed', error_message = NULL,
                     last_seen_at = ?4, last_seen_scan = ?5
                 WHERE id = ?1",
                params![
                    item.asset_id,
                    item.library_id,
                    item.relative_path,
                    timestamp,
                    item.generation,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn upsert_processed_asset(
        &self,
        library_id: i64,
        generation: i64,
        snapshot: &FileSnapshot,
        fingerprint: &str,
        processed: &ProcessedImage,
    ) -> AppResult<i64> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let asset_id = Self::upsert_processed_asset_in_transaction(
            &transaction,
            library_id,
            generation,
            snapshot,
            fingerprint,
            processed,
        )?;
        transaction.commit()?;
        Ok(asset_id)
    }

    pub fn upsert_processed_assets_batch(
        &self,
        items: &[ProcessedAssetWrite<'_>],
    ) -> AppResult<()> {
        if items.is_empty() {
            return Ok(());
        }
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        for item in items {
            Self::upsert_processed_asset_in_transaction(
                &transaction,
                item.library_id,
                item.generation,
                item.snapshot,
                item.fingerprint,
                item.processed,
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn upsert_processed_asset_in_transaction(
        transaction: &Transaction<'_>,
        library_id: i64,
        generation: i64,
        snapshot: &FileSnapshot,
        fingerprint: &str,
        processed: &ProcessedImage,
    ) -> AppResult<i64> {
        let timestamp = now();
        let asset_identity_key = identity_key(Path::new(&snapshot.absolute_path));
        transaction.execute(
            "INSERT INTO assets(
                library_id, asset_identity_key, absolute_path, relative_path,
                file_name, extension, file_size, modified_at, fingerprint,
                width, height, orientation, capture_time, camera_make, camera_model,
                lens_model, exposure_time, aperture, iso, focal_length,
                file_status, scan_status, analysis_status, error_message,
                first_seen_at, last_seen_at, last_seen_scan
             ) VALUES(
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20, 'present', 'indexed', 'completed',
                NULL, ?21, ?21, ?22
             )
             ON CONFLICT(asset_identity_key) DO UPDATE SET
                library_id = excluded.library_id,
                relative_path = excluded.relative_path,
                file_name = excluded.file_name,
                extension = excluded.extension,
                file_size = excluded.file_size,
                modified_at = excluded.modified_at,
                fingerprint = excluded.fingerprint,
                width = excluded.width,
                height = excluded.height,
                orientation = excluded.orientation,
                capture_time = excluded.capture_time,
                camera_make = excluded.camera_make,
                camera_model = excluded.camera_model,
                lens_model = excluded.lens_model,
                exposure_time = excluded.exposure_time,
                aperture = excluded.aperture,
                iso = excluded.iso,
                focal_length = excluded.focal_length,
                file_status = 'present',
                scan_status = 'indexed',
                analysis_status = 'completed',
                error_message = NULL,
                semantic_status = CASE
                    WHEN assets.fingerprint <> excluded.fingerprint THEN 'not_analyzed'
                    ELSE assets.semantic_status
                END,
                semantic_error = CASE
                    WHEN assets.fingerprint <> excluded.fingerprint THEN NULL
                    ELSE assets.semantic_error
                END,
                semantic_analyzed_at = CASE
                    WHEN assets.fingerprint <> excluded.fingerprint THEN NULL
                    ELSE assets.semantic_analyzed_at
                END,
                classification_revision = CASE
                    WHEN assets.fingerprint <> excluded.fingerprint
                    THEN assets.classification_revision + 1
                    ELSE assets.classification_revision
                END,
                last_seen_at = excluded.last_seen_at,
                last_seen_scan = excluded.last_seen_scan",
            params![
                library_id,
                asset_identity_key,
                snapshot.absolute_path,
                snapshot.relative_path,
                snapshot.file_name,
                snapshot.extension,
                snapshot.file_size,
                snapshot.modified_at,
                fingerprint,
                i64::from(processed.width),
                i64::from(processed.height),
                i64::from(processed.exif.orientation),
                processed.exif.capture_time,
                processed.exif.camera_make,
                processed.exif.camera_model,
                processed.exif.lens_model,
                processed.exif.exposure_time,
                processed.exif.aperture,
                processed.exif.iso,
                processed.exif.focal_length,
                timestamp,
                generation,
            ],
        )?;
        let asset_id: i64 = transaction.query_row(
            "SELECT id FROM assets WHERE asset_identity_key = ?1",
            [&asset_identity_key],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO thumbnails(
                asset_id, cache_path, spec, source_modified_at, source_size,
                status, error_message, updated_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, 'ready', NULL, ?6)
             ON CONFLICT(asset_id, spec) DO UPDATE SET
                cache_path = excluded.cache_path,
                source_modified_at = excluded.source_modified_at,
                source_size = excluded.source_size,
                status = 'ready', error_message = NULL, updated_at = excluded.updated_at",
            params![
                asset_id,
                processed.thumbnail_path,
                crate::imaging::THUMBNAIL_SPEC,
                snapshot.modified_at,
                snapshot.file_size,
                timestamp,
            ],
        )?;
        let features = &processed.features;
        transaction.execute(
            "INSERT INTO tone_features(
                asset_id, brightness_mean, brightness_median, brightness_low_percentile,
                brightness_high_percentile, shadow_ratio, highlight_ratio, contrast,
                dynamic_range, tone_label, exposure_label, contrast_label,
                algorithm_version, analyzed_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(asset_id) DO UPDATE SET
                brightness_mean=excluded.brightness_mean,
                brightness_median=excluded.brightness_median,
                brightness_low_percentile=excluded.brightness_low_percentile,
                brightness_high_percentile=excluded.brightness_high_percentile,
                shadow_ratio=excluded.shadow_ratio,
                highlight_ratio=excluded.highlight_ratio,
                contrast=excluded.contrast,
                dynamic_range=excluded.dynamic_range,
                tone_label=excluded.tone_label,
                exposure_label=excluded.exposure_label,
                contrast_label=excluded.contrast_label,
                algorithm_version=excluded.algorithm_version,
                analyzed_at=excluded.analyzed_at",
            params![
                asset_id,
                features.brightness_mean,
                features.brightness_median,
                features.brightness_low_percentile,
                features.brightness_high_percentile,
                features.shadow_ratio,
                features.highlight_ratio,
                features.contrast,
                features.dynamic_range,
                features.tone_label,
                features.exposure_label,
                features.contrast_label,
                features.algorithm_version,
                timestamp,
            ],
        )?;
        transaction.execute(
            "INSERT INTO color_features(
                asset_id, saturation_mean, saturation_median, chroma_mean, dominant_color_rgb,
                dominant_color_category, dominant_colors_json, hue_histogram_json,
                warmth_score, neutral_ratio, colorfulness, monochrome_probability,
                dominant_color_coverage, saturation_label, algorithm_version, analyzed_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT(asset_id) DO UPDATE SET
                saturation_mean=excluded.saturation_mean,
                saturation_median=excluded.saturation_median,
                chroma_mean=excluded.chroma_mean,
                dominant_color_rgb=excluded.dominant_color_rgb,
                dominant_color_category=excluded.dominant_color_category,
                dominant_colors_json=excluded.dominant_colors_json,
                hue_histogram_json=excluded.hue_histogram_json,
                warmth_score=excluded.warmth_score,
                neutral_ratio=excluded.neutral_ratio,
                colorfulness=excluded.colorfulness,
                monochrome_probability=excluded.monochrome_probability,
                dominant_color_coverage=excluded.dominant_color_coverage,
                saturation_label=excluded.saturation_label,
                algorithm_version=excluded.algorithm_version,
                analyzed_at=excluded.analyzed_at",
            params![
                asset_id,
                features.saturation_mean,
                features.saturation_median,
                features.chroma_mean,
                features.dominant_color_rgb,
                features.dominant_color_category,
                features.dominant_colors_json,
                features.hue_histogram_json,
                features.warmth_score,
                features.neutral_ratio,
                features.colorfulness,
                features.monochrome_probability,
                features.dominant_color_coverage,
                features.saturation_label,
                features.algorithm_version,
                timestamp,
            ],
        )?;
        Ok(asset_id)
    }

    pub fn upsert_failed_asset(
        &self,
        library_id: i64,
        generation: i64,
        snapshot: &FileSnapshot,
        fingerprint: &str,
        error: &str,
    ) -> AppResult<i64> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let timestamp = now();
        let asset_identity_key = identity_key(Path::new(&snapshot.absolute_path));
        transaction.execute(
            "INSERT INTO assets(
                library_id, asset_identity_key, absolute_path, relative_path,
                file_name, extension, file_size, modified_at, fingerprint,
                file_status, scan_status, analysis_status, error_message,
                first_seen_at, last_seen_at, last_seen_scan
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'present', 'failed',
                      'failed', ?10, ?11, ?11, ?12)
             ON CONFLICT(asset_identity_key) DO UPDATE SET
                library_id=excluded.library_id,
                relative_path=excluded.relative_path,
                file_name=excluded.file_name,
                extension=excluded.extension,
                file_size=excluded.file_size,
                modified_at=excluded.modified_at,
                fingerprint=excluded.fingerprint,
                width=NULL, height=NULL,
                file_status='present', scan_status='failed', analysis_status='failed',
                error_message=excluded.error_message,
                semantic_status='not_analyzed', semantic_error=NULL, semantic_analyzed_at=NULL,
                last_seen_at=excluded.last_seen_at,
                last_seen_scan=excluded.last_seen_scan",
            params![
                library_id,
                asset_identity_key,
                snapshot.absolute_path,
                snapshot.relative_path,
                snapshot.file_name,
                snapshot.extension,
                snapshot.file_size,
                snapshot.modified_at,
                fingerprint,
                error,
                timestamp,
                generation,
            ],
        )?;
        let asset_id: i64 = transaction.query_row(
            "SELECT id FROM assets WHERE asset_identity_key=?1",
            [&asset_identity_key],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO thumbnails(
                asset_id, cache_path, spec, source_modified_at, source_size,
                status, error_message, updated_at
             ) VALUES(?1, '', ?2, ?3, ?4, 'failed', ?5, ?6)
             ON CONFLICT(asset_id, spec) DO UPDATE SET
                status='failed', error_message=excluded.error_message,
                source_modified_at=excluded.source_modified_at,
                source_size=excluded.source_size, updated_at=excluded.updated_at",
            params![
                asset_id,
                crate::imaging::THUMBNAIL_SPEC,
                snapshot.modified_at,
                snapshot.file_size,
                error,
                timestamp,
            ],
        )?;
        transaction.commit()?;
        Ok(asset_id)
    }

    pub fn list_libraries(&self) -> AppResult<Vec<LibrarySummary>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "WITH RECURSIVE library_scope(root_id, library_id) AS (
                 SELECT id, id FROM libraries
                 UNION
                 SELECT scope.root_id, child.id
                 FROM library_scope scope
                 JOIN libraries child ON child.parent_library_id = scope.library_id
                     AND child.parent_relation='source'
             )
             SELECT l.id, l.root_path, l.name, l.source_path, l.source_identity_key,
                    l.parent_library_id, l.display_order, l.created_at, l.last_scan_at, l.status,
                    COUNT(a.id) AS asset_count,
                    COALESCE(SUM(CASE WHEN a.file_status='present' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN a.file_status='missing' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN a.file_status='present'
                                      AND a.analysis_status='completed'
                                      AND a.semantic_status NOT IN ('completed')
                                      THEN 1 ELSE 0 END), 0)
             FROM libraries l
             LEFT JOIN library_scope scope ON scope.root_id = l.id
             LEFT JOIN assets a
                ON a.library_id = scope.library_id
             GROUP BY l.id
             ORDER BY l.display_order ASC,
                      COALESCE(l.last_scan_at, l.created_at) DESC, l.id DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(LibrarySummary {
                id: row.get(0)?,
                root_path: row.get(1)?,
                name: row.get(2)?,
                source_path: row.get(3)?,
                source_identity_key: row.get(4)?,
                parent_library_id: row.get(5)?,
                display_order: row.get(6)?,
                created_at: row.get(7)?,
                last_scan_at: row.get(8)?,
                status: row.get(9)?,
                asset_count: row.get(10)?,
                present_count: row.get(11)?,
                missing_count: row.get(12)?,
                semantic_pending_count: row.get(13)?,
            })
        })?;
        let mut libraries = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        for library in &mut libraries {
            if library.status == "ready" && !Path::new(&library.source_path).is_dir() {
                library.status = "unavailable".into();
            }
        }
        Ok(libraries)
    }

    pub fn asset_source(&self, asset_id: i64) -> AppResult<(PathBuf, String)> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT absolute_path, fingerprint FROM assets WHERE id=?1",
                [asset_id],
                |row| Ok((PathBuf::from(row.get::<_, String>(0)?), row.get(1)?)),
            )
            .map_err(AppError::from)
    }

    /// Remove only the indexed representation of a library. Assets are
    /// reassigned to the most specific remaining source root before the
    /// library row is deleted. Source directories are never opened or
    /// modified here.
    pub fn remove_library_with_reconciliation(
        &self,
        library_id: i64,
    ) -> AppResult<LibraryRemovalResult> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM libraries WHERE id=?1)",
            [library_id],
            |row| row.get(0),
        )?;
        if !exists {
            transaction.commit()?;
            return Ok(LibraryRemovalResult::default());
        }

        let remaining_roots = {
            let mut statement = transaction.prepare(
                "SELECT id, source_path, source_identity_key
                 FROM libraries
                 WHERE id <> ?1 AND source_identity_key <> ''
                 ORDER BY id",
            )?;
            statement
                .query_map([library_id], |row| {
                    Ok(LibrarySourceRoot {
                        library_id: row.get(0)?,
                        source_path: PathBuf::from(row.get::<_, String>(1)?),
                        identity_key: row.get(2)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?
        };

        let affected_assets = {
            let mut statement = transaction.prepare(
                "SELECT a.id, a.absolute_path, a.fingerprint, a.file_status, t.cache_path
                 FROM assets a
                 LEFT JOIN thumbnails t
                   ON t.asset_id=a.id AND t.spec=?2
                 WHERE a.library_id=?1
                 ORDER BY a.id",
            )?;
            statement
                .query_map(params![library_id, crate::imaging::THUMBNAIL_SPEC], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        PathBuf::from(row.get::<_, String>(1)?),
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?.map(PathBuf::from),
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut removed_cache_entries = Vec::new();
        for (asset_id, absolute_path, _fingerprint, file_status, cache_path) in affected_assets {
            let path_key = identity_key(&absolute_path);
            let owner = remaining_roots
                .iter()
                .filter(|root| is_same_or_descendant(&root.identity_key, &path_key))
                .max_by_key(|root| path_depth(&root.identity_key));
            if let Some(owner) = owner {
                let relative_path = relative_path_for_owner(&owner.source_path, &absolute_path);
                transaction.execute(
                    "UPDATE assets
                     SET library_id=?2, relative_path=?3, file_status=?4
                     WHERE id=?1",
                    params![asset_id, owner.library_id, relative_path, file_status],
                )?;
            } else {
                let fingerprint = transaction.query_row(
                    "SELECT fingerprint FROM assets WHERE id=?1",
                    [asset_id],
                    |row| row.get::<_, String>(0),
                )?;
                transaction.execute("DELETE FROM assets WHERE id=?1", [asset_id])?;
                let removable_cache_path = if let Some(cache_path) = cache_path {
                    let cache_path_string = cache_path.to_string_lossy().into_owned();
                    let still_referenced: bool = transaction.query_row(
                        "SELECT EXISTS(
                            SELECT 1 FROM thumbnails WHERE cache_path=?1
                        )",
                        [&cache_path_string],
                        |row| row.get(0),
                    )?;
                    (!still_referenced).then_some(cache_path)
                } else {
                    None
                };
                removed_cache_entries.push((asset_id, fingerprint, removable_cache_path));
            }
        }

        transaction.execute("DELETE FROM libraries WHERE id=?1", [library_id])?;
        rebuild_library_hierarchy(&transaction)?;
        transaction.commit()?;
        Ok(LibraryRemovalResult {
            removed: true,
            removed_cache_entries,
        })
    }

    pub fn remove_library(&self, library_id: i64) -> AppResult<bool> {
        Ok(self.remove_library_with_reconciliation(library_id)?.removed)
    }

    pub fn active_job_ids_for_library(&self, library_id: i64) -> AppResult<Vec<(String, String)>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, job_type FROM analysis_jobs
             WHERE library_id=?1 AND status IN ('queued','running','paused','cancelling')",
        )?;
        let rows = statement.query_map([library_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn library_cache_entries(
        &self,
        library_id: i64,
    ) -> AppResult<Vec<(i64, String, Option<PathBuf>)>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT a.id, a.fingerprint, t.cache_path
             FROM assets a
             LEFT JOIN thumbnails t ON t.asset_id=a.id
             WHERE a.library_id=?1",
        )?;
        let rows = statement.query_map([library_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get::<_, Option<String>>(2)?.map(PathBuf::from),
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn list_assets(
        &self,
        library_id: i64,
        sort: AssetSortField,
        direction: SortDirection,
        page: u32,
        page_size: u32,
        filter: &AssetFilter,
    ) -> AppResult<AssetPage> {
        let query = legacy_asset_query(library_id, sort, direction, page, page_size, filter);
        self.query_assets(&query)
    }

    pub fn query_assets(&self, query: &AssetQuery) -> AppResult<AssetPage> {
        let full = self.query_assets_full(query)?;
        Ok(AssetPage {
            items: full.items.iter().map(AssetGridItem::from).collect(),
            total: full.total,
            page: full.page,
            page_size: full.page_size,
        })
    }

    /// Load a complete query result for workflow services that need to inspect
    /// every matching asset. The same root, filter, count and pagination SQL
    /// powers the grid command; this helper only walks its pages inside Rust.
    pub fn list_assets_for_query(&self, query: &AssetQuery) -> AppResult<Vec<AssetListItem>> {
        let mut page = 1;
        let mut items = Vec::new();
        loop {
            let paged_query = AssetQuery {
                page,
                page_size: 500,
                ..query.clone()
            };
            let result = self.query_assets_full(&paged_query)?;
            let reached_end = result.items.len() < result.page_size as usize;
            items.extend(result.items);
            if reached_end || items.len() as i64 >= result.total {
                break;
            }
            page += 1;
        }
        Ok(items)
    }

    fn ensure_organization_scope_is_single_source(
        &self,
        library_id: i64,
        filter: &AssetFilter,
    ) -> AppResult<()> {
        let connection = self.open()?;
        if let Some(collection_id) = filter.collection_id {
            let has_external_asset: bool = connection.query_row(
                "WITH RECURSIVE library_scope(library_id) AS (
                     SELECT id FROM libraries WHERE id=?2
                     UNION
                     SELECT child.id
                     FROM libraries child
                     JOIN library_scope scope
                       ON child.parent_library_id=scope.library_id
                      AND child.parent_relation='source'
                 ), collection_scope(collection_id) AS (
                     SELECT id FROM collections WHERE id=?1
                     UNION
                     SELECT child.id
                     FROM collections child
                     JOIN collection_scope parent
                       ON child.parent_collection_id=parent.collection_id
                 )
                 SELECT EXISTS(
                     SELECT 1
                     FROM collection_assets membership
                     JOIN collection_scope scope
                       ON scope.collection_id=membership.collection_id
                     JOIN assets asset ON asset.id=membership.asset_id
                     WHERE asset.library_id NOT IN (SELECT library_id FROM library_scope)
                 )",
                params![collection_id, library_id],
                |row| row.get(0),
            )?;
            if has_external_asset {
                return Err(AppError::InvalidArgument(
                    "cannot organize a collection spanning multiple Sources".into(),
                ));
            }
        }
        if filter.favorite_only {
            let has_external_asset: bool = connection.query_row(
                "WITH RECURSIVE library_scope(library_id) AS (
                     SELECT id FROM libraries WHERE id=?1
                     UNION
                     SELECT child.id
                     FROM libraries child
                     JOIN library_scope scope
                       ON child.parent_library_id=scope.library_id
                      AND child.parent_relation='source'
                 )
                 SELECT EXISTS(
                     SELECT 1
                     FROM collection_assets membership
                     JOIN collections favorite
                       ON favorite.id=membership.collection_id
                      AND favorite.system_key='default_favorites'
                     JOIN assets asset ON asset.id=membership.asset_id
                     WHERE asset.library_id NOT IN (SELECT library_id FROM library_scope)
                 )",
                [library_id],
                |row| row.get(0),
            )?;
            if has_external_asset {
                return Err(AppError::InvalidArgument(
                    "cannot organize default favorites spanning multiple Sources".into(),
                ));
            }
        }
        Ok(())
    }

    fn query_assets_full(&self, query: &AssetQuery) -> AppResult<FullAssetPage> {
        validate_asset_query(query)?;
        let connection = self.open()?;
        let page_size = query.page_size.clamp(1, 500);
        let page = query.page.max(1);
        let offset = i64::from(page.saturating_sub(1)) * i64::from(page_size);
        let (where_sql, values) = asset_filter_sql(query);
        let count_sql = format!(
            "SELECT COUNT(*)
             FROM assets a
             LEFT JOIN tone_features tf ON tf.asset_id=a.id
             LEFT JOIN color_features cf ON cf.asset_id=a.id
             WHERE {where_sql}"
        );
        let total = connection.query_row(&count_sql, params_from_iter(values.iter()), |row| {
            row.get(0)
        })?;
        let sql = format!(
            "SELECT a.id, a.library_id, a.absolute_path, a.relative_path, a.file_name,
                    a.extension, a.file_size, a.modified_at, a.width, a.height,
                    a.orientation, a.capture_time, a.camera_make, a.camera_model,
                    a.lens_model, a.exposure_time, a.aperture, a.iso, a.focal_length,
                    a.file_status, a.scan_status, a.analysis_status, a.error_message, t.status,
                    tf.brightness_mean, tf.contrast, tf.tone_label,
                    cf.saturation_mean, cf.chroma_mean, cf.saturation_label, cf.dominant_color_rgb,
                    cf.dominant_color_category, cf.neutral_ratio, cf.dominant_color_coverage,
                    a.semantic_status, a.semantic_error,
                    a.semantic_analyzed_at, a.rating, a.color_label, cf.dominant_colors_json,
                    a.is_favorite
             FROM assets a
             LEFT JOIN thumbnails t ON t.asset_id=a.id AND t.spec=?
             LEFT JOIN tone_features tf ON tf.asset_id=a.id
             LEFT JOIN color_features cf ON cf.asset_id=a.id
             WHERE {where_sql}
             ORDER BY {} {}, a.file_name COLLATE NOCASE ASC, a.id ASC
             LIMIT ? OFFSET ?",
            query.sort.sql_expression(),
            query.direction.sql(),
        );
        let mut query_values = vec![Value::Text(crate::imaging::THUMBNAIL_SPEC.into())];
        query_values.extend(values);
        query_values.push(Value::Integer(i64::from(page_size)));
        query_values.push(Value::Integer(offset));
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(query_values.iter()), |row| {
            let width: Option<i64> = row.get(8)?;
            let height: Option<i64> = row.get(9)?;
            let orientation: Option<i64> = row.get(10)?;
            let thumbnail_status: Option<String> = row.get(23)?;
            Ok(AssetListItem {
                id: row.get(0)?,
                library_id: row.get(1)?,
                absolute_path: row.get(2)?,
                relative_path: row.get(3)?,
                file_name: row.get(4)?,
                extension: row.get(5)?,
                file_size: row.get(6)?,
                modified_at: row.get(7)?,
                width: width.map(|value| value as u32),
                height: height.map(|value| value as u32),
                orientation: orientation.map(|value| value as u32),
                capture_time: row.get(11)?,
                camera_make: row.get(12)?,
                camera_model: row.get(13)?,
                lens_model: row.get(14)?,
                exposure_time: row.get(15)?,
                aperture: row.get(16)?,
                iso: row.get(17)?,
                focal_length: row.get(18)?,
                file_status: row.get(19)?,
                scan_status: row.get(20)?,
                analysis_status: row.get(21)?,
                error_message: row.get(22)?,
                thumbnail_available: thumbnail_status.as_deref() == Some("ready"),
                brightness: row.get(24)?,
                contrast: row.get(25)?,
                tone_label: row.get(26)?,
                saturation: row.get(27)?,
                chroma: row.get(28)?,
                saturation_label: row.get(29)?,
                dominant_color: row.get(30)?,
                dominant_color_category: row.get(31)?,
                color_palette: parse_color_palette(row.get(39)?),
                neutral_ratio: row.get(32)?,
                dominant_color_coverage: row.get(33)?,
                semantic_status: row.get(34)?,
                semantic_error: row.get(35)?,
                semantic_analyzed_at: row.get(36)?,
                rating: row.get(37)?,
                color_label: row.get(38)?,
                is_favorite: row.get(40)?,
                semantic_labels: Vec::new(),
                classification: EffectiveClassification::default(),
            })
        })?;
        let mut items = rows.collect::<Result<Vec<_>, _>>()?;
        let asset_ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
        let mut labels_by_asset = semantic_labels_for_assets(&connection, &asset_ids)?;
        let mut manual_by_asset = manual_classifications_for_assets(&connection, &asset_ids)?;
        let revisions_by_asset = classification_revisions_for_assets(&connection, &asset_ids)?;
        for item in &mut items {
            item.semantic_labels = labels_by_asset.remove(&item.id).unwrap_or_default();
            let semantic_labels = item.semantic_labels.clone();
            let revision = revisions_by_asset
                .get(&item.id)
                .copied()
                .unwrap_or_default();
            let manual = manual_by_asset.remove(&item.id).unwrap_or_default();
            item.classification =
                resolve_effective_classification(item, revision, &semantic_labels, manual);
            apply_effective_fields(item);
        }
        Ok(FullAssetPage {
            items,
            total,
            page: page.max(1),
            page_size,
        })
    }

    /// Load the complete source set for an organization preview through the same
    /// SQLite filter used by the browsing grid. Pagination is intentionally
    /// hidden from the planner so a preview can never silently omit matching
    /// files. Selection is applied after the database query, still inside the
    /// repository boundary rather than in the frontend.
    pub fn list_assets_for_organization(
        &self,
        library_id: i64,
        filter: &AssetFilter,
        selected_asset_ids: Option<&[i64]>,
    ) -> AppResult<Vec<AssetListItem>> {
        self.ensure_organization_scope_is_single_source(library_id, filter)?;
        let mut page = 1;
        let mut items = Vec::new();
        loop {
            let query = legacy_asset_query(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                page,
                500,
                filter,
            );
            let result = self.query_assets_full(&query)?;
            let reached_end = result.items.len() < result.page_size as usize;
            items.extend(result.items);
            if reached_end || items.len() as i64 >= result.total {
                break;
            }
            page += 1;
        }
        if let Some(selected) = selected_asset_ids {
            let selected: std::collections::HashSet<i64> = selected.iter().copied().collect();
            items.retain(|item| selected.contains(&item.id));
        }
        Ok(items)
    }

    pub fn save_organization_plan(&self, plan: &OrganizationPlan) -> AppResult<()> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let timestamp = now();
        let scope_json = serde_json::to_string(&plan.summary.scope)?;
        let rules_json = serde_json::to_string(&plan.summary.rules)?;
        let summary_json = serde_json::to_string(&plan.summary)?;
        transaction.execute(
            "INSERT OR REPLACE INTO organization_plans(
                id, library_id, target_root, scope_json, rules_json, summary_json,
                status, created_at, updated_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(
                (SELECT created_at FROM organization_plans WHERE id=?1), ?8
             ), ?8)",
            params![
                plan.summary.plan_id,
                plan.summary.library_id,
                plan.summary.target_root,
                scope_json,
                rules_json,
                summary_json,
                plan.summary.status,
                timestamp,
            ],
        )?;
        transaction.execute(
            "DELETE FROM organization_plan_items WHERE plan_id=?1",
            [&plan.summary.plan_id],
        )?;
        for item in &plan.items {
            transaction.execute(
                "INSERT INTO organization_plan_items(
                    plan_id, asset_id, source_fingerprint, target_relative_path,
                    file_size, ordinal, status
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    plan.summary.plan_id,
                    item.asset_id,
                    item.source_fingerprint,
                    item.target_relative_path,
                    i64::try_from(item.file_size).unwrap_or(i64::MAX),
                    item.ordinal,
                    serde_json::to_string(&item.status)?
                        .trim_matches('"')
                        .to_string(),
                ],
            )?;
            let item_id = transaction.last_insert_rowid();
            for issue in &item.issues {
                transaction.execute(
                    "INSERT INTO organization_plan_issues(
                        plan_id, item_id, code, severity, source_path, target_path, detail
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        plan.summary.plan_id,
                        item_id,
                        issue.code,
                        serde_json::to_string(&issue.severity)?
                            .trim_matches('"')
                            .to_string(),
                        issue.source_path,
                        issue.target_path,
                        issue.detail,
                    ],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn get_organization_plan(
        &self,
        plan_id: &str,
    ) -> AppResult<Option<OrganizationPlanRecord>> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT id, library_id, target_root, scope_json, rules_json,
                        summary_json, created_at, updated_at
                 FROM organization_plans WHERE id=?1",
                [plan_id],
                |row| {
                    let scope_json: String = row.get(3)?;
                    let rules_json: String = row.get(4)?;
                    let summary_json: String = row.get(5)?;
                    Ok(OrganizationPlanRecord {
                        plan_id: row.get(0)?,
                        library_id: row.get(1)?,
                        target_root: row.get(2)?,
                        scope: serde_json::from_str(&scope_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                        rules: serde_json::from_str(&rules_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                        summary: serde_json::from_str(&summary_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                5,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn list_organization_issues(&self, plan_id: &str) -> AppResult<Vec<OrganizationIssue>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT code, severity, source_path, target_path, detail
             FROM organization_plan_issues
             WHERE plan_id=?1
             ORDER BY id ASC",
        )?;
        let rows = statement.query_map([plan_id], |row| {
            let severity: String = row.get(1)?;
            let severity = match severity.as_str() {
                "warning" => OrganizationIssueSeverity::Warning,
                _ => OrganizationIssueSeverity::Error,
            };
            Ok(OrganizationIssue {
                code: row.get(0)?,
                severity,
                source_path: row.get(2)?,
                target_path: row.get(3)?,
                detail: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn delete_organization_plan(&self, plan_id: &str) -> AppResult<()> {
        let connection = self.open()?;
        connection.execute("DELETE FROM organization_plans WHERE id=?1", [plan_id])?;
        Ok(())
    }

    pub fn list_library_folders(&self, library_id: i64) -> AppResult<Vec<FolderSummary>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "WITH RECURSIVE library_scope(library_id) AS (
                 SELECT id FROM libraries WHERE id=?1
                 UNION
                 SELECT child.id
                 FROM libraries child
                 JOIN library_scope scope ON child.parent_library_id = scope.library_id
                     AND child.parent_relation='source'
             )
             SELECT a.relative_path FROM assets a
              WHERE a.library_id IN (SELECT library_id FROM library_scope)
               AND a.file_status='present'
             ORDER BY relative_path COLLATE NOCASE",
        )?;
        let paths = statement
            .query_map([library_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut counts = std::collections::BTreeMap::<String, i64>::new();
        for path in paths {
            let parent = Path::new(&path)
                .parent()
                .map(|value| value.to_string_lossy().into_owned())
                .filter(|value| value != ".")
                .unwrap_or_default();
            if parent.is_empty() {
                continue;
            }
            let mut current = PathBuf::from(parent);
            loop {
                let relative = current.to_string_lossy().into_owned();
                *counts.entry(relative).or_default() += 1;
                if !current.pop() {
                    break;
                }
            }
        }
        Ok(counts
            .into_iter()
            .map(|(relative_path, asset_count)| FolderSummary {
                relative_path,
                asset_count,
            })
            .collect())
    }

    pub fn list_semantic_groups(&self, library_id: i64) -> AppResult<Vec<SemanticGroupSummary>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "WITH RECURSIVE library_scope(library_id) AS (
                 SELECT id FROM libraries WHERE id=?1
                 UNION
                 SELECT child.id
                 FROM libraries child
                 JOIN library_scope scope ON child.parent_library_id = scope.library_id
                     AND child.parent_relation='source'
             )
             SELECT a.id
             FROM assets a
             WHERE a.library_id IN (SELECT library_id FROM library_scope)
               AND a.file_status='present'
             ORDER BY a.id ASC",
        )?;
        let asset_ids = statement
            .query_map([library_id], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut groups = HashMap::<String, (String, String, i64)>::new();
        for asset_id in asset_ids {
            let mut asset = load_asset_by_id(&connection, asset_id)?;
            asset.semantic_labels = semantic_labels_for_asset(&connection, asset_id)?;
            asset.classification = effective_classification_for_asset(&connection, &asset)?;
            let mut add_group = |label_id: String| {
                let label = asset
                    .semantic_labels
                    .iter()
                    .find(|label| label.label_id == label_id);
                let display_name = label
                    .map(|label| label.display_name.clone())
                    .or_else(|| known_display_name_for_label_id(&label_id).map(str::to_owned))
                    .unwrap_or_else(|| label_id.clone());
                let category_group = label
                    .map(|label| label.category_group.clone())
                    .or_else(|| category_group_for_label_id(&label_id).map(str::to_owned))
                    .unwrap_or_else(|| "subject".to_owned());
                let entry = groups
                    .entry(label_id)
                    .or_insert((display_name, category_group, 0));
                entry.2 += 1;
            };
            if let Some(label_id) = asset.classification.primary_category.effective.clone() {
                add_group(label_id);
            }
            for tag_id in &asset.classification.auxiliary_tags.effective {
                add_group(tag_id.clone());
            }
        }
        let mut result = groups
            .into_iter()
            .map(
                |(label_id, (display_name, category_group, asset_count))| SemanticGroupSummary {
                    label_id,
                    display_name,
                    category_group,
                    asset_count,
                },
            )
            .collect::<Vec<_>>();
        result.sort_by(|left, right| {
            right
                .asset_count
                .cmp(&left.asset_count)
                .then_with(|| left.label_id.cmp(&right.label_id))
        });
        Ok(result)
    }

    pub fn register_semantic_model(
        &self,
        model_path: &Path,
        tokenizer_path: &Path,
    ) -> AppResult<()> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        transaction.execute("UPDATE semantic_models SET is_active=0", [])?;
        transaction.execute(
            "INSERT INTO semantic_models(
                name, version, analysis_version, license, source_url, model_sha256,
                tokenizer_sha256, model_path, tokenizer_path, execution_backend,
                installed_at, is_active
             ) VALUES(?1, ?2, ?3, 'MIT', ?4, ?5, ?6, ?7, ?8, 'cpu', ?9, 1)
             ON CONFLICT(name, version, analysis_version) DO UPDATE SET
                model_path=excluded.model_path, tokenizer_path=excluded.tokenizer_path,
                model_sha256=excluded.model_sha256, tokenizer_sha256=excluded.tokenizer_sha256,
                execution_backend='cpu', installed_at=excluded.installed_at, is_active=1",
            params![
                MODEL_NAME,
                MODEL_VERSION,
                SEMANTIC_ANALYSIS_VERSION,
                "https://github.com/CSAILVision/places365",
                crate::semantic::MODEL_SHA256,
                crate::semantic::TOKENIZER_SHA256,
                model_path.to_string_lossy().into_owned(),
                tokenizer_path.to_string_lossy().into_owned(),
                now(),
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn register_active_semantic_model(
        &self,
        metadata: &ModelMetadata,
        model_path: &Path,
        tokenizer_path: &Path,
        source_url: &str,
    ) -> AppResult<()> {
        let tokenizer_sha256 = sha256_file(tokenizer_path)?;
        let model_sha256 = metadata
            .model_sha256
            .clone()
            .unwrap_or_else(|| String::from("unknown"));
        let license = metadata
            .license
            .clone()
            .unwrap_or_else(|| String::from("unknown"));
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        transaction.execute("UPDATE semantic_models SET is_active=0", [])?;
        transaction.execute(
            "INSERT INTO semantic_models(
                name, version, analysis_version, license, source_url, model_sha256,
                tokenizer_sha256, model_path, tokenizer_path, execution_backend,
                installed_at, is_active
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'cpu', ?10, 1)
             ON CONFLICT(name, version, analysis_version) DO UPDATE SET
                license=excluded.license, source_url=excluded.source_url,
                model_path=excluded.model_path, tokenizer_path=excluded.tokenizer_path,
                model_sha256=excluded.model_sha256, tokenizer_sha256=excluded.tokenizer_sha256,
                execution_backend='cpu', installed_at=excluded.installed_at, is_active=1",
            params![
                metadata.name,
                metadata.version,
                metadata.analysis_version,
                license,
                source_url,
                model_sha256,
                tokenizer_sha256,
                model_path.to_string_lossy().into_owned(),
                tokenizer_path.to_string_lossy().into_owned(),
                now(),
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn active_semantic_model_key(&self) -> AppResult<Option<(String, String, String)>> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT name, version, analysis_version
                 FROM semantic_models
                 WHERE is_active=1
                 ORDER BY id DESC
                 LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn create_semantic_job(
        &self,
        job_id: &str,
        library_id: i64,
        force: bool,
        only_asset_id: Option<i64>,
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        let semantic_model = default_semantic_model_metadata();
        self.create_semantic_job_with_semantic_model(
            job_id,
            library_id,
            force,
            only_asset_id,
            &semantic_model,
            None,
        )
    }

    pub fn create_semantic_job_with_models(
        &self,
        job_id: &str,
        library_id: i64,
        force: bool,
        only_asset_id: Option<i64>,
        subject_model: Option<&ModelMetadata>,
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        let semantic_model = default_semantic_model_metadata();
        self.create_semantic_job_with_semantic_model(
            job_id,
            library_id,
            force,
            only_asset_id,
            &semantic_model,
            subject_model,
        )
    }

    pub fn create_semantic_job_with_semantic_model(
        &self,
        job_id: &str,
        library_id: i64,
        force: bool,
        only_asset_id: Option<i64>,
        semantic_model: &ModelMetadata,
        subject_model: Option<&ModelMetadata>,
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        self.create_semantic_job_with_ids(
            job_id,
            library_id,
            force,
            only_asset_id,
            None,
            semantic_model,
            subject_model,
        )
    }

    pub fn create_semantic_job_for_assets(
        &self,
        job_id: &str,
        library_id: i64,
        asset_ids: &[i64],
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        let semantic_model = default_semantic_model_metadata();
        self.create_semantic_job_for_assets_with_semantic_model(
            job_id,
            library_id,
            asset_ids,
            &semantic_model,
            None,
        )
    }

    pub fn create_semantic_job_for_assets_with_model(
        &self,
        job_id: &str,
        library_id: i64,
        asset_ids: &[i64],
        subject_model: Option<&ModelMetadata>,
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        let semantic_model = default_semantic_model_metadata();
        self.create_semantic_job_for_assets_with_semantic_model(
            job_id,
            library_id,
            asset_ids,
            &semantic_model,
            subject_model,
        )
    }

    pub fn create_semantic_job_for_assets_with_semantic_model(
        &self,
        job_id: &str,
        library_id: i64,
        asset_ids: &[i64],
        semantic_model: &ModelMetadata,
        subject_model: Option<&ModelMetadata>,
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        self.create_semantic_job_with_ids(
            job_id,
            library_id,
            false,
            None,
            Some(asset_ids),
            semantic_model,
            subject_model,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create_semantic_job_with_ids(
        &self,
        job_id: &str,
        library_id: i64,
        force: bool,
        only_asset_id: Option<i64>,
        only_asset_ids: Option<&[i64]>,
        semantic_model: &ModelMetadata,
        subject_model: Option<&ModelMetadata>,
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let active_job: Option<String> = transaction
            .query_row(
                "SELECT id FROM analysis_jobs
                 WHERE library_id IN (
                     WITH RECURSIVE library_scope(library_id) AS (
                         SELECT id FROM libraries WHERE id=?1
                         UNION
                    SELECT child.id FROM libraries child
                 JOIN library_scope scope ON child.parent_library_id=scope.library_id
                     AND child.parent_relation='source'
                     )
                     SELECT library_id FROM library_scope
                 )
                   AND job_type='semantic_classification'
                   AND status IN('queued', 'running', 'paused', 'cancelling')
                 ORDER BY created_at DESC LIMIT 1",
                [library_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(active_job) = active_job {
            return Err(AppError::InvalidArgument(format!(
                "semantic job is already active: {active_job}"
            )));
        }
        let thumbnail_spec = sql_literal(crate::imaging::THUMBNAIL_SPEC);
        let mut sql = format!(
            "SELECT a.id, a.absolute_path, a.fingerprint, t.cache_path
             FROM assets a
             JOIN thumbnails t
               ON t.asset_id=a.id AND t.spec={thumbnail_spec} AND t.status='ready'
              AND t.source_modified_at=a.modified_at AND t.source_size=a.file_size
              WHERE a.library_id IN (
                 WITH RECURSIVE library_scope(library_id) AS (
                     SELECT id FROM libraries WHERE id=?1
                     UNION
                     SELECT child.id FROM libraries child
                      JOIN library_scope scope ON child.parent_library_id=scope.library_id
                          AND child.parent_relation='source'
                  )
                 SELECT library_id FROM library_scope
             )
               AND a.file_status='present' AND a.analysis_status='completed'",
        );
        let mut values = vec![Value::Integer(library_id)];
        if let Some(asset_ids) = only_asset_ids {
            if asset_ids.is_empty() {
                sql.push_str(" AND 0=1");
            } else {
                sql.push_str(&format!(" AND a.id IN ({})", placeholders(asset_ids.len())));
                values.extend(asset_ids.iter().copied().map(Value::Integer));
            }
        } else if let Some(asset_id) = only_asset_id {
            sql.push_str(" AND a.id=?");
            values.push(Value::Integer(asset_id));
        }
        if !force {
            let scene_not_exists = "NOT EXISTS(
                    SELECT 1 FROM semantic_labels sl
                    WHERE sl.asset_id=a.id AND sl.source_fingerprint=a.fingerprint
                      AND sl.model_name=? AND sl.model_version=? AND sl.analysis_version=?
                      AND sl.taxonomy_version=?
                 )";
            if subject_model.is_some() {
                sql.push_str(&format!(
                    " AND ({scene_not_exists} OR NOT EXISTS(
                        SELECT 1 FROM subject_analysis_runs sar
                        WHERE sar.asset_id=a.id AND sar.source_fingerprint=a.fingerprint
                          AND sar.status='completed'
                          AND sar.model_name=? AND sar.model_version=?
                          AND sar.analysis_version=? AND sar.taxonomy_version=?
                    ))"
                ));
            } else {
                sql.push_str(&format!(" AND {scene_not_exists}"));
            }
            values.push(Value::Text(semantic_model.name.clone()));
            values.push(Value::Text(semantic_model.version.clone()));
            values.push(Value::Text(semantic_model.analysis_version.clone()));
            values.push(Value::Text(TAXONOMY_VERSION.into()));
            if let Some(subject_model) = subject_model {
                values.push(Value::Text(subject_model.name.clone()));
                values.push(Value::Text(subject_model.version.clone()));
                values.push(Value::Text(subject_model.analysis_version.clone()));
                values.push(Value::Text(SUBJECT_TAXONOMY_VERSION.into()));
            }
        }
        sql.push_str(" ORDER BY a.id ASC");
        let candidates = {
            let mut statement = transaction.prepare(&sql)?;
            statement
                .query_map(params_from_iter(values.iter()), |row| {
                    let absolute_path = PathBuf::from(row.get::<_, String>(1)?);
                    Ok(SemanticAssetCandidate {
                        id: row.get(0)?,
                        absolute_path: absolute_path.clone(),
                        analysis_path: PathBuf::from(row.get::<_, String>(3)?),
                        fingerprint: row.get(2)?,
                        model_name: semantic_model.name.clone(),
                        model_version: semantic_model.version.clone(),
                        analysis_version: semantic_model.analysis_version.clone(),
                        taxonomy_version: TAXONOMY_VERSION.into(),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let timestamp = now();
        transaction.execute(
            "INSERT INTO analysis_jobs(
                id, library_id, job_type, status, progress_current, progress_total,
                completed_count, failed_count, skipped_count, execution_backend,
                model_name, model_version, analysis_version, created_at, updated_at
             ) VALUES(?1, ?2, 'semantic_classification', 'queued', 0, ?3, 0, 0, 0,
                      'cpu', ?4, ?5, ?6, ?7, ?7)",
            params![
                job_id,
                library_id,
                candidates.len() as i64,
                semantic_model.name,
                semantic_model.version,
                semantic_model.analysis_version,
                timestamp,
            ],
        )?;
        for candidate in &candidates {
            transaction.execute(
                "INSERT INTO analysis_job_items(job_id, asset_id, source_fingerprint, status, attempts, updated_at)
                 VALUES(?1, ?2, ?3, 'queued', 0, ?4)",
                params![job_id, candidate.id, candidate.fingerprint, timestamp],
            )?;
            transaction.execute(
                "UPDATE assets SET semantic_status='queued', semantic_error=NULL WHERE id=?1",
                [candidate.id],
            )?;
        }
        transaction.commit()?;
        Ok(candidates)
    }

    pub fn set_semantic_job_status(&self, job_id: &str, status: &str) -> AppResult<()> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE analysis_jobs SET status=?2, updated_at=?3 WHERE id=?1",
            params![job_id, status, now()],
        )?;
        Ok(())
    }

    pub fn mark_semantic_item_running(&self, job_id: &str, asset_id: i64) -> AppResult<()> {
        self.mark_semantic_items_running(job_id, &[asset_id])
    }

    pub fn mark_semantic_items_running(&self, job_id: &str, asset_ids: &[i64]) -> AppResult<()> {
        if asset_ids.is_empty() {
            return Ok(());
        }
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let timestamp = now();
        for asset_id in asset_ids {
            transaction.execute(
                "UPDATE analysis_job_items SET status='running', attempts=attempts+1, updated_at=?3
                 WHERE job_id=?1 AND asset_id=?2",
                params![job_id, asset_id, timestamp],
            )?;
            transaction.execute(
                "UPDATE assets SET semantic_status='running', semantic_error=NULL WHERE id=?1",
                [asset_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn save_semantic_result(
        &self,
        job_id: &str,
        candidate: &SemanticAssetCandidate,
        output: &SemanticAnalysisOutput,
    ) -> AppResult<bool> {
        let entries = [(candidate, output)];
        let (completed, _) = self.save_semantic_results(job_id, &entries)?;
        Ok(completed == 1)
    }

    pub fn save_semantic_results(
        &self,
        job_id: &str,
        entries: &[(&SemanticAssetCandidate, &SemanticAnalysisOutput)],
    ) -> AppResult<(usize, usize)> {
        if entries.is_empty() {
            return Ok((0, 0));
        }
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let mut completed = 0;
        let mut skipped = 0;

        for (candidate, output) in entries {
            let current_fingerprint: Option<String> = transaction
                .query_row(
                    "SELECT fingerprint FROM assets WHERE id=?1",
                    [candidate.id],
                    |row| row.get(0),
                )
                .optional()?;
            if current_fingerprint.as_deref() != Some(candidate.fingerprint.as_str()) {
                transaction.execute(
                    "UPDATE analysis_job_items SET status='skipped', error_message='source_changed', updated_at=?3
                     WHERE job_id=?1 AND asset_id=?2",
                    params![job_id, candidate.id, now()],
                )?;
                transaction.execute(
                    "UPDATE assets SET semantic_status='not_analyzed', semantic_error=NULL WHERE id=?1",
                    [candidate.id],
                )?;
                skipped += 1;
                continue;
            }
            transaction.execute(
                "DELETE FROM semantic_labels
                 WHERE asset_id=?1 AND model_name=?2 AND model_version=?3 AND analysis_version=?4",
                params![
                    candidate.id,
                    &candidate.model_name,
                    &candidate.model_version,
                    &candidate.analysis_version
                ],
            )?;
            transaction.execute(
                "DELETE FROM semantic_evidence
                 WHERE asset_id=?1 AND model_name=?2 AND model_version=?3 AND analysis_version=?4",
                params![
                    candidate.id,
                    &candidate.model_name,
                    &candidate.model_version,
                    &candidate.analysis_version
                ],
            )?;
            let timestamp = now();
            for prediction in &output.predictions {
                transaction.execute(
                    "INSERT INTO semantic_labels(
                        asset_id, label, display_name, similarity, threshold, model_name, model_version,
                        analysis_version, taxonomy_version, category_group, source_fingerprint,
                        is_manual, is_primary, generated_at
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12, ?13)",
                    params![
                        candidate.id,
                        prediction.label_id,
                        prediction.display_name,
                        prediction.similarity,
                        prediction.threshold,
                        &candidate.model_name,
                        &candidate.model_version,
                        &candidate.analysis_version,
                        &candidate.taxonomy_version,
                        prediction.category_group,
                        candidate.fingerprint,
                        if prediction.is_primary { 1_i64 } else { 0_i64 },
                        timestamp,
                    ],
                )?;
            }
            for (rank, evidence) in output.raw_similarities.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO semantic_evidence(
                        asset_id, model_name, model_version, analysis_version, taxonomy_version,
                        source_fingerprint, rank, label, display_name, similarity, category_group,
                        generated_at
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        candidate.id,
                        &candidate.model_name,
                        &candidate.model_version,
                        &candidate.analysis_version,
                        &candidate.taxonomy_version,
                        candidate.fingerprint,
                        rank as i64,
                        evidence.label_id,
                        evidence.display_name,
                        evidence.similarity,
                        evidence.category_group,
                        timestamp,
                    ],
                )?;
            }
            let mut embedding_bytes = Vec::with_capacity(output.embedding.len() * 4);
            for value in &output.embedding {
                embedding_bytes.extend_from_slice(&value.to_le_bytes());
            }
            transaction.execute(
                "INSERT INTO semantic_embeddings(
                    asset_id, model_name, model_version, analysis_version, source_fingerprint,
                    dimensions, vector_blob, generated_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(asset_id, model_name, model_version, analysis_version, source_fingerprint)
                 DO UPDATE SET dimensions=excluded.dimensions, vector_blob=excluded.vector_blob,
                               generated_at=excluded.generated_at",
                params![
                    candidate.id,
                    &candidate.model_name,
                    &candidate.model_version,
                    &candidate.analysis_version,
                    candidate.fingerprint,
                    output.embedding.len() as i64,
                    embedding_bytes,
                    timestamp,
                ],
            )?;
            transaction.execute(
                "UPDATE assets SET semantic_status='completed', semantic_error=NULL,
                                   semantic_analyzed_at=?2,
                                   classification_revision=classification_revision+1
                 WHERE id=?1",
                params![candidate.id, timestamp],
            )?;
            transaction.execute(
                "UPDATE analysis_job_items SET status='completed', error_message=NULL, updated_at=?3
                 WHERE job_id=?1 AND asset_id=?2",
                params![job_id, candidate.id, timestamp],
            )?;
            completed += 1;
        }
        transaction.commit()?;
        Ok((completed, skipped))
    }

    pub fn save_subject_result(
        &self,
        candidate: &SemanticAssetCandidate,
        output: &SubjectAnalysisOutput,
    ) -> AppResult<bool> {
        let entries = [(candidate, output)];
        let (completed, _) = self.save_subject_results(&entries)?;
        Ok(completed == 1)
    }

    pub fn save_subject_results(
        &self,
        entries: &[(&SemanticAssetCandidate, &SubjectAnalysisOutput)],
    ) -> AppResult<(usize, usize)> {
        if entries.is_empty() {
            return Ok((0, 0));
        }
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let mut completed = 0;
        let mut skipped = 0;
        for (candidate, output) in entries {
            let current_fingerprint: Option<String> = transaction
                .query_row(
                    "SELECT fingerprint FROM assets WHERE id=?1",
                    [candidate.id],
                    |row| row.get(0),
                )
                .optional()?;
            if current_fingerprint.as_deref() != Some(candidate.fingerprint.as_str()) {
                skipped += 1;
                continue;
            }
            delete_subject_identity(&transaction, candidate.id)?;
            let timestamp = now();
            transaction.execute(
                "INSERT INTO subject_analysis_runs(
                    asset_id, source_fingerprint, model_name, model_version,
                    analysis_version, taxonomy_version, status, error_message, analyzed_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'completed', NULL, ?7)",
                params![
                    candidate.id,
                    candidate.fingerprint,
                    SUBJECT_MODEL_NAME,
                    SUBJECT_MODEL_VERSION,
                    SUBJECT_ANALYSIS_VERSION,
                    SUBJECT_TAXONOMY_VERSION,
                    timestamp,
                ],
            )?;
            for prediction in &output.predictions {
                transaction.execute(
                    "INSERT INTO subject_labels(
                        asset_id, label, display_name, similarity, threshold, model_name,
                        model_version, analysis_version, taxonomy_version, source_fingerprint,
                        generated_at
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        candidate.id,
                        prediction.label_id,
                        prediction.display_name,
                        prediction.similarity,
                        prediction.threshold,
                        SUBJECT_MODEL_NAME,
                        SUBJECT_MODEL_VERSION,
                        SUBJECT_ANALYSIS_VERSION,
                        SUBJECT_TAXONOMY_VERSION,
                        candidate.fingerprint,
                        timestamp,
                    ],
                )?;
            }
            transaction.execute(
                "UPDATE assets SET classification_revision=classification_revision+1 WHERE id=?1",
                [candidate.id],
            )?;
            completed += 1;
        }
        transaction.commit()?;
        Ok((completed, skipped))
    }

    pub fn save_subject_failure(
        &self,
        candidate: &SemanticAssetCandidate,
        error: &str,
    ) -> AppResult<()> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let current_fingerprint: Option<String> = transaction
            .query_row(
                "SELECT fingerprint FROM assets WHERE id=?1",
                [candidate.id],
                |row| row.get(0),
            )
            .optional()?;
        if current_fingerprint.as_deref() == Some(candidate.fingerprint.as_str()) {
            delete_subject_identity(&transaction, candidate.id)?;
            transaction.execute(
                "INSERT INTO subject_analysis_runs(
                    asset_id, source_fingerprint, model_name, model_version,
                    analysis_version, taxonomy_version, status, error_message, analyzed_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'failed', ?7, ?8)",
                params![
                    candidate.id,
                    candidate.fingerprint,
                    SUBJECT_MODEL_NAME,
                    SUBJECT_MODEL_VERSION,
                    SUBJECT_ANALYSIS_VERSION,
                    SUBJECT_TAXONOMY_VERSION,
                    error,
                    now(),
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn clear_subject_data(&self) -> AppResult<u64> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let count = transaction.query_row(
            "SELECT COUNT(DISTINCT asset_id) FROM subject_analysis_runs",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        transaction.execute(
            "UPDATE assets SET classification_revision=classification_revision+1
             WHERE id IN (SELECT DISTINCT asset_id FROM subject_analysis_runs)",
            [],
        )?;
        transaction.execute("DELETE FROM subject_labels", [])?;
        transaction.execute("DELETE FROM subject_analysis_runs", [])?;
        transaction.commit()?;
        Ok(count as u64)
    }

    pub fn fail_semantic_item(&self, job_id: &str, asset_id: i64, error: &str) -> AppResult<()> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let timestamp = now();
        transaction.execute(
            "UPDATE analysis_job_items SET status='failed', error_message=?3, updated_at=?4
             WHERE job_id=?1 AND asset_id=?2",
            params![job_id, asset_id, error, timestamp],
        )?;
        transaction.execute(
            "DELETE FROM semantic_labels
             WHERE asset_id=?1
               AND model_name=(SELECT model_name FROM analysis_jobs WHERE id=?2)
               AND model_version=(SELECT model_version FROM analysis_jobs WHERE id=?2)
               AND analysis_version=(SELECT analysis_version FROM analysis_jobs WHERE id=?2)",
            params![asset_id, job_id],
        )?;
        transaction.execute(
            "DELETE FROM semantic_evidence
             WHERE asset_id=?1
               AND model_name=(SELECT model_name FROM analysis_jobs WHERE id=?2)
               AND model_version=(SELECT model_version FROM analysis_jobs WHERE id=?2)
               AND analysis_version=(SELECT analysis_version FROM analysis_jobs WHERE id=?2)",
            params![asset_id, job_id],
        )?;
        transaction.execute(
            "UPDATE assets SET semantic_status='failed', semantic_error=?2,
                               classification_revision=classification_revision+1
             WHERE id=?1",
            params![asset_id, error],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_semantic_job_progress(&self, progress: &SemanticProgress) -> AppResult<()> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE analysis_jobs SET status=?2, progress_current=?3, progress_total=?4,
                completed_count=?5, failed_count=?6, skipped_count=?7,
                error_message=?8, updated_at=?9 WHERE id=?1",
            params![
                progress.job_id,
                progress.status,
                as_i64(progress.processed),
                as_i64(progress.total),
                as_i64(progress.completed),
                as_i64(progress.failed),
                as_i64(progress.skipped),
                progress.error,
                now(),
            ],
        )?;
        Ok(())
    }

    pub fn latest_semantic_progress(&self, library_id: i64) -> AppResult<Option<SemanticProgress>> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT id, library_id, status, progress_total, progress_current,
                        completed_count, failed_count, skipped_count, execution_backend,
                        model_name, model_version, error_message
                 FROM analysis_jobs
                 WHERE library_id=?1 AND job_type='semantic_classification'
                 ORDER BY created_at DESC LIMIT 1",
                [library_id],
                |row| {
                    Ok(SemanticProgress {
                        job_id: row.get(0)?,
                        library_id: row.get(1)?,
                        status: row.get(2)?,
                        total: row.get::<_, i64>(3)?.max(0) as u64,
                        processed: row.get::<_, i64>(4)?.max(0) as u64,
                        completed: row.get::<_, i64>(5)?.max(0) as u64,
                        failed: row.get::<_, i64>(6)?.max(0) as u64,
                        skipped: row.get::<_, i64>(7)?.max(0) as u64,
                        current_asset_id: None,
                        current_path: None,
                        execution_backend: row.get(8)?,
                        model_name: row.get(9)?,
                        model_version: row.get(10)?,
                        error: row.get(11)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn semantic_progress_by_job(&self, job_id: &str) -> AppResult<Option<SemanticProgress>> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT id, library_id, status, progress_total, progress_current,
                        completed_count, failed_count, skipped_count, execution_backend,
                        model_name, model_version, error_message
                 FROM analysis_jobs
                 WHERE id=?1 AND job_type='semantic_classification'",
                [job_id],
                semantic_progress_from_row,
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn recoverable_semantic_jobs(&self) -> AppResult<Vec<(String, i64)>> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, library_id FROM analysis_jobs
             WHERE job_type='semantic_classification' AND status='queued'
             ORDER BY created_at ASC",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn pending_semantic_candidates(
        &self,
        job_id: &str,
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        let connection = self.open()?;
        let thumbnail_spec = sql_literal(crate::imaging::THUMBNAIL_SPEC);
        let mut statement = connection.prepare(&format!(
            "SELECT a.id, a.absolute_path, ji.source_fingerprint, t.cache_path,
                    aj.model_name, aj.model_version, aj.analysis_version
             FROM analysis_job_items ji
             JOIN analysis_jobs aj ON aj.id=ji.job_id
             JOIN assets a ON a.id=ji.asset_id
             LEFT JOIN thumbnails t
               ON t.asset_id=a.id AND t.spec={thumbnail_spec} AND t.status='ready'
              AND t.source_modified_at=a.modified_at AND t.source_size=a.file_size
             WHERE ji.job_id=?1 AND ji.status IN('queued', 'running')
             ORDER BY ji.id ASC"
        ))?;
        let rows = statement.query_map([job_id], |row| {
            let absolute_path = PathBuf::from(row.get::<_, String>(1)?);
            Ok(SemanticAssetCandidate {
                id: row.get(0)?,
                absolute_path: absolute_path.clone(),
                // A queued item can outlive its cache after a source change.
                // Keep it in the job so the worker records a visible failure,
                // but never substitute the full-resolution source path.
                analysis_path: row
                    .get::<_, Option<String>>(3)?
                    .map(PathBuf::from)
                    .unwrap_or_default(),
                fingerprint: row.get(2)?,
                model_name: row.get(4)?,
                model_version: row.get(5)?,
                analysis_version: row.get(6)?,
                taxonomy_version: TAXONOMY_VERSION.into(),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn cancel_semantic_job(&self, job_id: &str) -> AppResult<()> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let timestamp = now();
        transaction.execute(
            "UPDATE assets SET semantic_status='not_analyzed', semantic_error=NULL
             WHERE id IN(
                SELECT asset_id FROM analysis_job_items
                WHERE job_id=?1 AND status IN('queued', 'running')
             )",
            [job_id],
        )?;
        transaction.execute(
            "UPDATE analysis_job_items
             SET status='cancelled', error_message=NULL, updated_at=?2
             WHERE job_id=?1 AND status IN('queued', 'running')",
            params![job_id, timestamp],
        )?;
        transaction.execute(
            "UPDATE analysis_jobs SET status='cancelled', updated_at=?2 WHERE id=?1",
            params![job_id, timestamp],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_asset_detail(&self, asset_id: i64) -> AppResult<AssetDetail> {
        let connection = self.open()?;
        let mut asset = load_asset_by_id(&connection, asset_id)?;
        asset.semantic_labels = semantic_labels_for_asset(&connection, asset.id)?;
        asset.classification = effective_classification_for_asset(&connection, &asset)?;
        apply_effective_fields(&mut asset);
        Ok(AssetDetail { asset })
    }

    pub fn update_classification_override(
        &self,
        asset_id: i64,
        field: &str,
        value: Option<serde_json::Value>,
    ) -> AppResult<AssetDetail> {
        if field == FIELD_AUXILIARY_TAGS || !crate::classification::is_registry_field(field) {
            return Err(AppError::InvalidArgument(format!(
                "field {field} is not a scalar classification override"
            )));
        }
        let normalized = value
            .map(|value| normalize_override_value(field, value))
            .transpose()?;
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        ensure_asset_exists(&transaction, asset_id)?;
        match normalized {
            Some(value) => {
                transaction.execute(
                    "INSERT OR REPLACE INTO manual_classification_overrides(
                        asset_id, field, value_json, updated_at
                     ) VALUES(?1, ?2, ?3, ?4)",
                    params![asset_id, field, serde_json::to_string(&value)?, now()],
                )?;
            }
            None => {
                transaction.execute(
                    "DELETE FROM manual_classification_overrides
                     WHERE asset_id=?1 AND field=?2",
                    params![asset_id, field],
                )?;
            }
        }
        bump_classification_revision(&transaction, asset_id)?;
        transaction.commit()?;
        self.get_asset_detail(asset_id)
    }

    pub fn update_asset_rating(&self, asset_id: i64, rating: i64) -> AppResult<AssetDetail> {
        if !(0..=5).contains(&rating) {
            return Err(AppError::InvalidArgument(
                "asset rating must be between 0 and 5".to_owned(),
            ));
        }
        let connection = self.open()?;
        ensure_asset_exists(&connection, asset_id)?;
        connection.execute(
            "UPDATE assets SET rating=?1 WHERE id=?2",
            params![rating, asset_id],
        )?;
        self.get_asset_detail(asset_id)
    }

    pub fn update_asset_color_label(
        &self,
        asset_id: i64,
        color_label: Option<&str>,
    ) -> AppResult<AssetDetail> {
        if let Some(color_label) = color_label
            && !matches!(color_label, "red" | "yellow" | "green" | "blue" | "purple")
        {
            return Err(AppError::InvalidArgument(
                "asset color label is not supported".to_owned(),
            ));
        }
        let connection = self.open()?;
        ensure_asset_exists(&connection, asset_id)?;
        connection.execute(
            "UPDATE assets SET color_label=?1 WHERE id=?2",
            params![color_label, asset_id],
        )?;
        self.get_asset_detail(asset_id)
    }

    pub fn update_tag_override(
        &self,
        asset_id: i64,
        tag_id: &str,
        state: Option<&str>,
    ) -> AppResult<AssetDetail> {
        let tag_id = normalize_tag_id(tag_id)?;
        if let Some(state) = state
            && !matches!(state, "add" | "remove")
        {
            return Err(AppError::InvalidArgument(
                "tag override state must be add or remove".to_owned(),
            ));
        }
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        ensure_asset_exists(&transaction, asset_id)?;
        match state {
            Some(state) => {
                transaction.execute(
                    "INSERT OR REPLACE INTO manual_tag_overrides(
                        asset_id, tag_id, state, updated_at
                     ) VALUES(?1, ?2, ?3, ?4)",
                    params![asset_id, tag_id, state, now()],
                )?;
            }
            None => {
                transaction.execute(
                    "DELETE FROM manual_tag_overrides
                     WHERE asset_id=?1 AND tag_id=?2",
                    params![asset_id, tag_id],
                )?;
            }
        }
        bump_classification_revision(&transaction, asset_id)?;
        transaction.commit()?;
        self.get_asset_detail(asset_id)
    }

    pub fn restore_auto_classification(
        &self,
        asset_id: i64,
        field: Option<&str>,
    ) -> AppResult<AssetDetail> {
        if let Some(field) = field
            && !crate::classification::is_registry_field(field)
        {
            return Err(AppError::InvalidArgument(format!(
                "unknown classification field {field}"
            )));
        }
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        ensure_asset_exists(&transaction, asset_id)?;
        let changed = if field == Some(FIELD_AUXILIARY_TAGS) {
            transaction.execute(
                "DELETE FROM manual_tag_overrides WHERE asset_id=?1",
                [asset_id],
            )?
        } else if let Some(field) = field {
            transaction.execute(
                "DELETE FROM manual_classification_overrides
                 WHERE asset_id=?1 AND field=?2",
                params![asset_id, field],
            )?
        } else {
            let mut changed = transaction.execute(
                "DELETE FROM manual_classification_overrides WHERE asset_id=?1",
                [asset_id],
            )?;
            changed += transaction.execute(
                "DELETE FROM manual_tag_overrides WHERE asset_id=?1",
                [asset_id],
            )?;
            changed
        };
        if changed > 0 {
            bump_classification_revision(&transaction, asset_id)?;
        }
        transaction.commit()?;
        self.get_asset_detail(asset_id)
    }

    pub fn batch_update_classification(
        &self,
        asset_ids: &[i64],
        field: &str,
        value: serde_json::Value,
    ) -> AppResult<u64> {
        if asset_ids.is_empty() {
            return Ok(0);
        }
        if field == FIELD_AUXILIARY_TAGS || !crate::classification::is_registry_field(field) {
            return Err(AppError::InvalidArgument(format!(
                "field {field} is not supported by batch classification update"
            )));
        }
        let value = normalize_override_value(field, value)?;
        let value_json = serde_json::to_string(&value)?;
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        for asset_id in asset_ids {
            ensure_asset_exists(&transaction, *asset_id)?;
            transaction.execute(
                "INSERT OR REPLACE INTO manual_classification_overrides(
                    asset_id, field, value_json, updated_at
                 ) VALUES(?1, ?2, ?3, ?4)",
                params![asset_id, field, value_json, now()],
            )?;
            bump_classification_revision(&transaction, *asset_id)?;
        }
        transaction.commit()?;
        Ok(asset_ids.len() as u64)
    }

    pub fn thumbnail_path(&self, asset_id: i64) -> AppResult<PathBuf> {
        let connection = self.open()?;
        let path: Option<String> = connection
            .query_row(
                "SELECT cache_path FROM thumbnails
                 WHERE asset_id = ?1 AND spec = ?2 AND status = 'ready'",
                params![asset_id, crate::imaging::THUMBNAIL_SPEC],
                |row| row.get(0),
            )
            .optional()?;
        path.map(PathBuf::from)
            .ok_or_else(|| AppError::NotFound(format!("thumbnail for asset {asset_id}")))
    }

    #[cfg(test)]
    fn migration_version(&self) -> AppResult<i64> {
        let connection = self.open()?;
        connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .map_err(AppError::from)
    }
}

fn ensure_asset_exists(connection: &Connection, asset_id: i64) -> AppResult<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM assets WHERE id=?1)",
        [asset_id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("asset {asset_id}")))
    }
}

fn bump_classification_revision(connection: &Connection, asset_id: i64) -> AppResult<()> {
    connection.execute(
        "UPDATE assets SET classification_revision=classification_revision+1 WHERE id=?1",
        [asset_id],
    )?;
    Ok(())
}

fn load_asset_by_id(connection: &Connection, asset_id: i64) -> AppResult<AssetListItem> {
    connection
        .query_row(
            "SELECT a.id, a.library_id, a.absolute_path, a.relative_path, a.file_name,
                    a.extension, a.file_size, a.modified_at, a.width, a.height,
                    a.orientation, a.capture_time, a.camera_make, a.camera_model,
                    a.lens_model, a.exposure_time, a.aperture, a.iso, a.focal_length,
                    a.file_status, a.scan_status, a.analysis_status, a.error_message, t.status,
                    tf.brightness_mean, tf.contrast, tf.tone_label,
                    cf.saturation_mean, cf.chroma_mean, cf.saturation_label, cf.dominant_color_rgb,
                    cf.dominant_color_category, cf.neutral_ratio, cf.dominant_color_coverage,
                    a.semantic_status, a.semantic_error,
                    a.semantic_analyzed_at, a.rating, a.color_label, cf.dominant_colors_json,
                    a.is_favorite
             FROM assets a
             LEFT JOIN thumbnails t ON t.asset_id=a.id AND t.spec=?1
             LEFT JOIN tone_features tf ON tf.asset_id=a.id
             LEFT JOIN color_features cf ON cf.asset_id=a.id
             WHERE a.id=?2",
            params![crate::imaging::THUMBNAIL_SPEC, asset_id],
            |row| {
                let width: Option<i64> = row.get(8)?;
                let height: Option<i64> = row.get(9)?;
                let orientation: Option<i64> = row.get(10)?;
                let thumbnail_status: Option<String> = row.get(23)?;
                Ok(AssetListItem {
                    id: row.get(0)?,
                    library_id: row.get(1)?,
                    absolute_path: row.get(2)?,
                    relative_path: row.get(3)?,
                    file_name: row.get(4)?,
                    extension: row.get(5)?,
                    file_size: row.get(6)?,
                    modified_at: row.get(7)?,
                    width: width.map(|value| value as u32),
                    height: height.map(|value| value as u32),
                    orientation: orientation.map(|value| value as u32),
                    capture_time: row.get(11)?,
                    camera_make: row.get(12)?,
                    camera_model: row.get(13)?,
                    lens_model: row.get(14)?,
                    exposure_time: row.get(15)?,
                    aperture: row.get(16)?,
                    iso: row.get(17)?,
                    focal_length: row.get(18)?,
                    file_status: row.get(19)?,
                    scan_status: row.get(20)?,
                    analysis_status: row.get(21)?,
                    error_message: row.get(22)?,
                    thumbnail_available: thumbnail_status.as_deref() == Some("ready"),
                    brightness: row.get(24)?,
                    contrast: row.get(25)?,
                    tone_label: row.get(26)?,
                    saturation: row.get(27)?,
                    chroma: row.get(28)?,
                    saturation_label: row.get(29)?,
                    dominant_color: row.get(30)?,
                    dominant_color_category: row.get(31)?,
                    color_palette: parse_color_palette(row.get(39)?),
                    neutral_ratio: row.get(32)?,
                    dominant_color_coverage: row.get(33)?,
                    semantic_status: row.get(34)?,
                    semantic_error: row.get(35)?,
                    semantic_analyzed_at: row.get(36)?,
                    rating: row.get(37)?,
                    color_label: row.get(38)?,
                    is_favorite: row.get(40)?,
                    semantic_labels: Vec::new(),
                    classification: EffectiveClassification::default(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("asset {asset_id}")))
}

fn legacy_asset_query(
    library_id: i64,
    sort: AssetSortField,
    direction: SortDirection,
    page: u32,
    page_size: u32,
    filter: &AssetFilter,
) -> AssetQuery {
    let root = AssetQueryRoot::Source { library_id };
    AssetQuery {
        version: ASSET_QUERY_VERSION,
        root,
        include_descendants: true,
        filter: filter.clone(),
        sort,
        direction,
        page,
        page_size,
    }
}

fn validate_asset_query(query: &AssetQuery) -> AppResult<()> {
    if query.version != ASSET_QUERY_VERSION {
        return Err(AppError::InvalidArgument(format!(
            "unsupported asset query version: {}",
            query.version
        )));
    }
    match query.root {
        AssetQueryRoot::All => {}
        AssetQueryRoot::Source { library_id }
        | AssetQueryRoot::Collection {
            collection_id: library_id,
        } if library_id <= 0 => {
            return Err(AppError::InvalidArgument(
                "asset query root id must be positive".into(),
            ));
        }
        AssetQueryRoot::Favorites => {}
        AssetQueryRoot::Source { .. } | AssetQueryRoot::Collection { .. } => {}
    }
    Ok(())
}

fn asset_query_scope_sql(query: &AssetQuery) -> (String, Vec<Value>) {
    match query.root {
        AssetQueryRoot::All => ("1=1".into(), Vec::new()),
        AssetQueryRoot::Source { library_id } => {
            if query.include_descendants {
                (
                    "a.library_id IN (
                        WITH RECURSIVE library_scope(library_id) AS (
                            SELECT id FROM libraries WHERE id=?
                            UNION
                            SELECT child.id
                            FROM libraries child
                            JOIN library_scope scope
                              ON child.parent_library_id=scope.library_id
                             AND child.parent_relation='source'
                        )
                        SELECT library_id FROM library_scope
                    )"
                    .into(),
                    vec![Value::Integer(library_id)],
                )
            } else {
                ("a.library_id=?".into(), vec![Value::Integer(library_id)])
            }
        }
        AssetQueryRoot::Collection { collection_id } => {
            if query.include_descendants {
                (
                    "EXISTS(
                        WITH RECURSIVE collection_scope(collection_id) AS (
                            SELECT id FROM collections WHERE id=?
                            UNION
                            SELECT child.id
                            FROM collections child
                            JOIN collection_scope parent
                              ON child.parent_collection_id=parent.collection_id
                        )
                        SELECT 1
                        FROM collection_assets membership
                        JOIN collection_scope scope
                          ON scope.collection_id=membership.collection_id
                        WHERE membership.asset_id=a.id
                    )"
                    .into(),
                    vec![Value::Integer(collection_id)],
                )
            } else {
                (
                    "EXISTS(
                        SELECT 1 FROM collection_assets membership
                        WHERE membership.collection_id=?
                          AND membership.asset_id=a.id
                    )"
                    .into(),
                    vec![Value::Integer(collection_id)],
                )
            }
        }
        AssetQueryRoot::Favorites => (
            "EXISTS(
                SELECT 1
                FROM collection_assets favorite_membership
                JOIN collections favorite_collection
                  ON favorite_collection.id=favorite_membership.collection_id
                WHERE favorite_collection.system_key='default_favorites'
                  AND favorite_membership.asset_id=a.id
            )"
            .into(),
            Vec::new(),
        ),
    }
}

fn asset_filter_sql(query: &AssetQuery) -> (String, Vec<Value>) {
    let active_model_name = active_semantic_model_sql("name", SIGLIP2_MODEL_NAME);
    let active_model_version = active_semantic_model_sql("version", SIGLIP2_MODEL_VERSION);
    let active_analysis_version =
        active_semantic_model_sql("analysis_version", SIGLIP2_ANALYSIS_VERSION);
    let (scope_sql, mut values) = asset_query_scope_sql(query);
    let mut clauses = vec![scope_sql];
    let filter = &query.filter;
    if filter.favorite_only {
        clauses.push(
            "EXISTS(
                SELECT 1 FROM collection_assets favorite_membership
                JOIN collections favorite_collection
                  ON favorite_collection.id=favorite_membership.collection_id
                WHERE favorite_collection.system_key='default_favorites'
                  AND favorite_membership.asset_id=a.id
            )"
            .into(),
        );
    }
    if let Some(collection_id) = filter.collection_id {
        clauses.push(
            "EXISTS(SELECT 1 FROM collection_assets source_collection
                    WHERE source_collection.collection_id=? AND source_collection.asset_id=a.id)"
                .into(),
        );
        values.push(Value::Integer(collection_id));
    }
    let primary_categories = filter
        .primary_categories
        .iter()
        .map(|value| canonical_label_id(value).to_owned())
        .collect::<Vec<_>>();
    let auxiliary_tags = filter
        .auxiliary_tags
        .iter()
        .map(|value| canonical_label_id(value).to_owned())
        .collect::<Vec<_>>();
    if let Some(search) = filter
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        clauses
            .push("(a.file_name LIKE ? ESCAPE '\\' OR a.relative_path LIKE ? ESCAPE '\\')".into());
        let value = format!("%{}%", escape_like(search));
        values.push(Value::Text(value.clone()));
        values.push(Value::Text(value));
    }
    if !primary_categories.is_empty() {
        let expression = effective_scalar_expression(
            FIELD_PRIMARY_CATEGORY,
            &format!(
                "CASE WHEN a.semantic_status='completed' AND NOT EXISTS(
                         SELECT 1 FROM semantic_labels current_scene
                         WHERE current_scene.asset_id=a.id
                           AND current_scene.source_fingerprint=a.fingerprint
                           AND current_scene.is_manual=0 AND current_scene.is_primary=1
                           AND current_scene.model_name={model_name}
                           AND current_scene.model_version={model_version}
                           AND current_scene.analysis_version={analysis_version}
                           AND current_scene.taxonomy_version={taxonomy_version}
                       ) THEN 'photo_abstract'
                       ELSE (SELECT sl.label FROM semantic_labels sl
                             WHERE sl.asset_id=a.id AND sl.source_fingerprint=a.fingerprint
                               AND sl.is_manual=0 AND sl.is_primary=1
                               AND sl.model_name={model_name} AND sl.model_version={model_version}
                               AND sl.analysis_version={analysis_version}
                               AND sl.taxonomy_version={taxonomy_version}
                             ORDER BY sl.similarity DESC, sl.label ASC LIMIT 1)
                  END",
                model_name = active_model_name,
                model_version = active_model_version,
                analysis_version = active_analysis_version,
                taxonomy_version = sql_literal(TAXONOMY_VERSION),
            ),
        );
        clauses.push(format!(
            "{expression} IN ({})",
            placeholders(primary_categories.len())
        ));
        values.extend(primary_categories.into_iter().map(Value::Text));
    }
    if !auxiliary_tags.is_empty() {
        let predicates = auxiliary_tags
            .iter()
            .map(|_| {
                format!(
                    "(EXISTS(SELECT 1 FROM manual_tag_overrides add_tag
                            WHERE add_tag.asset_id=a.id AND add_tag.tag_id=? AND add_tag.state='add')
                     OR ((EXISTS(SELECT 1 FROM semantic_labels sl
                                  WHERE sl.asset_id=a.id AND sl.source_fingerprint=a.fingerprint
                                    AND sl.is_manual=0 AND sl.is_primary=0
                                    AND sl.model_name={model_name} AND sl.model_version={model_version}
                                    AND sl.analysis_version={analysis_version}
                                    AND sl.taxonomy_version={taxonomy_version} AND sl.label=?)
                            OR EXISTS(SELECT 1 FROM subject_labels subject
                                      WHERE subject.asset_id=a.id
                                        AND subject.source_fingerprint=a.fingerprint
                                        AND subject.model_name={subject_model_name}
                                        AND subject.model_version={subject_model_version}
                                        AND subject.analysis_version={subject_analysis_version}
                                        AND subject.taxonomy_version={subject_taxonomy_version}
                                        AND subject.label=?))
                         AND NOT EXISTS(SELECT 1 FROM manual_tag_overrides remove_tag
                                        WHERE remove_tag.asset_id=a.id AND remove_tag.tag_id=?
                                           AND remove_tag.state='remove')))" ,
                    model_name = active_model_name,
                     model_version = active_model_version,
                     analysis_version = active_analysis_version,
                     taxonomy_version = sql_literal(TAXONOMY_VERSION),
                     subject_model_name = sql_literal(SUBJECT_MODEL_NAME),
                     subject_model_version = sql_literal(SUBJECT_MODEL_VERSION),
                     subject_analysis_version = sql_literal(SUBJECT_ANALYSIS_VERSION),
                     subject_taxonomy_version = sql_literal(SUBJECT_TAXONOMY_VERSION),
                )
            })
            .collect::<Vec<_>>();
        let joiner = match filter.semantic_match {
            SemanticMatchMode::Any => " OR ",
            SemanticMatchMode::All => " AND ",
        };
        clauses.push(format!("({})", predicates.join(joiner)));
        for tag in &auxiliary_tags {
            values.push(Value::Text(tag.clone()));
            values.push(Value::Text(tag.clone()));
            values.push(Value::Text(tag.clone()));
            values.push(Value::Text(tag.clone()));
        }
    }
    add_string_in_filter(
        &mut clauses,
        &mut values,
        &effective_scalar_expression(FIELD_TONE, "tf.tone_label"),
        &filter.tone_labels,
    );
    add_palette_filter(&mut clauses, &mut values, &filter.color_categories);
    add_hue_range_filter(
        &mut clauses,
        &mut values,
        filter.color_hue_center,
        filter.color_hue_width,
        filter.color_hue_strictness,
    );
    add_rating_threshold_filter(&mut clauses, &mut values, "a.rating", &filter.ratings);
    add_string_in_filter(
        &mut clauses,
        &mut values,
        "a.color_label",
        &filter.color_labels,
    );
    add_string_in_filter(
        &mut clauses,
        &mut values,
        &effective_scalar_expression(FIELD_SATURATION_LEVEL, "cf.saturation_label"),
        &filter.saturation_levels,
    );
    add_number_bound(
        &mut clauses,
        &mut values,
        "tf.brightness_mean",
        ">=",
        filter.brightness_min,
    );
    add_number_bound(
        &mut clauses,
        &mut values,
        "tf.brightness_mean",
        "<=",
        filter.brightness_max,
    );
    add_number_bound(
        &mut clauses,
        &mut values,
        "cf.saturation_mean",
        ">=",
        filter.saturation_min,
    );
    add_number_bound(
        &mut clauses,
        &mut values,
        "cf.saturation_mean",
        "<=",
        filter.saturation_max,
    );
    if let Some(value) = filter
        .captured_from
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        clauses.push("a.capture_time>=?".into());
        values.push(Value::Text(value.into()));
    }
    if let Some(value) = filter
        .captured_to
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        clauses.push("a.capture_time<=?".into());
        values.push(Value::Text(if value.len() == 10 {
            format!("{value}T23:59:59.999")
        } else {
            value.into()
        }));
    }
    match filter.analysis_status.as_deref() {
        Some("not_analyzed") => {
            clauses.push("a.semantic_status IN ('not_analyzed', 'queued', 'running')".into())
        }
        Some("failed") => clauses.push("a.semantic_status='failed'".into()),
        Some("completed") => clauses.push("a.semantic_status='completed'".into()),
        _ => {}
    }
    (clauses.join(" AND "), values)
}

fn effective_scalar_expression(field: &str, auto_expression: &str) -> String {
    format!(
        "COALESCE((SELECT json_extract(mco.value_json, '$')
                   FROM manual_classification_overrides mco
                   WHERE mco.asset_id=a.id AND mco.field={}), {})",
        sql_literal(field),
        auto_expression,
    )
}

fn add_palette_filter(clauses: &mut Vec<String>, values: &mut Vec<Value>, selected: &[String]) {
    if selected.is_empty() {
        return;
    }
    let expression = format!(
        "EXISTS(SELECT 1 FROM json_each(
            COALESCE(
                (SELECT mco.value_json FROM manual_classification_overrides mco
                 WHERE mco.asset_id=a.id AND mco.field='{}'),
                CASE
                    WHEN json_type(cf.dominant_colors_json, '$.coveragePalette')='array'
                         AND json_array_length(cf.dominant_colors_json, '$.coveragePalette') > 0
                    THEN json_extract(cf.dominant_colors_json, '$.coveragePalette')
                    ELSE json_array(cf.dominant_color_category)
                END
            )
        ) palette WHERE CASE
            WHEN palette.type='object' THEN json_extract(palette.value, '$.category')
            ELSE palette.value
        END IN ({})
        AND (palette.type <> 'object'
             OR COALESCE(CAST(json_extract(palette.value, '$.areaCoverage') AS REAL), 1.0) >= ?))",
        FIELD_DOMINANT_COLOR_CATEGORY,
        placeholders(selected.len()),
    );
    clauses.push(expression);
    values.extend(selected.iter().cloned().map(Value::Text));
    values.push(Value::Real(crate::imaging::MIN_MAIN_COLOR_AREA));
}

fn add_hue_range_filter(
    clauses: &mut Vec<String>,
    values: &mut Vec<Value>,
    center: Option<f64>,
    width: Option<f64>,
    strictness: Option<f64>,
) {
    let (Some(center), Some(width)) = (center, width) else {
        return;
    };
    let bins = hue_bins_for_range(center, width);
    if bins.is_empty() {
        return;
    }

    let placeholders = placeholders(bins.len());
    clauses.push(format!(
        "(SELECT COALESCE(SUM(CASE WHEN CAST(selected_hue.key AS INTEGER) IN ({placeholders})\n                                  THEN CAST(selected_hue.value AS REAL) ELSE 0 END), 0.0)\n          FROM json_each(COALESCE(cf.hue_histogram_json, '[]')) selected_hue)\n         >= ? *\n        (SELECT COALESCE(SUM(CAST(all_hue.value AS REAL)), 0.0)\n          FROM json_each(COALESCE(cf.hue_histogram_json, '[]')) all_hue)\n       AND (SELECT COALESCE(SUM(CAST(non_empty_hue.value AS REAL)), 0.0)\n          FROM json_each(COALESCE(cf.hue_histogram_json, '[]')) non_empty_hue) > 0"
    ));
    values.extend(bins.into_iter().map(|bin| Value::Integer(bin as i64)));
    values.push(Value::Real(color_hue_match_threshold(strictness)));
}

const DEFAULT_COLOR_HUE_STRICTNESS: f64 = 0.5;
const MIN_COLOR_HUE_MATCH_RATIO: f64 = 0.08;
const MAX_COLOR_HUE_MATCH_RATIO: f64 = 0.75;

fn color_hue_match_threshold(strictness: Option<f64>) -> f64 {
    let normalized = strictness
        .unwrap_or(DEFAULT_COLOR_HUE_STRICTNESS)
        .clamp(0.0, 1.0);
    MIN_COLOR_HUE_MATCH_RATIO + (MAX_COLOR_HUE_MATCH_RATIO - MIN_COLOR_HUE_MATCH_RATIO) * normalized
}

fn hue_bins_for_range(center: f64, width: f64) -> Vec<usize> {
    let center = normalize_hue(center);
    let width = width.clamp(15.0, 330.0);
    let half_width = width / 2.0;
    (0..12)
        .filter(|bin| {
            let bin_center = *bin as f64 * 30.0 + 15.0;
            circular_distance(center, bin_center) <= half_width + 15.0
        })
        .collect()
}

fn normalize_hue(value: f64) -> f64 {
    value.rem_euclid(360.0)
}

fn circular_distance(left: f64, right: f64) -> f64 {
    let difference = (left - right).abs().rem_euclid(360.0);
    difference.min(360.0 - difference)
}

fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn active_semantic_model_sql(column: &str, fallback: &str) -> String {
    format!(
        "COALESCE((SELECT {column} FROM semantic_models WHERE is_active=1 ORDER BY id DESC LIMIT 1), {})",
        sql_literal(fallback)
    )
}

fn default_semantic_model_metadata() -> ModelMetadata {
    crate::semantic::default_topic_model_metadata()
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes = file.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        hasher.update(&buffer[..bytes]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn add_string_in_filter(
    clauses: &mut Vec<String>,
    values: &mut Vec<Value>,
    column: &str,
    selected: &[String],
) {
    if !selected.is_empty() {
        clauses.push(format!("{column} IN ({})", placeholders(selected.len())));
        values.extend(selected.iter().cloned().map(Value::Text));
    }
}

fn add_number_bound(
    clauses: &mut Vec<String>,
    values: &mut Vec<Value>,
    column: &str,
    operator: &str,
    value: Option<f64>,
) {
    if let Some(value) = value.filter(|value| value.is_finite()) {
        clauses.push(format!("{column}{operator}?"));
        values.push(Value::Real(value));
    }
}

fn add_rating_threshold_filter(
    clauses: &mut Vec<String>,
    values: &mut Vec<Value>,
    column: &str,
    selected: &[i64],
) {
    if let Some(threshold) = selected.iter().copied().max() {
        clauses.push(format!("{column} >= ?"));
        values.push(Value::Integer(threshold));
    }
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn semantic_labels_for_asset(
    connection: &Connection,
    asset_id: i64,
) -> AppResult<Vec<SemanticLabelResult>> {
    let active_model_name = active_semantic_model_sql("name", SIGLIP2_MODEL_NAME);
    let active_model_version = active_semantic_model_sql("version", SIGLIP2_MODEL_VERSION);
    let active_analysis_version =
        active_semantic_model_sql("analysis_version", SIGLIP2_ANALYSIS_VERSION);
    let semantic_taxonomy_version = sql_literal(TAXONOMY_VERSION);
    let subject_model_name = sql_literal(SUBJECT_MODEL_NAME);
    let subject_model_version = sql_literal(SUBJECT_MODEL_VERSION);
    let subject_analysis_version = sql_literal(SUBJECT_ANALYSIS_VERSION);
    let subject_taxonomy_version = sql_literal(SUBJECT_TAXONOMY_VERSION);
    let mut statement = connection.prepare(&format!(
        "SELECT sl.label, sl.display_name, sl.similarity, sl.threshold, sl.model_name,
                sl.model_version, sl.analysis_version, sl.category_group, sl.taxonomy_version,
                sl.generated_at, sl.is_manual, sl.is_primary
         FROM (
             SELECT label, display_name, similarity, threshold, model_name, model_version,
                    analysis_version, category_group, taxonomy_version, generated_at,
                    is_manual, is_primary, asset_id, source_fingerprint
             FROM semantic_labels
             WHERE asset_id=?1 AND model_name={active_model_name}
               AND model_version={active_model_version}
               AND analysis_version={active_analysis_version}
               AND taxonomy_version={semantic_taxonomy_version}
             UNION ALL
             SELECT label, display_name, similarity, threshold, model_name, model_version,
                    analysis_version, 'subject' AS category_group, taxonomy_version,
                    generated_at, 0 AS is_manual, 0 AS is_primary, asset_id, source_fingerprint
             FROM subject_labels
             WHERE asset_id=?1 AND model_name={subject_model_name}
               AND model_version={subject_model_version}
               AND analysis_version={subject_analysis_version}
               AND taxonomy_version={subject_taxonomy_version}
         ) sl
         JOIN assets a ON a.id=sl.asset_id AND sl.source_fingerprint=a.fingerprint
         ORDER BY sl.is_primary DESC, sl.similarity DESC, sl.label ASC"
    ))?;
    let rows = statement.query_map(params![asset_id], |row| {
        let stored_label_id: String = row.get(0)?;
        let label_id = canonical_label_id(&stored_label_id).to_owned();
        let stored_display_name: String = row.get(1)?;
        Ok(SemanticLabelResult {
            display_name: known_display_name_for_label_id(&label_id)
                .unwrap_or(stored_display_name.as_str())
                .to_owned(),
            label_id,
            category_group: row.get(7)?,
            similarity: row.get(2)?,
            threshold: row.get(3)?,
            model_name: row.get(4)?,
            model_version: row.get(5)?,
            analysis_version: row.get(6)?,
            taxonomy_version: row.get(8)?,
            analyzed_at: row.get(9)?,
            is_manual: row.get::<_, i64>(10)? != 0,
            is_primary: row.get::<_, i64>(11)? != 0,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

fn semantic_labels_for_assets(
    connection: &Connection,
    asset_ids: &[i64],
) -> AppResult<HashMap<i64, Vec<SemanticLabelResult>>> {
    if asset_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let active_model_name = active_semantic_model_sql("name", SIGLIP2_MODEL_NAME);
    let active_model_version = active_semantic_model_sql("version", SIGLIP2_MODEL_VERSION);
    let active_analysis_version =
        active_semantic_model_sql("analysis_version", SIGLIP2_ANALYSIS_VERSION);
    let semantic_taxonomy_version = sql_literal(TAXONOMY_VERSION);
    let subject_model_name = sql_literal(SUBJECT_MODEL_NAME);
    let subject_model_version = sql_literal(SUBJECT_MODEL_VERSION);
    let subject_analysis_version = sql_literal(SUBJECT_ANALYSIS_VERSION);
    let subject_taxonomy_version = sql_literal(SUBJECT_TAXONOMY_VERSION);
    let ids = placeholders(asset_ids.len());
    let sql = format!(
        "SELECT sl.asset_id, sl.label, sl.display_name, sl.similarity, sl.threshold,
                sl.model_name, sl.model_version, sl.analysis_version, sl.category_group,
                sl.taxonomy_version, sl.generated_at, sl.is_manual, sl.is_primary
         FROM (
             SELECT asset_id, label, display_name, similarity, threshold, model_name,
                    model_version, analysis_version, category_group, taxonomy_version,
                    generated_at, is_manual, is_primary, source_fingerprint
             FROM semantic_labels
             WHERE asset_id IN ({ids}) AND model_name={active_model_name}
               AND model_version={active_model_version}
               AND analysis_version={active_analysis_version}
               AND taxonomy_version={semantic_taxonomy_version}
             UNION ALL
             SELECT asset_id, label, display_name, similarity, threshold, model_name,
                    model_version, analysis_version, 'subject' AS category_group,
                    taxonomy_version, generated_at, 0 AS is_manual, 0 AS is_primary,
                    source_fingerprint
             FROM subject_labels
             WHERE asset_id IN ({ids}) AND model_name={subject_model_name}
               AND model_version={subject_model_version}
               AND analysis_version={subject_analysis_version}
               AND taxonomy_version={subject_taxonomy_version}
         ) sl
         JOIN assets a ON a.id=sl.asset_id AND sl.source_fingerprint=a.fingerprint
         ORDER BY sl.asset_id, sl.is_primary DESC, sl.similarity DESC, sl.label ASC",
        ids = ids,
        active_model_name = active_model_name,
        active_model_version = active_model_version,
        active_analysis_version = active_analysis_version,
        semantic_taxonomy_version = semantic_taxonomy_version,
        subject_model_name = subject_model_name,
        subject_model_version = subject_model_version,
        subject_analysis_version = subject_analysis_version,
        subject_taxonomy_version = subject_taxonomy_version,
    );
    let mut query_values = Vec::with_capacity(asset_ids.len() * 2);
    query_values.extend(asset_ids.iter().copied().map(Value::Integer));
    query_values.extend(asset_ids.iter().copied().map(Value::Integer));
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(query_values.iter()), |row| {
        let asset_id: i64 = row.get(0)?;
        let stored_label_id: String = row.get(1)?;
        let label_id = canonical_label_id(&stored_label_id).to_owned();
        let stored_display_name: String = row.get(2)?;
        Ok((
            asset_id,
            SemanticLabelResult {
                display_name: known_display_name_for_label_id(&label_id)
                    .unwrap_or(stored_display_name.as_str())
                    .to_owned(),
                label_id,
                category_group: row.get(8)?,
                similarity: row.get(3)?,
                threshold: row.get(4)?,
                model_name: row.get(5)?,
                model_version: row.get(6)?,
                analysis_version: row.get(7)?,
                taxonomy_version: row.get(9)?,
                analyzed_at: row.get(10)?,
                is_manual: row.get::<_, i64>(11)? != 0,
                is_primary: row.get::<_, i64>(12)? != 0,
            },
        ))
    })?;
    let mut result = HashMap::<i64, Vec<SemanticLabelResult>>::new();
    for row in rows {
        let (asset_id, label) = row?;
        result.entry(asset_id).or_default().push(label);
    }
    Ok(result)
}

fn manual_classifications_for_assets(
    connection: &Connection,
    asset_ids: &[i64],
) -> AppResult<HashMap<i64, ManualClassification>> {
    if asset_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids = placeholders(asset_ids.len());
    let mut result = HashMap::<i64, ManualClassification>::new();
    let mut values = asset_ids
        .iter()
        .copied()
        .map(Value::Integer)
        .collect::<Vec<_>>();
    let mut statement = connection.prepare(&format!(
        "SELECT asset_id, field, value_json
         FROM manual_classification_overrides
         WHERE asset_id IN ({ids})
         ORDER BY asset_id, field",
        ids = ids,
    ))?;
    let rows = statement.query_map(params_from_iter(values.iter()), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (asset_id, field, value_json) = row?;
        let value: serde_json::Value = serde_json::from_str(&value_json)?;
        let manual = result.entry(asset_id).or_default();
        match field.as_str() {
            FIELD_PRIMARY_CATEGORY => {
                manual.primary_category = value
                    .as_str()
                    .map(|value| canonical_label_id(value).to_owned())
            }
            FIELD_TONE => manual.tone = value.as_str().map(str::to_owned),
            FIELD_SATURATION_LEVEL => manual.saturation_level = value.as_str().map(str::to_owned),
            FIELD_DOMINANT_COLOR_CATEGORY => {
                manual.dominant_color_categories = Some(
                    value
                        .as_array()
                        .ok_or_else(|| {
                            AppError::InvalidArgument(
                                "stored dominant color override is not an array".to_owned(),
                            )
                        })?
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect(),
                )
            }
            _ => {}
        }
    }

    values = asset_ids.iter().copied().map(Value::Integer).collect();
    let mut statement = connection.prepare(&format!(
        "SELECT asset_id, tag_id, state
         FROM manual_tag_overrides
         WHERE asset_id IN ({ids})
         ORDER BY asset_id, tag_id",
        ids = ids,
    ))?;
    let rows = statement.query_map(params_from_iter(values.iter()), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (asset_id, tag_id, state) = row?;
        let manual = result.entry(asset_id).or_default();
        let tag_id = canonical_label_id(&tag_id).to_owned();
        match state.as_str() {
            "add" => manual.auxiliary_tag_additions.push(tag_id),
            "remove" => manual.auxiliary_tag_removals.push(tag_id),
            _ => {}
        }
    }
    Ok(result)
}

fn classification_revisions_for_assets(
    connection: &Connection,
    asset_ids: &[i64],
) -> AppResult<HashMap<i64, i64>> {
    if asset_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids = placeholders(asset_ids.len());
    let values = asset_ids
        .iter()
        .copied()
        .map(Value::Integer)
        .collect::<Vec<_>>();
    let mut statement = connection.prepare(&format!(
        "SELECT id, classification_revision FROM assets WHERE id IN ({ids})",
        ids = ids,
    ))?;
    let rows = statement.query_map(params_from_iter(values.iter()), |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(AppError::from)
}

fn delete_subject_identity(transaction: &Transaction<'_>, asset_id: i64) -> AppResult<()> {
    transaction.execute(
        "DELETE FROM subject_labels
         WHERE asset_id=?1 AND model_name=?2 AND model_version=?3
           AND analysis_version=?4 AND taxonomy_version=?5",
        params![
            asset_id,
            SUBJECT_MODEL_NAME,
            SUBJECT_MODEL_VERSION,
            SUBJECT_ANALYSIS_VERSION,
            SUBJECT_TAXONOMY_VERSION,
        ],
    )?;
    transaction.execute(
        "DELETE FROM subject_analysis_runs
         WHERE asset_id=?1 AND model_name=?2 AND model_version=?3
           AND analysis_version=?4 AND taxonomy_version=?5",
        params![
            asset_id,
            SUBJECT_MODEL_NAME,
            SUBJECT_MODEL_VERSION,
            SUBJECT_ANALYSIS_VERSION,
            SUBJECT_TAXONOMY_VERSION,
        ],
    )?;
    Ok(())
}

fn effective_classification_for_asset(
    connection: &Connection,
    asset: &AssetListItem,
) -> AppResult<EffectiveClassification> {
    let revision: i64 = connection.query_row(
        "SELECT classification_revision FROM assets WHERE id=?1",
        [asset.id],
        |row| row.get(0),
    )?;
    let semantic_labels = semantic_labels_for_asset(connection, asset.id)?;
    let manual = manual_classification_for_asset(connection, asset.id)?;
    Ok(resolve_effective_classification(
        asset,
        revision,
        &semantic_labels,
        manual,
    ))
}

fn resolve_effective_classification(
    asset: &AssetListItem,
    revision: i64,
    semantic_labels: &[SemanticLabelResult],
    manual: ManualClassification,
) -> EffectiveClassification {
    let mut auto = AutoClassification {
        primary_category: semantic_labels
            .iter()
            .find(|label| label.is_primary)
            .map(|label| canonical_label_id(&label.label_id).to_owned()),
        auxiliary_tags: semantic_labels
            .iter()
            .filter(|label| !label.is_primary)
            .map(|label| canonical_label_id(&label.label_id).to_owned())
            .collect::<Vec<_>>(),
        tone: asset.tone_label.clone(),
        dominant_color_categories: dominant_color_categories_for_asset(asset),
        saturation_level: asset.saturation_label.clone(),
    };
    if auto.primary_category.is_none()
        && asset.semantic_status == "completed"
        && asset.semantic_error.is_none()
    {
        // A rejected/low-confidence topic is intentionally not stored as a
        // model label. It is still grouped under the photographer-facing
        // abstract bucket so the UI never exposes a redundant "未知" class.
        auto.primary_category = Some("photo_abstract".to_owned());
    }
    resolve_classification(revision, auto, manual)
}

fn manual_classification_for_asset(
    connection: &Connection,
    asset_id: i64,
) -> AppResult<ManualClassification> {
    let mut manual = ManualClassification::default();
    let mut statement = connection.prepare(
        "SELECT field, value_json
         FROM manual_classification_overrides
         WHERE asset_id=?1
         ORDER BY field",
    )?;
    let rows = statement.query_map([asset_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (field, value_json) = row?;
        let value: serde_json::Value = serde_json::from_str(&value_json)?;
        match field.as_str() {
            FIELD_PRIMARY_CATEGORY => {
                manual.primary_category = value
                    .as_str()
                    .map(|value| canonical_label_id(value).to_owned())
            }
            FIELD_TONE => manual.tone = value.as_str().map(str::to_owned),
            FIELD_SATURATION_LEVEL => manual.saturation_level = value.as_str().map(str::to_owned),
            FIELD_DOMINANT_COLOR_CATEGORY => {
                manual.dominant_color_categories = Some(
                    value
                        .as_array()
                        .ok_or_else(|| {
                            AppError::InvalidArgument(
                                "stored dominant color override is not an array".to_owned(),
                            )
                        })?
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect(),
                )
            }
            _ => {}
        }
    }

    let mut statement = connection.prepare(
        "SELECT tag_id, state
         FROM manual_tag_overrides
         WHERE asset_id=?1
         ORDER BY tag_id",
    )?;
    let rows = statement.query_map([asset_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (tag_id, state) = row?;
        let tag_id = canonical_label_id(&tag_id).to_owned();
        match state.as_str() {
            "add" => manual.auxiliary_tag_additions.push(tag_id),
            "remove" => manual.auxiliary_tag_removals.push(tag_id),
            _ => {}
        }
    }
    Ok(manual)
}

fn apply_effective_fields(asset: &mut AssetListItem) {
    if let Some(candidate) = asset.color_palette.as_ref().and_then(main_color_candidate) {
        asset.dominant_color = Some(candidate.color.clone());
        asset.dominant_color_category = Some(candidate.category.clone());
    }
    asset.tone_label = asset.classification.tone.effective.clone();
    asset.saturation_label = asset.classification.saturation_level.effective.clone();
    asset.dominant_color_category = asset
        .classification
        .dominant_color_categories
        .effective
        .as_ref()
        .and_then(|values| values.first().cloned());
}

fn dominant_color_categories_for_asset(asset: &AssetListItem) -> Vec<String> {
    let mut categories = Vec::new();
    if let Some(palette) = &asset.color_palette {
        let meaningful = palette
            .coverage_palette
            .iter()
            .filter(|candidate| candidate.area_coverage >= crate::imaging::MIN_MAIN_COLOR_AREA)
            .take(2)
            .collect::<Vec<_>>();
        let candidates = if meaningful.is_empty() {
            palette.coverage_palette.first().into_iter().collect()
        } else {
            meaningful
        };
        for candidate in candidates {
            if !categories.iter().any(|value| value == &candidate.category) {
                categories.push(candidate.category.clone());
            }
        }
    }
    if categories.is_empty()
        && let Some(category) = &asset.dominant_color_category
    {
        categories.push(category.clone());
    }
    categories
}

fn main_color_candidate(palette: &ColorPalette) -> Option<&crate::models::ColorCandidate> {
    palette
        .coverage_palette
        .iter()
        .find(|candidate| candidate.area_coverage >= crate::imaging::MIN_MAIN_COLOR_AREA)
        .or_else(|| palette.coverage_palette.first())
        .or_else(|| palette.prominent_palette.first())
}

fn parse_color_palette(value: Option<String>) -> Option<ColorPalette> {
    value.and_then(|value| serde_json::from_str::<ColorPalette>(&value).ok())
}

fn semantic_progress_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SemanticProgress> {
    Ok(SemanticProgress {
        job_id: row.get(0)?,
        library_id: row.get(1)?,
        status: row.get(2)?,
        total: row.get::<_, i64>(3)?.max(0) as u64,
        processed: row.get::<_, i64>(4)?.max(0) as u64,
        completed: row.get::<_, i64>(5)?.max(0) as u64,
        failed: row.get::<_, i64>(6)?.max(0) as u64,
        skipped: row.get::<_, i64>(7)?.max(0) as u64,
        current_asset_id: None,
        current_path: None,
        execution_backend: row.get(8)?,
        model_name: row.get(9)?,
        model_version: row.get(10)?,
        error: row.get(11)?,
    })
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn merge_duplicate_assets(
    transaction: &Transaction<'_>,
    duplicates: &HashMap<String, Vec<i64>>,
) -> AppResult<()> {
    for ids in duplicates.values().filter(|ids| ids.len() > 1) {
        let survivor = select_duplicate_survivor(
            transaction,
            "assets",
            ids,
            "CASE WHEN file_status='present' THEN 0 ELSE 1 END,
             CASE WHEN scan_status='indexed' THEN 0 ELSE 1 END,
             CASE WHEN analysis_status='completed' THEN 0 ELSE 1 END,
             COALESCE(last_seen_at, '') DESC,
             last_seen_scan DESC,
             id DESC",
        )?;
        for duplicate in ids.iter().copied().filter(|id| *id != survivor) {
            transaction.execute(
                "UPDATE assets
                 SET rating=MAX(rating, (SELECT rating FROM assets WHERE id=?2)),
                     color_label=COALESCE(color_label,
                                         (SELECT color_label FROM assets WHERE id=?2)),
                     is_favorite=MAX(is_favorite,
                                     (SELECT is_favorite FROM assets WHERE id=?2)),
                     classification_revision=MAX(
                         classification_revision,
                         (SELECT classification_revision FROM assets WHERE id=?2)
                     )
                 WHERE id=?1",
                params![survivor, duplicate],
            )?;
            merge_asset_library_assignment(transaction, survivor, duplicate)?;
            merge_manual_classification_overrides(transaction, survivor, duplicate)?;
            merge_manual_tag_overrides(transaction, survivor, duplicate)?;
            transaction.execute(
                "INSERT OR IGNORE INTO collection_assets(collection_id, asset_id, added_at)
                 SELECT collection_id, ?1, added_at
                 FROM collection_assets WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "UPDATE edit_export_plans SET asset_id=?1 WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "UPDATE face_detections SET asset_id=?1 WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO thumbnails(
                    asset_id, cache_path, spec, source_modified_at, source_size,
                    status, error_message, updated_at
                 )
                 SELECT ?1, cache_path, spec, source_modified_at, source_size,
                        status, error_message, updated_at
                 FROM thumbnails WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO tone_features(
                    asset_id, brightness_mean, brightness_median, brightness_low_percentile,
                    brightness_high_percentile, shadow_ratio, highlight_ratio, contrast,
                    dynamic_range, tone_label, exposure_label, contrast_label,
                    algorithm_version, analyzed_at
                 )
                 SELECT ?1, brightness_mean, brightness_median, brightness_low_percentile,
                        brightness_high_percentile, shadow_ratio, highlight_ratio, contrast,
                        dynamic_range, tone_label, exposure_label, contrast_label,
                        algorithm_version, analyzed_at
                 FROM tone_features WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO color_features(
                    asset_id, saturation_mean, saturation_median, chroma_mean,
                    dominant_color_rgb, dominant_color_category, dominant_colors_json,
                    hue_histogram_json, warmth_score, neutral_ratio, colorfulness,
                    monochrome_probability, dominant_color_coverage, saturation_label,
                    algorithm_version, analyzed_at
                 )
                 SELECT ?1, saturation_mean, saturation_median, chroma_mean,
                        dominant_color_rgb, dominant_color_category, dominant_colors_json,
                        hue_histogram_json, warmth_score, neutral_ratio, colorfulness,
                        monochrome_probability, dominant_color_coverage, saturation_label,
                        algorithm_version, analyzed_at
                 FROM color_features WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO semantic_labels(
                    asset_id, label, similarity, model_name, model_version,
                    analysis_version, generated_at, is_manual, is_excluded,
                    display_name, threshold, source_fingerprint, is_primary,
                    category_group, taxonomy_version
                 )
                 SELECT ?1, label, similarity, model_name, model_version,
                        analysis_version, generated_at, is_manual, is_excluded,
                        display_name, threshold, source_fingerprint, is_primary,
                        category_group, taxonomy_version
                 FROM semantic_labels WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO semantic_embeddings(
                    asset_id, model_name, model_version, analysis_version,
                    source_fingerprint, dimensions, vector_blob, generated_at
                 )
                 SELECT ?1, model_name, model_version, analysis_version,
                        source_fingerprint, dimensions, vector_blob, generated_at
                 FROM semantic_embeddings WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO subject_analysis_runs(
                    asset_id, source_fingerprint, model_name, model_version,
                    analysis_version, taxonomy_version, status, error_message, analyzed_at
                 )
                 SELECT ?1, (SELECT fingerprint FROM assets WHERE id=?1), model_name,
                        model_version, analysis_version, taxonomy_version, status,
                        error_message, analyzed_at
                 FROM subject_analysis_runs WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO subject_labels(
                    asset_id, label, display_name, similarity, threshold, model_name,
                    model_version, analysis_version, taxonomy_version, source_fingerprint,
                    generated_at
                 )
                 SELECT ?1, label, display_name, similarity, threshold, model_name,
                        model_version, analysis_version, taxonomy_version,
                        (SELECT fingerprint FROM assets WHERE id=?1), generated_at
                 FROM subject_labels WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "INSERT OR IGNORE INTO analysis_job_items(
                    job_id, asset_id, source_fingerprint, status, attempts,
                    error_message, updated_at
                 )
                 SELECT job_id, ?1, source_fingerprint, status, attempts,
                        error_message, updated_at
                 FROM analysis_job_items WHERE asset_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "DELETE FROM analysis_job_items WHERE asset_id=?1",
                [duplicate],
            )?;

            let organization_items = {
                let mut statement = transaction.prepare(
                    "SELECT id, plan_id
                     FROM organization_plan_items
                     WHERE asset_id=?1",
                )?;
                statement
                    .query_map([duplicate], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?
            };
            for (duplicate_item_id, plan_id) in organization_items {
                let survivor_item_id: Option<i64> = transaction
                    .query_row(
                        "SELECT id FROM organization_plan_items
                         WHERE plan_id=?1 AND asset_id=?2",
                        params![plan_id, survivor],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(survivor_item_id) = survivor_item_id {
                    transaction.execute(
                        "UPDATE organization_plan_issues SET item_id=?1 WHERE item_id=?2",
                        params![survivor_item_id, duplicate_item_id],
                    )?;
                    transaction.execute(
                        "DELETE FROM organization_plan_items WHERE id=?1",
                        [duplicate_item_id],
                    )?;
                } else {
                    transaction.execute(
                        "UPDATE organization_plan_items SET asset_id=?1 WHERE id=?2",
                        params![survivor, duplicate_item_id],
                    )?;
                }
            }

            transaction.execute("DELETE FROM assets WHERE id=?1", [duplicate])?;
        }
    }
    Ok(())
}

fn merge_duplicate_libraries(
    transaction: &Transaction<'_>,
    duplicates: &HashMap<String, Vec<i64>>,
) -> AppResult<()> {
    for ids in duplicates.values().filter(|ids| ids.len() > 1) {
        let survivor = select_duplicate_survivor(
            transaction,
            "libraries",
            ids,
            "CASE status
                 WHEN 'ready' THEN 0
                 WHEN 'scanning' THEN 1
                 ELSE 2
             END,
             COALESCE(last_scan_at, '') DESC,
             scan_generation DESC,
             id DESC",
        )?;
        for duplicate in ids.iter().copied().filter(|id| *id != survivor) {
            merge_library_assignments(transaction, survivor, duplicate)?;
            transaction.execute(
                "UPDATE assets SET library_id=?1 WHERE library_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "UPDATE analysis_jobs SET library_id=?1 WHERE library_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "UPDATE file_operation_jobs SET library_id=?1 WHERE library_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "UPDATE organization_plans SET library_id=?1 WHERE library_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute(
                "UPDATE saved_views SET library_id=?1 WHERE library_id=?2",
                params![survivor, duplicate],
            )?;
            let duplicate_parent: Option<i64> = transaction
                .query_row(
                    "SELECT parent_library_id FROM libraries WHERE id=?1",
                    [duplicate],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .optional()?
                .flatten();
            let survivor_parent: Option<i64> = transaction
                .query_row(
                    "SELECT parent_library_id FROM libraries WHERE id=?1",
                    [survivor],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .optional()?
                .flatten();
            if survivor_parent.is_none()
                && duplicate_parent.is_some_and(|parent| parent != survivor && parent != duplicate)
            {
                transaction.execute(
                    "UPDATE libraries SET parent_library_id=?2 WHERE id=?1",
                    params![survivor, duplicate_parent],
                )?;
            }
            let duplicate_parent_relation: String = transaction.query_row(
                "SELECT parent_relation FROM libraries WHERE id=?1",
                [duplicate],
                |row| row.get(0),
            )?;
            if duplicate_parent_relation == "manual" {
                transaction.execute(
                    "UPDATE libraries SET parent_relation='manual' WHERE id=?1",
                    [survivor],
                )?;
            }
            transaction.execute(
                "UPDATE libraries SET parent_library_id=?1 WHERE parent_library_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute("DELETE FROM libraries WHERE id=?1", [duplicate])?;
        }
    }
    Ok(())
}

fn select_duplicate_survivor(
    transaction: &Transaction<'_>,
    table: &str,
    ids: &[i64],
    order_by: &str,
) -> AppResult<i64> {
    let sql = format!(
        "SELECT id FROM {table} WHERE id IN ({}) ORDER BY {order_by} LIMIT 1",
        placeholders(ids.len())
    );
    transaction
        .query_row(&sql, params_from_iter(ids.iter()), |row| row.get(0))
        .map_err(AppError::from)
}

fn merge_asset_library_assignment(
    transaction: &Transaction<'_>,
    survivor: i64,
    duplicate: i64,
) -> AppResult<()> {
    let duplicate_assignment: Option<(i64, String)> = transaction
        .query_row(
            "SELECT library_id, assigned_at
             FROM asset_library_assignments WHERE asset_id=?1",
            [duplicate],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((library_id, assigned_at)) = duplicate_assignment else {
        return Ok(());
    };

    let survivor_assigned_at: Option<String> = transaction
        .query_row(
            "SELECT assigned_at FROM asset_library_assignments WHERE asset_id=?1",
            [survivor],
            |row| row.get(0),
        )
        .optional()?;
    match survivor_assigned_at {
        None => {
            transaction.execute(
                "INSERT INTO asset_library_assignments(asset_id, library_id, assigned_at)
                 VALUES(?1, ?2, ?3)",
                params![survivor, library_id, assigned_at],
            )?;
        }
        Some(current) if assigned_at > current => {
            transaction.execute(
                "UPDATE asset_library_assignments
                 SET library_id=?2, assigned_at=?3 WHERE asset_id=?1",
                params![survivor, library_id, assigned_at],
            )?;
        }
        Some(_) => {}
    }
    Ok(())
}

fn merge_manual_classification_overrides(
    transaction: &Transaction<'_>,
    survivor: i64,
    duplicate: i64,
) -> AppResult<()> {
    let rows = {
        let mut statement = transaction.prepare(
            "SELECT field, value_json, updated_at
             FROM manual_classification_overrides WHERE asset_id=?1",
        )?;
        statement
            .query_map([duplicate], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (field, value_json, updated_at) in rows {
        let current: Option<String> = transaction
            .query_row(
                "SELECT updated_at FROM manual_classification_overrides
                 WHERE asset_id=?1 AND field=?2",
                params![survivor, field],
                |row| row.get(0),
            )
            .optional()?;
        match current {
            None => {
                transaction.execute(
                    "INSERT INTO manual_classification_overrides(
                        asset_id, field, value_json, updated_at
                     ) VALUES(?1, ?2, ?3, ?4)",
                    params![survivor, field, value_json, updated_at],
                )?;
            }
            Some(current) if updated_at > current => {
                transaction.execute(
                    "UPDATE manual_classification_overrides
                     SET value_json=?3, updated_at=?4
                     WHERE asset_id=?1 AND field=?2",
                    params![survivor, field, value_json, updated_at],
                )?;
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn merge_manual_tag_overrides(
    transaction: &Transaction<'_>,
    survivor: i64,
    duplicate: i64,
) -> AppResult<()> {
    let rows = {
        let mut statement = transaction.prepare(
            "SELECT tag_id, state, updated_at
             FROM manual_tag_overrides WHERE asset_id=?1",
        )?;
        statement
            .query_map([duplicate], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (tag_id, state, updated_at) in rows {
        let current: Option<String> = transaction
            .query_row(
                "SELECT updated_at FROM manual_tag_overrides
                 WHERE asset_id=?1 AND tag_id=?2",
                params![survivor, tag_id],
                |row| row.get(0),
            )
            .optional()?;
        match current {
            None => {
                transaction.execute(
                    "INSERT INTO manual_tag_overrides(asset_id, tag_id, state, updated_at)
                     VALUES(?1, ?2, ?3, ?4)",
                    params![survivor, tag_id, state, updated_at],
                )?;
            }
            Some(current) if updated_at > current => {
                transaction.execute(
                    "UPDATE manual_tag_overrides
                     SET state=?3, updated_at=?4
                     WHERE asset_id=?1 AND tag_id=?2",
                    params![survivor, tag_id, state, updated_at],
                )?;
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn merge_library_assignments(
    transaction: &Transaction<'_>,
    survivor: i64,
    duplicate: i64,
) -> AppResult<()> {
    let rows = {
        let mut statement = transaction.prepare(
            "SELECT asset_id, assigned_at
             FROM asset_library_assignments WHERE library_id=?1",
        )?;
        statement
            .query_map([duplicate], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (asset_id, assigned_at) in rows {
        let current: Option<(i64, String)> = transaction
            .query_row(
                "SELECT library_id, assigned_at
                 FROM asset_library_assignments WHERE asset_id=?1",
                [asset_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match current {
            None => {
                transaction.execute(
                    "INSERT INTO asset_library_assignments(asset_id, library_id, assigned_at)
                     VALUES(?1, ?2, ?3)",
                    params![asset_id, survivor, assigned_at],
                )?;
            }
            Some((current_library, current))
                if current_library == duplicate || assigned_at > current =>
            {
                transaction.execute(
                    "UPDATE asset_library_assignments
                     SET library_id=?2, assigned_at=?3 WHERE asset_id=?1",
                    params![asset_id, survivor, assigned_at],
                )?;
            }
            Some(_) => {}
        }
    }
    transaction.execute(
        "DELETE FROM asset_library_assignments WHERE library_id=?1",
        [duplicate],
    )?;
    Ok(())
}

fn as_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

fn library_name(source_path: &Path, fallback: &str) -> String {
    source_path
        .file_name()
        .or_else(|| Path::new(fallback).file_name())
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "图库".into())
}

fn path_depth(value: &str) -> usize {
    value
        .split('/')
        .filter(|component| !component.is_empty())
        .count()
}

fn migrate_unified_source_collection(transaction: &Transaction<'_>) -> AppResult<()> {
    normalize_source_hierarchy_for_migration(transaction)?;

    let default_collection_id = ensure_default_collection(transaction)?;
    let timestamp = now();
    transaction.execute(
        "INSERT OR IGNORE INTO collection_assets(collection_id, asset_id, added_at)
         SELECT ?1, id, ?2 FROM assets WHERE is_favorite=1",
        params![default_collection_id, timestamp],
    )?;
    transaction.execute(
        "UPDATE assets
         SET is_favorite=CASE WHEN EXISTS(
             SELECT 1 FROM collection_assets ca
             WHERE ca.collection_id=?1 AND ca.asset_id=assets.id
         ) THEN 1 ELSE 0 END",
        [default_collection_id],
    )?;

    migrate_legacy_asset_library_assignments(transaction)?;
    Ok(())
}

fn normalize_source_hierarchy_for_migration(transaction: &Transaction<'_>) -> AppResult<()> {
    let rows = {
        let mut statement = transaction.prepare(
            "SELECT id, root_path, source_path
             FROM libraries ORDER BY id",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };

    let mut libraries = Vec::with_capacity(rows.len());
    for (library_id, root_path, source_path) in rows {
        let source_path = if source_path.trim().is_empty() {
            root_path
        } else {
            source_path
        };
        let source_identity_key = if source_path.trim().is_empty() {
            String::new()
        } else {
            identity_key(Path::new(&source_path))
        };
        transaction.execute(
            "UPDATE libraries
             SET source_path=?2, source_identity_key=?3,
                 parent_library_id=NULL, parent_relation='source'
             WHERE id=?1",
            params![library_id, source_path, source_identity_key],
        )?;
        if !source_identity_key.is_empty() {
            libraries.push((library_id, source_identity_key));
        }
    }

    for (library_id, source_identity_key) in &libraries {
        let parent_library_id = libraries
            .iter()
            .filter(|(candidate_id, candidate_key)| {
                candidate_id != library_id
                    && is_same_or_descendant(candidate_key, source_identity_key)
            })
            .max_by_key(|(candidate_id, candidate_key)| (path_depth(candidate_key), -*candidate_id))
            .map(|(candidate_id, _)| *candidate_id);
        transaction.execute(
            "UPDATE libraries SET parent_library_id=?2, parent_relation='source'
             WHERE id=?1",
            params![library_id, parent_library_id],
        )?;
    }
    Ok(())
}

fn root_collection_name_exists(transaction: &Transaction<'_>, name: &str) -> AppResult<bool> {
    Ok(transaction.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM collections
             WHERE parent_collection_id IS NULL AND name=?1 COLLATE NOCASE
         )",
        [name],
        |row| row.get(0),
    )?)
}

fn available_root_collection_name(
    transaction: &Transaction<'_>,
    base_name: &str,
    conflict_suffix: &str,
) -> AppResult<String> {
    let base_name = if base_name.trim().is_empty() {
        "收藏夹"
    } else {
        base_name.trim()
    };
    for attempt in 0..10_000_i64 {
        let candidate = match attempt {
            0 => base_name.to_owned(),
            1 => format!("{base_name}{conflict_suffix}"),
            value => format!("{base_name}{conflict_suffix} {value}"),
        };
        if !root_collection_name_exists(transaction, &candidate)? {
            return Ok(candidate);
        }
    }
    Err(AppError::InvalidArgument(format!(
        "unable to allocate a unique collection name for {base_name}"
    )))
}

fn ensure_default_collection(transaction: &Transaction<'_>) -> AppResult<i64> {
    if let Some(collection_id) = transaction
        .query_row(
            "SELECT id FROM collections WHERE system_key='default_favorites'",
            [],
            |row| row.get(0),
        )
        .optional()?
    {
        return Ok(collection_id);
    }

    let name = available_root_collection_name(transaction, "默认收藏", "（系统）")?;
    let timestamp = now();
    transaction.execute(
        "INSERT INTO collections(
             name, description, created_at, updated_at,
             parent_collection_id, collection_kind, system_key, display_order
         ) VALUES(?1, '', ?2, ?2, NULL, 'system_favorites',
                  'default_favorites', -1)",
        params![name, timestamp],
    )?;
    Ok(transaction.last_insert_rowid())
}

fn migrate_legacy_asset_library_assignments(transaction: &Transaction<'_>) -> AppResult<()> {
    let assignments = {
        let mut statement = transaction.prepare(
            "SELECT assignment.asset_id, assignment.library_id, assignment.assigned_at,
                    library.name, library.source_path, library.root_path
             FROM asset_library_assignments assignment
             LEFT JOIN libraries library ON library.id=assignment.library_id
             ORDER BY assignment.library_id, assignment.asset_id",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };

    let mut collection_by_library = HashMap::<i64, i64>::new();
    for (asset_id, library_id, assigned_at, library_name, source_path, root_path) in assignments {
        let Some(library_name) = library_name
            .or(source_path)
            .or(root_path)
            .filter(|value| !value.trim().is_empty())
        else {
            return Err(AppError::InvalidArgument(format!(
                "legacy asset assignment {asset_id} points to missing library {library_id}"
            )));
        };
        let collection_id = if let Some(collection_id) = collection_by_library.get(&library_id) {
            *collection_id
        } else {
            let name = available_root_collection_name(transaction, &library_name, "（迁移）")?;
            let timestamp = now();
            transaction.execute(
                "INSERT INTO collections(
                     name, description, created_at, updated_at,
                     parent_collection_id, collection_kind, system_key, display_order
                 ) VALUES(?1, '由旧图库归属迁移', ?2, ?2, NULL, 'manual', NULL,
                          COALESCE((SELECT MAX(display_order)+1 FROM collections), 0))",
                params![name, timestamp],
            )?;
            let collection_id = transaction.last_insert_rowid();
            collection_by_library.insert(library_id, collection_id);
            collection_id
        };
        let added_at = if assigned_at.trim().is_empty() {
            now()
        } else {
            assigned_at
        };
        transaction.execute(
            "INSERT OR IGNORE INTO collection_assets(collection_id, asset_id, added_at)
             VALUES(?1, ?2, ?3)",
            params![collection_id, asset_id, added_at],
        )?;
    }
    Ok(())
}

fn relative_path_for_owner(owner_source_path: &Path, absolute_path: &Path) -> String {
    if let Ok(relative) = absolute_path.strip_prefix(owner_source_path) {
        return relative.to_string_lossy().into_owned();
    }

    let owner_key = identity_key(owner_source_path);
    let asset_key = identity_key(absolute_path);
    if is_same_or_descendant(&owner_key, &asset_key) {
        let owner_component_count = owner_source_path.components().count();
        let mut relative = PathBuf::new();
        for component in absolute_path.components().skip(owner_component_count) {
            relative.push(component.as_os_str());
        }
        return relative.to_string_lossy().into_owned();
    }

    absolute_path.to_string_lossy().into_owned()
}

fn rebuild_library_hierarchy(transaction: &Transaction<'_>) -> AppResult<()> {
    let libraries = {
        let mut statement = transaction.prepare(
            "SELECT id, source_identity_key
             FROM libraries
             WHERE source_identity_key <> ''
             ORDER BY id",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };

    let source_library_ids = {
        let mut statement = transaction.prepare(
            "SELECT id FROM libraries
             WHERE source_identity_key <> '' AND parent_relation='source'
             ORDER BY id",
        )?;
        statement
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };

    for library_id in source_library_ids {
        let Some((_, source_identity_key)) = libraries
            .iter()
            .find(|(candidate_id, _)| *candidate_id == library_id)
        else {
            continue;
        };
        let parent_library_id = libraries
            .iter()
            .filter(|(candidate_id, candidate_key)| {
                *candidate_id != library_id
                    && is_same_or_descendant(candidate_key, source_identity_key)
            })
            .max_by_key(|(_, candidate_key)| path_depth(candidate_key))
            .map(|(candidate_id, _)| *candidate_id);
        transaction.execute(
            "UPDATE libraries SET parent_library_id=?2 WHERE id=?1",
            params![library_id, parent_library_id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_range_bins_wrap_around_the_red_boundary() {
        assert_eq!(hue_bins_for_range(0.0, 15.0), vec![0, 11]);
        assert_eq!(hue_bins_for_range(45.0, 30.0), vec![0, 1, 2]);
        assert_eq!(hue_bins_for_range(-30.0, 15.0), vec![10, 11]);
    }

    #[test]
    fn hue_match_threshold_scales_with_strictness() {
        assert!((color_hue_match_threshold(None) - 0.415).abs() < 1e-9);
        assert!((color_hue_match_threshold(Some(0.0)) - 0.08).abs() < 1e-9);
        assert!((color_hue_match_threshold(Some(1.0)) - 0.75).abs() < 1e-9);
        assert!((color_hue_match_threshold(Some(2.0)) - 0.75).abs() < 1e-9);
    }

    #[test]
    fn initialization_and_migration_are_idempotent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("first initialization");
        repository.initialize().expect("second initialization");
        assert_eq!(repository.migration_version().expect("version"), 16);
        let connection = repository.open().expect("connection");
        for table in [
            "organization_plans",
            "organization_plan_items",
            "organization_plan_issues",
            "asset_library_assignments",
            "manual_classification_overrides",
            "manual_tag_overrides",
            "semantic_evidence",
            "subject_analysis_runs",
            "subject_labels",
        ] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("table lookup");
            assert_eq!(exists, 1, "missing table {table}");
        }
        for column in ["rating", "color_label"] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('assets') WHERE name=?1",
                    [column],
                    |row| row.get(0),
                )
                .expect("column lookup");
            assert_eq!(exists, 1, "missing asset column {column}");
        }
    }

    fn initialize_legacy_schema_at_version_15(repository: &Repository) {
        let mut connection = repository.open().expect("legacy database");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations(
                     version INTEGER PRIMARY KEY,
                     applied_at TEXT NOT NULL
                 )",
            )
            .expect("schema migrations");
        let migrations = [
            (1_i64, INITIAL_MIGRATION),
            (2_i64, SEMANTIC_MIGRATION),
            (3_i64, ORGANIZATION_MIGRATION),
            (4_i64, LIBRARY_UX_MIGRATION),
            (5_i64, LIBRARY_SOURCE_MIGRATION),
            (6_i64, ASSET_IDENTITY_MIGRATION),
            (7_i64, MANUAL_LIBRARY_HIERARCHY_MIGRATION),
            (8_i64, ASSET_LIBRARY_ASSIGNMENT_MIGRATION),
            (9_i64, MANUAL_CLASSIFICATION_MIGRATION),
            (10_i64, MANUAL_ASSET_MARKS_MIGRATION),
            (11_i64, PHOTO_WORKFLOW_MVP_MIGRATION),
            (12_i64, SEMANTIC_TAXONOMY_MIGRATION),
            (13_i64, PLACES365_EVIDENCE_MIGRATION),
            (14_i64, SUBJECT_ANALYSIS_MIGRATION),
            (15_i64, IMPORT_SUBFOLDER_SCOPE_MIGRATION),
        ];
        for (version, sql) in migrations {
            let transaction = connection
                .transaction()
                .expect("legacy migration transaction");
            transaction
                .execute_batch(sql)
                .expect("legacy migration SQL");
            transaction
                .execute(
                    "INSERT INTO schema_migrations(version, applied_at)
                     VALUES(?1, ?2)",
                    params![version, now()],
                )
                .expect("legacy migration marker");
            transaction.commit().expect("legacy migration commit");
        }
    }

    #[test]
    fn unified_source_collection_migration_preserves_ownership_and_memberships() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("legacy.sqlite3"));
        initialize_legacy_schema_at_version_15(&repository);

        let connection = repository.open().expect("legacy fixture connection");
        connection
            .execute(
                "INSERT INTO libraries(
                     root_path, created_at, name, source_path, source_identity_key,
                     parent_library_id, parent_relation
                 ) VALUES('C:\\Photos', ?1, 'Photos', 'C:\\Photos', 'legacy-parent', NULL, 'source')",
                [now()],
            )
            .expect("parent source");
        let parent_library_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO libraries(
                     root_path, created_at, name, source_path, source_identity_key,
                     parent_library_id, parent_relation
                 ) VALUES('C:\\Photos\\Travel', ?1, 'Travel', 'C:\\Photos\\Travel',
                          'legacy-child', ?2, 'manual')",
                params![now(), parent_library_id],
            )
            .expect("manual child source");
        let child_library_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO libraries(
                     root_path, created_at, name, source_path, source_identity_key,
                     parent_library_id, parent_relation
                 ) VALUES('D:\\Old Target', ?1, '精选', 'D:\\Old Target',
                          'legacy-target', NULL, 'source')",
                [now()],
            )
            .expect("assignment target source");
        let target_library_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO collections(name, description, created_at, updated_at)
                 VALUES('精选', 'legacy collection', ?1, ?1)",
                [now()],
            )
            .expect("legacy collection");
        let legacy_collection_id = connection.last_insert_rowid();

        for (asset_id, library_id, file_name, favorite) in [
            (1_i64, parent_library_id, "parent.jpg", 1_i64),
            (2_i64, child_library_id, "child.jpg", 0_i64),
        ] {
            let absolute_path = if asset_id == 1 {
                "C:\\Photos\\parent.jpg"
            } else {
                "C:\\Photos\\Travel\\child.jpg"
            };
            connection
                .execute(
                    "INSERT INTO assets(
                         id, library_id, absolute_path, relative_path, file_name, extension,
                         file_size, modified_at, fingerprint, file_status, scan_status,
                         analysis_status, first_seen_at, last_seen_at, last_seen_scan,
                         asset_identity_key, is_favorite
                     ) VALUES(?1, ?2, ?3, ?4, ?5, 'jpg', 10, 1, ?6, 'present',
                              'indexed', 'completed', ?7, ?7, 1, ?8, ?9)",
                    params![
                        asset_id,
                        library_id,
                        absolute_path,
                        file_name,
                        file_name,
                        format!("legacy-{asset_id}"),
                        now(),
                        format!("legacy-identity-{asset_id}"),
                        favorite,
                    ],
                )
                .expect("legacy asset");
        }
        connection
            .execute(
                "INSERT INTO collection_assets(collection_id, asset_id, added_at)
                 VALUES(?1, 1, ?2)",
                params![legacy_collection_id, now()],
            )
            .expect("legacy collection member");
        connection
            .execute(
                "INSERT INTO asset_library_assignments(asset_id, library_id, assigned_at)
                 VALUES(1, ?1, ?2)",
                params![target_library_id, now()],
            )
            .expect("legacy assignment");
        drop(connection);

        repository.initialize().expect("apply unified migration");

        let connection = repository.open().expect("migrated connection");
        let default_collection: (i64, String, String) = connection
            .query_row(
                "SELECT id, collection_kind, system_key
                 FROM collections WHERE system_key='default_favorites'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("default collection");
        assert_eq!(default_collection.1, "system_favorites");
        assert_eq!(default_collection.2, "default_favorites");
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM collection_assets
                     WHERE collection_id=?1 AND asset_id=1",
                    [default_collection.0],
                    |row| row.get::<_, i64>(0),
                )
                .expect("default member"),
            1
        );
        assert_eq!(
            connection
                .query_row("SELECT is_favorite FROM assets WHERE id=1", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("favorite mirror"),
            1
        );
        let migrated_collection: (i64, String) = connection
            .query_row(
                "SELECT id, name FROM collections
                 WHERE description='由旧图库归属迁移'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("assignment collection");
        assert_eq!(migrated_collection.1, "精选（迁移）");
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM collection_assets
                     WHERE collection_id=?1 AND asset_id=1",
                    [migrated_collection.0],
                    |row| row.get::<_, i64>(0),
                )
                .expect("assignment member"),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM asset_library_assignments
                     WHERE asset_id=1 AND library_id=?1",
                    [target_library_id],
                    |row| row.get::<_, i64>(0),
                )
                .expect("legacy assignment retained"),
            1
        );
        let child_parent: (Option<i64>, String) = connection
            .query_row(
                "SELECT parent_library_id, parent_relation
                 FROM libraries WHERE id=?1",
                [child_library_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("source hierarchy");
        assert_eq!(child_parent, (Some(parent_library_id), "source".into()));
        drop(connection);

        let target_assets = repository
            .list_assets(
                target_library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &AssetFilter::default(),
            )
            .expect("target source query");
        assert_eq!(target_assets.total, 0);
        let source_assets = repository
            .list_assets(
                parent_library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &AssetFilter::default(),
            )
            .expect("source query");
        assert_eq!(source_assets.total, 2);

        repository.initialize().expect("repeat unified migration");
        let connection = repository.open().expect("reopened migrated connection");
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM collections
                     WHERE system_key='default_favorites'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("one default collection"),
            1
        );
    }

    #[test]
    fn active_semantic_model_key_is_persisted_for_startup_restore() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let connection = repository.open().expect("open database");
        connection
            .execute(
                "INSERT INTO semantic_models(
                    name, version, analysis_version, license, source_url,
                    model_sha256, tokenizer_sha256, model_path, tokenizer_path,
                    execution_backend, installed_at, is_active
                 ) VALUES(?1, ?2, ?3, 'Apache-2.0', 'https://example.test/model',
                          'model-sha', 'tokenizer-sha', 'model.onnx', 'tokenizer.json',
                          'cpu', '2026-08-17T00:00:00Z', 1)",
                params![
                    SIGLIP2_MODEL_NAME,
                    SIGLIP2_MODEL_VERSION,
                    SIGLIP2_ANALYSIS_VERSION
                ],
            )
            .expect("insert active semantic model");
        drop(connection);

        assert_eq!(
            repository
                .active_semantic_model_key()
                .expect("read active semantic model"),
            Some((
                SIGLIP2_MODEL_NAME.into(),
                SIGLIP2_MODEL_VERSION.into(),
                SIGLIP2_ANALYSIS_VERSION.into()
            ))
        );
    }

    #[test]
    fn semantic_jobs_only_schedule_current_thumbnail_cache() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (library_id, _) = repository
            .begin_scan("C:\\fixtures\\semantic-thumbnails", "semantic-thumbnails")
            .expect("begin scan");
        let source_path = "C:\\fixtures\\semantic-thumbnails\\source.jpg";
        let connection = repository.open().expect("open database");
        connection
            .execute(
                "INSERT INTO assets(
                    library_id, asset_identity_key, absolute_path, relative_path, file_name,
                    extension, file_size, modified_at, fingerprint, file_status, scan_status,
                    analysis_status, first_seen_at, last_seen_at
                 ) VALUES(?1, 'c:/fixtures/semantic-thumbnails/source.jpg', ?2,
                          'source.jpg', 'source.jpg', 'jpg', 42, 123, 'fingerprint',
                          'present', 'indexed', 'completed', '2026-08-10T00:00:00Z',
                          '2026-08-10T00:00:00Z')",
                params![library_id, source_path],
            )
            .expect("asset without thumbnail");
        let asset_id = connection.last_insert_rowid();

        let no_thumbnail = repository
            .create_semantic_job("semantic-without-thumbnail", library_id, false, None)
            .expect("create job without thumbnail");
        assert!(no_thumbnail.is_empty());
        connection
            .execute(
                "UPDATE analysis_jobs SET status='completed' WHERE id='semantic-without-thumbnail'",
                [],
            )
            .expect("complete empty job");

        let thumbnail_path = temp.path().join("grid-640-v1.jpg");
        std::fs::write(&thumbnail_path, b"thumbnail").expect("write thumbnail fixture");
        connection
            .execute(
                "INSERT INTO thumbnails(
                    asset_id, cache_path, spec, source_modified_at, source_size, status, updated_at
                 ) VALUES(?1, ?2, ?3, 123, 42, 'ready', '2026-08-10T00:00:00Z')",
                params![
                    asset_id,
                    thumbnail_path.to_string_lossy().into_owned(),
                    crate::imaging::THUMBNAIL_SPEC,
                ],
            )
            .expect("thumbnail row");

        let with_thumbnail = repository
            .create_semantic_job("semantic-with-thumbnail", library_id, false, None)
            .expect("create job with thumbnail");
        assert_eq!(with_thumbnail.len(), 1);
        assert_eq!(with_thumbnail[0].absolute_path, PathBuf::from(source_path));
        assert_eq!(with_thumbnail[0].analysis_path, thumbnail_path);

        connection
            .execute("UPDATE assets SET modified_at=124 WHERE id=?1", [asset_id])
            .expect("stale source metadata");
        drop(connection);
        let recovered = repository
            .pending_semantic_candidates("semantic-with-thumbnail")
            .expect("recover pending job");
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].absolute_path, PathBuf::from(source_path));
        assert!(recovered[0].analysis_path.as_os_str().is_empty());
    }

    #[test]
    fn subject_labels_are_non_primary_and_filterable() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (library_id, _) = repository
            .begin_scan("C:\\fixtures\\subject-labels", "subject-labels")
            .expect("begin scan");
        let source_path = "C:\\fixtures\\subject-labels\\portrait.jpg";
        let connection = repository.open().expect("open database");
        connection
            .execute(
                "INSERT INTO assets(
                    library_id, asset_identity_key, absolute_path, relative_path, file_name,
                    extension, file_size, modified_at, fingerprint, file_status, scan_status,
                    analysis_status, first_seen_at, last_seen_at
                 ) VALUES(?1, 'c:/fixtures/subject-labels/portrait.jpg', ?2,
                          'portrait.jpg', 'portrait.jpg', 'jpg', 42, 123, 'subject-fingerprint',
                          'present', 'indexed', 'completed', '2026-08-10T00:00:00Z',
                          '2026-08-10T00:00:00Z')",
                params![library_id, source_path],
            )
            .expect("asset");
        let asset_id = connection.last_insert_rowid();
        let thumbnail_path = temp.path().join("grid-640-v1.jpg");
        std::fs::write(&thumbnail_path, b"thumbnail").expect("write thumbnail fixture");
        connection
            .execute(
                "INSERT INTO thumbnails(
                    asset_id, cache_path, spec, source_modified_at, source_size, status, updated_at
                 ) VALUES(?1, ?2, ?3, 123, 42, 'ready', '2026-08-10T00:00:00Z')",
                params![
                    asset_id,
                    thumbnail_path.to_string_lossy().into_owned(),
                    crate::imaging::THUMBNAIL_SPEC,
                ],
            )
            .expect("thumbnail");
        drop(connection);

        let candidates = repository
            .create_semantic_job("subject-labels-job", library_id, false, None)
            .expect("create semantic job");
        let output = SubjectAnalysisOutput {
            predictions: vec![crate::subject::SubjectPrediction {
                label_id: "single_person".into(),
                display_name: "单人".into(),
                category_group: "subject".into(),
                similarity: 0.91,
                threshold: 0.45,
            }],
        };
        repository
            .save_subject_result(&candidates[0], &output)
            .expect("save subject result");

        let detail = repository.get_asset_detail(asset_id).expect("detail");
        assert!(
            detail
                .asset
                .classification
                .primary_category
                .effective
                .is_none()
        );
        let person = detail
            .asset
            .semantic_labels
            .iter()
            .find(|label| label.label_id == "single_person")
            .expect("person label");
        assert_eq!(person.display_name, "单人");
        assert_eq!(person.category_group, "subject");
        assert!(!person.is_primary);
        assert_eq!(
            detail.asset.classification.auxiliary_tags.effective,
            vec!["single_person"]
        );

        let filtered = repository
            .list_assets(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &AssetFilter {
                    auxiliary_tags: vec!["single_person".into()],
                    ..AssetFilter::default()
                },
            )
            .expect("filter subject label");
        assert_eq!(filtered.total, 1);
    }

    #[test]
    fn identity_backfill_merges_asset_state_and_all_user_relations() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (first_library, _) = repository
            .begin_scan("C:\\fixtures\\merge-first", "merge-first")
            .expect("first library");
        repository
            .cancel_scan("merge-first", first_library)
            .expect("cancel first library");
        let (second_library, _) = repository
            .begin_scan("D:\\fixtures\\merge-second", "merge-second")
            .expect("second library");
        repository
            .cancel_scan("merge-second", second_library)
            .expect("cancel second library");

        let connection = repository.open().expect("open database");
        connection
            .execute("DROP INDEX idx_assets_identity", [])
            .expect("allow duplicate identities for migration fixture");
        let identity = "c:/fixtures/merged/photo.jpg";
        connection
            .execute(
                "INSERT INTO assets(
                    library_id, asset_identity_key, absolute_path, relative_path, file_name,
                    extension, file_size, modified_at, fingerprint, file_status, scan_status,
                    analysis_status, first_seen_at, last_seen_at, last_seen_scan, rating,
                    color_label, is_favorite
                 ) VALUES(?1, ?2, ?3, 'photo.jpg', 'photo.jpg', 'jpg', 100, 1, 'same',
                          'missing', 'indexed', 'completed', '2026-08-01T00:00:00Z',
                          '2026-08-01T00:00:00Z', 1, 5, 'purple', 0)",
                params![first_library, identity, "C:\\fixtures\\merged\\photo.jpg"],
            )
            .expect("first duplicate asset");
        let first_asset = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO assets(
                    library_id, asset_identity_key, absolute_path, relative_path, file_name,
                    extension, file_size, modified_at, fingerprint, file_status, scan_status,
                    analysis_status, first_seen_at, last_seen_at, last_seen_scan, rating,
                    color_label, is_favorite
                 ) VALUES(?1, ?2, ?3, 'photo.jpg', 'photo.jpg', 'jpg', 100, 1, 'same',
                          'present', 'indexed', 'completed', '2026-08-02T00:00:00Z',
                          '2026-08-02T00:00:00Z', 2, 1, NULL, 1)",
                params![second_library, identity, "D:\\fixtures\\merged\\photo.jpg"],
            )
            .expect("second duplicate asset");
        let second_asset = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO asset_library_assignments(asset_id, library_id, assigned_at)
                 VALUES(?1, ?2, '2026-08-03T00:00:00Z')",
                params![first_asset, second_library],
            )
            .expect("asset assignment");
        connection
            .execute(
                "INSERT INTO manual_classification_overrides(
                    asset_id, field, value_json, updated_at
                 ) VALUES(?1, 'primary_category', 'product', '2026-08-03T00:00:00Z')",
                [first_asset],
            )
            .expect("classification override");
        connection
            .execute(
                "INSERT INTO manual_tag_overrides(asset_id, tag_id, state, updated_at)
                 VALUES(?1, 'favorite', 'add', '2026-08-03T00:00:00Z')",
                [first_asset],
            )
            .expect("tag override");
        connection
            .execute(
                "INSERT INTO collections(name, description, created_at, updated_at)
                 VALUES('Merged collection', '', '2026-08-03T00:00:00Z', '2026-08-03T00:00:00Z')",
                [],
            )
            .expect("collection");
        let collection_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO collection_assets(collection_id, asset_id, added_at)
                 VALUES(?1, ?2, '2026-08-03T00:00:00Z')",
                params![collection_id, first_asset],
            )
            .expect("collection membership");
        connection
            .execute(
                "INSERT INTO edit_export_plans(
                    id, asset_id, source_fingerprint, target_path, recipe_json,
                    status, created_at
                 ) VALUES('merge-plan', ?1, 'same', 'D:\\exports\\copy.jpg', '{}',
                          'ready', '2026-08-03T00:00:00Z')",
                [first_asset],
            )
            .expect("edit plan");
        connection
            .execute(
                "INSERT INTO face_detections(
                    asset_id, source_fingerprint, bounds_json, confidence,
                    model_name, model_version, created_at
                 ) VALUES(?1, 'same', '{}', 0.9, 'fixture', '1', '2026-08-03T00:00:00Z')",
                [first_asset],
            )
            .expect("face detection");
        drop(connection);

        repository
            .backfill_path_identities()
            .expect("merge duplicate identities");

        let connection = repository.open().expect("reopen database");
        let merged: (i64, i64, Option<String>, bool, String) = connection
            .query_row(
                "SELECT id, rating, color_label, is_favorite, file_status
                 FROM assets WHERE asset_identity_key=?1",
                [identity],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("merged asset");
        assert_eq!(merged.0, second_asset);
        assert_eq!(merged.1, 5);
        assert_eq!(merged.2.as_deref(), Some("purple"));
        assert!(merged.3);
        assert_eq!(merged.4, "present");
        for (table, column) in [
            ("asset_library_assignments", "asset_id"),
            ("manual_classification_overrides", "asset_id"),
            ("manual_tag_overrides", "asset_id"),
            ("collection_assets", "asset_id"),
            ("edit_export_plans", "asset_id"),
            ("face_detections", "asset_id"),
        ] {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {column}=?1"),
                    [second_asset],
                    |row| row.get(0),
                )
                .expect("merged relation count");
            assert_eq!(count, 1, "missing merged relation in {table}");
        }
        let duplicate_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM assets WHERE id=?1",
                [first_asset],
                |row| row.get(0),
            )
            .expect("duplicate count");
        assert_eq!(duplicate_count, 0);
    }

    #[test]
    fn identity_backfill_merges_duplicate_library_relations() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let connection = repository.open().expect("open database");
        connection
            .execute("DROP INDEX idx_libraries_source_identity", [])
            .expect("allow duplicate library identities for migration fixture");
        connection
            .execute(
                "INSERT INTO libraries(
                    root_path, created_at, name, source_path, source_identity_key,
                    status, last_scan_at, scan_generation, parent_relation
                 ) VALUES('C:\\fixtures\\same-library', '2026-08-01T00:00:00Z',
                          'Old', 'C:\\fixtures\\same-library', 'c:/fixtures/same-library',
                          'error', NULL, 1, 'source')",
                [],
            )
            .expect("first duplicate library");
        connection
            .execute(
                "INSERT INTO libraries(
                    root_path, created_at, name, source_path, source_identity_key,
                    status, last_scan_at, scan_generation, parent_relation
                 ) VALUES('c:\\fixtures\\same-library-alt', '2026-08-02T00:00:00Z',
                          'Current', 'c:\\fixtures\\same-library-alt',
                          'c:/fixtures/same-library', 'ready', '2026-08-02T00:00:00Z',
                          2, 'source')",
                [],
            )
            .expect("second duplicate library");
        let second_library = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO assets(
                    library_id, asset_identity_key, absolute_path, relative_path,
                    file_name, extension, file_size, modified_at, fingerprint,
                    file_status, scan_status, analysis_status, first_seen_at,
                    last_seen_at, last_seen_scan
                 ) VALUES(?1, 'c:/fixtures/same-library/photo.jpg',
                          'C:\\fixtures\\same-library\\photo.jpg', 'photo.jpg',
                          'photo.jpg', 'jpg', 100, 1, 'library-asset', 'present',
                          'indexed', 'completed', '2026-08-02T00:00:00Z',
                          '2026-08-02T00:00:00Z', 2)",
                [second_library],
            )
            .expect("library asset");
        let asset_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO asset_library_assignments(asset_id, library_id, assigned_at)
                 VALUES(?1, ?2, '2026-08-03T00:00:00Z')",
                params![asset_id, second_library],
            )
            .expect("library assignment");
        connection
            .execute(
                "INSERT INTO saved_views(
                    name, library_id, query_json, created_at, updated_at
                 ) VALUES('Saved duplicate view', ?1, '{}',
                          '2026-08-03T00:00:00Z', '2026-08-03T00:00:00Z')",
                [second_library],
            )
            .expect("saved view");
        drop(connection);

        repository
            .backfill_path_identities()
            .expect("merge duplicate libraries");

        let connection = repository.open().expect("reopen database");
        let library_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM libraries WHERE source_identity_key=?1",
                ["c:/fixtures/same-library"],
                |row| row.get(0),
            )
            .expect("library count");
        assert_eq!(library_count, 1);
        let remaining_status: String = connection
            .query_row(
                "SELECT status FROM libraries WHERE source_identity_key=?1",
                ["c:/fixtures/same-library"],
                |row| row.get(0),
            )
            .expect("remaining library");
        assert_eq!(remaining_status, "ready");
        let asset_library: i64 = connection
            .query_row(
                "SELECT library_id FROM assets WHERE id=?1",
                [asset_id],
                |row| row.get(0),
            )
            .expect("asset library");
        assert_eq!(asset_library, second_library);
        let assignment_library: i64 = connection
            .query_row(
                "SELECT library_id FROM asset_library_assignments WHERE asset_id=?1",
                [asset_id],
                |row| row.get(0),
            )
            .expect("assignment library");
        assert_eq!(assignment_library, second_library);
        let saved_view_library: i64 = connection
            .query_row(
                "SELECT library_id FROM saved_views WHERE name='Saved duplicate view'",
                [],
                |row| row.get(0),
            )
            .expect("saved view library");
        assert_eq!(saved_view_library, second_library);
    }

    #[test]
    fn library_removal_only_returns_unreferenced_thumbnail_cache_paths() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (first_library, first_generation) = repository
            .begin_scan("C:\\fixtures\\cache-first", "cache-first")
            .expect("first library");
        repository
            .cancel_scan("cache-first", first_library)
            .expect("cancel first library");
        let (second_library, second_generation) = repository
            .begin_scan("D:\\fixtures\\cache-second", "cache-second")
            .expect("second library");
        repository
            .cancel_scan("cache-second", second_library)
            .expect("cancel second library");
        let connection = repository.open().expect("open database");
        for (library_id, generation, identity, path) in [
            (
                first_library,
                first_generation,
                "c:/fixtures/cache-first/photo.jpg",
                "C:\\fixtures\\cache-first\\photo.jpg",
            ),
            (
                second_library,
                second_generation,
                "d:/fixtures/cache-second/photo.jpg",
                "D:\\fixtures\\cache-second\\photo.jpg",
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO assets(
                        library_id, asset_identity_key, absolute_path, relative_path,
                        file_name, extension, file_size, modified_at, fingerprint,
                        file_status, scan_status, analysis_status, first_seen_at,
                        last_seen_at, last_seen_scan
                     ) VALUES(?1, ?2, ?3, 'photo.jpg', 'photo.jpg', 'jpg', 100, 1,
                              'shared-fingerprint', 'present', 'indexed', 'completed',
                              '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z', ?4)",
                    params![library_id, identity, path, generation],
                )
                .expect("asset");
            let asset_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO thumbnails(
                        asset_id, cache_path, spec, source_modified_at, source_size,
                        status, updated_at
                     ) VALUES(?1, ?2, ?3, 1, 100, 'ready', '2026-08-01T00:00:00Z')",
                    params![
                        asset_id,
                        temp.path().join("shared-thumb.jpg").to_string_lossy(),
                        crate::imaging::THUMBNAIL_SPEC
                    ],
                )
                .expect("shared thumbnail");
        }
        drop(connection);

        let first_removal = repository
            .remove_library_with_reconciliation(first_library)
            .expect("remove first library");
        assert_eq!(first_removal.removed_cache_entries.len(), 1);
        assert!(first_removal.removed_cache_entries[0].2.is_none());

        let second_removal = repository
            .remove_library_with_reconciliation(second_library)
            .expect("remove second library");
        assert_eq!(second_removal.removed_cache_entries.len(), 1);
        assert_eq!(
            second_removal.removed_cache_entries[0]
                .2
                .as_deref()
                .map(Path::to_path_buf),
            Some(temp.path().join("shared-thumb.jpg"))
        );
    }

    fn seed_classifiable_asset(repository: &Repository, suffix: &str) -> (i64, i64) {
        let (library_id, _) = repository
            .begin_scan(
                &format!("C:\\fixtures\\classification-{suffix}"),
                &format!("seed-{suffix}"),
            )
            .expect("begin seed scan");
        let connection = repository.open().expect("open seed database");
        let timestamp = now();
        let absolute_path = format!("C:\\fixtures\\classification-{suffix}\\photo.jpg");
        let fingerprint = format!("fingerprint-{suffix}");
        connection
            .execute(
                "INSERT INTO assets(
                    library_id, asset_identity_key, absolute_path, relative_path, file_name,
                    extension, file_size, modified_at, fingerprint, width, height, orientation,
                    file_status, scan_status, analysis_status, first_seen_at, last_seen_at,
                    last_seen_scan
                 ) VALUES(?1, ?2, ?3, 'photo.jpg', 'photo.jpg', 'jpg', 100, 1, ?4,
                          1000, 800, 1, 'present', 'indexed', 'completed', ?5, ?5, 1)",
                params![
                    library_id,
                    identity_key(Path::new(&absolute_path)),
                    absolute_path,
                    fingerprint,
                    timestamp
                ],
            )
            .expect("insert seed asset");
        let asset_id: i64 = connection
            .query_row(
                "SELECT id FROM assets WHERE asset_identity_key=?1",
                [identity_key(Path::new(&absolute_path))],
                |row| row.get(0),
            )
            .expect("seed asset id");
        connection
            .execute(
                "INSERT INTO tone_features(
                    asset_id, brightness_mean, contrast, tone_label, algorithm_version, analyzed_at
                 ) VALUES(?1, 0.5, 0.4, 'mid_tone', 'test', ?2)",
                params![asset_id, timestamp],
            )
            .expect("seed tone");
        connection
            .execute(
                "INSERT INTO color_features(
                    asset_id, saturation_mean, chroma_mean, dominant_color_category,
                    dominant_colors_json, saturation_label, algorithm_version, analyzed_at
                 ) VALUES(?1, 0.6, 0.5, 'blue',
                    '{\"algorithmVersion\":\"accent-oklab-v3\",\"coveragePalette\":[
                        {\"rank\":1,\"color\":\"#376BB5\",\"category\":\"blue\",\"areaCoverage\":0.62,\"saliencyCoverage\":0.50,\"localContrast\":0.3,\"chroma\":0.14,\"spatialCoherence\":0.8}
                    ],\"prominentPalette\":[
                        {\"rank\":1,\"color\":\"#E83A2F\",\"category\":\"red\",\"areaCoverage\":0.34,\"saliencyCoverage\":0.46,\"localContrast\":0.5,\"chroma\":0.18,\"spatialCoherence\":0.7},
                        {\"rank\":2,\"color\":\"#376BB5\",\"category\":\"blue\",\"areaCoverage\":0.28,\"saliencyCoverage\":0.29,\"localContrast\":0.3,\"chroma\":0.14,\"spatialCoherence\":0.6}
                    ]}', 'high', 'test', ?2)",
                params![asset_id, timestamp],
            )
            .expect("seed color");
        connection
            .execute(
                "INSERT INTO semantic_labels(
                    asset_id, label, display_name, similarity, threshold, model_name,
                    model_version, analysis_version, taxonomy_version, category_group,
                    generated_at, source_fingerprint, is_primary, is_manual
                 ) VALUES(?1, 'landscape', 'Landscape', 0.9, 0.2, ?2, ?3, ?4, ?5, 'scene', ?6, ?7, 1, 0)",
                params![
                    asset_id,
                    SIGLIP2_MODEL_NAME,
                    SIGLIP2_MODEL_VERSION,
                    SIGLIP2_ANALYSIS_VERSION,
                    TAXONOMY_VERSION,
                    timestamp,
                    fingerprint
                ],
            )
            .expect("seed primary label");
        connection
            .execute(
                "INSERT INTO semantic_labels(
                    asset_id, label, display_name, similarity, threshold, model_name,
                    model_version, analysis_version, taxonomy_version, category_group,
                    generated_at, source_fingerprint, is_primary, is_manual
                 ) VALUES(?1, 'outdoor', 'Outdoor', 0.8, 0.2, ?2, ?3, ?4, ?5, 'context', ?6, ?7, 0, 0)",
                params![
                    asset_id,
                    SIGLIP2_MODEL_NAME,
                    SIGLIP2_MODEL_VERSION,
                    SIGLIP2_ANALYSIS_VERSION,
                    TAXONOMY_VERSION,
                    timestamp,
                    fingerprint
                ],
            )
            .expect("seed auxiliary label");
        connection
            .execute(
                "UPDATE libraries SET status='ready' WHERE id=?1",
                [library_id],
            )
            .expect("finish seed library");
        (library_id, asset_id)
    }

    #[test]
    fn hue_range_filter_respects_user_strictness_threshold() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (library_id, asset_id) = seed_classifiable_asset(&repository, "hue-strictness");
        let connection = repository.open().expect("open database");
        connection
            .execute(
                "UPDATE color_features
                 SET hue_histogram_json=?1
                 WHERE asset_id=?2",
                params!["[0.5,0.5,0,0,0,0,0,0,0,0,0,0]", asset_id],
            )
            .expect("seed hue histogram");
        drop(connection);

        let list_with_strictness = |strictness| {
            repository
                .list_assets(
                    library_id,
                    AssetSortField::FileName,
                    SortDirection::Asc,
                    1,
                    100,
                    &AssetFilter {
                        color_hue_center: Some(0.0),
                        color_hue_width: Some(15.0),
                        color_hue_strictness: Some(strictness),
                        ..AssetFilter::default()
                    },
                )
                .expect("hue range filter")
                .total
        };

        assert_eq!(list_with_strictness(0.0), 1);
        assert_eq!(list_with_strictness(0.5), 1);
        assert_eq!(list_with_strictness(1.0), 0);
    }

    #[test]
    fn favorite_and_collection_sources_share_the_asset_query_contract() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (library_id, asset_id) = seed_classifiable_asset(&repository, "sources");
        let connection = repository.open().expect("open database");
        crate::workflow::set_favorite(&repository, asset_id, true).expect("mark favorite");

        let favorite_page = repository
            .list_assets(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &AssetFilter {
                    favorite_only: true,
                    ..AssetFilter::default()
                },
            )
            .expect("favorite source");
        assert_eq!(favorite_page.total, 1);

        connection
            .execute(
                "INSERT INTO collections(name, description, created_at, updated_at)
                 VALUES('Source collection', '', ?1, ?1)",
                [now()],
            )
            .expect("create collection");
        let collection_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO collection_assets(collection_id, asset_id, added_at)
                 VALUES(?1, ?2, ?3)",
                params![collection_id, asset_id, now()],
            )
            .expect("add collection member");

        let collection_page = repository
            .list_assets(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &AssetFilter {
                    collection_id: Some(collection_id),
                    ..AssetFilter::default()
                },
            )
            .expect("collection source");
        assert_eq!(collection_page.total, 1);
    }

    #[test]
    fn asset_query_roots_share_paging_and_collection_descendants() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (first_library_id, first_asset_id) =
            seed_classifiable_asset(&repository, "query-root-first");
        let (second_library_id, second_asset_id) =
            seed_classifiable_asset(&repository, "query-root-second");
        crate::workflow::set_favorite(&repository, first_asset_id, true).expect("favorite");

        let connection = repository.open().expect("open database");
        connection
            .execute(
                "INSERT INTO collections(name, description, created_at, updated_at)
                 VALUES('Query parent', '', ?1, ?1)",
                [now()],
            )
            .expect("create parent collection");
        let parent_collection_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO collections(
                     name, description, created_at, updated_at, parent_collection_id
                 ) VALUES('Query child', '', ?1, ?1, ?2)",
                params![now(), parent_collection_id],
            )
            .expect("create child collection");
        let child_collection_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO collection_assets(collection_id, asset_id, added_at)
                 VALUES(?1, ?2, ?3), (?4, ?5, ?3)",
                params![
                    parent_collection_id,
                    second_asset_id,
                    now(),
                    child_collection_id,
                    first_asset_id
                ],
            )
            .expect("add collection members");
        connection
            .execute(
                "INSERT INTO collection_assets(collection_id, asset_id, added_at)
                 VALUES(?1, ?2, ?3)",
                params![parent_collection_id, first_asset_id, now()],
            )
            .expect("add overlapping parent member");
        drop(connection);

        let query = |root| {
            repository
                .query_assets(&AssetQuery {
                    version: ASSET_QUERY_VERSION,
                    root,
                    include_descendants: true,
                    filter: AssetFilter::default(),
                    sort: AssetSortField::FileName,
                    direction: SortDirection::Asc,
                    page: 1,
                    page_size: 100,
                })
                .expect("query assets")
        };

        assert_eq!(query(AssetQueryRoot::All).total, 2);
        assert_eq!(
            query(AssetQueryRoot::Source {
                library_id: first_library_id
            })
            .total,
            1
        );
        assert_eq!(query(AssetQueryRoot::Favorites).total, 1);
        let legacy_favorite_page = repository
            .list_assets(
                second_library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &AssetFilter {
                    favorite_only: true,
                    ..AssetFilter::default()
                },
            )
            .expect("legacy source favorite query");
        assert_eq!(legacy_favorite_page.total, 0);
        assert_eq!(
            query(AssetQueryRoot::Collection {
                collection_id: parent_collection_id
            })
            .total,
            2
        );

        let direct_collection = repository
            .query_assets(&AssetQuery {
                version: ASSET_QUERY_VERSION,
                root: AssetQueryRoot::Collection {
                    collection_id: parent_collection_id,
                },
                include_descendants: false,
                filter: AssetFilter::default(),
                sort: AssetSortField::FileName,
                direction: SortDirection::Asc,
                page: 1,
                page_size: 100,
            })
            .expect("direct collection query");
        assert_eq!(direct_collection.total, 2);
        assert!(
            direct_collection
                .items
                .iter()
                .any(|asset| asset.id == second_asset_id)
        );
        assert!(
            repository
                .list_assets_for_organization(
                    first_library_id,
                    &AssetFilter {
                        collection_id: Some(parent_collection_id),
                        ..AssetFilter::default()
                    },
                    None,
                )
                .is_err()
        );
        assert!(
            repository
                .list_assets_for_organization(
                    second_library_id,
                    &AssetFilter {
                        favorite_only: true,
                        ..AssetFilter::default()
                    },
                    None,
                )
                .is_err()
        );
    }

    #[test]
    fn manual_overrides_resolve_filter_restore_and_survive_auto_failure() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (library_id, asset_id) = seed_classifiable_asset(&repository, "one");
        let (_, second_asset_id) = seed_classifiable_asset(&repository, "two");

        let detail = repository
            .get_asset_detail(asset_id)
            .expect("initial detail");
        assert_eq!(detail.asset.rating, 0);
        assert_eq!(detail.asset.color_label, None);
        let marked = repository
            .update_asset_rating(asset_id, 4)
            .expect("rating update");
        assert_eq!(marked.asset.rating, 4);
        let marked = repository
            .update_asset_color_label(asset_id, Some("purple"))
            .expect("color label update");
        assert_eq!(marked.asset.color_label.as_deref(), Some("purple"));
        assert!(repository.update_asset_rating(asset_id, 6).is_err());
        assert!(
            repository
                .update_asset_color_label(asset_id, Some("orange"))
                .is_err()
        );
        let high_rated_path = "C:\\fixtures\\classification-one\\high-rated.jpg";
        let connection = repository
            .open()
            .expect("open database for rating threshold");
        connection
            .execute(
                "INSERT INTO assets(
                    library_id, asset_identity_key, absolute_path, relative_path, file_name,
                    extension, file_size, modified_at, fingerprint, width, height, orientation,
                    file_status, scan_status, analysis_status, first_seen_at, last_seen_at,
                    last_seen_scan, rating
                 ) VALUES(?1, ?2, ?3, 'high-rated.jpg', 'high-rated.jpg', 'jpg', 100, 1,
                          'fingerprint-high-rated', 1000, 800, 1, 'present', 'indexed',
                          'completed', 1, 1, 1, 5)",
                params![
                    library_id,
                    identity_key(Path::new(high_rated_path)),
                    high_rated_path
                ],
            )
            .expect("insert high-rated asset");
        let threshold_filter = repository
            .list_assets(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &AssetFilter {
                    ratings: vec![4],
                    ..AssetFilter::default()
                },
            )
            .expect("rating threshold filter");
        assert_eq!(threshold_filter.total, 2);
        let marked_filter = repository
            .list_assets(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &AssetFilter {
                    ratings: vec![4],
                    color_labels: vec!["purple".into()],
                    ..AssetFilter::default()
                },
            )
            .expect("manual mark filter");
        assert_eq!(marked_filter.total, 1);
        assert_eq!(
            detail.asset.classification.primary_category.auto.as_deref(),
            Some("landscape")
        );
        assert_eq!(
            detail
                .asset
                .classification
                .primary_category
                .effective
                .as_deref(),
            Some("landscape")
        );
        assert_eq!(
            detail.asset.classification.auxiliary_tags.effective,
            vec!["outdoor"]
        );
        assert_eq!(
            detail.asset.classification.dominant_color_categories.auto,
            Some(vec!["blue".to_owned()])
        );
        let auto_palette_filter = repository
            .list_assets(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &AssetFilter {
                    color_categories: vec!["red".into()],
                    ..AssetFilter::default()
                },
            )
            .expect("automatic palette filter");
        assert_eq!(auto_palette_filter.total, 0);

        let detail = repository
            .update_classification_override(
                asset_id,
                FIELD_PRIMARY_CATEGORY,
                Some(serde_json::json!("architecture")),
            )
            .expect("primary override");
        assert_eq!(
            detail
                .asset
                .classification
                .primary_category
                .manual
                .as_deref(),
            Some("architecture")
        );
        assert_eq!(
            detail
                .asset
                .classification
                .primary_category
                .effective
                .as_deref(),
            Some("architecture")
        );

        let _detail = repository
            .update_tag_override(asset_id, "favorite", Some("add"))
            .expect("tag add");
        let detail = repository
            .update_tag_override(asset_id, "outdoor", Some("remove"))
            .expect("tag remove");
        assert_eq!(
            detail.asset.classification.auxiliary_tags.effective,
            vec!["favorite"]
        );

        let detail = repository
            .update_classification_override(
                asset_id,
                FIELD_DOMINANT_COLOR_CATEGORY,
                Some(serde_json::json!(["orange", "blue"])),
            )
            .expect("palette override");
        assert_eq!(
            detail
                .asset
                .classification
                .dominant_color_categories
                .effective,
            Some(vec!["orange".to_owned(), "blue".to_owned()])
        );

        let filtered = repository
            .list_assets(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &AssetFilter {
                    primary_categories: vec!["architecture".into()],
                    auxiliary_tags: vec!["favorite".into()],
                    color_categories: vec!["orange".into()],
                    ..AssetFilter::default()
                },
            )
            .expect("effective filter");
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.items[0].tone_label.as_deref(), Some("mid_tone"));
        assert_eq!(
            filtered.items[0].dominant_color_category.as_deref(),
            Some("orange")
        );

        let connection = repository.open().expect("open database");
        connection
            .execute(
                "UPDATE assets SET fingerprint='changed', semantic_status='failed' WHERE id=?1",
                [asset_id],
            )
            .expect("fail auto analysis");
        let failed_detail = repository
            .get_asset_detail(asset_id)
            .expect("failed detail");
        assert_eq!(failed_detail.asset.semantic_status, "failed");
        assert_eq!(
            failed_detail
                .asset
                .classification
                .primary_category
                .effective
                .as_deref(),
            Some("architecture")
        );

        let restored = repository
            .restore_auto_classification(asset_id, None)
            .expect("restore all");
        assert_eq!(
            restored.asset.classification.primary_category.effective,
            None
        );
        assert!(
            restored
                .asset
                .classification
                .auxiliary_tags
                .effective
                .is_empty()
        );

        assert_eq!(
            repository
                .batch_update_classification(
                    &[asset_id, second_asset_id],
                    FIELD_PRIMARY_CATEGORY,
                    serde_json::json!("product"),
                )
                .expect("batch primary override"),
            2
        );
        assert_eq!(
            repository
                .get_asset_detail(second_asset_id)
                .expect("second detail")
                .asset
                .classification
                .primary_category
                .effective
                .as_deref(),
            Some("product")
        );
    }

    #[test]
    fn completed_without_current_scene_is_virtual_abstract_and_filterable() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (library_id, asset_id) = seed_classifiable_asset(&repository, "unknown");
        let connection = repository.open().expect("open database");
        connection
            .execute("DELETE FROM semantic_labels WHERE asset_id=?1", [asset_id])
            .expect("remove current labels");
        connection
            .execute(
                "UPDATE assets SET semantic_status='completed', semantic_error=NULL WHERE id=?1",
                [asset_id],
            )
            .expect("mark empty semantic analysis complete");
        connection
            .execute(
                "INSERT INTO semantic_labels(
                    asset_id, label, display_name, similarity, threshold, model_name,
                    model_version, analysis_version, taxonomy_version, category_group,
                    generated_at, source_fingerprint, is_primary, is_manual
                 ) VALUES(?1, 'landscape', '旧风景', 0.95, 0.16, ?2, ?3, ?4, 'legacy-v1',
                          'scene', ?5, 'fingerprint-unknown', 1, 0)",
                params![
                    asset_id,
                    MODEL_NAME,
                    MODEL_VERSION,
                    SEMANTIC_ANALYSIS_VERSION,
                    now()
                ],
            )
            .expect("seed stale taxonomy label");
        let detail = repository
            .get_asset_detail(asset_id)
            .expect("load virtual abstract detail");
        assert!(detail.asset.semantic_labels.is_empty());
        assert_eq!(
            detail.asset.classification.primary_category.auto.as_deref(),
            Some("photo_abstract")
        );
        assert_eq!(
            detail
                .asset
                .classification
                .primary_category
                .effective
                .as_deref(),
            Some("photo_abstract")
        );

        let abstract_filter = repository
            .list_assets(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                100,
                &AssetFilter {
                    primary_categories: vec!["photo_abstract".into()],
                    ..AssetFilter::default()
                },
            )
            .expect("filter virtual abstract");
        assert_eq!(abstract_filter.total, 1);
        let groups = repository
            .list_semantic_groups(library_id)
            .expect("list virtual abstract group");
        assert!(groups.iter().any(|group| {
            group.label_id == "photo_abstract"
                && group.category_group == "scene"
                && group.asset_count == 1
        }));
    }

    #[test]
    fn library_root_is_unique_and_survives_repository_restart() {
        let temp = tempfile::tempdir().expect("temp dir");
        let database = temp.path().join("database.sqlite3");
        let repository = Repository::new(&database);
        repository.initialize().expect("initialize");
        let (first_id, first_generation) = repository
            .begin_scan("C:\\synthetic 图库", "task-one")
            .expect("first scan");
        repository
            .cancel_scan("task-one", first_id)
            .expect("cancel first");
        let (second_id, second_generation) = repository
            .begin_scan("C:\\synthetic 图库", "task-two")
            .expect("second scan");
        repository
            .cancel_scan("task-two", second_id)
            .expect("cancel second");

        assert_eq!(first_id, second_id);
        assert_eq!(first_generation + 1, second_generation);

        let reopened = Repository::new(database);
        reopened.initialize().expect("reopen");
        let libraries = reopened.list_libraries().expect("libraries");
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].root_path, "C:\\synthetic 图库");
    }

    #[test]
    fn manual_library_parent_is_persistent_and_cycles_are_rejected() {
        let temp = tempfile::tempdir().expect("temp dir");
        let database = temp.path().join("database.sqlite3");
        let repository = Repository::new(&database);
        repository.initialize().expect("initialize");

        let (a, _) = repository
            .begin_scan("C:\\photos\\A", "manual-a")
            .expect("library a");
        repository.cancel_scan("manual-a", a).expect("cancel a");
        let (b, _) = repository
            .begin_scan("D:\\exports\\B", "manual-b")
            .expect("library b");
        repository.cancel_scan("manual-b", b).expect("cancel b");
        let (c, _) = repository
            .begin_scan("E:\\selected\\C", "manual-c")
            .expect("library c");
        repository.cancel_scan("manual-c", c).expect("cancel c");

        assert!(
            repository
                .set_library_parent(b, Some(a))
                .expect("b under a")
        );
        assert!(
            repository
                .set_library_parent(c, Some(b))
                .expect("c under b")
        );
        assert!(matches!(
            repository.set_library_parent(a, Some(c)),
            Err(AppError::InvalidArgument(_))
        ));
        assert!(matches!(
            repository.set_library_parent(a, Some(a)),
            Err(AppError::InvalidArgument(_))
        ));

        let reopened = Repository::new(database);
        reopened.initialize().expect("reopen");
        let libraries = reopened.list_libraries().expect("libraries");
        assert_eq!(
            libraries
                .iter()
                .find(|library| library.id == b)
                .and_then(|library| library.parent_library_id),
            Some(a)
        );
        assert_eq!(
            libraries
                .iter()
                .find(|library| library.id == c)
                .and_then(|library| library.parent_library_id),
            Some(b)
        );

        reopened
            .set_library_parent(c, None)
            .expect("move c to root");
        let root_libraries = reopened.list_libraries().expect("root libraries");
        assert_eq!(
            root_libraries
                .iter()
                .find(|library| library.id == c)
                .and_then(|library| library.parent_library_id),
            None
        );
    }

    #[test]
    fn asset_library_assignment_is_virtual_and_survives_restore() {
        let temp = tempfile::tempdir().expect("temp dir");
        let database = temp.path().join("database.sqlite3");
        let repository = Repository::new(&database);
        repository.initialize().expect("initialize");

        let (source_library, source_generation) = repository
            .begin_scan("C:\\photos\\source", "asset-source-task")
            .expect("source library");
        repository
            .cancel_scan("asset-source-task", source_library)
            .expect("cancel source");
        let (target_library, _) = repository
            .begin_scan("D:\\photos\\target", "asset-target-task")
            .expect("target library");
        repository
            .cancel_scan("asset-target-task", target_library)
            .expect("cancel target");

        let snapshot = FileSnapshot {
            absolute_path: "C:\\photos\\source\\photo.jpg".into(),
            relative_path: "photo.jpg".into(),
            file_name: "photo.jpg".into(),
            extension: "jpg".into(),
            file_size: 42,
            modified_at: 100,
        };
        let asset_id = repository
            .upsert_failed_asset(
                source_library,
                source_generation,
                &snapshot,
                "fingerprint-source",
                "fixture failure",
            )
            .expect("asset");
        let before_source = repository.asset_source(asset_id).expect("source before");

        let source_assets = repository
            .list_assets(
                source_library,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &AssetFilter::default(),
            )
            .expect("source assets");
        assert_eq!(source_assets.total, 1);

        assert!(
            repository
                .assign_asset_to_library(asset_id, target_library)
                .expect("assign asset")
        );
        let target_assets = repository
            .list_assets(
                target_library,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &AssetFilter::default(),
            )
            .expect("target assets");
        assert_eq!(target_assets.total, 0);
        assert_eq!(
            repository.asset_source(asset_id).expect("source after"),
            before_source
        );

        assert!(
            repository
                .assign_asset_to_library(asset_id, source_library)
                .expect("restore automatic assignment")
        );
        let restored_target_assets = repository
            .list_assets(
                target_library,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &AssetFilter::default(),
            )
            .expect("restored target assets");
        assert_eq!(restored_target_assets.total, 0);
        let restored_source_assets = repository
            .list_assets(
                source_library,
                AssetSortField::FileName,
                SortDirection::Asc,
                1,
                20,
                &AssetFilter::default(),
            )
            .expect("restored source assets");
        assert_eq!(restored_source_assets.total, 1);
    }

    #[test]
    fn removing_library_clears_index_but_not_source_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("图库 😀");
        std::fs::create_dir_all(&source).expect("source dir");
        let image = source.join("原图.png");
        std::fs::write(&image, b"fixture source bytes").expect("source file");
        let before = std::fs::read(&image).expect("read source");

        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("initialize");
        let (library_id, _) = repository
            .begin_scan(&source.to_string_lossy(), "remove-library-task")
            .expect("begin scan");
        repository
            .cancel_scan("remove-library-task", library_id)
            .expect("cancel scan");

        assert!(repository.remove_library(library_id).expect("remove index"));
        assert!(source.is_dir());
        assert_eq!(std::fs::read(&image).expect("source after"), before);
        assert!(repository.list_libraries().expect("libraries").is_empty());
    }
}
