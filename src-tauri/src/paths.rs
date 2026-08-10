use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
    pub thumbnail_dir: PathBuf,
    pub preview_dir: PathBuf,
    pub log_dir: PathBuf,
    pub semantic_model_dir: PathBuf,
    pub tinyclip_model_dir: PathBuf,
    pub onnx_runtime_path: PathBuf,
}

impl AppPaths {
    pub fn initialize(data_dir: impl AsRef<Path>) -> AppResult<Self> {
        Self::initialize_with_resources(
            data_dir,
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources"),
        )
    }

    pub fn initialize_with_resources(
        data_dir: impl AsRef<Path>,
        resource_dir: impl AsRef<Path>,
    ) -> AppResult<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let thumbnail_dir = data_dir.join("thumbnails");
        let preview_dir = data_dir.join("previews");
        let log_dir = data_dir.join("logs");
        let resource_dir = resource_dir.as_ref();
        let packaged_root = if resource_dir.join("resources").is_dir() {
            resource_dir.join("resources")
        } else {
            resource_dir.to_path_buf()
        };
        let development_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        let resource_root = if packaged_root
            .join("models")
            .join("places365-resnet18")
            .join(crate::semantic::MODEL_FILE)
            .is_file()
            || packaged_root
                .join("models")
                .join("tinyclip-vit-8m-16-text-3m-yfcc15m")
                .join(crate::semantic::TINYCLIP_MODEL_FILE)
                .is_file()
        {
            packaged_root
        } else {
            development_root
        };

        fs::create_dir_all(&thumbnail_dir)?;
        fs::create_dir_all(&preview_dir)?;
        fs::create_dir_all(&log_dir)?;

        Ok(Self {
            database_path: data_dir.join("photo-organizer.sqlite3"),
            data_dir,
            thumbnail_dir,
            preview_dir,
            log_dir,
            semantic_model_dir: resource_root.join("models").join("places365-resnet18"),
            tinyclip_model_dir: resource_root
                .join("models")
                .join("tinyclip-vit-8m-16-text-3m-yfcc15m"),
            onnx_runtime_path: resource_root.join("runtime").join("onnxruntime.dll"),
        })
    }
}
