ALTER TABLE assets ADD COLUMN asset_identity_key TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_assets_library_identity
    ON assets(library_id, asset_identity_key);
