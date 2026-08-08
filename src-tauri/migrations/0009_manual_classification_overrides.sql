ALTER TABLE assets ADD COLUMN classification_revision INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS manual_classification_overrides (
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    field TEXT NOT NULL CHECK (
        field IN (
            'primary_category',
            'tone',
            'dominant_color_category',
            'saturation_level'
        )
    ),
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (asset_id, field)
);

CREATE TABLE IF NOT EXISTS manual_tag_overrides (
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('add', 'remove')),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (asset_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_manual_classification_overrides_asset
    ON manual_classification_overrides(asset_id);

CREATE INDEX IF NOT EXISTS idx_manual_tag_overrides_asset
    ON manual_tag_overrides(asset_id);
