use crate::backend::Backend;
use crate::entry::{Entry, Op};
use crate::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Database implementation on top of the backend.
///
/// This database is the base DB, other 'overlays' or 'plugins' should be implemented on top of this.
pub struct BaseDB {
    /// The backend used by the database.
    backend: Arc<Mutex<Box<dyn Backend>>>,
    // Blob storage will be separate
    // storage: IPFS;
}

impl BaseDB {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self {
            backend: Arc::new(Mutex::new(backend)),
        }
    }

    /// Create a new tree in the database.
    pub fn new_tree(&self, settings: String) -> Result<Tree> {
        Tree::new(settings, self.backend.clone())
    }
}

/// Equivalent to a DB table.
pub struct Tree {
    root: String,
    backend: Arc<Mutex<Box<dyn Backend>>>,
}

impl Tree {
    pub fn new(settings: String, backend: Arc<Mutex<Box<dyn Backend>>>) -> Result<Self> {
        let mut data = HashMap::new();
        data.insert("settings".to_string(), settings);

        // Get current timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        // Create a root entry for this tree
        let entry = Entry::new(
            String::new(), // Empty string for root, as it's the first entry
            Op::Root,
            data,
            vec![], // No parents for root
            timestamp,
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
    pub fn root_id(&self) -> &str {
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
