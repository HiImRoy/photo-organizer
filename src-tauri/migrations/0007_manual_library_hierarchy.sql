ALTER TABLE libraries
ADD COLUMN parent_relation TEXT NOT NULL DEFAULT 'source'
CHECK (parent_relation IN ('source', 'manual'));

CREATE INDEX IF NOT EXISTS idx_libraries_parent_relation
    ON libraries(parent_relation, parent_library_id);
