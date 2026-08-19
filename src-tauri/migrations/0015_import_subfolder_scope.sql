ALTER TABLE libraries
ADD COLUMN include_subfolder_images INTEGER NOT NULL DEFAULT 1
CHECK (include_subfolder_images IN (0, 1));

CREATE INDEX IF NOT EXISTS idx_libraries_subfolder_scope
    ON libraries(include_subfolder_images, id);
