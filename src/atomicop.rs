use crate::data::CRDT;
use crate::entry::Entry;
use crate::entry::ID;
use crate::subtree::SubTree;
use crate::tree::Tree;
use crate::Error;
use crate::Result;
use std::cell::RefCell;
use std::rc::Rc;

/// Represents a single, atomic transaction for modifying a `Tree`.
///
/// An `AtomicOp` encapsulates a new `Entry` being constructed. Users interact with
/// specific `SubTree` instances obtained via `AtomicOp::get_subtree` to stage changes.
/// All staged changes across different subtrees within the operation are recorded
/// in the internal `Entry`.
///
/// Calling `commit()` finalizes the operation, calculates the new entry's ID,
/// sets the correct parent links based on the tree's state when the operation began,
/// removes any subtrees that didn't have data staged, and persists the resulting
/// `Entry` to the backend.
///
/// `AtomicOp` instances are typically created via `Tree::new_operation()`.
#[derive(Clone)]
pub struct AtomicOp {
    /// The entry being modified, wrapped in Option to support consuming on commit
    entry: Rc<RefCell<Option<Entry>>>,
    /// The tree this operation belongs to
    tree: Tree,
}

impl AtomicOp {
    /// Creates a new atomic operation for a specific `Tree`.
    ///
    /// Initializes an internal `Entry` with its main parent pointers set to the
    /// current tips of the target `Tree`. This captures the state upon which
    /// the operation builds.
    ///
    /// This is typically called internally by `Tree::new_operation()`.
    ///
    /// # Arguments
    /// * `tree` - The `Tree` this operation will modify.
    pub(crate) fn new(tree: &Tree) -> Result<Self> {
        // Create a new entry
        let mut entry = Entry::new(tree.root_id().clone(), "".to_string());

        // Get current tree tips
        let tree_tips = {
            let backend_guard = tree.lock_backend()?;
            backend_guard.get_tips(tree.root_id())?
        };
        entry.set_parents(tree_tips);

        Ok(Self {
            entry: Rc::new(RefCell::new(Some(entry))),
            tree: tree.clone(),
        })
    }

    /// Stages an update for a specific subtree within this atomic operation.
    ///
    /// This method is primarily intended for internal use by `SubTree` implementations
    /// (like `KVStore::set`). It records the serialized `data` for the given `subtree`
    /// name within the operation's internal `Entry`.
    ///
    /// If this is the first modification to the named subtree within this operation,
    /// it also fetches and records the current tips of that subtree from the backend
    /// to set the correct `subtree_parents` for the new entry.
    ///
    /// # Arguments
    /// * `subtree` - The name of the subtree to update.
    /// * `data` - The serialized CRDT data to stage for the subtree.
    ///
    /// # Returns
    /// A `Result<()>` indicating success or an error.
    pub(crate) fn update_subtree(&self, subtree: &str, data: &str) -> Result<()> {
        let mut entry_ref = self.entry.borrow_mut();
        let entry = entry_ref.as_mut().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Operation has already been committed",
            ))
        })?;

        // If we haven't cached the tips for this subtree yet, get them now
        if !entry.subtrees().contains(&subtree.to_string()) {
            let backend_guard = self.tree.lock_backend()?;
            // FIXME: we should get the subtree tips while still using the parent pointers
            let tips = backend_guard.get_subtree_tips(self.tree.root_id(), subtree)?;
            entry.set_subtree_data(subtree.to_string(), data.to_string())?;
            entry.set_subtree_parents(subtree, tips);
        } else {
            // Add/Update the subtree with the data
            entry.set_subtree_data(subtree.to_string(), data.to_string())?;
        }

        Ok(())
    }

    /// Gets a handle to a specific `SubTree` for modification within this operation.
    ///
    /// This method creates and returns an instance of the specified `SubTree` type `T`,
    /// associated with this `AtomicOp`. The returned `SubTree` handle can be used to
    /// stage changes (e.g., using `KVStore::set`) for the `subtree_name`.
    /// These changes are recorded within this `AtomicOp`.
    ///
    /// If this is the first time this `subtree_name` is accessed within the operation,
    /// its parent tips will be fetched and stored.
    ///
    /// # Type Parameters
    /// * `T` - The concrete `SubTree` implementation type to create.
    ///
    /// # Arguments
    /// * `subtree_name` - The name of the subtree to get a modification handle for.
    ///
    /// # Returns
    /// A `Result<T>` containing the `SubTree` handle.
    pub fn get_subtree<T>(&self, subtree_name: &str) -> Result<T>
    where
        T: SubTree,
    {
        {
            let mut entry_ref = self.entry.borrow_mut();
            let entry = entry_ref.as_mut().ok_or_else(|| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Operation has already been committed",
                ))
            })?;

            // If we haven't cached the tips for this subtree yet, get them now
            if !entry.subtrees().contains(&subtree_name.to_string()) {
                let backend_guard = self.tree.lock_backend()?;
                // FIXME: we should get the subtree tips while still using the parent pointers
                let tips = backend_guard.get_subtree_tips(self.tree.root_id(), subtree_name)?;
                entry.set_subtree_data(subtree_name.to_string(), "".to_string())?;
                entry.set_subtree_parents(subtree_name, tips);
            }
        }

        // Now create the SubTree with the atomic operation
        T::new(self, subtree_name)
    }

    /// Gets the currently staged data for a specific subtree within this operation.
    ///
    /// This is intended for use by `SubTree` implementations to retrieve the data
    /// they have staged locally within the `AtomicOp` before potentially merging
    /// it with historical data.
    ///
    /// # Type Parameters
    /// * `T` - The data type (expected to be a CRDT) to deserialize the staged data into.
    ///
    /// # Arguments
    /// * `subtree_name` - The name of the subtree whose staged data is needed.
    ///
    /// # Returns
    /// A `Result<T>` containing the deserialized staged data. Returns `Ok(T::default())`
    /// if no data has been staged for this subtree in this operation yet.
    pub fn get_local_data<T>(&self, subtree_name: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        let entry_ref = self.entry.borrow();
        let entry = entry_ref.as_ref().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Operation has already been committed",
            ))
        })?;

        if let Ok(data) = entry.data(subtree_name) {
            serde_json::from_str(data).map_err(Error::from)
        } else {
            // If subtree doesn't exist or has no data, return default
            Ok(T::default())
        }
    }

    /// Gets the fully merged historical state of a subtree up to the point this operation began.
    ///
    /// This retrieves all relevant historical entries for the `subtree_name` from the backend,
    /// considering the parent tips recorded when this `AtomicOp` was created (or when the
    /// subtree was first accessed within the operation). It deserializes the data from each
    /// relevant entry into the CRDT type `T` and merges them according to `T`'s `CRDT::merge`
    /// implementation.
    ///
    /// This is intended for use by `SubTree` implementations (e.g., in their `get` or `get_all` methods)
    /// to provide the historical context against which staged changes might be applied or compared.
    ///
    /// # Type Parameters
    /// * `T` - The CRDT type to deserialize and merge the historical subtree data into.
    ///
    /// # Arguments
    /// * `subtree_name` - The name of the subtree.
    ///
    /// # Returns
    /// A `Result<T>` containing the merged historical data of type `T`. Returns `Ok(T::default())`
    /// if the subtree has no history prior to this operation.
    pub(crate) fn get_full_state<T>(&self, subtree_name: &str) -> Result<T>
    where
        T: CRDT,
    {
        // Get the entry to get parent pointers
        let mut entry_ref = self.entry.borrow_mut();
        let entry = entry_ref.as_mut().ok_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Operation has already been committed",
            ))
        })?;

        // If we haven't cached the tips for this subtree yet, get them now
        if !entry.subtrees().contains(&subtree_name.to_string()) {
            let backend_guard = self.tree.lock_backend()?;
            // FIXME: we should get the subtree tips while still using the parent pointers
            let tips = backend_guard.get_subtree_tips(self.tree.root_id(), subtree_name)?;
            entry.set_subtree_data(subtree_name.to_string(), "".to_string())?;
            entry.set_subtree_parents(subtree_name, tips);
        }

        // Get the parent pointers for this subtree
        let parents = entry.subtree_parents(subtree_name).unwrap_or_default();

        // If there are no parents, return a default
        if parents.is_empty() {
            return Ok(T::default());
        }

        // Get the entries from the backend up to these parent pointers
        let backend_guard = self.tree.lock_backend()?;
        let entries =
            backend_guard.get_subtree_from_tips(self.tree.root_id(), subtree_name, &parents)?;

        // Merge all the entries
        let mut result = T::default();
        for entry in entries {
            if let Ok(data) = entry.data(subtree_name) {
                let parsed: T = serde_json::from_str(data)?;
                result = result.merge(&parsed)?;
            }
        }

        Ok(result)
    }

    /// Commits the atomic operation, persisting the staged changes as a new `Entry`.
    ///
    /// This finalizes the transaction:
    /// 1. Takes ownership of the internal `Entry` being built.
    /// 2. Removes any subtrees from the entry that had no data staged during the operation.
    /// 3. Calculates the final content-addressable ID of the new `Entry`.
    /// 4. Inserts the finalized `Entry` into the backend.
    ///
    /// This operation consumes the `AtomicOp` instance.
    ///
    /// # Returns
    /// A `Result<ID>` containing the `ID` of the newly committed `Entry`.
    /// Returns an error if the operation has already been committed or if there's a backend error.
    pub fn commit(self) -> Result<ID> {
        // Take the entry out of the RefCell, consuming it
        let mut entry = match self.entry.borrow_mut().take() {
            Some(entry) => entry,
            None => {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Operation has already been committed",
                )))
            }
        };

        // Remove subtrees that do not have any data
        entry.remove_empty_subtrees()?;

        // Calculate the entry ID
        let id = entry.id();

        // Insert the entry into the backend
        let mut backend_guard = self.tree.lock_backend()?;
        backend_guard.put(entry)?;

        Ok(id)
    }
}
