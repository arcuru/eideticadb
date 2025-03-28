use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// An entry in the database.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The root ID of the tree containing the entry.
    /// This is the ID of the entry that is the root of the tree. Most immediate tree.
    ///
    /// IDs are SRI compatible hashes of the entry, including the signature.
    root: String,
    /// The op of the entry.
    op: Op,
    /// The data of the entry.
    /// TODO: The data type here is incorrect. It needs to be the CRDT data type that we can merge with.
    data: HashMap<String, String>,
    /// Parents of the entry within this tree.
    /// IDs of the entrie(s) that are the direct parents.
    /// NB: Includes the parents for certain OPs. e.g. "Settings" ops must always point to the last known "Settings" op.
    /// NB    It's sort of a tree within the tree, which I think will be necessary to support sparse checkouts.
    parents: Vec<String>,
    /// The timestamp of the entry.
    timestamp: u64,
    // TODO: Security
    // The ID of the key that was used to sign the entry.
    // This is an Entry ID pointing to the entry that allows the key used to sign this.
    // key: String,
    // The signature of the entry.
    // signature: String,
}

#[derive(Clone, Debug)]
pub enum Op {
    /// Merge the included data to the tree.
    /// This follows the merge pattern defined by Merkle-CRDTs.
    Merge,

    /// Root of a tree.
    /// The contents of the root are the initial settings of the new tree.
    Root,

    /// DB Settings
    /// Ops that modify the settings of the DB. e.g.
    ///     Adding/Removing keys
    ///     Object references
    ///     Encryption settings
    Settings,
}

impl Entry {
    pub fn new(
        root: String,
        op: Op,
        data: HashMap<String, String>,
        parents: Vec<String>,
        timestamp: u64,
    ) -> Self {
        Self {
            root,
            op,
            data,
            parents,
            timestamp,
        }
    }

    /// Caluculate the ID of the entry.
    /// This is the SRI compatible hash of the entry.
    ///
    /// TODO: This needs to be formalized, and may change until then
    pub fn id(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.root.as_bytes());
        hasher.update(self.op.to_bytes());

        // Convert HashMap to a deterministic byte representation
        let mut data_keys: Vec<&String> = self.data.keys().collect();
        data_keys.sort(); // Sort for deterministic order
        for key in data_keys {
            hasher.update(key.as_bytes());
            hasher.update(self.data.get(key).unwrap().as_bytes());
        }

        // Convert Vec<String> to bytes
        for parent in &self.parents {
            hasher.update(parent.as_bytes());
        }

        hasher.update(self.timestamp.to_le_bytes()); // Already bytes, no as_bytes() needed

        // Convert to hex string for SRI compatibility
        format!("{:x}", hasher.finalize())
    }
}

impl Op {
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            Op::Merge => b"merge".to_vec(),
            Op::Root => b"root".to_vec(),
            Op::Settings => b"settings".to_vec(),
        }
    }
}
