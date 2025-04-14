//!
//! Provides the main database structures (`BaseDB` and `Tree`).
//!
//! `BaseDB` manages multiple `Tree` instances and interacts with the storage `Backend`.
//! `Tree` represents a single, independent history of data entries, analogous to a table or branch.

use crate::backend::Backend;
use crate::data::{KVOverWrite, CRDT};
use crate::entry::{Entry, ID};
use crate::Result;
use serde_json;
use std::collections::HashMap;
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
            crate::Error::Io(std::io::Error::new(
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
        Ok(Tree {
            root: root_id.clone(),
            backend: self.backend.clone(),
        })
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
            trees.push(Tree {
                root: root_id.clone(),
                backend: self.backend.clone(),
            });
        }

        Ok(trees)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;

    use std::collections::HashMap;

    #[test]
    fn test_create_basedb() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let _db = BaseDB::new(backend);
    }

    #[test]
    fn test_tree_creation() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create a new tree with some settings
        let mut settings = HashMap::new();
        settings.insert("name".to_string(), "test_tree".to_string());
        settings.insert("description".to_string(), "A test tree".to_string());

        let tree = db
            .new_tree(KVOverWrite::from_hashmap(settings))
            .expect("Failed to create tree");

        // Verify that tree was created and has a valid root ID
        let root_id = tree.root_id();
        assert!(!root_id.is_empty(), "Root ID should not be empty");
    }

    #[test]
    fn test_tree_root_data() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create settings as a HashMap and serialize to JSON RawData
        let mut settings_map = HashMap::new();
        settings_map.insert("name".to_string(), "test_tree".to_string());
        settings_map.insert("description".to_string(), "A test tree".to_string());

        let settings = KVOverWrite::from_hashmap(settings_map);
        let settings_json = serde_json::to_string(&settings).unwrap();

        let tree = db.new_tree(settings).expect("Failed to create tree");

        // Retrieve the root entry
        let root_entry = tree.get_root().expect("Failed to get root entry");

        // Verify that the root entry has the correct data (as a JSON string)
        assert_eq!(
            &root_entry.get_settings().unwrap(),
            &settings_json,
            "Root entry should contain the settings as a JSON string"
        );

        // Verify that the root entry has the "root" subtree
        assert!(
            root_entry.in_subtree("root"),
            "Root entry should have 'root' subtree"
        );

        // Verify that the root entry has no parents
        assert!(
            root_entry.parents().unwrap_or_default().is_empty(),
            "Root entry should have no parents"
        );
    }

    #[test]
    fn test_multiple_trees() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create multiple trees with different settings, serialized to JSON
        let mut settings1_map = HashMap::new();
        settings1_map.insert("name".to_string(), "tree1".to_string());

        let settings1 = KVOverWrite::from_hashmap(settings1_map);
        let settings1_json = serde_json::to_string(&settings1).unwrap();

        let mut settings2_map = HashMap::new();
        settings2_map.insert("name".to_string(), "tree2".to_string());

        let settings2 = KVOverWrite::from_hashmap(settings2_map);
        let settings2_json = serde_json::to_string(&settings2).unwrap();

        let tree1 = db.new_tree(settings1).expect("Failed to create tree1");
        let tree2 = db.new_tree(settings2).expect("Failed to create tree2");

        // Verify that trees have different root IDs
        assert_ne!(
            tree1.root_id(),
            tree2.root_id(),
            "Trees should have different root IDs"
        );

        // Verify that each tree has the correct settings
        let root1 = tree1.get_root().expect("Failed to get root for tree1");
        let root2 = tree2.get_root().expect("Failed to get root for tree2");

        // Compare the raw JSON data strings
        assert_eq!(
            &root1.get_settings().unwrap(),
            &settings1_json,
            "Tree1 should have correct settings"
        );
        assert_eq!(
            &root2.get_settings().unwrap(),
            &settings2_json,
            "Tree2 should have correct settings"
        );
    }

    #[test]
    fn test_tree_insert_and_tips() {
        // Setup
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);

        // Create a new tree
        let tree = db.new_tree(KVOverWrite::new()).unwrap();
        let root_id = tree.root_id().clone();

        // Initial state - we might or might not have tips depending on implementation
        // So we'll first insert an entry to establish a known state

        // Insert first entry
        let mut entry1 = Entry::new(tree.root_id().clone(), "{}".to_string());
        entry1
            .add_subtree("operation1".to_string(), "{}".to_string())
            .unwrap();
        entry1.set_parents(vec![root_id.clone()]);
        let id1 = tree.insert(entry1).unwrap();

        // Verify we can get at least one tip after insertion
        let tips_after_first_insert = tree.get_tips().unwrap();
        assert!(
            !tips_after_first_insert.is_empty(),
            "Should have at least one tip after first insert"
        );

        // Insert second entry
        let mut entry2 = Entry::new(tree.root_id().clone(), "{}".to_string());
        entry2
            .add_subtree("operation1".to_string(), "{}".to_string())
            .unwrap();
        entry2.set_parents(vec![id1.clone()]);
        let _id2 = tree.insert(entry2).unwrap();

        // Verify we still have at least one tip
        let tips_after_second_insert = tree.get_tips().unwrap();
        assert!(
            !tips_after_second_insert.is_empty(),
            "Should have at least one tip after second insert"
        );
    }

    #[test]
    fn test_subtree_tips() {
        // Setup
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);

        // Create a new tree
        let tree = db.new_tree(KVOverWrite::new()).unwrap();

        // Insert entries with different operations
        let mut entry1 = Entry::new(tree.root_id().clone(), "{}".to_string());
        entry1
            .add_subtree("operation1".to_string(), "{}".to_string())
            .unwrap();
        let _id1 = tree.insert(entry1).unwrap();

        let mut entry2 = Entry::new(tree.root_id().clone(), "{}".to_string());
        entry2
            .add_subtree("operation2".to_string(), "{}".to_string())
            .unwrap();
        let id2 = tree.insert(entry2).unwrap();

        let mut entry3 = Entry::new(tree.root_id().clone(), "{}".to_string());
        entry3
            .add_subtree("operation1".to_string(), "{}".to_string())
            .unwrap();
        let id3 = tree.insert(entry3).unwrap();

        // Check subtree tips for operation1
        let subtree_tips1 = tree.get_subtree_tips("operation1").unwrap();
        assert_eq!(subtree_tips1.len(), 1);
        assert_eq!(subtree_tips1[0], id3);

        // Check subtree tips for operation2
        let subtree_tips2 = tree.get_subtree_tips("operation2").unwrap();
        assert_eq!(subtree_tips2.len(), 1);
        assert_eq!(subtree_tips2[0], id2);

        // We still only have 1 tip, the last one we inserted
        let tips = tree.get_tips().unwrap();
        assert_eq!(tips.len(), 1);
        assert!(tips.contains(&id3));
    }

    #[test]
    fn test_multiple_branches() {
        // Setup
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);

        // Create a new tree
        let tree: Tree = db.new_tree(KVOverWrite::new()).unwrap();

        // Insert first entry
        let mut entry1 = Entry::new(tree.root_id().clone(), "{}".to_string());
        entry1
            .add_subtree("operation1".to_string(), "{}".to_string())
            .unwrap();

        // Use the normal insert method for the first entry
        let id1 = tree.insert(entry1).unwrap();

        // At this point, there should be one tip
        let tips = tree.get_tips().unwrap();
        assert_eq!(tips.len(), 1);
        assert_eq!(tips[0], id1);

        // Create two independent entries with custom parent relationships
        // Simulates sync scenarios where entries may have been added concurrently

        // Create entry2 with id1 as parent
        let mut data2_map = HashMap::new();
        data2_map.insert("key".to_string(), "value2".to_string());
        let data2_json = serde_json::to_string(&data2_map).unwrap();
        let mut entry2 = Entry::new(tree.root_id().clone(), data2_json.clone());
        entry2
            .add_subtree("operation1".to_string(), data2_json)
            .unwrap();
        entry2.set_parents(vec![id1.clone()]);
        let id2 = entry2.id(); // Calculate ID before moving
        tree.insert_raw(entry2).unwrap(); // Insert entry2

        // Create entry3 also with id1 as parent but with different content
        let mut data3_map = HashMap::new();
        data3_map.insert("key".to_string(), "value3".to_string());
        let data3_json = serde_json::to_string(&data3_map).unwrap();
        let mut entry3 = Entry::new(tree.root_id().clone(), data3_json.clone());
        entry3
            .add_subtree("operation1".to_string(), data3_json)
            .unwrap();
        entry3.set_parents(vec![id1.clone()]);
        let id3 = entry3.id(); // Calculate ID before moving
        tree.insert_raw(entry3).unwrap(); // Insert entry3

        // Important: Check that these IDs are different
        assert_ne!(
            id2, id3,
            "Entry IDs should be different for branches to work"
        );

        // At this point, there should be two tips (both entry2 and entry3)
        let tips = tree.get_tips().unwrap();
        assert_eq!(tips.len(), 2, "Should have two tips (branches)");
        assert!(tips.contains(&id2), "Tips should include entry2");
        assert!(tips.contains(&id3), "Tips should include entry3");

        // All three entries should be tips of the "operation1" subtree
        let subtree_tips = tree.get_subtree_tips("operation1").unwrap();
        assert_eq!(
            subtree_tips.len(),
            3,
            "Should have three subtree tips (entry1, entry2, entry3)"
        );
        assert!(
            subtree_tips.contains(&id1),
            "Subtree tips should include entry1"
        );
        assert!(
            subtree_tips.contains(&id2),
            "Subtree tips should include entry2"
        );
        assert!(
            subtree_tips.contains(&id3),
            "Subtree tips should include entry3"
        );
    }

    #[test]
    fn test_get_settings() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create initial settings
        let mut initial_settings = HashMap::new();
        initial_settings.insert("name".to_string(), "test_tree".to_string());
        initial_settings.insert("version".to_string(), "1.0".to_string());
        initial_settings.insert("author".to_string(), "original".to_string());

        // Create the tree with initial settings
        let tree = db
            .new_tree(KVOverWrite::from_hashmap(initial_settings))
            .expect("Failed to create tree");

        // Create an update entry with modified settings
        let mut updated_settings = HashMap::new();
        updated_settings.insert("version".to_string(), "2.0".to_string()); // Update existing key
        updated_settings.insert("updated_at".to_string(), "today".to_string()); // Add new key

        // Create and insert the new entry with updated settings
        let entry = Entry::new(
            tree.root_id().clone(),
            serde_json::to_string(&KVOverWrite::from_hashmap(updated_settings)).unwrap(),
        );
        tree.insert(entry).expect("Failed to insert entry");

        // Create another update with different settings
        let mut more_settings = HashMap::new();
        more_settings.insert("author".to_string(), "new_author".to_string()); // Override existing key
        more_settings.insert("status".to_string(), "active".to_string()); // Add another key

        let entry2 = Entry::new(
            tree.root_id().clone(),
            serde_json::to_string(&KVOverWrite::from_hashmap(more_settings)).unwrap(),
        );
        tree.insert(entry2).expect("Failed to insert second entry");

        // Get the merged settings
        let merged_settings = tree.get_settings().expect("Failed to get settings");

        // Check that all keys are present with the correct, merged values
        assert_eq!(merged_settings.get("name"), Some(&"test_tree".to_string()));
        assert_eq!(merged_settings.get("version"), Some(&"2.0".to_string())); // Updated value
        assert_eq!(
            merged_settings.get("author"),
            Some(&"new_author".to_string())
        ); // Updated value
        assert_eq!(
            merged_settings.get("updated_at"),
            Some(&"today".to_string())
        ); // New key from first update
        assert_eq!(merged_settings.get("status"), Some(&"active".to_string())); // New key from second update

        // Make sure we have the right number of keys
        assert_eq!(merged_settings.as_hashmap().len(), 5);
    }

    #[test]
    fn test_get_settings_with_branches() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create initial settings
        let mut initial_settings = HashMap::new();
        initial_settings.insert("name".to_string(), "branched_tree".to_string());
        initial_settings.insert("version".to_string(), "1.0".to_string());

        // Create the tree with initial settings
        let tree = db
            .new_tree(KVOverWrite::from_hashmap(initial_settings))
            .expect("Failed to create tree");

        // Get the root ID
        let root_id = tree.root_id().clone();

        // Create first entry (A) with additional settings
        let mut settings_a = HashMap::new();
        settings_a.insert("owner".to_string(), "user1".to_string());

        let mut entry_a = Entry::new(
            root_id.clone(),
            serde_json::to_string(&KVOverWrite::from_hashmap(settings_a)).unwrap(),
        );
        entry_a.set_parents(vec![root_id.clone()]);
        let id_a = tree.insert_raw(entry_a).expect("Failed to insert entry A");

        // Create two divergent branches from A

        // Branch 1: Entry B changes version and adds description
        let mut settings_b = HashMap::new();
        settings_b.insert("version".to_string(), "1.1-beta".to_string());
        settings_b.insert(
            "description".to_string(),
            "Branch 1 description".to_string(),
        );

        let mut entry_b = Entry::new(
            root_id.clone(),
            serde_json::to_string(&KVOverWrite::from_hashmap(settings_b)).unwrap(),
        );
        entry_b.set_parents(vec![id_a.clone()]);
        let _id_b = tree.insert_raw(entry_b).expect("Failed to insert entry B");

        // Branch 2: Entry C changes owner and adds priority
        let mut settings_c = HashMap::new();
        settings_c.insert("owner".to_string(), "user2".to_string()); // Will override A's owner
        settings_c.insert("priority".to_string(), "high".to_string());

        let mut entry_c = Entry::new(
            root_id.clone(),
            serde_json::to_string(&KVOverWrite::from_hashmap(settings_c)).unwrap(),
        );
        entry_c.set_parents(vec![id_a.clone()]);
        let _id_c = tree.insert_raw(entry_c).expect("Failed to insert entry C");

        // Get the merged settings
        let merged_settings = tree.get_settings().expect("Failed to get settings");

        // Check that all keys are present with the expected values
        assert_eq!(
            merged_settings.get("name"),
            Some(&"branched_tree".to_string())
        ); // From root
        assert_eq!(
            merged_settings.get("version"),
            Some(&"1.1-beta".to_string())
        ); // From B
        assert_eq!(merged_settings.get("owner"), Some(&"user2".to_string())); // From C (latest override)
        assert_eq!(
            merged_settings.get("description"),
            Some(&"Branch 1 description".to_string())
        ); // From B
        assert_eq!(merged_settings.get("priority"), Some(&"high".to_string())); // From C

        // Make sure we have exactly these 5 keys
        assert_eq!(merged_settings.as_hashmap().len(), 5);
    }

    #[test]
    fn test_get_settings_empty_tree() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create an empty tree (with empty settings)
        let tree = db
            .new_tree(KVOverWrite::new())
            .expect("Failed to create empty tree");

        // Get the settings from the empty tree
        let settings = tree
            .get_settings()
            .expect("Failed to get settings from empty tree");

        // The settings should be empty (just containing the root entry's empty settings)
        assert_eq!(settings.as_hashmap().len(), 0, "Settings should be empty");

        // Verify we can still use the settings object normally
        assert_eq!(settings.get("nonexistent"), None);
    }

    #[test]
    fn test_get_subtree_data() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create initial settings
        let tree = db
            .new_tree(KVOverWrite::new())
            .expect("Failed to create tree");

        // Create first entry with a custom subtree
        let mut initial_data = HashMap::new();
        initial_data.insert("key1".to_string(), "value1".to_string());
        initial_data.insert("key2".to_string(), "value2".to_string());

        let mut entry = Entry::new(tree.root_id().clone(), "{}".to_string());
        entry
            .add_subtree(
                "custom_subtree".to_string(),
                serde_json::to_string(&KVOverWrite::from_hashmap(initial_data)).unwrap(),
            )
            .unwrap();

        let _id1 = tree.insert(entry).expect("Failed to insert first entry");

        // Create second entry with updated data for the custom subtree
        let mut updated_data = HashMap::new();
        updated_data.insert("key2".to_string(), "updated2".to_string()); // Override existing
        updated_data.insert("key3".to_string(), "value3".to_string()); // Add new

        let mut entry2 = Entry::new(tree.root_id().clone(), "{}".to_string());
        entry2
            .add_subtree(
                "custom_subtree".to_string(),
                serde_json::to_string(&KVOverWrite::from_hashmap(updated_data)).unwrap(),
            )
            .unwrap();

        tree.insert(entry2).expect("Failed to insert second entry");

        // Retrieve and test the merged subtree data
        let subtree_data: KVOverWrite = tree
            .get_subtree_data("custom_subtree")
            .expect("Failed to get subtree data");

        // Verify the merged data
        assert_eq!(subtree_data.get("key1"), Some(&"value1".to_string())); // Unchanged
        assert_eq!(subtree_data.get("key2"), Some(&"updated2".to_string())); // Updated
        assert_eq!(subtree_data.get("key3"), Some(&"value3".to_string())); // Added
        assert_eq!(subtree_data.as_hashmap().len(), 3); // Should have 3 keys
    }

    #[test]
    fn test_get_subtree_data_with_branches() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create a tree
        let tree = db
            .new_tree(KVOverWrite::new())
            .expect("Failed to create tree");
        let root_id = tree.root_id().clone();

        // Create first entry with a custom subtree
        let mut initial_data = HashMap::new();
        initial_data.insert("name".to_string(), "subtree-data".to_string());
        initial_data.insert("version".to_string(), "1.0".to_string());

        let mut entry1 = Entry::new(root_id.clone(), "{}".to_string());
        entry1
            .add_subtree(
                "data_subtree".to_string(),
                serde_json::to_string(&KVOverWrite::from_hashmap(initial_data)).unwrap(),
            )
            .unwrap();

        let id1 = tree
            .insert_raw(entry1)
            .expect("Failed to insert first entry");

        // Create two divergent branches

        // Branch 1: Update version and add description
        let mut branch1_data = HashMap::new();
        branch1_data.insert("version".to_string(), "1.1-branch1".to_string());
        branch1_data.insert("description".to_string(), "From branch 1".to_string());

        let mut entry2 = Entry::new(root_id.clone(), "{}".to_string());
        entry2
            .add_subtree(
                "data_subtree".to_string(),
                serde_json::to_string(&KVOverWrite::from_hashmap(branch1_data)).unwrap(),
            )
            .unwrap();
        entry2.set_parents(vec![id1.clone()]);
        entry2.set_subtree_parents("data_subtree", vec![id1.clone()]);

        let _id2 = tree
            .insert_raw(entry2)
            .expect("Failed to insert branch 1 entry");

        // Branch 2: Update name and add status
        let mut branch2_data = HashMap::new();
        branch2_data.insert("name".to_string(), "subtree-data-renamed".to_string());
        branch2_data.insert("status".to_string(), "active".to_string());

        let mut entry3 = Entry::new(root_id.clone(), "{}".to_string());
        entry3
            .add_subtree(
                "data_subtree".to_string(),
                serde_json::to_string(&KVOverWrite::from_hashmap(branch2_data)).unwrap(),
            )
            .unwrap();
        entry3.set_parents(vec![id1.clone()]);
        entry3.set_subtree_parents("data_subtree", vec![id1.clone()]);

        let _id3 = tree
            .insert_raw(entry3)
            .expect("Failed to insert branch 2 entry");

        // Retrieve and test the merged subtree data
        let subtree_data: KVOverWrite = tree
            .get_subtree_data("data_subtree")
            .expect("Failed to get subtree data");

        // Verify the merged data (should contain all keys with latest values)
        assert_eq!(
            subtree_data.get("name"),
            Some(&"subtree-data-renamed".to_string())
        ); // From branch 2
        assert_eq!(
            subtree_data.get("version"),
            Some(&"1.1-branch1".to_string())
        ); // From branch 1
        assert_eq!(
            subtree_data.get("description"),
            Some(&"From branch 1".to_string())
        ); // From branch 1
        assert_eq!(subtree_data.get("status"), Some(&"active".to_string())); // From branch 2

        // Should have exactly 4 keys
        assert_eq!(subtree_data.as_hashmap().len(), 4);
    }

    #[test]
    fn test_get_subtree_data_empty() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create a tree
        let tree = db
            .new_tree(KVOverWrite::new())
            .expect("Failed to create tree");

        // Create an entry with an empty subtree
        let mut entry = Entry::new(tree.root_id().clone(), "{}".to_string());
        entry
            .add_subtree(
                "empty_subtree".to_string(),
                serde_json::to_string(&KVOverWrite::new()).unwrap(),
            )
            .unwrap();

        tree.insert(entry).expect("Failed to insert entry");

        // Retrieve and test the empty subtree data
        let subtree_data: KVOverWrite = tree
            .get_subtree_data("empty_subtree")
            .expect("Failed to get subtree data");

        // Should be empty
        assert_eq!(subtree_data.as_hashmap().len(), 0);
    }

    #[test]
    fn test_get_subtree_data_nonexistent() {
        // Create a new in-memory backend
        let backend = Box::new(InMemoryBackend::new());

        // Create a new BaseDB
        let db = BaseDB::new(backend);

        // Create a tree
        let tree = db
            .new_tree(KVOverWrite::new())
            .expect("Failed to create tree");

        // Try to get data from a nonexistent subtree
        let result: KVOverWrite = tree
            .get_subtree_data("nonexistent_subtree")
            .expect("Should return an empty CRDT for nonexistent subtree");

        // For a nonexistent subtree, we expect an empty CRDT
        assert_eq!(result.as_hashmap().len(), 0, "Should return an empty CRDT");
    }
}
