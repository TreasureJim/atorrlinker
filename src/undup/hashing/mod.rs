pub mod file_cache;
pub mod no_cache;

use sha2::Digest as _;
use std::{
    fs::{self, File},
    io::{self, BufReader, Read as _},
    path::Path,
};

pub type Hash = String;

pub(crate) fn compute_file_hash(path: &Path) -> io::Result<Hash> {
    log::info!("Hashing: {path:?}");
    let input = File::open(path)?;
    let mut reader = BufReader::new(input);

    let digest = {
        let mut hasher = sha2::Sha256::new();
        let mut buffer = [0; 1024];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        hasher.finalize()
    };
    Ok(format!("{:X}", digest))
}

pub trait HashCache {
    fn retrieve_hash(&mut self, path: &Path)
    -> Option<(String, std::time::SystemTime)>;
    fn cache_hash(&mut self, path: &Path, hash: &str, last_modified: &std::time::SystemTime);
    fn hash_file(&mut self, path: &Path) -> io::Result<String>;
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn test_hash_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let mut file = std::fs::File::create(&path).unwrap();

        write!(&mut file, "test").unwrap();

        assert_eq!(
            compute_file_hash(&path).unwrap(),
            "9F86D081884C7D659A2FEAA0C55AD015A3BF4F1B2B0B822CD15D6C15B0F00A08"
        );
    }
}
