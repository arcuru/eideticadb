use crate::entry::Entry;
use crate::Error;
use crate::Result;
use std::collections::HashMap;

/// Backend trait abstracting the underlying storage.
pub trait Backend: Send + Sync {
    fn get(&self, id: &str) -> Result<&Entry>;
    fn put(&mut self, entry: Entry) -> Result<()>;
}

pub struct InMemoryBackend {
    entries: HashMap<String, Entry>,
}

impl Backend for InMemoryBackend {
    fn get(&self, id: &str) -> Result<&Entry> {
        self.entries.get(id).ok_or(Error::NotFound)
    }

    fn put(&mut self, entry: Entry) -> Result<()> {
        self.entries.insert(entry.id(), entry);
        Ok(())
    }
}
