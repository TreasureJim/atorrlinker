use sha2::Digest as _;
use std::{
    fs::File,
    io::{self, BufReader, Read as _},
    path::{Path, PathBuf},
};

pub struct MatchingFile {
    src_path: PathBuf,
    dest_path: PathBuf,
}

/**
fn find_matching_files

- find and compute hashes of files in target_dir that aren't symlinked
- find files in source_dir except ones that were symlinked


- find files
- compute hash
- check match

two separate
**/

type Hash = String;

#[derive(Debug)]
enum FileType {
    File(PathBuf),
    Symlink { source: PathBuf, target: PathBuf }, // Directory,
}

impl FileType {
    fn src_path(&self) -> &Path {
        match self {
            Self::File(path) => path,
            Self::Symlink { source, target } => source,
        }
    }
}

#[derive(Default)]
struct DiscoveredFiles {
    files: std::collections::HashMap<Hash, Vec<FileType>>,
}

impl DiscoveredFiles {
    fn add_hash(&mut self, hash: Hash, path: FileType) {
        self.files.entry(hash).or_default().push(path);
    }

    fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn hash_file(path: &Path) -> io::Result<String> {
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

/// Traverse through any subdirectories and find any files that exist then hash them.
/// Records any symlinks found
fn find_and_hash_files(
    disc_files: &mut DiscoveredFiles,
    dir: &Path,
    // skip_cb: &dyn Fn(&Path) -> bool,
) -> std::io::Result<()> {
    let mut queue = std::collections::VecDeque::<PathBuf>::from(vec![dir.to_path_buf()]);

    if !dir.metadata()?.is_dir() {
        disc_files.add_hash(
            hash_file(&dir.to_path_buf())?,
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
                    disc_files.add_hash(hash_file(&entry.path())?, FileType::File(entry.path()));
                }
                ft if ft.is_symlink() => disc_files.add_hash(
                    hash_file(&entry.path())?,
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

/// Hash files in source and target directories and find matches between them.
/// Target directory will contain files that will be deleted and symlinked to the target dirs
pub fn find_matching_files(
    source_dir: &[&Path],
    target_dir: &[&Path],
) -> io::Result<Vec<MatchingFile>> {
    let mut source_hashes = DiscoveredFiles::default();
    let mut target_hashes = DiscoveredFiles::default();

    for dir in source_dir {
        let _ = find_and_hash_files(&mut source_hashes, dir)
            .inspect_err(|e| log::error!("IO error in {dir:?}: {e}"))?;
    }
    for dir in target_dir {
        let _ = find_and_hash_files(&mut target_hashes, dir)
            .inspect_err(|e| log::error!("IO error in {dir:?}: {e}"))?;
    }

    let mut matches = Vec::new();
    for target in target_hashes.files.into_iter() {
        // Find first symlink and use as source if exists
        let source_path = if let Some(FileType::Symlink {
            source: _,
            target: sym_target,
        }) = target
            .1
            .iter()
            .find(|p| matches!(**p, FileType::Symlink { .. }))
        {
            // If symlink is the only one then skip the hash
            if target.1.len() == 1 {
                continue;
            }
            sym_target
        }
        // Find source in source directories
        else if let Some(source_file) = source_hashes.files.get(&target.0) {
            source_file[0].src_path()
        }
        // Couldn't find matching source
        else {
            log::info!(
                "Couldn't find file to symlink to for the following files: {0:?}",
                target.1
            );
            continue;
        };

        // Check for non-linked file
        for f in target.1.iter().filter(|f| matches!(f, FileType::File(_))) {
            matches.push(MatchingFile {
                src_path: source_path.to_path_buf(),
                dest_path: f.src_path().to_path_buf(),
            });
        }
    }

    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    mod find_and_hash_files {
        use super::*;

        #[test]
        fn test_empty_directory() {
            let temp_dir = tempdir().unwrap();
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path()).unwrap();

            assert!(result.files.is_empty());
        }

        #[test]
        fn test_single_file() {
            let temp_dir = tempdir().unwrap();
            let file_path = temp_dir.path().join("test.txt");

            let mut file = File::create(&file_path).unwrap();
            file.write_all(b"Hello, World!").unwrap();

            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path()).unwrap();

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

            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path()).unwrap();

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

            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path()).unwrap();

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

            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path()).unwrap();

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
            if let FileType::Symlink { source, target } = symlink {
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
            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, &file_path).unwrap();

            assert_eq!(result.files.len(), 1);

            let (hash, file_types) = result.files.iter().next().unwrap();
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

            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path()).unwrap();

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

            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path()).unwrap();

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

            let mut result = DiscoveredFiles::default();
            find_and_hash_files(&mut result, temp_dir.path()).unwrap();

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

    mod find_matching_files {
        use super::*;
        use tempfile::TempDir;

        // Helper function to create test files with content
        fn create_test_file(path: &Path, content: &str) -> io::Result<()> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = File::create(path)?;
            file.write_all(content.as_bytes())?;
            Ok(())
        }

        // Helper function to create a symlink (Unix-like systems)
        #[cfg(unix)]
        fn create_symlink(original: &Path, link: &Path) -> io::Result<()> {
            if let Some(parent) = link.parent() {
                fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(original, link)
        }

        // For Windows, you'd need a different implementation
        #[cfg(windows)]
        fn create_symlink(original: &Path, link: &Path) -> io::Result<()> {
            if let Some(parent) = link.parent() {
                fs::create_dir_all(parent)?;
            }
            std::os::windows::fs::symlink_file(original, link)
        }

        #[test]
        fn test_find_matching_files_basic_matching() {
            let temp_dir = TempDir::new().unwrap();
            let source_dir = temp_dir.path().join("source");
            let target_dir = temp_dir.path().join("target");

            fs::create_dir_all(&source_dir).unwrap();
            fs::create_dir_all(&target_dir).unwrap();

            // Create identical files in both directories
            create_test_file(&source_dir.join("file1.txt"), "content1").unwrap();
            create_test_file(&target_dir.join("file1.txt"), "content1").unwrap();
            create_test_file(&source_dir.join("file2.txt"), "content2").unwrap();
            create_test_file(&target_dir.join("file2.txt"), "content2").unwrap();

            let matches = find_matching_files(&[&source_dir], &[&target_dir]).unwrap();

            assert_eq!(matches.len(), 2);
            assert!(
                matches.iter().any(
                    |m| m.src_path.ends_with("file1.txt") && m.dest_path.ends_with("file1.txt")
                )
            );
            assert!(
                matches.iter().any(
                    |m| m.src_path.ends_with("file2.txt") && m.dest_path.ends_with("file2.txt")
                )
            );
        }

        #[test]
        fn test_find_matching_files_different_content() {
            let temp_dir = TempDir::new().unwrap();
            let source_dir = temp_dir.path().join("source");
            let target_dir = temp_dir.path().join("target");

            fs::create_dir_all(&source_dir).unwrap();
            fs::create_dir_all(&target_dir).unwrap();

            // Create files with different content (different hashes)
            create_test_file(&source_dir.join("file1.txt"), "content1").unwrap();
            create_test_file(&target_dir.join("file1.txt"), "different_content").unwrap();

            let matches = find_matching_files(&[&source_dir], &[&target_dir]).unwrap();

            // Files with different content should not match
            assert_eq!(matches.len(), 0);
        }

        #[test]
        fn test_find_matching_files_multiple_directories() {
            let temp_dir = TempDir::new().unwrap();
            let source_dir1 = temp_dir.path().join("source1");
            let source_dir2 = temp_dir.path().join("source2");
            let target_dir1 = temp_dir.path().join("target1");
            let target_dir2 = temp_dir.path().join("target2");

            fs::create_dir_all(&source_dir1).unwrap();
            fs::create_dir_all(&source_dir2).unwrap();
            fs::create_dir_all(&target_dir1).unwrap();
            fs::create_dir_all(&target_dir2).unwrap();

            create_test_file(&source_dir1.join("file1.txt"), "content1").unwrap();
            create_test_file(&source_dir2.join("file2.txt"), "content2").unwrap();
            create_test_file(&target_dir1.join("file1.txt"), "content1").unwrap();
            create_test_file(&target_dir2.join("file2.txt"), "content2").unwrap();

            let matches =
                find_matching_files(&[&source_dir1, &source_dir2], &[&target_dir1, &target_dir2])
                    .unwrap();

            assert_eq!(matches.len(), 2);
        }

        #[test]
        #[cfg(unix)] // Symlink test is platform-specific
        fn test_find_matching_files_with_symlinks() {
            let temp_dir = TempDir::new().unwrap();
            let source_dir = temp_dir.path().join("source");
            let target_dir = temp_dir.path().join("target");

            fs::create_dir_all(&source_dir).unwrap();
            fs::create_dir_all(&target_dir).unwrap();

            // Create source file
            create_test_file(&source_dir.join("file1.txt"), "content1").unwrap();

            // Create symlink in target directory
            create_symlink(&source_dir.join("file1.txt"), &target_dir.join("file1.txt")).unwrap();

            let matches = find_matching_files(&[&source_dir], &[&target_dir]).unwrap();

            // Should skip the symlink-only case
            assert_eq!(matches.len(), 0);
        }

        #[test]
        fn test_find_matching_files_nonexistent_directories() {
            let temp_dir = TempDir::new().unwrap();
            let nonexistent_dir = temp_dir.path().join("nonexistent");

            let result = find_matching_files(&[&nonexistent_dir], &[&nonexistent_dir]);

            assert!(result.is_err());
        }

        #[test]
        fn test_find_matching_files_empty_directories() {
            let temp_dir = TempDir::new().unwrap();
            let empty_dir1 = temp_dir.path().join("empty1");
            let empty_dir2 = temp_dir.path().join("empty2");

            fs::create_dir_all(&empty_dir1).unwrap();
            fs::create_dir_all(&empty_dir2).unwrap();

            let matches = find_matching_files(&[&empty_dir1], &[&empty_dir2]).unwrap();

            assert_eq!(matches.len(), 0);
        }

        #[test]
        fn test_find_matching_files_subdirectories() {
            let temp_dir = TempDir::new().unwrap();
            let source_dir = temp_dir.path().join("source");
            let target_dir = temp_dir.path().join("target");

            fs::create_dir_all(&source_dir.join("subdir")).unwrap();
            fs::create_dir_all(&target_dir.join("subdir")).unwrap();

            create_test_file(&source_dir.join("subdir/file1.txt"), "content1").unwrap();
            create_test_file(&target_dir.join("subdir/file1.txt"), "content1").unwrap();

            let matches = find_matching_files(&[&source_dir], &[&target_dir]).unwrap();

            assert_eq!(matches.len(), 1);
            assert!(matches[0].src_path.ends_with("subdir/file1.txt"));
            assert!(matches[0].dest_path.ends_with("subdir/file1.txt"));
        }

        #[test]
        fn test_find_matching_files_partial_matches() {
            let temp_dir = TempDir::new().unwrap();
            let source_dir = temp_dir.path().join("source");
            let target_dir = temp_dir.path().join("target");

            fs::create_dir_all(&source_dir).unwrap();
            fs::create_dir_all(&target_dir).unwrap();

            // Only some files match
            create_test_file(&source_dir.join("match1.txt"), "content1").unwrap();
            create_test_file(&source_dir.join("match2.txt"), "content2").unwrap();
            create_test_file(&source_dir.join("nomatch.txt"), "source_content").unwrap();

            create_test_file(&target_dir.join("match1.txt"), "content1").unwrap();
            create_test_file(&target_dir.join("match2.txt"), "content2").unwrap();
            create_test_file(&target_dir.join("nomatch.txt"), "target_content").unwrap();

            let matches = find_matching_files(&[&source_dir], &[&target_dir]).unwrap();

            assert_eq!(matches.len(), 2);
            assert!(matches.iter().all(
                |m| m.dest_path.ends_with("match1.txt") || m.dest_path.ends_with("match2.txt")
            ));
        }

        #[test]
        fn test_find_matching_files_duplicate_hashes() {
            let temp_dir = TempDir::new().unwrap();
            let source_dir = temp_dir.path().join("source");
            let target_dir = temp_dir.path().join("target");

            fs::create_dir_all(&source_dir).unwrap();
            fs::create_dir_all(&target_dir).unwrap();

            // Create files with same content (same hash) but different names
            create_test_file(&source_dir.join("file1.txt"), "same_content").unwrap();
            create_test_file(&source_dir.join("file2.txt"), "same_content").unwrap();
            create_test_file(&target_dir.join("target_file.txt"), "same_content").unwrap();

            let matches = find_matching_files(&[&source_dir], &[&target_dir]).unwrap();

            // Should match based on hash, regardless of filename
            assert_eq!(matches.len(), 1);
            assert!(matches[0].dest_path.ends_with("target_file.txt"));
        }
    }
}
