mod find;

use std::{
    io,
    path::{Path, PathBuf},
};

use find::{DiscoveredFiles, FileType, find_and_hash_files};

use crate::hashing::HashingBackend;

pub struct MatchingFile {
    /// The path of the actual file
    pub src_path: PathBuf,
    /// The path of the file to be replaced with a symlink
    pub dest_path: PathBuf,
}

/// Hash files in source and target directories and find matches between them.
/// Target directory will contain files that will be deleted and symlinked to the target dirs
pub fn find_matching_files(
    source_dir: &[impl AsRef<Path>],
    target_dir: &[impl AsRef<Path>],
    hasher: &impl HashingBackend,
) -> io::Result<Vec<MatchingFile>> {
    let mut source_hashes = DiscoveredFiles::default();
    let mut target_hashes = DiscoveredFiles::default();

    for dir in source_dir {
        let dir = dir.as_ref();
        let _ = find_and_hash_files(&mut source_hashes, dir, hasher)
            .inspect_err(|e| log::error!("IO error in {dir:?}: {e}"))?;
    }
    for dir in target_dir {
        let dir = dir.as_ref();
        let _ = find_and_hash_files(&mut target_hashes, dir, hasher)
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
    use crate::hashing::no_cache::HashingNoCache;

    use super::*;
    use std::{fs, io::Write as _};
    use tempfile::TempDir;

    // Helper function to create test files with content
    fn create_test_file(path: &Path, content: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
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

        let hasher = HashingNoCache {};
        let matches = find_matching_files(&[&source_dir], &[&target_dir], &hasher).unwrap();

        assert_eq!(matches.len(), 2);
        assert!(
            matches
                .iter()
                .any(|m| m.src_path.ends_with("file1.txt") && m.dest_path.ends_with("file1.txt"))
        );
        assert!(
            matches
                .iter()
                .any(|m| m.src_path.ends_with("file2.txt") && m.dest_path.ends_with("file2.txt"))
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

        let hasher = HashingNoCache {};
        let matches = find_matching_files(&[&source_dir], &[&target_dir], &hasher).unwrap();

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

        let hasher = HashingNoCache {};
        let matches =
            find_matching_files(&[&source_dir1, &source_dir2], &[&target_dir1, &target_dir2], &hasher)
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

        let hasher = HashingNoCache {};
        let matches = find_matching_files(&[&source_dir], &[&target_dir], &hasher).unwrap();

        // Should skip the symlink-only case
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_find_matching_files_nonexistent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_dir = temp_dir.path().join("nonexistent");

        let hasher = HashingNoCache {};
        let result = find_matching_files(&[&nonexistent_dir], &[&nonexistent_dir], &hasher);

        assert!(result.is_err());
    }

    #[test]
    fn test_find_matching_files_empty_directories() {
        let temp_dir = TempDir::new().unwrap();
        let empty_dir1 = temp_dir.path().join("empty1");
        let empty_dir2 = temp_dir.path().join("empty2");

        fs::create_dir_all(&empty_dir1).unwrap();
        fs::create_dir_all(&empty_dir2).unwrap();

        let hasher = HashingNoCache {};
        let matches = find_matching_files(&[&empty_dir1], &[&empty_dir2], &hasher).unwrap();

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

        let hasher = HashingNoCache {};
        let matches = find_matching_files(&[&source_dir], &[&target_dir], &hasher).unwrap();

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

        let hasher = HashingNoCache {};
        let matches = find_matching_files(&[&source_dir], &[&target_dir], &hasher).unwrap();

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

        let hasher = HashingNoCache {};
        let matches = find_matching_files(&[&source_dir], &[&target_dir], &hasher).unwrap();

        // Should match based on hash, regardless of filename
        assert_eq!(matches.len(), 1);
        assert!(matches[0].dest_path.ends_with("target_file.txt"));
    }
}
