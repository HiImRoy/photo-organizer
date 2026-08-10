CREATE TABLE IF NOT EXISTS subject_analysis_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    source_fingerprint TEXT NOT NULL,
    model_name TEXT NOT NULL,
    model_version TEXT NOT NULL,
    analysis_version TEXT NOT NULL,
    taxonomy_version TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('completed', 'failed')),
    error_message TEXT,
    analyzed_at TEXT NOT NULL,
    UNIQUE(
        asset_id,
        source_fingerprint,
        model_name,
        model_version,
        analysis_version,
        taxonomy_version
    )
);

CREATE INDEX IF NOT EXISTS idx_subject_analysis_runs_asset
    ON subject_analysis_runs(asset_id, status, analyzed_at DESC);

CREATE TABLE IF NOT EXISTS subject_labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    display_name TEXT NOT NULL,
    similarity REAL NOT NULL,
    threshold REAL NOT NULL,
    model_name TEXT NOT NULL,
    model_version TEXT NOT NULL,
    analysis_version TEXT NOT NULL,
    taxonomy_version TEXT NOT NULL,
    source_fingerprint TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    UNIQUE(
        asset_id,
        label,
        source_fingerprint,
        model_name,
        model_version,
        analysis_version,
        taxonomy_version
    )
);

CREATE INDEX IF NOT EXISTS idx_subject_labels_asset
    ON subject_labels(asset_id, similarity DESC, label ASC);
CREATE INDEX IF NOT EXISTS idx_subject_labels_filter
    ON subject_labels(label, source_fingerprint, model_name, model_version, analysis_version, taxonomy_version);
