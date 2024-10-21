use anyhow::{anyhow, Error};

use crate::command::CommandResult;
use crate::debug;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::ops::Add;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub enum CacheResult {
    Fresh(Box<CommandResult>),
    Stale(SystemTime),
    Expired(SystemTime),
    Missing,
}

pub trait Cache {
    fn read(&self, hash: &str) -> anyhow::Result<Option<CommandResult>>;
    fn write(&self, hash: &str, result: CommandResult) -> anyhow::Result<()>;
    fn remove(&self, hash: &str) -> anyhow::Result<bool>;
    fn result(
        &self,
        hash: &str,
        look_back: Option<Duration>,
        now: Option<SystemTime>,
    ) -> anyhow::Result<CacheResult> {
        if let Some(result) = self.read(hash)? {
            let now = now.unwrap_or(SystemTime::now());

            if let Some(expires_at) = result.expires {
                if expires_at < now {
                    return Ok(CacheResult::Expired(expires_at));
                }
            }

            if let Some(look_back) = look_back {
                if result.created.add(look_back) < now {
                    return Ok(CacheResult::Stale(result.created));
                }
            }

            Ok(CacheResult::Fresh(Box::new(result)))
        } else {
            Ok(CacheResult::Missing)
        }
    }
}

pub struct DiskCache {
    root: std::path::PathBuf,
    shared: bool,
}

impl DiskCache {
    pub fn new(root: PathBuf, shared: bool) -> DiskCache {
        DiskCache { root, shared }
    }

    fn path(&self, hash: &str) -> std::path::PathBuf {
        self.root.join(hash)
    }
}

pub fn unable_to_write_to_cache_error(path: &Path) -> Error {
    anyhow!("unable to write to cache {}", path.display())
}

pub fn unable_to_read_cache_entry_error(path: &Path) -> Error {
    anyhow!("unable to read cache entry {}", path.display())
}

fn create_cache_dir(path: &Path, shared: bool) -> anyhow::Result<()> {
    if !path.exists() {
        let grandparent = path.parent().unwrap();
        if !grandparent.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(grandparent)?;
        }

        std::fs::DirBuilder::new().create(path)?;
        let mode = if shared { 0o777 } else { 0o700 };
        let mut cache_permissions = path.metadata()?.permissions();
        cache_permissions.set_mode(mode);
        std::fs::set_permissions(path, cache_permissions)?;
    }
    Ok(())
}

impl Cache for DiskCache {
    fn read(&self, hash: &str) -> anyhow::Result<Option<CommandResult>> {
        let path = self.path(hash);
        debug(format!("looking for path: {}", path.display()));
        if path.exists() {
            let file =
                std::fs::File::open(&path).map_err(|_| unable_to_read_cache_entry_error(&path))?;
            let reader = BufReader::new(file);
            let result: CommandResult = ron::de::from_reader(reader)?;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn write(&self, hash: &str, result: CommandResult) -> anyhow::Result<()> {
        let path = self.path(hash);
        create_cache_dir(path.parent().unwrap(), self.shared)
            .map_err(|_| unable_to_write_to_cache_error(&self.root))?;

        debug(format!("cache write: {}, {}", hash, path.display()));
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .map_err(|_| unable_to_write_to_cache_error(&self.root))?;

        let mode = if self.shared { 0o666 } else { 0o600 };
        let mut file_permissions = file.metadata()?.permissions();
        file_permissions.set_mode(mode);
        std::fs::set_permissions(path, file_permissions)?;

        ron::ser::to_writer(file, &result)
            .map_err(|_| unable_to_write_to_cache_error(&self.root))?;
        Ok(())
    }

    fn remove(&self, hash: &str) -> anyhow::Result<bool> {
        let path = self.path(hash);
        debug(format!("cache remove: {}, {}", hash, path.display()));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|_| unable_to_write_to_cache_error(&path))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
