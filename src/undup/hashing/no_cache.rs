use std::{io, path::Path};

use crate::hashing::HashingBackend;

pub struct HashingNoCache { }

impl HashingNoCache {
    pub fn new() -> Self {
        Self {} 
    }
}

impl HashingBackend for HashingNoCache {
    fn hash_file(&self, path: &Path) -> io::Result<String> {
        super::hash_file(path)
    }
}

