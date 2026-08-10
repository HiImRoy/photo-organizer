ALTER TABLE semantic_labels ADD COLUMN category_group TEXT NOT NULL DEFAULT 'legacy';
ALTER TABLE semantic_labels ADD COLUMN taxonomy_version TEXT NOT NULL DEFAULT 'legacy-v1';

CREATE INDEX IF NOT EXISTS idx_semantic_labels_current_taxonomy
    ON semantic_labels(asset_id, model_name, model_version, analysis_version, taxonomy_version, source_fingerprint);
