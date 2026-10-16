pub mod raw;

use std::ffi::OsString;
use std::path::PathBuf;

use walkdir::{DirEntry, WalkDir};

pub trait GenericFilter {
    fn get_files(&self) -> impl Iterator<Item = impl Iterator<Item = DirEntry> + '_> + '_;
}

pub struct BasicFilter {
    path: PathBuf,
    depth: usize,
    extension: OsString,
}

impl BasicFilter {
    pub fn new(path: impl Into<PathBuf>, depth: usize, extension: impl Into<OsString>) -> Self {
        Self {
            path: path.into(),
            depth,
            extension: extension.into(),
        }
    }
    pub fn set_base(&mut self, base: impl Into<PathBuf>) {
        self.path = base.into();
    }
    pub fn set_depth(&mut self, depth: usize) {
        self.depth = depth;
    }

    pub fn read(&self) -> impl Iterator<Item = DirEntry> {
        WalkDir::new(&self.path)
            .max_depth(self.depth)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && e.path()
                        .extension()
                        .is_some_and(|ext| ext == self.extension)
            })
    }
}

impl GenericFilter for BasicFilter {
    fn get_files(&self) -> impl Iterator<Item = impl Iterator<Item = DirEntry> + '_> + '_ {
        std::iter::once(self.read())
    }
}
