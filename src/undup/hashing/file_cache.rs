use std::{fs, io};

use super::HashingBackend;

struct HashingFileCache {
    hande: File
}

impl HashingFileCache {
    pub fn new(path: &Path) -> io::Result<Self> {
        fs::File::options().read(true).write(true).tr
    }
}

impl HashingBackend {

}
