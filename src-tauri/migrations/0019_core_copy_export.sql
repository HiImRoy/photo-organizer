CREATE TABLE IF NOT EXISTS catalog_backup_records (
    id TEXT PRIMARY KEY,
    backup_path TEXT NOT NULL UNIQUE,
    purpose TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    database_identity TEXT NOT NULL,
    backup_blake3 TEXT NOT NULL,
    integrity_check TEXT NOT NULL,
    foreign_key_check_count INTEGER NOT NULL,
    restore_check TEXT NOT NULL,
    created_at TEXT NOT NULL,
    verified_at TEXT NOT NULL
);

-- A stable catalog UUID survives ordinary restarts; the epoch distinguishes
-- a newly opened runtime after recovery/relaunch. Filesystem jobs are never
-- replayed merely because the SQLite catalog was restored.
CREATE TABLE IF NOT EXISTS catalog_runtime_state (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    catalog_id TEXT NOT NULL,
    instance_epoch INTEGER NOT NULL CHECK(instance_epoch >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS organization_plans_v2 (
    id TEXT PRIMARY KEY,
    snapshot_version INTEGER NOT NULL,
    query_version INTEGER NOT NULL,
    scope_json TEXT NOT NULL,
    rules_json TEXT NOT NULL,
    target_parent_path TEXT NOT NULL,
    target_parent_identity_json TEXT NOT NULL,
    target_child_name TEXT NOT NULL,
    target_root_path TEXT NOT NULL,
    target_root_state TEXT NOT NULL CHECK(target_root_state IN ('expected_absent', 'created')),
    target_root_identity_json TEXT,
    protected_roots_json TEXT NOT NULL,
    summary_json TEXT NOT NULL,
    snapshot_blake3 TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('ready', 'blocked', 'stale', 'consumed')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_organization_plans_v2_recent
    ON organization_plans_v2(created_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS organization_plan_items_v2 (
    plan_id TEXT NOT NULL REFERENCES organization_plans_v2(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    library_id INTEGER NOT NULL,
    source_root_path TEXT NOT NULL,
    source_root_identity_json TEXT NOT NULL,
    source_path TEXT NOT NULL,
    source_relative_path TEXT NOT NULL,
    source_file_identity_json TEXT NOT NULL,
    source_fingerprint_blake3 TEXT NOT NULL,
    source_size INTEGER NOT NULL,
    source_modified_at INTEGER NOT NULL,
    -- Raw filesystem timestamp captured from the file handle; source_modified_at
    -- is the scan/catalog Unix-millisecond value and is a different unit.
    source_modified_filetime INTEGER NOT NULL DEFAULT 0,
    capture_time TEXT,
    classification_revision INTEGER NOT NULL,
    target_relative_path TEXT NOT NULL,
    target_path TEXT NOT NULL,
    target_path_key TEXT NOT NULL,
    conflict_decision TEXT NOT NULL,
    item_status TEXT NOT NULL,
    variables_json TEXT NOT NULL,
    issues_json TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    PRIMARY KEY(plan_id, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_organization_plan_items_v2_target
    ON organization_plan_items_v2(plan_id, target_path_key);

CREATE INDEX IF NOT EXISTS idx_organization_plan_items_v2_asset
    ON organization_plan_items_v2(asset_id);

CREATE TABLE IF NOT EXISTS copy_export_jobs (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL REFERENCES organization_plans_v2(id) ON DELETE RESTRICT,
    state TEXT NOT NULL CHECK(state IN (
        'queued', 'running', 'cancelling', 'cancelled', 'completed',
        'partial', 'failed', 'interrupted', 'reconcile_required', 'rolling_back', 'rolled_back'
    )),
    target_root_path TEXT NOT NULL,
    target_root_identity_json TEXT,
    protected_roots_json TEXT NOT NULL,
    progress_completed_items INTEGER NOT NULL DEFAULT 0,
    progress_total_items INTEGER NOT NULL DEFAULT 0,
    progress_completed_bytes INTEGER NOT NULL DEFAULT 0,
    progress_total_bytes INTEGER NOT NULL DEFAULT 0,
    current_item_ordinal INTEGER,
    cancel_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancel_requested IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_copy_export_jobs_recent
    ON copy_export_jobs(created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_copy_export_jobs_plan
    ON copy_export_jobs(plan_id, created_at DESC);

CREATE TABLE IF NOT EXISTS copy_export_job_items (
    job_id TEXT NOT NULL REFERENCES copy_export_jobs(id) ON DELETE RESTRICT,
    item_ordinal INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    source_path TEXT NOT NULL,
    source_file_identity_json TEXT NOT NULL,
    source_fingerprint_blake3 TEXT NOT NULL,
    target_path TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN (
        'pending', 'copying', 'copied', 'skipped', 'failed', 'cancelled',
        'partial', 'reconcile_required', 'rollback_pending', 'rolled_back', 'rollback_blocked'
    )),
    attempts INTEGER NOT NULL DEFAULT 0,
    partial_file_identity_json TEXT,
    output_file_identity_json TEXT,
    output_fingerprint_blake3 TEXT,
    output_size INTEGER,
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(job_id, item_ordinal)
);

CREATE INDEX IF NOT EXISTS idx_copy_export_job_items_state
    ON copy_export_job_items(job_id, state, item_ordinal);

CREATE TABLE IF NOT EXISTS file_operation_log (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES copy_export_jobs(id) ON DELETE RESTRICT,
    item_ordinal INTEGER,
    operation_kind TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('prepared', 'completed', 'failed', 'partial', 'blocked')),
    intent_json TEXT NOT NULL,
    result_json TEXT,
    created_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_file_operation_log_job
    ON file_operation_log(job_id, created_at, id);
