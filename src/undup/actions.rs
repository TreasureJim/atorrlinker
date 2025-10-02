use crate::matching::MatchingFile;
use std::fs;
use std::io;
use std::os;

pub fn dry_run(matching: &[MatchingFile]) {
    for matching_files in matching {
        println!(
            "Symlinking {0:?} with {1:?}",
            &matching_files.dest_path,
            &matching_files.src_path
        );
    }
}

pub fn symlink_matching_files(matching: &[MatchingFile]) -> io::Result<()> {
    for matching_files in matching {
        // Make temporary symlink
        let tmp_path = &matching_files.dest_path.with_extension("tmp");
        #[cfg(unix)]
        os::unix::fs::symlink(&matching_files.src_path, tmp_path)?;
        #[cfg(windows)]
        os::windows::fs::symlink_file(&matching_files.src_path, tmp_path)?;

        println!(
            "Symlinking {0:?} with {1:?}",
            &matching_files.dest_path,
            &matching_files.src_path
        );

        // Replace the file
        fs::rename(tmp_path, &matching_files.dest_path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile;

    use super::*;

    #[test]
    fn replace_file() {
        const FILE_CONTENT: &str = "hello test test";

        let src_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();

        let src_file_path = src_dir.path().join("original_file.txt");
        let target_file_path = target_dir.path().join("copied_file.txt");
        fs::write(&src_file_path, FILE_CONTENT).unwrap();
        fs::write(&target_file_path, FILE_CONTENT).unwrap();

        let matching = vec![MatchingFile {
            src_path: src_file_path,
            dest_path: target_file_path.clone(),
        }];

        // TEST
        symlink_matching_files(&matching).unwrap();

        // CONFIRM
        assert!(
            fs::symlink_metadata(&target_file_path)
                .unwrap()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&target_file_path).unwrap(), FILE_CONTENT);
    }
}
