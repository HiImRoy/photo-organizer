ALTER TABLE assets ADD COLUMN rating INTEGER NOT NULL DEFAULT 0 CHECK (rating BETWEEN 0 AND 5);

ALTER TABLE assets ADD COLUMN color_label TEXT CHECK (
    color_label IS NULL OR color_label IN ('red', 'yellow', 'green', 'blue', 'purple')
);

CREATE INDEX IF NOT EXISTS idx_assets_rating ON assets(rating, id);
CREATE INDEX IF NOT EXISTS idx_assets_color_label ON assets(color_label, id);
