use crate::backend::Backend;
use crate::entry::{Entry, CRDT, ID};
use crate::Result;
use std::sync::{Arc, Mutex, MutexGuard};

/// Database implementation on top of the backend.
///
/// This database is the base DB, other 'overlays' or 'plugins' should be implemented on top of this.
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
    pub fn new_tree(&self, settings: CRDT) -> Result<Tree> {
        Tree::new(settings, self.backend.clone())
    }

    /// Load an existing tree from the database by its root ID
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

/// Equivalent to a DB table.
pub struct Tree {
    root: ID,
    backend: Arc<Mutex<Box<dyn Backend>>>,
}

impl Tree {
    pub fn new(settings: CRDT, backend: Arc<Mutex<Box<dyn Backend>>>) -> Result<Self> {
        // Create a root entry for this tree
        let mut entry = Entry::new("".to_string(), settings);
        // Add a subtree with the name "root" to mark this as a root entry
        entry.add_subtree("root".to_string(), CRDT::new());

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
        root_entry
            .tree
            .data
            .get("name")
            .cloned()
            .ok_or(crate::Error::NotFound)
    }

    /// Insert an entry into the tree.
    /// The entry should only include the op and data fields.
    /// Other fields will be computed on entry.
    pub fn insert(&self, mut entry: Entry) -> Result<ID> {
        entry.set_root(self.root.clone());
        let id: ID;
        {
            let mut backend_guard = self.lock_backend()?;

            // Calculate all the tips based on what we know locally
            entry.tree.parents = backend_guard.get_tips(&self.root).unwrap_or_default();
            for subtree in &mut entry.subtrees {
                let subtree_tips = backend_guard
                    .get_subtree_tips(&self.root, &subtree.name)
                    .unwrap_or_default();
                subtree.parents = subtree_tips;
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

    pub fn get_tips(&self) -> Result<Vec<ID>> {
        let backend_guard = self.lock_backend()?;
        backend_guard.get_tips(&self.root)
    }

    pub fn get_subtree_tips(&self, subtree: &str) -> Result<Vec<ID>> {
        let backend_guard = self.lock_backend()?;
        backend_guard.get_subtree_tips(&self.root, subtree)
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
            .new_tree(settings.clone())
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

        // Create a new tree with some settings
        let mut settings = HashMap::new();
        settings.insert("name".to_string(), "test_tree".to_string());
        settings.insert("description".to_string(), "A test tree".to_string());

        let tree = db
            .new_tree(settings.clone())
            .expect("Failed to create tree");

        // Retrieve the root entry
        let root_entry = tree.get_root().expect("Failed to get root entry");

        // Verify that the root entry has the correct data
        // The data is now in the tree node, not directly in the first subtree
        assert_eq!(
            &root_entry.tree.data, &settings,
            "Root entry should contain the settings"
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

        // Create multiple trees with different settings
        let mut settings1 = HashMap::new();
        settings1.insert("name".to_string(), "tree1".to_string());

        let mut settings2 = HashMap::new();
        settings2.insert("name".to_string(), "tree2".to_string());

        let tree1 = db
            .new_tree(settings1.clone())
            .expect("Failed to create tree1");
        let tree2 = db
            .new_tree(settings2.clone())
            .expect("Failed to create tree2");

        // Verify that trees have different root IDs
        assert_ne!(
            tree1.root_id(),
            tree2.root_id(),
            "Trees should have different root IDs"
        );

        // Verify that each tree has the correct settings
        let root1 = tree1.get_root().expect("Failed to get root for tree1");
        let root2 = tree2.get_root().expect("Failed to get root for tree2");

        assert_eq!(
            &root1.tree.data, &settings1,
            "Tree1 should have correct settings"
        );
        assert_eq!(
            &root2.tree.data, &settings2,
            "Tree2 should have correct settings"
        );
    }

    #[test]
    fn test_tree_insert_and_tips() {
        // Setup
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);

        // Create a new tree
        let tree = db.new_tree(CRDT::new()).unwrap();

        // Verify the initial state - there should be exactly one tip (the root)
        let initial_tips = tree.get_tips().unwrap();
        assert_eq!(initial_tips.len(), 1);
        assert_eq!(&initial_tips[0], tree.root_id());

        // Insert an entry
        let mut entry1 = Entry::new(tree.root_id().clone(), CRDT::new());
        // Add subtree with operation name
        entry1.add_subtree("operation1".to_string(), CRDT::new());
        let id1 = tree.insert(entry1).unwrap();

        // Verify the tips
        let tips = tree.get_tips().unwrap();
        assert_eq!(tips.len(), 1);
        assert_eq!(tips[0], id1);

        // Insert another entry
        let mut entry2 = Entry::new(tree.root_id().clone(), CRDT::new());
        // Add subtree with operation name
        entry2.add_subtree("operation1".to_string(), CRDT::new());
        let id2 = tree.insert(entry2).unwrap();

        // Verify the tips
        let tips = tree.get_tips().unwrap();
        assert_eq!(tips.len(), 1);
        assert_eq!(tips[0], id2);
    }

    #[test]
    fn test_subtree_tips() {
        // Setup
        let backend = Box::new(InMemoryBackend::new());
        let db = BaseDB::new(backend);

        // Create a new tree
        let tree = db.new_tree(CRDT::new()).unwrap();

        // Insert entries with different operations
        let mut entry1 = Entry::new(tree.root_id().clone(), CRDT::new());
        entry1.add_subtree("operation1".to_string(), CRDT::new());
        let _id1 = tree.insert(entry1).unwrap();

        let mut entry2 = Entry::new(tree.root_id().clone(), CRDT::new());
        entry2.add_subtree("operation2".to_string(), CRDT::new());
        let id2 = tree.insert(entry2).unwrap();

        let mut entry3 = Entry::new(tree.root_id().clone(), CRDT::new());
        entry3.add_subtree("operation1".to_string(), CRDT::new());
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
        let tree = db.new_tree(CRDT::new()).unwrap();
        let root_id = tree.root_id().clone();

        // Insert first entry
        let mut entry1 = Entry::new(tree.root_id().clone(), CRDT::new());
        entry1.add_subtree("operation1".to_string(), CRDT::new());

        // Use the normal insert method for the first entry
        let id1 = tree.insert(entry1).unwrap();

        // At this point, there should be one tip
        let tips = tree.get_tips().unwrap();
        assert_eq!(tips.len(), 1);
        assert_eq!(tips[0], id1);

        // Create two independent entries with custom parent relationships
        // Simulates sync scenarios where entries may have been added concurrently

        // Create entry2 with id1 as parent
        let mut data2 = CRDT::new();
        data2.insert("key".to_string(), "value2".to_string());
        let mut entry2 = Entry::new(tree.root_id().clone(), data2.clone());
        entry2.add_subtree("operation1".to_string(), data2);
        entry2.set_root(root_id.clone());
        entry2.set_parents(vec![id1.clone()]);

        // Create entry3 also with id1 as parent but with different content
        let mut data3 = CRDT::new();
        data3.insert("key".to_string(), "value3".to_string());
        let mut entry3 = Entry::new(tree.root_id().clone(), data3.clone());
        entry3.add_subtree("operation1".to_string(), data3);
        entry3.set_root(root_id.clone());
        entry3.set_parents(vec![id1.clone()]);

        // Insert both entries and get their IDs
        let id2 = entry2.id();
        let id3 = entry3.id();

        // Important: Check that these IDs are different
        assert_ne!(
            id2, id3,
            "Entry IDs should be different for branches to work"
        );

        // Insert the entries
        tree.insert_raw(entry2).unwrap();
        tree.insert_raw(entry3).unwrap();

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
}
