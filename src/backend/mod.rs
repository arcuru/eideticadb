use crate::entry::{Entry, ID};
use crate::Result;
use std::any::Any;

mod in_memory;

pub use in_memory::InMemoryBackend;

/// Backend trait abstracting the underlying storage.
pub trait Backend: Send + Sync + Any {
    fn get(&self, id: &ID) -> Result<&Entry>;
    fn put(&mut self, entry: Entry) -> Result<()>;

    /// Get the tips of a tree.
    /// The tips are defined as the set of all entries in the given tree with no children.
    fn get_tips(&self, tree: &ID) -> Result<Vec<ID>>;

    /// Get the tips of a subtree.
    fn get_subtree_tips(&self, tree: &ID, subtree: &str) -> Result<Vec<ID>>;

    /// Get all top-level root entry IDs stored in the backend.
    /// Top-level roots are Entries where tree.root is an empty string.
    fn all_roots(&self) -> Result<Vec<ID>>;

    /// Get a reference to self as Any
    fn as_any(&self) -> &dyn Any;
}
