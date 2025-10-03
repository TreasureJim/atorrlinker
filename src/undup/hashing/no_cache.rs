use std::{io, path::Path};

use crate::hashing::{compute_file_hash, HashCache};

pub struct HashingNoCache {}

impl HashingNoCache {
    pub fn new() -> Self {
        Self {}
    }
}

impl HashCache for HashingNoCache {
    fn retrieve_hash(
        &mut self,
        _path: &Path,
    ) -> Option<(String, std::time::SystemTime)> {
        None
    }
    fn cache_hash(&mut self, _path: &Path, _hash: &str, _last_modified: &std::time::SystemTime) { }

    fn hash_file(&mut self, path: &Path) -> io::Result<String> {
        compute_file_hash(path)
    }
}
