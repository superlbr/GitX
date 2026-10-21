use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitxError {
    #[error("git command failed: {0}")]
    Git(String),
    #[error("not a git repository: {0}")]
    NotARepo(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("db error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("utf8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, GitxError>;
