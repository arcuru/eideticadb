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

/// An entry in the database.
#[derive(Default, Clone, Debug)]
pub struct Entry {
    /// The root ID of the tree containing the entry.
    /// This is the ID of the entry that is the root of the tree. The most immediate tree.
    /// IDs are SRI compatible hashes of the entry, including the signature.
    root: ID,
    /// The op of the entry.
    op: String,
    /// The data of the entry.
    data: CRDT,
    /// Parents of the entry within this tree.
    /// IDs of the entrie(s) that are the direct parents.
    parents: Parents,
    /// Metadata about the entry.
    /// This is for internal EDB use, and is not part of the entry's data.
    /// Will store things like the timestamp or height of the tree, depending on user settings.
    metadata: CRDT,
    // TODO: Security
    // The ID of the key that was used to sign the entry.
    // This is an Entry ID pointing to the entry that allows the key used to sign this.
    // key: String,
    // The signature of the entry.
    // signature: String,
}

/// Parents of the entry within this tree.
#[derive(Default, Clone, Debug)]
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
    pub fn new(
        root: ID,
        op: String,
        data: HashMap<String, String>,
        parents: Parents,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            root,
            op,
            data,
            parents,
            metadata,
        }
    }

    /// Calculate the ID of the entry.
    /// This is the SRI compatible hash of the entry.
    ///
    /// TODO: This needs to be formalized, and may change until then
    pub fn id(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.root.as_bytes());
        hasher.update(self.op.as_bytes());

        // Convert HashMap to a deterministic byte representation
        let mut data_keys: Vec<&String> = self.data.keys().collect();
        data_keys.sort(); // Sort for deterministic order
        for key in data_keys {
            hasher.update(key.as_bytes());
            hasher.update(self.data.get(key).unwrap().as_bytes());
        }

        // Convert Vec<String> to bytes
        for parent in &self.parents.tree {
            hasher.update(parent.as_bytes());
        }
        for parent in &self.parents.subtree {
            hasher.update(parent.as_bytes());
        }

        // Convert to hex string for SRI compatibility
        format!("{:x}", hasher.finalize())
    }

    // Getter methods

    /// Get the root ID of the entry
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Get the operation type of the entry
    pub fn op(&self) -> &String {
        &self.op
    }

    /// Get the data of the entry
    pub fn data(&self) -> &HashMap<String, String> {
        &self.data
    }

    /// Get the parents of the entry
    pub fn parents(&self) -> &Parents {
        &self.parents
    }

    /// Get the timestamp of the entry
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
}
