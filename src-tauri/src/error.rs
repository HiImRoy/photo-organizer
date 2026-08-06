use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("image decode error: {0}")]
    Image(#[from] image::ImageError),
    #[error("directory traversal error: {0}")]
    WalkDir(#[from] walkdir::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("the selected path is not a readable directory: {0}")]
    InvalidRoot(PathBuf),
    #[error("record not found: {0}")]
    NotFound(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("security boundary rejected path: {0}")]
    UnsafePath(PathBuf),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

pub type AppResult<T> = Result<T, AppError>;
