pub mod db;
pub mod error;
pub mod git;
pub mod models;
pub mod preview;
pub mod provider;
pub mod ssh;
pub mod update;

pub use error::{GitcraftError, Result};
pub use models::*;

// Re-export so the UI layer (gitcraft-app) can name the Connection type
// without taking a second rusqlite dependency / duplicate SQLite build.
pub use rusqlite;
