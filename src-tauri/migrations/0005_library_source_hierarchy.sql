ALTER TABLE libraries ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE libraries ADD COLUMN source_path TEXT NOT NULL DEFAULT '';
ALTER TABLE libraries ADD COLUMN source_identity_key TEXT NOT NULL DEFAULT '';
ALTER TABLE libraries ADD COLUMN parent_library_id INTEGER REFERENCES libraries(id) ON DELETE SET NULL;
ALTER TABLE libraries ADD COLUMN display_order INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_libraries_parent
    ON libraries(parent_library_id, display_order, id);
