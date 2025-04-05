use crate::backend::Backend;
use crate::entry::{Entry, ID};
use crate::Error;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// In-memory backend for testing and development.
#[derive(Serialize, Deserialize, Debug)]
pub struct InMemoryBackend {
    entries: HashMap<ID, Entry>,
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryBackend {
    /// Create a new in-memory backend
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Save the backend state to a JSON file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string(self).map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to serialize: {}", e),
            ))
        })?;

        fs::write(path, json).map_err(Error::Io)?;
        Ok(())
    }

    /// Load the backend state from a JSON file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file_exists = path.as_ref().exists();
        if !file_exists {
            return Ok(Self::new());
        }

        let json = fs::read_to_string(path).map_err(Error::Io)?;
        serde_json::from_str(&json).map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to deserialize: {}", e),
            ))
        })
    }

    /// Get all entry IDs in the backend
    pub fn all_ids(&self) -> Vec<ID> {
        self.entries.keys().cloned().collect()
    }

    /// Get an entry by ID directly from the backend
    pub fn get_entry(&self, id: &ID) -> Result<&Entry> {
        self.get(id)
    }

    /// Check if an entry is a tip.
    /// An entry is a tip if it has no children.
    fn is_tip(&self, tree: &ID, entry_id: &ID) -> bool {
        for other_entry in self.entries.values() {
            if other_entry.root() == tree
                && other_entry.parents().unwrap_or_default().contains(entry_id)
            {
                return false;
            }
        }
        true
    }

    /// Check if an entry is a subtree tip.
    /// An entry is a subtree tip if it has no children in the given subtree.
    fn is_subtree_tip(&self, tree: &ID, subtree: &str, entry_id: &ID) -> bool {
        for other_entry in self.entries.values() {
            if other_entry.root() == tree {
                for other_subtree in other_entry.subtrees.iter() {
                    if other_subtree.name == subtree && other_subtree.parents.contains(entry_id) {
                        return false;
                    }
                }
            }
        }
        true
    }
}

impl Backend for InMemoryBackend {
    fn get(&self, id: &ID) -> Result<&Entry> {
        self.entries.get(id).ok_or(Error::NotFound)
    }

    fn put(&mut self, entry: Entry) -> Result<()> {
        self.entries.insert(entry.id(), entry);
        Ok(())
    }

    fn get_tips(&self, tree: &ID) -> Result<Vec<ID>> {
        let mut tips = Vec::new();
        for (id, entry) in &self.entries {
            if entry.tree() == *tree && self.is_tip(tree, id) {
                tips.push(id.clone());
            }
        }
        Ok(tips)
    }

    fn get_subtree_tips(&self, tree: &ID, subtree: &str) -> Result<Vec<ID>> {
        let mut tips = Vec::new();
        for (id, entry) in &self.entries {
            if entry.root() == tree
                && entry.in_subtree(subtree)
                && self.is_subtree_tip(tree, subtree, id)
            {
                tips.push(id.clone());
            }
        }
        Ok(tips)
    }

    fn all_roots(&self) -> Result<Vec<ID>> {
        let mut roots = Vec::new();
        for (id, entry) in &self.entries {
            if entry.root().is_empty() && entry.is_root() {
                roots.push(id.clone());
            }
        }
        Ok(roots)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{Entry, SubTreeNode, TreeNode, CRDT};
    use std::fs;
    use std::io::Write;

    // Helper to create a simple entry for testing
    fn create_test_entry(value: &str) -> Entry {
        let mut data = CRDT::new();
        data.insert("key".to_string(), value.to_string());

        Entry {
            tree: TreeNode {
                root: "".to_string(), // Empty root indicates a top-level root
                parents: vec![],
                data: data.clone(),
            },
            subtrees: vec![
                // Add a "root" subtree to make this a root entry
                SubTreeNode {
                    name: "root".to_string(),
                    parents: vec![],
                    data: data.clone(),
                },
            ],
        }
    }

    #[test]
    fn test_save_load_in_memory_backend() -> Result<()> {
        let mut backend1 = InMemoryBackend::new();
        let entry1 = create_test_entry("value1");
        let entry2 = create_test_entry("value2");

        let id1 = entry1.id();
        let id2 = entry2.id();

        backend1.put(entry1.clone())?;
        backend1.put(entry2.clone())?;

        // Ensure target directory exists
        fs::create_dir_all("target/test_data")?;
        let path = "target/test_data/test_save_load.json";

        // Save
        backend1.save_to_file(path)?;

        // Load into a new backend
        let backend2 = InMemoryBackend::load_from_file(path)?;

        // Verify contents
        assert_eq!(backend2.entries.len(), 2);
        assert_eq!(backend2.get(&id1)?, &entry1);
        assert_eq!(backend2.get(&id2)?, &entry2);

        // Clean up
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn test_load_non_existent_file() -> Result<()> {
        let path = "target/test_data/non_existent_file.json";
        // Ensure file does not exist
        let _ = fs::remove_file(path); // Ignore error if it doesn't exist

        // Load
        let backend = InMemoryBackend::load_from_file(path)?;

        // Verify it's empty
        assert!(backend.entries.is_empty());
        Ok(())
    }

    #[test]
    fn test_load_invalid_file() -> Result<()> {
        // Ensure target directory exists
        fs::create_dir_all("target/test_data")?;
        let path = "target/test_data/invalid_file.json";

        // Create an invalid JSON file
        {
            let mut file = fs::File::create(path)?;
            writeln!(file, "{{invalid json")?;
        }

        // Attempt to load
        let result = InMemoryBackend::load_from_file(path);

        // Verify it's an error (specifically an Io error wrapping deserialize)
        assert!(result.is_err());
        if let Err(Error::Io(io_err)) = result {
            assert!(io_err.to_string().contains("Failed to deserialize"));
        } else {
            panic!("Expected Io error, got {:?}", result);
        }

        // Clean up
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn test_all_roots() -> Result<()> {
        let mut backend = InMemoryBackend::new();

        // Create two root entries (they have empty root string and a subtree named "root")
        let root1 = create_test_entry("root1");
        let root2 = create_test_entry("root2");

        // Create a non-root entry (child of root1)
        let mut child = create_test_entry("child");
        child.tree.root = root1.id(); // Set root to point to root1
        child.subtrees.clear(); // Remove the "root" subtree

        // Store all entries
        backend.put(root1.clone())?;
        backend.put(root2.clone())?;
        backend.put(child.clone())?;

        // Get all roots
        let roots = backend.all_roots()?;

        // Verify we get only the two root entries
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&root1.id()));
        assert!(roots.contains(&root2.id()));
        assert!(!roots.contains(&child.id()));

        Ok(())
    }

    #[test]
    fn test_save_load_with_various_entries() -> Result<()> {
        let mut backend = InMemoryBackend::new();

        // Create different types of entries
        // 1. A root entry
        let root = create_test_entry("root");
        let root_id = root.id();

        // 2. A child entry with parent
        let mut child = create_test_entry("child");
        child.tree.root = root_id.clone(); // Set root to point to root
        child.subtrees.clear(); // Remove the "root" subtree
        child.tree.parents = vec![root_id.clone()]; // Set parent
        let child_id = child.id();

        // 3. An entry with subtrees
        let mut with_subtree = create_test_entry("with_subtree");
        // Add an additional subtree
        let mut subtree_data = CRDT::new();
        subtree_data.insert("subtree_key".to_string(), "subtree_value".to_string());
        with_subtree.subtrees.push(SubTreeNode {
            name: "custom_subtree".to_string(),
            parents: vec![],
            data: subtree_data,
        });
        let with_subtree_id = with_subtree.id();

        // Store all entries
        backend.put(root)?;
        backend.put(child)?;
        backend.put(with_subtree)?;

        // Save to file
        fs::create_dir_all("target/test_data")?;
        let path = "target/test_data/test_complex_save_load.json";
        backend.save_to_file(path)?;

        // Load into a new backend
        let loaded_backend = InMemoryBackend::load_from_file(path)?;

        // Verify the content
        assert_eq!(loaded_backend.entries.len(), 3);

        // Check basic properties of each entry
        let loaded_root = loaded_backend.get(&root_id)?;
        assert!(loaded_root.is_root());
        assert_eq!(loaded_root.root(), "");

        let loaded_child = loaded_backend.get(&child_id)?;
        assert!(!loaded_child.is_root());
        assert_eq!(loaded_child.root(), &root_id);
        assert_eq!(loaded_child.parents()?.len(), 1);

        let loaded_with_subtree = loaded_backend.get(&with_subtree_id)?;
        assert!(loaded_with_subtree.is_root());
        assert_eq!(loaded_with_subtree.subtrees.len(), 2);
        assert!(loaded_with_subtree
            .subtrees
            .iter()
            .any(|st| st.name == "custom_subtree"));

        // Clean up
        fs::remove_file(path)?;
        Ok(())
    }
}
