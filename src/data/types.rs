use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A trait for data that can be serialized to and from a RawData.
///
/// Users can implement this for any type they wish to store in the tree.
pub trait Data: Serialize + for<'de> Deserialize<'de> {}

/// A trait for data that can be merged with another instance of itself.
///
/// This defines the very simple Conflict-free Replicated Data Type (CRDT)
/// requirements for data in the tree. This type is only responsible for
/// maintaining state given an ordered list of operations.
pub trait CRDT: Default + Data {
    /// Merge another CRDT into this one.
    ///
    /// The order matters, `self` is the older value, and we are adding `other` on top of it.
    fn merge(&self, other: &Self) -> Result<Self>
    where
        Self: Sized;
}

/// A simple key-value CRDT implementation that uses the "last write wins" strategy.
/// When merging, values from the other map will overwrite values in the current map.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct KVOverWrite {
    data: HashMap<String, String>,
}

impl Data for KVOverWrite {}

impl CRDT for KVOverWrite {
    fn merge(&self, other: &Self) -> Result<Self> {
        let mut data = self.data.clone();
        data.extend(other.data.clone());
        Ok(KVOverWrite { data })
    }
}

// Additional helper methods for KVOverWrite for ease of use
impl KVOverWrite {
    /// Create a new empty KVOverWrite
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a KVOverWrite from an existing HashMap
    pub fn from_hashmap(data: HashMap<String, String>) -> Self {
        Self { data }
    }

    /// Get a value by key
    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }

    /// Set a key-value pair
    pub fn set(&mut self, key: String, value: String) -> &mut Self {
        self.data.insert(key, value);
        self
    }

    /// Remove a key-value pair
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.data.remove(key)
    }

    /// Get the underlying HashMap
    pub fn as_hashmap(&self) -> &HashMap<String, String> {
        &self.data
    }

    /// Get a mutable reference to the underlying HashMap
    pub fn as_hashmap_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.data
    }
}

#[cfg(test)]
mod tests {
    use super::{KVOverWrite, CRDT};
    use std::collections::HashMap;

    #[test]
    fn test_kvoverwrite_new() {
        // Test creation of a new KVOverWrite
        let kv = KVOverWrite::new();
        assert_eq!(kv.as_hashmap().len(), 0);
    }

    #[test]
    fn test_kvoverwrite_from_hashmap() {
        // Test creation from an existing HashMap
        let mut data = HashMap::new();
        data.insert("key1".to_string(), "value1".to_string());
        data.insert("key2".to_string(), "value2".to_string());

        let kv = KVOverWrite::from_hashmap(data.clone());
        assert_eq!(kv.as_hashmap().len(), 2);
        assert_eq!(kv.get("key1"), Some(&"value1".to_string()));
        assert_eq!(kv.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_kvoverwrite_set_get() {
        // Test setting and getting values
        let mut kv = KVOverWrite::new();

        // Set a value and check it was set
        kv.set("key1".to_string(), "value1".to_string());
        assert_eq!(kv.get("key1"), Some(&"value1".to_string()));

        // Update an existing value
        kv.set("key1".to_string(), "updated".to_string());
        assert_eq!(kv.get("key1"), Some(&"updated".to_string()));

        // Check a non-existent key
        assert_eq!(kv.get("nonexistent"), None);
    }

    #[test]
    fn test_kvoverwrite_remove() {
        // Test removing values
        let mut kv = KVOverWrite::new();

        // Add a value then remove it
        kv.set("key1".to_string(), "value1".to_string());
        assert_eq!(kv.get("key1"), Some(&"value1".to_string()));

        let removed = kv.remove("key1");
        assert_eq!(removed, Some("value1".to_string()));
        assert_eq!(kv.get("key1"), None);

        // Try removing a non-existent key
        let removed = kv.remove("nonexistent");
        assert_eq!(removed, None);
    }

    #[test]
    fn test_kvoverwrite_merge() {
        // Test merging two KVOverWrite instances
        let mut kv1 = KVOverWrite::new();
        kv1.set("key1".to_string(), "value1".to_string())
            .set("key2".to_string(), "value2".to_string());

        let mut kv2 = KVOverWrite::new();
        kv2.set("key2".to_string(), "updated".to_string())
            .set("key3".to_string(), "value3".to_string());

        // Merge kv2 into kv1
        let merged = kv1.merge(&kv2).expect("Merge should succeed");

        // Check the merged result
        assert_eq!(merged.get("key1"), Some(&"value1".to_string())); // Kept from kv1
        assert_eq!(merged.get("key2"), Some(&"updated".to_string())); // Overwritten by kv2
        assert_eq!(merged.get("key3"), Some(&"value3".to_string())); // Added from kv2
    }

    #[test]
    fn test_kvoverwrite_as_hashmap_mut() {
        // Test mutable access to the underlying HashMap
        let mut kv = KVOverWrite::new();

        // Modify through the KVOverWrite methods
        kv.set("key1".to_string(), "value1".to_string());

        // Modify through the mutable HashMap reference
        kv.as_hashmap_mut()
            .insert("key2".to_string(), "value2".to_string());

        // Verify both modifications worked
        assert_eq!(kv.get("key1"), Some(&"value1".to_string()));
        assert_eq!(kv.get("key2"), Some(&"value2".to_string()));
    }
}
