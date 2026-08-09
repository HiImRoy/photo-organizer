CREATE TABLE IF NOT EXISTS asset_library_assignments (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    assigned_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_asset_library_assignments_library
    ON asset_library_assignments(library_id, asset_id);
