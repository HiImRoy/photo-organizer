CREATE TABLE IF NOT EXISTS organization_plans (
    id TEXT PRIMARY KEY,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    target_root TEXT NOT NULL,
    scope_json TEXT NOT NULL,
    rules_json TEXT NOT NULL,
    summary_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'preview',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_organization_plans_library
    ON organization_plans(library_id, updated_at DESC);

-- These rows are a compact audit snapshot only. The full mapping is deliberately
-- recomputed on demand and is not used as an execution queue.
CREATE TABLE IF NOT EXISTS organization_plan_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id TEXT NOT NULL REFERENCES organization_plans(id) ON DELETE CASCADE,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    source_fingerprint TEXT NOT NULL,
    target_relative_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    ordinal INTEGER NOT NULL,
    status TEXT NOT NULL,
    UNIQUE(plan_id, asset_id)
);

CREATE INDEX IF NOT EXISTS idx_organization_plan_items_plan
    ON organization_plan_items(plan_id, ordinal);

CREATE TABLE IF NOT EXISTS organization_plan_issues (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id TEXT NOT NULL REFERENCES organization_plans(id) ON DELETE CASCADE,
    item_id INTEGER REFERENCES organization_plan_items(id) ON DELETE CASCADE,
    code TEXT NOT NULL,
    severity TEXT NOT NULL,
    source_path TEXT,
    target_path TEXT,
    detail TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_organization_plan_issues_plan
    ON organization_plan_issues(plan_id, severity, code);
