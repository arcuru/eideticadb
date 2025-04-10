use crate::Error;
use crate::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// An ID in Eidetica is an identifier for an entry or object.
/// This should be an SRI compatible hash of the entry or object.
pub type ID = String;

/// RawData represents serialized data, expected to be JSON.
/// This type is passed to/from the user, allowing them to manage
/// the specific data structure and serialization format.
pub type RawData = String;

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeNode {
    pub root: ID,
    pub parents: Vec<ID>,
    pub data: RawData, // Serialized data (e.g., JSON) for the tree metadata.
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubTreeNode {
    /// Subtrees are _named_, and not identified by an ID.
    /// They are intended as equivalent to Tables in a relational database.
    pub name: String,
    pub parents: Vec<ID>, // Parents specific to this entry within this subtree
    pub data: RawData, // Serialized data (e.g., JSON) specific to this entry within this subtree.
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
    pub fn new(root: String, data: RawData) -> Self {
        let mut entry = Entry::default();
        entry.tree.root = root;
        entry.tree.data = data;
        entry
    }

    pub fn new_top_level(data: RawData) -> Self {
        let mut entry = Entry::default();
        entry.tree.root = "".to_string();
        entry.tree.data = data;
        // Add a subtree with the name "root" to mark this as a root entry
        entry.add_subtree("root".to_string(), "{}".to_string());
        entry
    }

    /// Add a subtree to the entry.
    pub fn add_subtree(&mut self, name: String, data: RawData) {
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

    /// Get the root ID of the entry
    pub fn root(&self) -> &str {
        &self.tree.root
    }

    /// Check if the entry is a root
    pub fn is_root(&self) -> bool {
        // TODO: Roots are a case that requires special handling.
        self.subtrees.iter().any(|node| node.name == "root")
    }

    pub fn is_toplevel_root(&self) -> bool {
        self.root().is_empty() && self.is_root()
    }

    /// Check if the entry is in a subtree
    pub fn in_subtree(&self, subtree: &str) -> bool {
        self.subtrees.iter().any(|node| node.name == subtree)
    }

    /// Check if the entry is in a tree
    pub fn in_tree(&self, tree: &str) -> bool {
        // Entries that are roots exist in both trees
        self.root() == tree || (self.is_root() && (self.id() == tree))
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

    pub fn get_settings(&self) -> Result<RawData> {
        Ok(self.tree.data.clone())
    }

    /// Get the data of the subtree
    pub fn data(&self, subtree: &str) -> Result<&RawData> {
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
