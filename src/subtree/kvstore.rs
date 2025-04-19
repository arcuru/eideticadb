use crate::atomicop::AtomicOp;
use crate::data::{KVOverWrite, CRDT};
use crate::subtree::SubTree;
use crate::{Error, Result};

/// A simple key-value store SubTree
///
/// It assumes that the SubTree data is a KVOverWrite CRDT.
pub struct KVStore {
    name: String,
    atomic_op: AtomicOp,
}

impl SubTree for KVStore {
    fn new(op: &AtomicOp, subtree_name: &str) -> Result<Self> {
        Ok(Self {
            name: subtree_name.to_string(),
            atomic_op: op.clone(),
        })
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl KVStore {
    /// Gets a value associated with a key from the SubTree.
    ///
    /// This method prioritizes returning data staged within the current `AtomicOp`.
    /// If the key is not found in the staged data it retrieves the fully merged historical
    /// state from the backend up to the point defined by the `AtomicOp`'s parents and
    /// returns the value from there.
    ///
    /// # Arguments
    /// * `key` - The key to retrieve the value for.
    ///
    /// # Returns
    /// A `Result` containing the value string if found, or `Error::NotFound`.
    pub fn get(&self, key: &str) -> Result<String> {
        // First check if there's any data in the atomic op itself
        let local_data: Result<KVOverWrite> = self.atomic_op.get_local_data(&self.name);

        // If there's data in the operation and it contains the key, return that
        if let Ok(data) = local_data {
            if let Some(value) = data.get(key) {
                return Ok(value.clone());
            }
        }

        // Otherwise, get the full state from the backend
        let data: KVOverWrite = self.atomic_op.get_full_state(&self.name)?;

        // Get the value
        match data.get(key) {
            Some(value) => Ok(value.clone()),
            None => Err(Error::NotFound),
        }
    }

    /// Stages the setting of a key-value pair within the associated `AtomicOp`.
    ///
    /// This method updates the `KVOverWrite` data held within the `AtomicOp` for this
    /// `KVStore` instance's subtree name. The change is **not** persisted to the backend
    /// until the `AtomicOp::commit()` method is called.
    ///
    /// Calling this method on a `KVStore` obtained via `Tree::get_subtree_viewer` is possible
    /// but the changes will be ephemeral and discarded, as the viewer's underlying `AtomicOp`
    /// is not intended to be committed.
    ///
    /// # Arguments
    /// * `key` - The key to set.
    /// * `value` - The value to associate with the key.
    ///
    /// # Returns
    /// A `Result<()>` indicating success or an error during serialization or staging.
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        // Get current data from the atomic op, or create new if not existing
        let mut data = self
            .atomic_op
            .get_local_data::<KVOverWrite>(&self.name)
            .unwrap_or_default();

        // Update the data
        data.set(key.to_string(), value.to_string());

        // Serialize and update the atomic op
        let serialized = serde_json::to_string(&data)?;
        self.atomic_op.update_subtree(&self.name, &serialized)
    }

    /// Retrieves all key-value pairs, merging staged data with historical state.
    ///
    /// This method combines the data staged within the current `AtomicOp` with the
    /// fully merged historical state from the backend, providing a complete view
    /// of the key-value store as it would appear if the operation were committed.
    /// The staged data takes precedence in case of conflicts (overwrites).
    ///
    /// # Returns
    /// A `Result` containing the merged `KVOverWrite` data structure.
    pub fn get_all(&self) -> Result<KVOverWrite> {
        // First get the local data directly from the atomic op
        let local_data = self.atomic_op.get_local_data::<KVOverWrite>(&self.name);

        // Get the full state from the backend
        let mut data = self.atomic_op.get_full_state::<KVOverWrite>(&self.name)?;

        // If there's also local data, merge it with the full state
        if let Ok(local) = local_data {
            data = data.merge(&local)?;
        }

        Ok(data)
    }
}
