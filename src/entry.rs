//!
//! Defines the fundamental data unit (`Entry`) and related types.
//!
//! An `Entry` is the core, content-addressable building block of the database,
//! representing a snapshot of data in the main tree and potentially multiple named subtrees.
//! This module also defines the `ID` type and `RawData` type.

use crate::Error;
use crate::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A content-addressable identifier for an `Entry` or other database object.
///
/// Currently represented as a hex-encoded SHA-256 hash string.
pub type ID = String;

/// Represents serialized data, typically JSON, provided by the user.
///
/// This allows users to manage their own data structures and serialization formats.
pub type RawData = String;

/// Internal representation of the main tree node within an `Entry`.
#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct TreeNode {
    /// The ID of the root `Entry` of the tree this node belongs to.
    pub root: ID,
    /// IDs of the parent `Entry`s in the main tree history.
    pub parents: Vec<ID>,
    /// Serialized data associated with this `Entry` in the main tree.
    pub data: RawData,
}

/// Internal representation of a named subtree node within an `Entry`.
#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SubTreeNode {
    /// The name of the subtree, analogous to a table name.
    /// Subtrees are _named_, and not identified by an ID.
    pub name: String,
    /// IDs of the parent `Entry`s specific to this subtree's history.
    pub parents: Vec<ID>, // Parents specific to this entry within this subtree
    /// Serialized data specific to this `Entry` within this named subtree.
    pub data: RawData,
}

/// The fundamental unit of data in EideticaDB.
///
/// An `Entry` represents a snapshot of data within a `Tree` and potentially one or more named `SubTree`s.
/// It is content-addressable, meaning its `ID` is a cryptographic hash of its contents.
/// Entries form a Merkle-DAG (Directed Acyclic Graph) structure through parent references.
#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    /// The main tree node data, including the root ID, parents in the main tree, and associated data.
    tree: TreeNode,
    /// A collection of named subtrees this entry contains data for.
    subtrees: Vec<SubTreeNode>,
    // TODO: Security
    // The ID of the key that was used to sign the entry.
    // This is an Entry ID pointing to the entry that allows the key used to sign this.
    // key: String,
    // The signature of the entry.
    // signature: String,
}

impl Entry {
    /// Create a new entry associated with a specific tree root.
    ///
    /// # Arguments
    /// * `root` - The `ID` of the root `Entry` of the tree this entry belongs to.
    /// * `data` - `RawData` (serialized string) for the main tree node (`tree.data`).
    pub fn new(root: String, data: RawData) -> Self {
        let mut entry = Entry::default();
        entry.tree.root = root;
        entry.tree.data = data;
        entry
    }

    /// Creates a new top-level (root) entry for a new tree.
    ///
    /// Root entries have an empty string as their `root` ID and include a special "root" subtree marker.
    ///
    /// # Arguments
    /// * `data` - `RawData` (serialized string) for the root entry's main data (`tree.data`), often tree settings.
    pub fn new_top_level(data: RawData) -> Self {
        let mut entry = Entry::default();
        entry.tree.root = "".to_string();
        entry.tree.data = data;
        // Add a subtree with the name "root" to mark this as a root entry
        entry
            .add_subtree("root".to_string(), "{}".to_string())
            .unwrap();
        entry
    }

    /// Add data for a named subtree to this entry.
    ///
    /// If an entry contributes data to a specific domain or table, it's added via a `SubTreeNode`.
    ///
    /// # Arguments
    /// * `name` - The name of the subtree (e.g., "users", "products").
    /// * `data` - `RawData` (serialized string) specific to this entry for the named subtree.
    pub fn add_subtree(&mut self, name: String, data: RawData) -> Result<()> {
        // Verify that the subtree does not already exist
        if self.subtrees.iter().any(|node| node.name == name) {
            return Err(Error::AlreadyExists);
        }
        self.subtrees.push(SubTreeNode {
            name,
            data,
            parents: vec![],
        });
        Ok(())
    }

    /// Calculate the content-addressable ID (SHA-256 hash) of the entry.
    ///
    /// The hash includes the root ID, main tree data, and data from all subtrees,
    /// ensuring that any change to the entry results in a different ID.
    /// Subtrees and parents are sorted before hashing for determinism.
    ///
    /// TODO: Formalize the hashing scheme for SRI compatibility.
    pub fn id(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.tree.root.as_bytes());

        // Hash the raw tree data
        hasher.update(self.tree.data.as_bytes());

        // Hash all subtrees in a deterministic order
        let mut sorted_subtrees = self.subtrees.clone();
        sorted_subtrees.sort_by(|a, b| a.name.cmp(&b.name)); // Sort by name

        for subtree in &sorted_subtrees {
            // Use sorted list
            hasher.update(subtree.name.as_bytes());
            hasher.update(subtree.data.as_bytes()); // Hash raw data directly

            // Hash subtree parents deterministically
            let mut sorted_parents = subtree.parents.clone();
            sorted_parents.sort(); // Sort parent IDs
            for parent in &sorted_parents {
                // Use sorted list
                hasher.update(parent.as_bytes());
            }
        }

        // Hash tree parents deterministically
        let mut sorted_tree_parents = self.tree.parents.clone();
        sorted_tree_parents.sort(); // Sort parent IDs
        for parent in &sorted_tree_parents {
            // Use sorted list
            hasher.update(parent.as_bytes());
        }

        // Convert to hex string for SRI compatibility
        format!("{:x}", hasher.finalize())
    }

    /// Get the ID of the root `Entry` of the tree this entry belongs to.
    pub fn root(&self) -> &str {
        &self.tree.root
    }

    /// Check if this entry is a root entry of a tree.
    ///
    /// Determined by the presence of a special "root" subtree.
    pub fn is_root(&self) -> bool {
        // TODO: Roots are a case that requires special handling.
        self.subtrees.iter().any(|node| node.name == "root")
    }

    /// Check if this entry is the absolute top-level root entry (has no parent tree).
    pub fn is_toplevel_root(&self) -> bool {
        self.root().is_empty() && self.is_root()
    }

    /// Check if this entry contains data for a specific named subtree.
    pub fn in_subtree(&self, subtree: &str) -> bool {
        self.subtrees.iter().any(|node| node.name == subtree)
    }

    /// Check if this entry belongs to a specific tree, identified by its root ID.
    pub fn in_tree(&self, tree: &str) -> bool {
        // Entries that are roots exist in both trees
        self.root() == tree || (self.is_root() && (self.id() == tree))
    }

    /// Get the names of all subtrees this entry contains data for.
    pub fn subtrees(&self) -> Result<Vec<String>> {
        if self.subtrees.is_empty() {
            return Err(Error::NotFound);
        }
        Ok(self
            .subtrees
            .iter()
            .map(|subtree| subtree.name.clone())
            .collect())
    }

    /// Get the `RawData` associated with the main tree node (`tree.data`).
    /// Often used for tree settings or metadata.
    pub fn get_settings(&self) -> Result<RawData> {
        Ok(self.tree.data.clone())
    }

    /// Get the `RawData` for a specific named subtree within this entry.
    pub fn data(&self, subtree: &str) -> Result<&RawData> {
        self.subtrees
            .iter()
            .find(|node| node.name == subtree)
            .map(|node| &node.data)
            .ok_or(Error::NotFound)
    }

    /// Get the IDs of the parent entries in the main tree history.
    /// Note: The returned Parents struct should be used for checking containment
    pub fn parents(&self) -> Result<Vec<ID>> {
        Ok(self.tree.parents.clone())
    }

    /// Get the IDs of the parent entries specific to a named subtree's history.
    pub fn subtree_parents(&self, subtree: &str) -> Result<Vec<ID>> {
        self.subtrees
            .iter()
            .find(|node| node.name == subtree)
            .map(|node| node.parents.clone())
            .ok_or(Error::NotFound)
    }

    /// Set the root ID for this entry.
    /// Typically used internally by `Tree::insert`.
    pub fn set_root(&mut self, root: ID) {
        self.tree.root = root;
    }

    /// Set the parent IDs for the main tree history.
    /// Typically used internally by `Tree::insert`.
    pub fn set_parents(&mut self, parents: Vec<ID>) {
        self.tree.parents = parents.clone();
    }

    /// Set the parent IDs for a specific named subtree's history.
    /// Typically used internally by `Tree::insert`.
    pub fn set_subtree_parents(&mut self, subtree: &str, parents: Vec<ID>) {
        self.subtrees
            .iter_mut()
            .find(|node| node.name == subtree)
            .unwrap()
            .parents = parents;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    #[test]
    fn test_add_subtree_success() {
        let mut entry = Entry::new("root_id".to_string(), "{}".to_string());
        let result = entry.add_subtree("my_subtree".to_string(), "{}".to_string());
        assert!(result.is_ok());
        assert!(entry.in_subtree("my_subtree"));
        assert_eq!(entry.subtrees.len(), 1);
    }

    #[test]
    fn test_add_subtree_duplicate() {
        let mut entry = Entry::new("root_id".to_string(), "{}".to_string());
        // Add first time
        entry
            .add_subtree("my_subtree".to_string(), "{}".to_string())
            .expect("First add should succeed");

        // Try adding again
        let result = entry.add_subtree("my_subtree".to_string(), "{}".to_string());

        // Assert error is AlreadyExists
        match result {
            Err(Error::AlreadyExists) => { /* Expected error */ }
            Ok(_) => panic!("Adding duplicate subtree should have failed"),
            Err(e) => panic!("Unexpected error type: {:?}", e),
        }

        // Ensure only one subtree exists
        assert_eq!(entry.subtrees.len(), 1);
    }
}
