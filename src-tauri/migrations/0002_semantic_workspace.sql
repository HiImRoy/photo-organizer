CREATE TABLE IF NOT EXISTS semantic_models (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    analysis_version TEXT NOT NULL,
    license TEXT NOT NULL,
    source_url TEXT NOT NULL,
    model_sha256 TEXT NOT NULL,
    tokenizer_sha256 TEXT NOT NULL,
    model_path TEXT NOT NULL,
    tokenizer_path TEXT NOT NULL,
    execution_backend TEXT NOT NULL DEFAULT 'cpu',
    installed_at TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),
    UNIQUE(name, version, analysis_version)
);

ALTER TABLE assets ADD COLUMN semantic_status TEXT NOT NULL DEFAULT 'not_analyzed';
ALTER TABLE assets ADD COLUMN semantic_error TEXT;
ALTER TABLE assets ADD COLUMN semantic_analyzed_at TEXT;

ALTER TABLE semantic_labels ADD COLUMN display_name TEXT NOT NULL DEFAULT '';
ALTER TABLE semantic_labels ADD COLUMN threshold REAL NOT NULL DEFAULT 0.0;
ALTER TABLE semantic_labels ADD COLUMN source_fingerprint TEXT NOT NULL DEFAULT '';
ALTER TABLE semantic_labels ADD COLUMN is_primary INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0, 1));

CREATE TABLE IF NOT EXISTS semantic_embeddings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL,
    model_name TEXT NOT NULL,
    model_version TEXT NOT NULL,
    analysis_version TEXT NOT NULL,
    source_fingerprint TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    vector_blob BLOB NOT NULL,
    generated_at TEXT NOT NULL,
    FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    UNIQUE(asset_id, model_name, model_version, analysis_version, source_fingerprint)
);

ALTER TABLE analysis_jobs ADD COLUMN completed_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE analysis_jobs ADD COLUMN failed_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE analysis_jobs ADD COLUMN skipped_count INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS analysis_job_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL,
    asset_id INTEGER NOT NULL,
    source_fingerprint TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    attempts INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(job_id) REFERENCES analysis_jobs(id) ON DELETE CASCADE,
    FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    UNIQUE(job_id, asset_id)
);

CREATE INDEX IF NOT EXISTS idx_assets_library_semantic_status
    ON assets(library_id, semantic_status, file_status);
CREATE INDEX IF NOT EXISTS idx_assets_library_capture_time
    ON assets(library_id, capture_time, file_status);
CREATE INDEX IF NOT EXISTS idx_semantic_labels_current
    ON semantic_labels(asset_id, label, source_fingerprint, model_name, model_version, analysis_version);
CREATE INDEX IF NOT EXISTS idx_semantic_labels_primary
    ON semantic_labels(label, is_primary, asset_id);
CREATE INDEX IF NOT EXISTS idx_tone_features_label
    ON tone_features(tone_label, asset_id);
CREATE INDEX IF NOT EXISTS idx_color_features_category
    ON color_features(dominant_color_category, asset_id);
CREATE INDEX IF NOT EXISTS idx_analysis_job_items_status
    ON analysis_job_items(job_id, status);
