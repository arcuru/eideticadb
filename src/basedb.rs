use crate::backend::Backend;
use crate::entry::{Entry, Parents, CRDT, ID};
use crate::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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

    /// Create a new tree in the database.
    pub fn new_tree(&self, settings: CRDT) -> Result<Tree> {
        Tree::new(settings, self.backend.clone())
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
        let entry = Entry::new(
            String::new(), // Empty string for root, as it's the first entry
            "root".to_string(),
            settings,
            Parents::new(vec![], vec![]),
            HashMap::new(),
        );

        let root_id = entry.id();

        // Insert the entry into the backend
        {
            let mut backend_guard = backend.lock().map_err(|_| {
                crate::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to lock backend",
                ))
            })?;

            backend_guard.put(entry)?;
        }

        Ok(Self {
            root: root_id,
            backend,
        })
    }

    /// Get the ID of the root entry
    pub fn root_id(&self) -> &ID {
        &self.root
    }

    /// Retrieve the root entry from the backend
    pub fn get_root(&self) -> Result<Entry> {
        let backend_guard = self.backend.lock().map_err(|_| {
            crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to lock backend",
            ))
        })?;

        backend_guard.get(&self.root).cloned()
    }
}
