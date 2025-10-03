use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::Hash;
use super::HashCache;

type HashesHashmap = HashMap<PathBuf, (Hash, SystemTime)>;

pub struct HashingFileCache {
    path: PathBuf,
    hashes: HashesHashmap,
}

impl HashingFileCache {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        if !path.exists() {
            return Ok(Self {
                path,
                hashes: HashMap::new()
            });
        } 

        Ok(Self {
            hashes: HashingFileCache::deseralise_hashes(&fs::read_to_string(&path)?)?,
            path,
        })
    }

    fn serialise_hashes(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.hashes)
            .inspect_err(|e| log::error!("Failed to serialise hashes from cache: {}", e))
    }

    fn deseralise_hashes(s: &str) -> Result<HashesHashmap, serde_json::Error> {
        serde_json::from_str(s)
    }

    fn get_file_last_modified(path: &Path) -> io::Result<SystemTime> {
        Ok(fs::metadata(path)?.modified().expect("Retrieving last modified information on files not available on this system. Do not use cache."))
    }

    fn compute_and_cache_hash(
        &mut self,
        path: &Path,
        last_modified: &SystemTime,
    ) -> io::Result<Hash> {
        let hash = super::compute_file_hash(path)?;
        self.cache_hash(path, &hash, &last_modified);
        return Ok(hash);
    }
}

impl Drop for HashingFileCache {
    fn drop(&mut self) {
        let file = fs::File::create(&self.path);
        if let Err(e) = file {
            log::error!(
                "Failed to write hash cache file to {}: {}",
                self.path.display(),
                e
            );
            return;
        }
        let mut file = file.unwrap();

        if let Ok(serialised_hashes) = self.serialise_hashes() {
            if let Err(e) = file.write_all(serialised_hashes.as_bytes()) {
                log::error!("Unable to write cached hashes to: {:?}: {e}", self.path);
            }
        }
    }
}

impl HashCache for HashingFileCache {
    fn retrieve_hash(&self, path: &Path) -> Option<(String, std::time::SystemTime)> {
        self.hashes.get(path).cloned()
    }

    fn cache_hash(&mut self, path: &Path, hash: &str, last_modified: &std::time::SystemTime) {
        self.hashes
            .entry(path.to_path_buf())
            .insert_entry((hash.to_string(), last_modified.clone()));
    }

    fn hash_file(&mut self, path: &Path) -> io::Result<String> {
        if let Some((hash_cache, last_modified_cache)) = self.retrieve_hash(path) {
            let last_modified = HashingFileCache::get_file_last_modified(path)?;
            if last_modified > last_modified_cache {
                log::debug!("Cache: Found outdated cache hash value for {:?}", path);
                return self.compute_and_cache_hash(path, &last_modified);
            } else {
                log::debug!("Cache: Found cached hash value for {:?}", path);
                return Ok(hash_cache);
            }
        } else {
            log::debug!("Cache: No cached hash value for {:?}", path);
            let last_modified = HashingFileCache::get_file_last_modified(path)?;
            let hash = super::compute_file_hash(path)?;
            self.cache_hash(path, &hash, &last_modified);
            return Ok(hash);
        }
    }
}
