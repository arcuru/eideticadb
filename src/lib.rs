pub mod backend;
pub mod basedb;
pub mod entry;
// mod id;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Entry not found")]
    NotFound,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
