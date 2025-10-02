pub mod no_cache;
pub mod file_cache;

use sha2::Digest as _;
use std::{
    fs::File,
    io::{self, BufReader, Read as _},
    path::Path,
};

pub(crate) fn hash_file(path: &Path) -> io::Result<String> {
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


pub trait HashingBackend {
    fn hash_file(&self, path: &Path) -> io::Result<String>;
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

        assert_eq!(hash_file(&path).unwrap(), "9F86D081884C7D659A2FEAA0C55AD015A3BF4F1B2B0B822CD15D6C15B0F00A08");
    }
}
