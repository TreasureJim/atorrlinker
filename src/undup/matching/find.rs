use std::path::{Path, PathBuf};
use crate::hashing::Hash;

use crate::hashing::HashCache;

#[derive(Debug)]
pub(super) enum FileType {
    File(PathBuf),
    Symlink { source: PathBuf, target: PathBuf }, // Directory,
}

impl FileType {
    pub fn src_path(&self) -> &Path {
        match self {
            Self::File(path) => path,
            Self::Symlink { source, target: _ } => source,
        }
    }
}

#[derive(Default)]
pub(crate) struct DiscoveredFiles {
    pub files: std::collections::HashMap<Hash, Vec<FileType>>,
}

impl DiscoveredFiles {
    fn add_hash(&mut self, hash: Hash, path: FileType) {
        self.files.entry(hash).or_default().push(path);
    }
}

/// Traverse through any subdirectories and find any files that exist then hash them.
/// Records any symlinks found
pub(crate) fn find_and_hash_files(
    disc_files: &mut DiscoveredFiles,
    dir: &Path,
    hasher: &mut dyn HashCache
) -> std::io::Result<()> {
    let mut queue = std::collections::VecDeque::<PathBuf>::from(vec![dir.to_path_buf()]);

    if !dir.symlink_metadata()?.is_dir() {
        disc_files.add_hash(
            hasher.hash_file(&dir.to_path_buf())?,
            FileType::File(dir.to_path_buf()),
        );
        return Ok(());
    }

    while let Some(dir) = queue.pop_back() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            // if skip_cb(&entry.path()) {
            //     continue;
            // };

            match entry.metadata()? {
                ft if ft.is_dir() => {
                    queue.push_back(entry.path());
                    continue;
                }
                ft if ft.is_file() => {
                    disc_files.add_hash(hasher.hash_file(&entry.path())?, FileType::File(entry.path()));
                }
                ft if ft.is_symlink() => disc_files.add_hash(
                    hasher.hash_file(&entry.path())?,
                    FileType::Symlink {
                        source: entry.path(),
                        target: std::fs::read_link(&entry.path()).expect("Should be a symlink"),
                    },
                ),
                _ => {
                    log::error!("Entry is not directory, file or symlink");
                }
            }
        }
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    mod find_and_hash_files {
        use crate::hashing::no_cache::HashingNoCache;

        use super::*;

        #[test]
        fn test_empty_directory() {
            let temp_dir = tempdir().unwrap();
            let mut result = DiscoveredFiles::default();
            let mut hasher = HashingNoCache::new();
            find_and_hash_files(&mut result, temp_dir.path(), &mut hasher).unwrap();

            assert!(result.files.is_empty());
        }

        #[test]
        fn test_single_file() {
            let temp_dir = tempdir().unwrap();
            let file_path = temp_dir.path().join("test.txt");

            let mut file = File::create(&file_path).unwrap();
            file.write_all(b"Hello, World!").unwrap();

            let mut hasher = HashingNoCache::new();
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path(), &mut hasher).unwrap();

            assert_eq!(result.files.len(), 1);

            let (hash, file_types) = result.files.iter().next().unwrap();
            assert_eq!(file_types.len(), 1);

            // Verify the hash is correct for "Hello, World!"
            let expected_hash = "DFFD6021BB2BD5B0AF676290809EC3A53191DD81C7F70A4B28688A362182986F";
            assert_eq!(hash, expected_hash);

            if let FileType::File(path) = &file_types[0] {
                assert_eq!(path, &file_path);
            } else {
                panic!("Expected FileType::File");
            }
        }

        #[test]
        fn test_nested_directories() {
            let temp_dir = tempdir().unwrap();

            // Create nested directory structure
            let sub_dir = temp_dir.path().join("subdir");
            fs::create_dir(&sub_dir).unwrap();

            let file1 = temp_dir.path().join("file1.txt");
            let file2 = sub_dir.join("file2.txt");

            let mut f1 = File::create(&file1).unwrap();
            f1.write_all(b"File 1 content").unwrap();

            let mut f2 = File::create(&file2).unwrap();
            f2.write_all(b"File 2 content").unwrap();

            let mut hasher = HashingNoCache::new();
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path(), &mut hasher).unwrap();

            // Should have 2 entries in the map (different hashes for different content)
            assert_eq!(result.files.len(), 2);

            // Each hash should have exactly one file
            for (_, file_types) in result.files.iter() {
                assert_eq!(file_types.len(), 1);
            }
        }

        #[test]
        fn test_duplicate_files_same_hash() {
            let temp_dir = tempdir().unwrap();

            let file1 = temp_dir.path().join("file1.txt");
            let file2 = temp_dir.path().join("file2.txt");

            let content = b"Identical content";

            let mut f1 = File::create(&file1).unwrap();
            f1.write_all(content).unwrap();

            let mut f2 = File::create(&file2).unwrap();
            f2.write_all(content).unwrap();

            let mut hasher = HashingNoCache::new();
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path(), &mut hasher).unwrap();

            // Should have only one hash entry (both files have same content)
            assert_eq!(result.files.len(), 1);

            let (_, file_types) = result.files.iter().next().unwrap();
            // But that hash should have two files associated with it
            assert_eq!(file_types.len(), 2);

            let paths: Vec<&PathBuf> = file_types
                .iter()
                .filter_map(|ft| {
                    if let FileType::File(path) = ft {
                        Some(path)
                    } else {
                        None
                    }
                })
                .collect();

            assert_eq!(paths.len(), 2);
            assert!(paths.contains(&&file1));
            assert!(paths.contains(&&file2));
        }

        #[test]
        fn test_symlink_hashing() {
            let temp_dir = tempdir().unwrap();

            // Create a target file
            let target_file = temp_dir.path().join("target.txt");
            let mut file = File::create(&target_file).unwrap();
            file.write_all(b"Target content").unwrap();

            // Create a symlink
            let symlink_path = temp_dir.path().join("link.txt");
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target_file, &symlink_path).unwrap();
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(&target_file, &symlink_path).unwrap();

            let mut hasher = HashingNoCache::new();
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path(), &mut hasher).unwrap();

            // Retrieve the first hash result
            let files_entry = result.files.iter().next().unwrap();

            // Should have 1 entry because both files should share the same hash
            assert_eq!(result.files.len(), 1);
            // Should have 2 entries: one for the file, one for the symlink
            // Note: The symlink will be hashed (its content is the path it points to)
            assert_eq!(files_entry.1.len(), 2);

            // Find the symlink entry
            let symlink = files_entry
                .1
                .iter()
                .filter(|f| matches!(f, FileType::Symlink { .. }))
                .next()
                .expect("There should be at least 1 symlink");

            // The symlink should link back to the original file
            if let FileType::Symlink { source: _, target } = symlink {
                assert_eq!(target, &target_file);
            } else {
                panic!("Is not a symlink somehow!");
            }
        }

        #[test]
        fn test_file_as_input() {
            let temp_dir = tempdir().unwrap();
            let file_path = temp_dir.path().join("single_file.txt");

            let mut file = File::create(&file_path).unwrap();
            file.write_all(b"Single file content").unwrap();

            // Call find_and_hash_files directly on the file path
            let mut hasher = HashingNoCache::new();
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, &file_path, &mut hasher).unwrap();

            assert_eq!(result.files.len(), 1);

            let (_hash, file_types) = result.files.iter().next().unwrap();
            assert_eq!(file_types.len(), 1);

            if let FileType::File(path) = &file_types[0] {
                assert_eq!(path, &file_path);
            } else {
                panic!("Expected FileType::File");
            }
        }

        #[test]
        fn test_mixed_content_with_duplicates() {
            let temp_dir = tempdir().unwrap();

            // Create files with same content
            let file1 = temp_dir.path().join("file1.txt");
            let file2 = temp_dir.path().join("file2.txt");

            let common_content = b"Common content";
            let mut f1 = File::create(&file1).unwrap();
            f1.write_all(common_content).unwrap();
            let mut f2 = File::create(&file2).unwrap();
            f2.write_all(common_content).unwrap();

            // Create file with different content
            let file3 = temp_dir.path().join("file3.txt");
            let mut f3 = File::create(&file3).unwrap();
            f3.write_all(b"Different content").unwrap();

            // Create subdirectory with another file
            let sub_dir = temp_dir.path().join("subdir");
            fs::create_dir(&sub_dir).unwrap();
            let file4 = sub_dir.join("file4.txt");
            let mut f4 = File::create(&file4).unwrap();
            f4.write_all(common_content).unwrap(); // Same content again

            let mut hasher = HashingNoCache::new();
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path(), &mut hasher).unwrap();

            // Should have 2 unique hashes: one for the common content, one for different content
            assert_eq!(result.files.len(), 2);

            // Find the common content hash (should have 3 files)
            let common_hash_entry = result
                .files
                .iter()
                .find(|(_, file_types)| file_types.len() == 3)
                .expect("Should find hash with 3 files");

            let (_, common_files) = common_hash_entry;
            assert_eq!(common_files.len(), 3);

            // Find the unique content hash (should have 1 file)
            let unique_hash_entry = result
                .files
                .iter()
                .find(|(_, file_types)| file_types.len() == 1)
                .expect("Should find hash with 1 file");

            let (_, unique_files) = unique_hash_entry;
            assert_eq!(unique_files.len(), 1);
        }

        #[test]
        fn test_empty_file() {
            let temp_dir = tempdir().unwrap();
            let file_path = temp_dir.path().join("empty.txt");

            File::create(&file_path).unwrap();

            let mut hasher = HashingNoCache::new();
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path(), &mut hasher).unwrap();

            assert_eq!(result.files.len(), 1);

            let (hash, file_types) = result.files.iter().next().unwrap();
            // SHA-256 hash of empty string
            let expected_hash = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
            assert_eq!(hash, expected_hash);

            assert_eq!(file_types.len(), 1);
            if let FileType::File(path) = &file_types[0] {
                assert_eq!(path, &file_path);
            } else {
                panic!("Expected FileType::File");
            }
        }

        #[test]
        fn test_directory_entries_ignored_in_map() {
            let temp_dir = tempdir().unwrap();

            // Create a directory (should not appear in the files map)
            let sub_dir = temp_dir.path().join("subdir");
            fs::create_dir(&sub_dir).unwrap();

            // Create a file in the directory
            let file_path = sub_dir.join("file.txt");
            let mut file = File::create(&file_path).unwrap();
            file.write_all(b"File content").unwrap();

            let mut hasher = HashingNoCache::new();
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path(), &mut hasher).unwrap();

            // Should only have the file, not the directory
            assert_eq!(result.files.len(), 1);

            let (_, file_types) = result.files.iter().next().unwrap();
            assert_eq!(file_types.len(), 1);

            // Verify it's a file, not a directory
            if let FileType::File(path) = &file_types[0] {
                assert_eq!(path, &file_path);
            } else {
                panic!("Expected FileType::File");
            }
        }
    }

}
