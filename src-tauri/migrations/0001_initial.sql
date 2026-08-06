CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    root_path TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    last_scan_at TEXT,
    status TEXT NOT NULL DEFAULT 'ready',
    last_error TEXT,
    scan_generation INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    absolute_path TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    extension TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    fingerprint TEXT NOT NULL,
    width INTEGER,
    height INTEGER,
    orientation INTEGER,
    capture_time TEXT,
    camera_make TEXT,
    camera_model TEXT,
    lens_model TEXT,
    exposure_time TEXT,
    aperture REAL,
    iso INTEGER,
    focal_length REAL,
    file_status TEXT NOT NULL DEFAULT 'present',
    scan_status TEXT NOT NULL DEFAULT 'pending',
    analysis_status TEXT NOT NULL DEFAULT 'pending',
    error_message TEXT,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    last_seen_scan INTEGER NOT NULL DEFAULT 0,
    UNIQUE (library_id, absolute_path)
);

CREATE INDEX IF NOT EXISTS idx_assets_library_status ON assets(library_id, file_status);
CREATE INDEX IF NOT EXISTS idx_assets_library_name ON assets(library_id, file_name, id);
CREATE INDEX IF NOT EXISTS idx_assets_library_capture ON assets(library_id, capture_time, id);
CREATE INDEX IF NOT EXISTS idx_assets_library_modified ON assets(library_id, modified_at, id);
CREATE INDEX IF NOT EXISTS idx_assets_library_relative ON assets(library_id, relative_path);

CREATE TABLE IF NOT EXISTS thumbnails (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    cache_path TEXT NOT NULL,
    spec TEXT NOT NULL,
    source_modified_at INTEGER NOT NULL,
    source_size INTEGER NOT NULL,
    status TEXT NOT NULL,
    error_message TEXT,
    updated_at TEXT NOT NULL,
    UNIQUE(asset_id, spec)
);

CREATE TABLE IF NOT EXISTS tone_features (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    brightness_mean REAL,
    brightness_median REAL,
    brightness_low_percentile REAL,
    brightness_high_percentile REAL,
    shadow_ratio REAL,
    highlight_ratio REAL,
    contrast REAL,
    dynamic_range REAL,
    tone_label TEXT,
    exposure_label TEXT,
    contrast_label TEXT,
    algorithm_version TEXT NOT NULL,
    analyzed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tone_brightness ON tone_features(brightness_mean, asset_id);
CREATE INDEX IF NOT EXISTS idx_tone_contrast ON tone_features(contrast, asset_id);

CREATE TABLE IF NOT EXISTS color_features (
    asset_id INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    saturation_mean REAL,
    saturation_median REAL,
    dominant_color_rgb TEXT,
    dominant_color_category TEXT,
    dominant_colors_json TEXT,
    hue_histogram_json TEXT,
    warmth_score REAL,
    neutral_ratio REAL,
    colorfulness REAL,
    monochrome_probability REAL,
    saturation_label TEXT,
    algorithm_version TEXT NOT NULL,
    analyzed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_color_saturation ON color_features(saturation_mean, asset_id);
CREATE INDEX IF NOT EXISTS idx_color_category ON color_features(dominant_color_category, asset_id);
CREATE INDEX IF NOT EXISTS idx_color_warmth ON color_features(warmth_score, asset_id);

CREATE TABLE IF NOT EXISTS semantic_labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    similarity REAL NOT NULL,
    model_name TEXT NOT NULL,
    model_version TEXT NOT NULL,
    analysis_version TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    is_manual INTEGER NOT NULL DEFAULT 0,
    is_excluded INTEGER NOT NULL DEFAULT 0,
    UNIQUE(asset_id, label, model_name, model_version, analysis_version)
);

CREATE INDEX IF NOT EXISTS idx_semantic_lookup ON semantic_labels(label, similarity DESC, asset_id);

CREATE TABLE IF NOT EXISTS analysis_jobs (
    id TEXT PRIMARY KEY,
    library_id INTEGER REFERENCES libraries(id) ON DELETE CASCADE,
    job_type TEXT NOT NULL,
    status TEXT NOT NULL,
    progress_current INTEGER NOT NULL DEFAULT 0,
    progress_total INTEGER NOT NULL DEFAULT 0,
    execution_backend TEXT,
    model_name TEXT,
    model_version TEXT,
    analysis_version TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_jobs_library_status ON analysis_jobs(library_id, status, updated_at);

CREATE TABLE IF NOT EXISTS file_operation_jobs (
    id TEXT PRIMARY KEY,
    library_id INTEGER REFERENCES libraries(id) ON DELETE SET NULL,
    operation_type TEXT NOT NULL,
    status TEXT NOT NULL,
    dry_run INTEGER NOT NULL DEFAULT 1,
    conflict_strategy TEXT NOT NULL DEFAULT 'skip',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS file_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL REFERENCES file_operation_jobs(id) ON DELETE CASCADE,
    source_path TEXT NOT NULL,
    target_path TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    plan_status TEXT NOT NULL,
    execution_status TEXT NOT NULL,
    conflict_strategy TEXT NOT NULL,
    source_hash TEXT,
    target_hash TEXT,
    error_message TEXT,
    rollback_status TEXT NOT NULL DEFAULT 'not_requested',
    UNIQUE(job_id, target_path)
);

CREATE INDEX IF NOT EXISTS idx_file_operations_job ON file_operations(job_id, execution_status);
