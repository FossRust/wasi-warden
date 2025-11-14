use std::fs::File;

use camino::Utf8PathBuf;

#[derive(Debug)]
pub struct DirHandleResource {
    pub path: Utf8PathBuf,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct FileHandleResource {
    pub path: Utf8PathBuf,
    pub file: File,
}
