ALTER TABLE assets ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1));

CREATE INDEX IF NOT EXISTS idx_assets_library_favorite
    ON assets(library_id, is_favorite, id);

CREATE TABLE IF NOT EXISTS collections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS collection_assets (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL,
    PRIMARY KEY(collection_id, asset_id)
);

CREATE INDEX IF NOT EXISTS idx_collection_assets_asset
    ON collection_assets(asset_id, collection_id);

CREATE TABLE IF NOT EXISTS saved_views (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    library_id INTEGER REFERENCES libraries(id) ON DELETE CASCADE,
    query_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS edit_export_plans (
    id TEXT PRIMARY KEY,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    source_fingerprint TEXT NOT NULL,
    target_path TEXT NOT NULL,
    recipe_json TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    executed_at TEXT,
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS face_detections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    source_fingerprint TEXT NOT NULL,
    bounds_json TEXT NOT NULL,
    confidence REAL NOT NULL,
    model_name TEXT NOT NULL,
    model_version TEXT NOT NULL,
    embedding_blob BLOB,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_face_detections_asset
    ON face_detections(asset_id, source_fingerprint);

CREATE TABLE IF NOT EXISTS face_clusters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name TEXT,
    representative_face_id INTEGER REFERENCES face_detections(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS face_cluster_members (
    cluster_id INTEGER NOT NULL REFERENCES face_clusters(id) ON DELETE CASCADE,
    face_id INTEGER NOT NULL REFERENCES face_detections(id) ON DELETE CASCADE,
    similarity REAL NOT NULL,
    PRIMARY KEY(cluster_id, face_id)
);

CREATE TABLE IF NOT EXISTS workflow_preferences (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO workflow_preferences(key, value_json, updated_at)
VALUES('face_analysis_enabled', 'false', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
