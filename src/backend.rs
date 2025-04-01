use crate::entry::{Entry, ID};
use crate::Error;
use crate::Result;
use std::collections::HashMap;

/// Backend trait abstracting the underlying storage.
pub trait Backend: Send + Sync {
    fn get(&self, id: &ID) -> Result<&Entry>;
    fn put(&mut self, entry: Entry) -> Result<()>;

    /// Get the tips of a tree.
    /// The tips are defined as the set of all entries in the given tree with no children.
    fn get_tips(&self, tree: &ID) -> Result<Vec<ID>>;

    /// Get the tips of a subtree.
    fn get_subtree_tips(&self, tree: &ID, subtree: &str) -> Result<Vec<ID>>;
}

/// In-memory backend for testing and development.
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
}
