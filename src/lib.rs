pub mod backend;
pub mod basedb;
pub mod data;
// pub mod crdt; // Removed as CRDT logic is now handled externally via RawData
pub mod entry;
// pub mod error; // Error type is defined below, no need for a separate module

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Entry not found")]
    NotFound,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}
