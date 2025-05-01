//!
//! Provides the main database structures (`BaseDB` and `Tree`).
//!
//! `BaseDB` manages multiple `Tree` instances and interacts with the storage `Backend`.
//! `Tree` represents a single, independent history of data entries, analogous to a table or branch.

use crate::backend::Backend;
use crate::data::KVOverWrite;
use crate::entry::ID;
use crate::tree::Tree;
use crate::{Error, Result};
use std::sync::{Arc, Mutex, MutexGuard};

/// Database implementation on top of the backend.
///
/// This database is the base DB, other 'overlays' or 'plugins' should be implemented on top of this.
/// It manages collections of related entries, called `Tree`s, and interacts with a
/// pluggable `Backend` for storage and retrieval.
/// Each `Tree` represents an independent history of data, identified by a root `Entry`.
pub struct BaseDB {
    /// The backend used by the database.
    backend: Arc<Mutex<Box<dyn Backend>>>,
    // Blob storage will be separate, maybe even just an extension
    // storage: IPFS;
}

impl BaseDB {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self {
            backend: Arc::new(Mutex::new(backend)),
        }
    }

    /// Get a reference to the backend
    pub fn backend(&self) -> &Arc<Mutex<Box<dyn Backend>>> {
        &self.backend
    }

    /// Helper function to lock the backend mutex.
    fn lock_backend(&self) -> Result<MutexGuard<'_, Box<dyn Backend>>> {
        self.backend.lock().map_err(|_| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to lock backend",
            ))
        })
    }

    /// Create a new tree in the database.
    ///
    /// A `Tree` represents a collection of related entries, analogous to a table.
    /// It is initialized with settings defined by a `KVOverWrite` CRDT.
    ///
    /// # Arguments
    /// * `settings` - The initial settings for the tree, typically including metadata like a name.
    ///
    /// # Returns
    /// A `Result` containing the newly created `Tree` or an error.
    pub fn new_tree(&self, settings: KVOverWrite) -> Result<Tree> {
        Tree::new(settings, Arc::clone(&self.backend))
    }

    /// Load an existing tree from the database by its root ID.
    ///
    /// # Arguments
    /// * `root_id` - The content-addressable ID of the root `Entry` of the tree to load.
    ///
    /// # Returns
    /// A `Result` containing the loaded `Tree` or an error if the root ID is not found.
    pub fn load_tree(&self, root_id: &ID) -> Result<Tree> {
        // First validate the root_id exists in the backend
        {
            let backend_guard = self.lock_backend()?;
            // Make sure the entry exists
            backend_guard.get(root_id)?;
        }

        // Create a tree object with the given root_id
        Tree::new_from_id(root_id.clone(), Arc::clone(&self.backend))
    }

    /// Load all trees stored in the backend.
    ///
    /// This retrieves all known root entry IDs from the backend and constructs
    /// `Tree` instances for each.
    ///
    /// # Returns
    /// A `Result` containing a vector of all `Tree` instances or an error.
    pub fn all_trees(&self) -> Result<Vec<Tree>> {
        let root_ids = {
            let backend_guard = self.lock_backend()?;
            backend_guard.all_roots()?
        };
        let mut trees = Vec::new();

        for root_id in root_ids {
            trees.push(Tree::new_from_id(
                root_id.clone(),
                Arc::clone(&self.backend),
            )?);
        }

        Ok(trees)
    }

    /// Find trees by their assigned name.
    ///
    /// Searches through all trees in the database and returns those whose "name"
    /// setting matches the provided name.
    ///
    /// # Arguments
    /// * `name` - The name to search for.
    ///
    /// # Returns
    /// A `Result` containing a vector of `Tree` instances whose name matches,
    /// or an error.
    ///
    /// # Errors
    /// Returns `Error::NotFound` if no trees with the specified name are found.
    pub fn find_tree(&self, name: &str) -> Result<Vec<Tree>> {
        let all_trees = self.all_trees()?;
        let mut matching_trees = Vec::new();

        for tree in all_trees {
            // Attempt to get the name from the tree's settings
            if let Ok(tree_name) = tree.get_name() {
                if tree_name == name {
                    matching_trees.push(tree);
                }
            }
            // Ignore trees where getting the name fails or doesn't match
        }

        if matching_trees.is_empty() {
            Err(Error::NotFound)
        } else {
            Ok(matching_trees)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    use crate::data::KVOverWrite;

    #[test]
    fn test_new_db_and_tree() {
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);
        let settings = KVOverWrite::new();
        let tree_result = db.new_tree(settings);
        assert!(tree_result.is_ok());
    }

    #[test]
    fn test_load_tree() {
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);
        let settings = KVOverWrite::new();
        let tree = db.new_tree(settings).expect("Failed to create tree");
        let root_id = tree.root_id().clone();

        // Drop the original tree instance
        drop(tree);

        // Create a new DB instance with the same backend (or reuse db)
        let loaded_tree_result = db.load_tree(&root_id);
        assert!(loaded_tree_result.is_ok());
        let loaded_tree = loaded_tree_result.unwrap();
        assert_eq!(loaded_tree.root_id(), &root_id);
    }

    #[test]
    fn test_all_trees() {
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);

        let settings1 = KVOverWrite::new();
        let tree1 = db.new_tree(settings1).expect("Failed to create tree 1");
        let root_id1 = tree1.root_id().clone();

        let mut settings2 = KVOverWrite::new();
        settings2.set("name".to_string(), "Tree2".to_string());
        let tree2 = db.new_tree(settings2).expect("Failed to create tree 2");
        let root_id2 = tree2.root_id().clone();

        let trees = db.all_trees().expect("Failed to get all trees");
        assert_eq!(trees.len(), 2);

        let found_ids: Vec<String> = trees.iter().map(|t| t.root_id().clone()).collect();
        assert!(found_ids.contains(&root_id1));
        assert!(found_ids.contains(&root_id2));
    }

    #[test]
    fn test_find_tree() {
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);

        // Tree 1: No name
        let settings1 = KVOverWrite::new();
        db.new_tree(settings1).expect("Failed to create tree 1");

        // Tree 2: Name "Tree2"
        let mut settings2 = KVOverWrite::new();
        settings2.set("name".to_string(), "Tree2".to_string());
        db.new_tree(settings2).expect("Failed to create tree 2");

        // Tree 3: Name "Tree3"
        let mut settings3 = KVOverWrite::new();
        settings3.set("name".to_string(), "Tree3".to_string());
        db.new_tree(settings3).expect("Failed to create tree 3");

        // Tree 4: Name "Tree3" (duplicate name)
        let mut settings4 = KVOverWrite::new();
        settings4.set("name".to_string(), "Tree3".to_string());
        db.new_tree(settings4).expect("Failed to create tree 4");

        // Test: Find non-existent name
        let found_none_result = db.find_tree("NonExistent");
        assert!(matches!(found_none_result, Err(Error::NotFound)));

        // Test: Find unique name
        let found_tree2 = db.find_tree("Tree2").expect("find_tree failed");
        assert_eq!(found_tree2.len(), 1);
        assert_eq!(found_tree2[0].get_name().unwrap(), "Tree2");

        // Test: Find duplicate name
        let found_tree3 = db.find_tree("Tree3").expect("find_tree failed");
        assert_eq!(found_tree3.len(), 2);
        // Check if both found trees have the name "Tree3"
        assert!(found_tree3.iter().all(|t| t.get_name().unwrap() == "Tree3"));

        // Test: Find when no trees exist
        let empty_backend = Box::new(InMemoryBackend::new());
        let empty_db = BaseDB::new(empty_backend);
        let found_empty_result = empty_db.find_tree("AnyName");
        assert!(matches!(found_empty_result, Err(Error::NotFound)));
    }

    #[test]
    fn test_get_backend() {
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);
        let retrieved_backend = db.backend();
        assert!(retrieved_backend.lock().unwrap().all_roots().is_ok());
    }
}
