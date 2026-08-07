ALTER TABLE color_features ADD COLUMN dominant_color_coverage REAL;
ALTER TABLE color_features ADD COLUMN chroma_mean REAL;

CREATE INDEX IF NOT EXISTS idx_color_coverage
    ON color_features(dominant_color_coverage, asset_id);
