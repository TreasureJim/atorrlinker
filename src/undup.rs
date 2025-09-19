use std::path::{Path, PathBuf};
use thiserror::Error;

struct MatchingFile {
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

struct DiscoveredFiles {
    files: Vec<PathBuf>,
    sym_files: Vec<(PathBuf, PathBuf)>,
}

impl DiscoveredFiles {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            sym_files: Vec::new(),
        }
    }
}

fn hash_files(
    dir: &Path,
    hash_cb: &dyn Fn(),
    skip_cb: &dyn Fn(&Path) -> bool,
) -> std::io::Result<DiscoveredFiles> {
    let mut queue = std::collections::VecDeque::<PathBuf>::from(vec![dir.to_path_buf()]);
    let mut disc_files = DiscoveredFiles::default();

    if !dir.metadata()?.is_dir() {
        disc_files.files.push(dir.to_path_buf());
        return Ok(disc_files);
    }

    while let Some(dir) = queue.pop_back() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if skip_cb(&entry.path()) {
                continue;
            };

            match entry.metadata()? {
                ft if ft.is_dir() => {
                    queue.push_back(entry.path());
                    continue;
                }
                ft if ft.is_file() => {
                    todo!("Hash file and push to struct");
                    disc_files.files.push(entry.path());
                },
                ft if ft.is_symlink() => disc_files.sym_files.push((
                    entry.path(),
                    std::fs::read_link(&entry.path()).expect("Should be a symlink"),
                )),
                _ => {
                    log::error!("Entry is not directory, file or symlink");
                }
            }
        }
    }

    Ok(disc_files)
}

// fn find_matching_files(source_dir: &Path, target_dir: &Path) -> std::sync::mpsc::Receiver<MatchingFile> {
//
// }

#[cfg(test)]
mod testing {
    use tempfile;
    use uuid;

    fn random_name() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}
