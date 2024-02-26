use crate::command::CommandResult;
use crate::debug;
use std::io::BufReader;
use std::path::PathBuf;

pub trait Cache {
    fn read(&self, hash: &String) -> Option<CommandResult>;
    fn write(&self, hash: &String, result: &CommandResult) -> Result<(), crate::error::Error>;
    fn remove(&self, hash: &String) -> bool;
}

pub struct DiskCache {
    root: std::path::PathBuf,
}

impl DiskCache {
    pub fn new(root: PathBuf) -> DiskCache {
        DiskCache { root }
    }

    fn path(&self, hash: &String) -> std::path::PathBuf {
        self.root.join(hash)
    }
}

pub fn unable_to_write_to_cache_error(path: &PathBuf) -> crate::error::Error {
    crate::error::anticipated(&format!("unable to write to cache {}", path.display()), 1)
}

impl Cache for DiskCache {
    fn read(&self, hash: &String) -> Option<CommandResult> {
        let path = self.path(hash);
        debug(format!("looking for path: {}", path.display()));
        if path.exists() {
            let file = std::fs::File::open(path).unwrap();
            let reader = BufReader::new(file);
            let result: CommandResult = ron::de::from_reader(reader).unwrap();
            return Some(result);
        }
        None
    }

    fn write(&self, hash: &String, result: &CommandResult) -> Result<(), crate::error::Error> {
        let path = self.path(hash);
        debug(format!("cache write: {}, {}", hash, path.display()));
        let parent = path
            .parent()
            .ok_or(unable_to_write_to_cache_error(&self.root))?;
        std::fs::create_dir_all(parent).map_err(|_| unable_to_write_to_cache_error(&self.root))?;
        let file =
            std::fs::File::create(&path).map_err(|_| unable_to_write_to_cache_error(&self.root))?;
        ron::ser::to_writer(file, result).unwrap();
        Ok(())
    }

    fn remove(&self, hash: &String) -> bool {
        let path = self.path(hash);
        debug(format!("cache remove: {}, {}", hash, path.display()));
        if path.exists() {
            std::fs::remove_file(path).unwrap();
            true
        } else {
            false
        }
    }
}
