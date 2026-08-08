use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};

use crate::error::{AppError, AppResult};
use crate::models::{
    AssetFilter, AssetListItem, AssetPage, AssetSortField, ExistingAssetSnapshot, FileSnapshot,
    FolderSummary, LibrarySummary, OrganizationIssue, OrganizationIssueSeverity, OrganizationPlan,
    OrganizationPlanRecord, ProcessedImage, SemanticGroupSummary, SemanticLabelResult,
    SemanticMatchMode, SemanticProgress, SortDirection,
};
use crate::semantic::{
    ANALYSIS_VERSION as SEMANTIC_ANALYSIS_VERSION, MODEL_NAME, MODEL_VERSION,
    SemanticAnalysisOutput,
};
use crate::source_identity::{identity_key, is_same_or_descendant};

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const SEMANTIC_MIGRATION: &str = include_str!("../migrations/0002_semantic_workspace.sql");
const ORGANIZATION_MIGRATION: &str = include_str!("../migrations/0003_organization_dry_run.sql");
const LIBRARY_UX_MIGRATION: &str = include_str!("../migrations/0004_library_ux_refinement.sql");
const LIBRARY_SOURCE_MIGRATION: &str =
    include_str!("../migrations/0005_library_source_hierarchy.sql");
const ASSET_IDENTITY_MIGRATION: &str = include_str!("../migrations/0006_asset_global_identity.sql");

#[derive(Debug, Clone)]
pub struct Repository {
    database_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SemanticAssetCandidate {
    pub id: i64,
    pub absolute_path: PathBuf,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySourceRoot {
    pub library_id: i64,
    pub source_path: PathBuf,
    pub identity_key: String,
}

#[derive(Debug, Clone, Default)]
pub struct LibraryRemovalResult {
    pub removed: bool,
    pub removed_cache_entries: Vec<(i64, String, Option<PathBuf>)>,
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
        ] {
            if current < version {
                let transaction = connection.transaction()?;
                transaction.execute_batch(sql)?;
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
        self.begin_scan_with_identity(root_path, &source_identity_key, &name, task_id)
    }

    pub fn begin_scan_with_identity(
        &self,
        root_path: &str,
        source_identity_key: &str,
        name: &str,
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
                 status = 'scanning', last_error = NULL, scan_generation = scan_generation + 1
             WHERE source_identity_key = ?1",
            params![source_identity_key, root_path, name],
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
                 SELECT id FROM libraries WHERE parent_library_id=?1
                 UNION
                 SELECT child.id
                 FROM libraries child
                 JOIN descendants parent ON child.parent_library_id=parent.library_id
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
        transaction.commit()?;
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
             LEFT JOIN assets a ON a.library_id = scope.library_id
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
                removed_cache_entries.push((asset_id, fingerprint, cache_path));
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
        let connection = self.open()?;
        let page_size = page_size.clamp(1, 500);
        let offset = i64::from(page.saturating_sub(1)) * i64::from(page_size);
        let (where_sql, values) = asset_filter_sql(library_id, filter);
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
                    a.semantic_analyzed_at
             FROM assets a
             LEFT JOIN thumbnails t ON t.asset_id=a.id AND t.spec=?
             LEFT JOIN tone_features tf ON tf.asset_id=a.id
             LEFT JOIN color_features cf ON cf.asset_id=a.id
             WHERE {where_sql}
             ORDER BY {} {}, a.file_name COLLATE NOCASE ASC, a.id ASC
             LIMIT ? OFFSET ?",
            sort.sql_expression(),
            direction.sql(),
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
                neutral_ratio: row.get(32)?,
                dominant_color_coverage: row.get(33)?,
                semantic_status: row.get(34)?,
                semantic_error: row.get(35)?,
                semantic_analyzed_at: row.get(36)?,
                semantic_labels: Vec::new(),
            })
        })?;
        let mut items = rows.collect::<Result<Vec<_>, _>>()?;
        for item in &mut items {
            item.semantic_labels = semantic_labels_for_asset(&connection, item.id)?;
        }
        Ok(AssetPage {
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
        let mut page = 1;
        let mut items = Vec::new();
        loop {
            let result = self.list_assets(
                library_id,
                AssetSortField::FileName,
                SortDirection::Asc,
                page,
                500,
                filter,
            )?;
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
            "SELECT relative_path FROM assets
             WHERE library_id=?1 AND file_status='present'
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
             )
             SELECT sl.label, sl.display_name, COUNT(*)
             FROM semantic_labels sl
             JOIN assets a ON a.id=sl.asset_id
             WHERE a.library_id IN (SELECT library_id FROM library_scope)
               AND a.file_status='present'
               AND sl.is_primary=1 AND sl.source_fingerprint=a.fingerprint
               AND sl.model_name=?2 AND sl.model_version=?3 AND sl.analysis_version=?4
             GROUP BY sl.label, sl.display_name
             ORDER BY COUNT(*) DESC, sl.label ASC",
        )?;
        let rows = statement.query_map(
            params![
                library_id,
                MODEL_NAME,
                MODEL_VERSION,
                SEMANTIC_ANALYSIS_VERSION
            ],
            |row| {
                Ok(SemanticGroupSummary {
                    label_id: row.get(0)?,
                    display_name: row.get(1)?,
                    asset_count: row.get(2)?,
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
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
                "https://huggingface.co/onnx-community/TinyCLIP-ViT-8M-16-Text-3M-YFCC15M-ONNX",
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

    pub fn create_semantic_job(
        &self,
        job_id: &str,
        library_id: i64,
        force: bool,
        only_asset_id: Option<i64>,
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        self.create_semantic_job_with_ids(job_id, library_id, force, only_asset_id, None)
    }

    pub fn create_semantic_job_for_assets(
        &self,
        job_id: &str,
        library_id: i64,
        asset_ids: &[i64],
    ) -> AppResult<Vec<SemanticAssetCandidate>> {
        self.create_semantic_job_with_ids(job_id, library_id, false, None, Some(asset_ids))
    }

    fn create_semantic_job_with_ids(
        &self,
        job_id: &str,
        library_id: i64,
        force: bool,
        only_asset_id: Option<i64>,
        only_asset_ids: Option<&[i64]>,
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
        let mut sql = String::from(
            "SELECT a.id, a.absolute_path, a.fingerprint
             FROM assets a
             WHERE a.library_id IN (
                 WITH RECURSIVE library_scope(library_id) AS (
                     SELECT id FROM libraries WHERE id=?1
                     UNION
                     SELECT child.id FROM libraries child
                     JOIN library_scope scope ON child.parent_library_id=scope.library_id
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
            sql.push_str(
                " AND NOT EXISTS(
                    SELECT 1 FROM semantic_labels sl
                    WHERE sl.asset_id=a.id AND sl.source_fingerprint=a.fingerprint
                      AND sl.model_name=? AND sl.model_version=? AND sl.analysis_version=?
                 )",
            );
            values.push(Value::Text(MODEL_NAME.into()));
            values.push(Value::Text(MODEL_VERSION.into()));
            values.push(Value::Text(SEMANTIC_ANALYSIS_VERSION.into()));
        }
        sql.push_str(" ORDER BY a.id ASC");
        let candidates = {
            let mut statement = transaction.prepare(&sql)?;
            statement
                .query_map(params_from_iter(values.iter()), |row| {
                    Ok(SemanticAssetCandidate {
                        id: row.get(0)?,
                        absolute_path: PathBuf::from(row.get::<_, String>(1)?),
                        fingerprint: row.get(2)?,
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
                MODEL_NAME,
                MODEL_VERSION,
                SEMANTIC_ANALYSIS_VERSION,
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
        let connection = self.open()?;
        let timestamp = now();
        connection.execute(
            "UPDATE analysis_job_items SET status='running', attempts=attempts+1, updated_at=?3
             WHERE job_id=?1 AND asset_id=?2",
            params![job_id, asset_id, timestamp],
        )?;
        connection.execute(
            "UPDATE assets SET semantic_status='running', semantic_error=NULL WHERE id=?1",
            [asset_id],
        )?;
        Ok(())
    }

    pub fn save_semantic_result(
        &self,
        job_id: &str,
        candidate: &SemanticAssetCandidate,
        output: &SemanticAnalysisOutput,
    ) -> AppResult<bool> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
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
            transaction.commit()?;
            return Ok(false);
        }
        transaction.execute(
            "DELETE FROM semantic_labels
             WHERE asset_id=?1 AND model_name=?2 AND model_version=?3 AND analysis_version=?4",
            params![
                candidate.id,
                MODEL_NAME,
                MODEL_VERSION,
                SEMANTIC_ANALYSIS_VERSION
            ],
        )?;
        let timestamp = now();
        for prediction in &output.predictions {
            transaction.execute(
                "INSERT INTO semantic_labels(
                    asset_id, label, display_name, similarity, threshold, model_name, model_version,
                    analysis_version, source_fingerprint, is_manual, is_primary, generated_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11)",
                params![
                    candidate.id,
                    prediction.label_id,
                    prediction.display_name,
                    prediction.similarity,
                    prediction.threshold,
                    MODEL_NAME,
                    MODEL_VERSION,
                    SEMANTIC_ANALYSIS_VERSION,
                    candidate.fingerprint,
                    if prediction.is_primary { 1_i64 } else { 0_i64 },
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
                MODEL_NAME,
                MODEL_VERSION,
                SEMANTIC_ANALYSIS_VERSION,
                candidate.fingerprint,
                output.embedding.len() as i64,
                embedding_bytes,
                timestamp,
            ],
        )?;
        transaction.execute(
            "UPDATE assets SET semantic_status='completed', semantic_error=NULL,
                               semantic_analyzed_at=?2 WHERE id=?1",
            params![candidate.id, timestamp],
        )?;
        transaction.execute(
            "UPDATE analysis_job_items SET status='completed', error_message=NULL, updated_at=?3
             WHERE job_id=?1 AND asset_id=?2",
            params![job_id, candidate.id, timestamp],
        )?;
        transaction.commit()?;
        Ok(true)
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
            "UPDATE assets SET semantic_status='failed', semantic_error=?2 WHERE id=?1",
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
        let mut statement = connection.prepare(
            "SELECT a.id, a.absolute_path, ji.source_fingerprint
             FROM analysis_job_items ji
             JOIN assets a ON a.id=ji.asset_id
             WHERE ji.job_id=?1 AND ji.status IN('queued', 'running')
             ORDER BY ji.id ASC",
        )?;
        let rows = statement.query_map([job_id], |row| {
            Ok(SemanticAssetCandidate {
                id: row.get(0)?,
                absolute_path: PathBuf::from(row.get::<_, String>(1)?),
                fingerprint: row.get(2)?,
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

fn asset_filter_sql(library_id: i64, filter: &AssetFilter) -> (String, Vec<Value>) {
    let mut clauses = vec![
        "a.library_id IN (
            WITH RECURSIVE library_scope(library_id) AS (
                SELECT id FROM libraries WHERE id=?
                UNION
                SELECT child.id
                FROM libraries child
                JOIN library_scope scope ON child.parent_library_id = scope.library_id
            )
            SELECT library_id FROM library_scope
        )"
        .to_string(),
    ];
    let mut values = vec![Value::Integer(library_id)];
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
    if !filter.semantic_labels.is_empty() {
        let placeholders = placeholders(filter.semantic_labels.len());
        match filter.semantic_match {
            SemanticMatchMode::Any => clauses.push(format!(
                "EXISTS(SELECT 1 FROM semantic_labels sl
                 WHERE sl.asset_id=a.id AND sl.source_fingerprint=a.fingerprint
                   AND sl.model_name=? AND sl.model_version=? AND sl.analysis_version=?
                   AND sl.label IN ({placeholders}))"
            )),
            SemanticMatchMode::All => clauses.push(format!(
                "(SELECT COUNT(DISTINCT sl.label) FROM semantic_labels sl
                  WHERE sl.asset_id=a.id AND sl.source_fingerprint=a.fingerprint
                    AND sl.model_name=? AND sl.model_version=? AND sl.analysis_version=?
                    AND sl.label IN ({placeholders})) = ?"
            )),
        }
        values.push(Value::Text(MODEL_NAME.into()));
        values.push(Value::Text(MODEL_VERSION.into()));
        values.push(Value::Text(SEMANTIC_ANALYSIS_VERSION.into()));
        values.extend(filter.semantic_labels.iter().cloned().map(Value::Text));
        if filter.semantic_match == SemanticMatchMode::All {
            values.push(Value::Integer(filter.semantic_labels.len() as i64));
        }
    }
    add_string_in_filter(
        &mut clauses,
        &mut values,
        "tf.tone_label",
        &filter.tone_labels,
    );
    add_string_in_filter(
        &mut clauses,
        &mut values,
        "cf.dominant_color_category",
        &filter.color_categories,
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
    if let Some(prefix) = filter
        .folder_prefix
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        clauses.push(
            "(a.relative_path=? OR a.relative_path LIKE ? ESCAPE '\\' OR a.relative_path LIKE ? ESCAPE '\\')"
                .into(),
        );
        let escaped = escape_like(prefix);
        values.push(Value::Text(prefix.into()));
        values.push(Value::Text(format!("{}\\\\%", escaped)));
        values.push(Value::Text(format!("{}/%", escaped)));
    }
    match filter.semantic_state.as_deref() {
        Some("not_analyzed") => {
            clauses.push("a.semantic_status IN ('not_analyzed', 'queued', 'running')".into())
        }
        Some("failed") => clauses.push("a.semantic_status='failed'".into()),
        _ => {}
    }
    (clauses.join(" AND "), values)
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
    let mut statement = connection.prepare(
        "SELECT sl.label, sl.display_name, sl.similarity, sl.threshold, sl.model_name,
                sl.model_version, sl.analysis_version, sl.generated_at, sl.is_manual, sl.is_primary
         FROM semantic_labels sl
         JOIN assets a ON a.id=sl.asset_id
         WHERE sl.asset_id=?1 AND sl.source_fingerprint=a.fingerprint
           AND sl.model_name=?2 AND sl.model_version=?3 AND sl.analysis_version=?4
         ORDER BY sl.is_primary DESC, sl.similarity DESC, sl.label ASC",
    )?;
    let rows = statement.query_map(
        params![
            asset_id,
            MODEL_NAME,
            MODEL_VERSION,
            SEMANTIC_ANALYSIS_VERSION
        ],
        |row| {
            Ok(SemanticLabelResult {
                label_id: row.get(0)?,
                display_name: row.get(1)?,
                similarity: row.get(2)?,
                threshold: row.get(3)?,
                model_name: row.get(4)?,
                model_version: row.get(5)?,
                analysis_version: row.get(6)?,
                analyzed_at: row.get(7)?,
                is_manual: row.get::<_, i64>(8)? != 0,
                is_primary: row.get::<_, i64>(9)? != 0,
            })
        },
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
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
        let survivor = ids[0];
        for duplicate in ids.iter().skip(1).copied() {
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
                    display_name, threshold, source_fingerprint, is_primary
                 )
                 SELECT ?1, label, similarity, model_name, model_version,
                        analysis_version, generated_at, is_manual, is_excluded,
                        display_name, threshold, source_fingerprint, is_primary
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
        let survivor = ids[0];
        for duplicate in ids.iter().skip(1).copied() {
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
                "UPDATE libraries SET parent_library_id=?1 WHERE parent_library_id=?2",
                params![survivor, duplicate],
            )?;
            transaction.execute("DELETE FROM libraries WHERE id=?1", [duplicate])?;
        }
    }
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

    for (library_id, source_identity_key) in &libraries {
        let parent_library_id = libraries
            .iter()
            .filter(|(candidate_id, candidate_key)| {
                candidate_id != library_id
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
    fn initialization_and_migration_are_idempotent() {
        let temp = tempfile::tempdir().expect("temp dir");
        let repository = Repository::new(temp.path().join("database.sqlite3"));
        repository.initialize().expect("first initialization");
        repository.initialize().expect("second initialization");
        assert_eq!(repository.migration_version().expect("version"), 6);
        let connection = repository.open().expect("connection");
        for table in [
            "organization_plans",
            "organization_plan_items",
            "organization_plan_issues",
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
