//! Tree module provides functionality for managing collections of related entries.
//!
//! A `Tree` represents a hierarchical structure of entries, similar to a table in a database
//! or a branch in a version control system. Each tree has a root entry and maintains
//! the history and relationships between entries, interfacing with a backend storage system.

use crate::backend::Backend;
use crate::data::{KVOverWrite, CRDT};
use crate::entry::{Entry, ID};
use crate::Result;
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// Represents a collection of related entries, analogous to a table or a branch in a version control system.
///
/// Each `Tree` is identified by the ID of its root `Entry` and manages the history of data
/// associated with that root. It interacts with the underlying `Backend` for storage.
pub struct Tree {
    root: ID,
    backend: Arc<Mutex<Box<dyn Backend>>>,
}

impl Tree {
    /// Creates a new `Tree` instance.
    ///
    /// Initializes the tree by creating a root `Entry` containing the provided settings
    /// and storing it in the backend.
    ///
    /// # Arguments
    /// * `settings` - A `KVOverWrite` CRDT containing the initial settings for the tree.
    /// * `backend` - An `Arc<Mutex<>>` protected reference to the backend where the tree's entries will be stored.
    ///
    /// # Returns
    /// A `Result` containing the new `Tree` instance or an error.
    pub fn new(settings: KVOverWrite, backend: Arc<Mutex<Box<dyn Backend>>>) -> Result<Self> {
        // Create a root entry for this tree
        let entry = Entry::new_top_level(serde_json::to_string(&settings)?);

        let root_id = entry.id();

        // Insert the entry into the backend
        {
            // Lock the backend using the provided Arc<Mutex> directly
            let mut backend_guard = backend.lock().map_err(|_| {
                crate::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to lock backend in Tree::new",
                ))
            })?;
            backend_guard.put(entry)?;
        }

        Ok(Self {
            root: root_id,
            backend,
        })
    }

    /// Creates a new `Tree` instance from an existing ID.
    ///
    /// This constructor takes an existing `ID` and an `Arc<Mutex<Box<dyn Backend>>>`
    /// and constructs a `Tree` instance with the specified root ID.
    ///
    /// # Arguments
    /// * `id` - The `ID` of the root entry.
    /// * `backend` - An `Arc<Mutex<Box<dyn Backend>>>` protected reference to the backend where the tree's entries will be stored.
    ///
    /// # Returns
    /// A `Result` containing the new `Tree` instance or an error.
    pub(crate) fn new_from_id(id: ID, backend: Arc<Mutex<Box<dyn Backend>>>) -> Result<Self> {
        Ok(Self { root: id, backend })
    }

    /// Helper function to lock the backend mutex.
    fn lock_backend(&self) -> Result<MutexGuard<'_, Box<dyn Backend>>> {
        self.backend.lock().map_err(|_| {
            crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to lock backend",
            ))
        })
    }

    /// Get the ID of the root entry
    pub fn root_id(&self) -> &ID {
        &self.root
    }

    /// Get a reference to the backend
    pub fn backend(&self) -> &Arc<Mutex<Box<dyn Backend>>> {
        &self.backend
    }

    /// Retrieve the root entry from the backend
    pub fn get_root(&self) -> Result<Entry> {
        let backend_guard = self.lock_backend()?;
        backend_guard.get(&self.root).cloned()
    }

    /// Get the name of the tree from its root entry's data
    pub fn get_name(&self) -> Result<String> {
        let root_entry = self.get_root()?;
        let data_map: HashMap<String, String> = serde_json::from_str(&root_entry.get_settings()?)?;
        data_map.get("name").cloned().ok_or(crate::Error::NotFound)
    }

    /// Insert an entry into the tree, automatically managing parent references.
    ///
    /// This method takes an `Entry`, sets its root ID to this tree's root,
    /// determines the current tips (leaf entries) of the main tree and relevant subtrees
    /// using the backend, sets these tips as the parents of the new entry, calculates the entry's ID,
    /// and stores it in the backend.
    ///
    /// The provided entry should primarily contain the user data in its `tree.data` and `subtrees` fields.
    /// The `root`, `parents`, and `subtree_parents` fields will be overwritten.
    ///
    /// # Arguments
    /// * `entry` - The `Entry` to insert, containing the data to be added.
    ///
    /// # Returns
    /// A `Result` containing the content-addressable ID of the newly inserted `Entry` or an error.
    pub fn insert(&self, mut entry: Entry) -> Result<ID> {
        entry.set_root(self.root.clone());
        let id: ID;
        {
            let mut backend_guard = self.lock_backend()?;

            // Calculate all the tips based on what we know locally
            let tips = backend_guard.get_tips(&self.root).unwrap_or_default();

            // If there are no tips, use the root ID as parent
            if tips.is_empty() {
                entry.set_parents(vec![self.root.clone()]);
            } else {
                entry.set_parents(tips);
            }

            // Update subtrees with their tips
            if let Ok(subtrees) = entry.subtrees() {
                for subtree in &subtrees {
                    let subtree_tips = backend_guard
                        .get_subtree_tips(&self.root, subtree)
                        .unwrap_or_default();
                    entry.set_subtree_parents(subtree, subtree_tips);
                }
            }

            id = entry.id();
            backend_guard.put(entry)?;
        }
        Ok(id)
    }

    /// Insert an entry into the tree without modifying it.
    /// This is primarily for testing purposes or when you need full control over the entry.
    pub fn insert_raw(&self, entry: Entry) -> Result<ID> {
        let id = entry.id();

        let mut backend_guard = self.lock_backend()?;
        backend_guard.put(entry)?;

        Ok(id)
    }

    /// Get the current tips (leaf entries) of the main tree branch.
    ///
    /// Tips represent the latest entries in the tree's main history, forming the heads of the DAG.
    ///
    /// # Returns
    /// A `Result` containing a vector of `ID`s for the tip entries or an error.
    pub fn get_tips(&self) -> Result<Vec<ID>> {
        let backend_guard = self.lock_backend()?;
        backend_guard.get_tips(&self.root)
    }

    /// Get the full `Entry` objects for the current tips of the main tree branch.
    ///
    /// # Returns
    /// A `Result` containing a vector of the tip `Entry` objects or an error.
    pub fn get_tip_entries(&self) -> Result<Vec<Entry>> {
        let backend_guard = self.lock_backend()?;
        let tips = backend_guard.get_tips(&self.root)?;
        let entries: Result<Vec<_>> = tips
            .iter()
            .map(|id| backend_guard.get(id).cloned())
            .collect();
        entries
    }

    /// Get the merged settings for the tree.
    ///
    /// This retrieves all entries in the main tree history from the backend,
    /// deserializes the settings data from each entry's `tree.data` field into a `KVOverWrite` CRDT,
    /// and merges them according to CRDT rules to produce the final, consolidated settings.
    ///
    /// # Returns
    /// A `Result` containing the merged `KVOverWrite` settings or an error.
    pub fn get_settings(&self) -> Result<KVOverWrite> {
        let all_entries = {
            let backend_guard = self.lock_backend()?;
            backend_guard.get_tree(&self.root)?
        };
        let mut settings = KVOverWrite::default();
        for entry in all_entries {
            let entry_settings: KVOverWrite = serde_json::from_str(&entry.get_settings()?)?;
            settings = settings.merge(&entry_settings)?;
        }

        Ok(settings)
    }

    /// Get the current tips (leaf entries) for a specific subtree within this tree.
    ///
    /// Subtrees represent separate, named histories within the main tree.
    ///
    /// # Arguments
    /// * `subtree` - The name of the subtree.
    ///
    /// # Returns
    /// A `Result` containing a vector of `ID`s for the tip entries of the specified subtree or an error.
    pub fn get_subtree_tips(&self, subtree: &str) -> Result<Vec<ID>> {
        let backend_guard = self.lock_backend()?;
        backend_guard.get_subtree_tips(&self.root, subtree)
    }

    /// Get the full `Entry` objects for the current tips of a specific subtree.
    ///
    /// # Arguments
    /// * `subtree` - The name of the subtree.
    ///
    /// # Returns
    /// A `Result` containing a vector of the tip `Entry` objects for the specified subtree or an error.
    pub fn get_subtree_tip_entries(&self, subtree: &str) -> Result<Vec<Entry>> {
        let backend_guard = self.lock_backend()?;
        let tips = backend_guard.get_subtree_tips(&self.root, subtree)?;
        let entries: Result<Vec<_>> = tips
            .iter()
            .map(|id| backend_guard.get(id).cloned())
            .collect();
        entries
    }

    /// Get the merged data for a specific subtree, interpreted as a specific CRDT type.
    ///
    /// This retrieves all entries belonging to the specified subtree history from the backend,
    /// deserializes the data from each entry's corresponding `SubTreeNode` into the specified CRDT type `T`,
    /// and merges them according to the `CRDT` trait implementation for `T` to produce the final,
    /// consolidated data state for the subtree.
    ///
    /// # Type Parameters
    /// * `T` - The CRDT type to deserialize and merge the subtree data into. Must implement `CRDT` and `Default`.
    ///
    /// # Arguments
    /// * `subtree` - The name of the subtree.
    ///
    /// # Returns
    /// A `Result` containing the merged data of type `T` or an error.
    pub fn get_subtree_data<T>(&self, subtree: &str) -> Result<T>
    where
        T: CRDT,
    {
        let all_entries = {
            let backend_guard = self.lock_backend()?;
            backend_guard.get_subtree(&self.root, subtree)?
        };

        let mut settings = T::default();
        for entry in all_entries {
            let entry_settings: T = serde_json::from_str(entry.data(subtree)?)?;
            settings = settings.merge(&entry_settings)?;
        }

        Ok(settings)
    }
}
