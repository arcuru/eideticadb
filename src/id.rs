/// An ID in Eidetica is an identifier for an entry or object.
///
/// The actual ID is a hash of the data contained in the entry or object.
///
/// IDs are viewed as SRI compatible hashes, and any hash of the same data is equivalent?
pub struct ID {
    /// List of known equivalent hashes.
    known_hashes: Vec<Hash>,
}

enum Hash {
    SHA256(String),
}

impl ID {
    pub fn new_from_hash(hash: Hash) -> Self {
        Self {
            known_hashes: vec![hash],
        }
    }

    pub fn add_hash(&mut self, hash: Hash) {
        self.known_hashes.push(hash);
    }

    pub fn get_hash(&self, hash_type: HashType) -> &Hash {
        self.known_hashes.last().unwrap()
    }
}
