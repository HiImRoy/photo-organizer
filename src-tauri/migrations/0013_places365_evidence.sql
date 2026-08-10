CREATE TABLE IF NOT EXISTS semantic_evidence (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    model_name TEXT NOT NULL,
    model_version TEXT NOT NULL,
    analysis_version TEXT NOT NULL,
    taxonomy_version TEXT NOT NULL,
    source_fingerprint TEXT NOT NULL,
    rank INTEGER NOT NULL,
    label TEXT NOT NULL,
    display_name TEXT NOT NULL,
    similarity REAL NOT NULL,
    category_group TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    UNIQUE(asset_id, model_name, model_version, analysis_version, source_fingerprint, rank)
);

CREATE INDEX IF NOT EXISTS idx_semantic_evidence_current
    ON semantic_evidence(asset_id, model_name, model_version, analysis_version, taxonomy_version, source_fingerprint, rank);
