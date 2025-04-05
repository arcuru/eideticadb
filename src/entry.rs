use crate::Error;
use crate::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// An ID in Eidetica is an identifier for an entry or object.
/// This should be an SRI compatible hash of the entry or object.
pub type ID = String;

/// A CRDT is a data structure that can be merged with other CRDTs without conflict.
/// We are using it with the idea that a total ordering of operations result in the same state.
/// This is a simple key-value store, but could be extended to more complex data structures.
/// TODO: This data type is not correct.
pub type CRDT = HashMap<String, String>;

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeNode {
    pub root: ID,
    pub parents: Vec<ID>,
    pub data: CRDT, // Will be metadata applying to the tree. timestamp, height, etc.
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubTreeNode {
    /// Subtrees are _named_, and not identified by an ID.
    /// They are intended as equivalent to Tables in a relational database.
    pub name: String,
    pub parents: Vec<ID>,
    pub data: CRDT,
}

/// An entry in the database.
#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub tree: TreeNode,
    pub subtrees: Vec<SubTreeNode>,
    // TODO: Security
    // The ID of the key that was used to sign the entry.
    // This is an Entry ID pointing to the entry that allows the key used to sign this.
    // key: String,
    // The signature of the entry.
    // signature: String,
}

/// Parents of the entry within this tree.
#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Parents {
    /// IDs of the parent in the base tree.
    tree: Vec<ID>,
    /// IDs of the parents in the embedded subtree.
    subtree: Vec<ID>,
}

impl Parents {
    pub fn new(tree: Vec<ID>, subtree: Vec<ID>) -> Self {
        Self { tree, subtree }
    }

    pub fn tree(&self) -> &Vec<ID> {
        &self.tree
    }

    pub fn subtree(&self) -> &Vec<ID> {
        &self.subtree
    }
}

impl Entry {
    /// Create a new entry with a root.
    pub fn new(root: String, data: CRDT) -> Self {
        let mut entry = Entry::default();
        entry.tree.root = root;
        entry.tree.data = data;
        entry
    }

    /// Add a subtree to the entry.
    pub fn add_subtree(&mut self, name: String, data: CRDT) {
        self.subtrees.push(SubTreeNode {
            name,
            data,
            parents: vec![],
        });
    }

    /// Calculate the ID of the entry.
    /// This is the SRI compatible hash of the entry.
    ///
    /// TODO: This needs to be formalized, and may change until then
    pub fn id(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.tree.root.as_bytes());

        // Hash the tree data
        let mut tree_data_keys: Vec<&String> = self.tree.data.keys().collect();
        tree_data_keys.sort(); // Sort for deterministic order
        for key in tree_data_keys {
            hasher.update(key.as_bytes());
            hasher.update(self.tree.data.get(key).unwrap().as_bytes());
        }

        // Hash all subtrees in a deterministic order
        for subtree in &self.subtrees {
            hasher.update(subtree.name.as_bytes());

            // Convert HashMap to a deterministic byte representation
            let mut data_keys: Vec<&String> = subtree.data.keys().collect();
            data_keys.sort(); // Sort for deterministic order
            for key in data_keys {
                hasher.update(key.as_bytes());
                hasher.update(subtree.data.get(key).unwrap().as_bytes());
            }

            // Hash subtree parents
            for parent in &subtree.parents {
                hasher.update(parent.as_bytes());
            }
        }

        // Hash tree parents
        for parent in &self.tree.parents {
            hasher.update(parent.as_bytes());
        }

        // Convert to hex string for SRI compatibility
        format!("{:x}", hasher.finalize())
    }

    /// Get the tree ID of the entry.
    /// If the entry is a root, return the ID of the entry.
    /// Otherwise, return the root ID of the tree.
    pub fn tree(&self) -> ID {
        if self.is_root() {
            self.id()
        } else {
            self.tree.root.clone()
        }
    }
    /// Get the root ID of the entry
    pub fn root(&self) -> &str {
        &self.tree.root
    }

    /// Check if the entry is a root
    pub fn is_root(&self) -> bool {
        // TODO: Roots are a case that requires special handling.
        self.subtrees.iter().any(|node| node.name == "root")
    }

    /// Check if the entry is in a subtree
    pub fn in_subtree(&self, subtree: &str) -> bool {
        self.subtrees.iter().any(|node| node.name == subtree)
    }

    /// Get the subtrees of the entry
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

    /// Get the data of the subtree
    pub fn data(&self, subtree: &str) -> Result<&HashMap<String, String>> {
        self.subtrees
            .iter()
            .find(|node| node.name == subtree)
            .map(|node| &node.data)
            .ok_or(Error::NotFound)
    }

    /// Get the parents of the entry
    /// Note: The returned Parents struct should be used for checking containment
    pub fn parents(&self) -> Result<Vec<ID>> {
        Ok(self.tree.parents.clone())
    }

    /// Get the parents of a subtree
    pub fn subtree_parents(&self, subtree: &str) -> Result<Vec<ID>> {
        self.subtrees
            .iter()
            .find(|node| node.name == subtree)
            .map(|node| node.parents.clone())
            .ok_or(Error::NotFound)
    }

    /// Set the root ID of the entry
    pub fn set_root(&mut self, root: ID) {
        self.tree.root = root;
    }

    /// Set the parents of the entry
    pub fn set_parents(&mut self, parents: Vec<ID>) {
        self.tree.parents = parents.clone();
    }

    /// Set the parents of a subtree
    pub fn set_subtree_parents(&mut self, subtree: &str, parents: Vec<ID>) {
        self.subtrees
            .iter_mut()
            .find(|node| node.name == subtree)
            .unwrap()
            .parents = parents.clone();
    }
}
