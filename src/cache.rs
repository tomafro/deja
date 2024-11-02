use anyhow::{anyhow, Error};
use serde::{Deserialize, Serialize};

use crate::command::{Command, CommandResult};
use crate::debug;
use crate::deja::RecordOptions;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub enum CacheResult<T> {
    Fresh(Box<T>),
    Stale(SystemTime),
    Expired(SystemTime),
    Missing,
}

pub trait Cache<T: CommandResult> {
    fn read(&self, hash: &str) -> anyhow::Result<Option<T>>;
    fn remove(&self, hash: &str) -> anyhow::Result<bool>;
    fn record(&self, command: &mut Command, options: RecordOptions) -> anyhow::Result<i32>;
    fn find(&self, hash: &str, look_back: Option<Duration>) -> anyhow::Result<CacheResult<T>> {
        if let Some(result) = self.read(hash)? {
            if result.has_expired() {
                return Ok(CacheResult::Expired(result.expires().unwrap()));
            }

            if let Some(duration) = look_back {
                if result.is_older_than(duration) {
                    return Ok(CacheResult::Stale(result.created()));
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

    fn write(&self, hash: &str, entry: DiskCacheEntry) -> anyhow::Result<()> {
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

        ron::ser::to_writer(file, &entry)
            .map_err(|_| unable_to_write_to_cache_error(&self.root))?;
        Ok(())
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

#[derive(Debug, Deserialize, Serialize)]
pub struct DiskCacheEntry {
    command: Command,
    created: SystemTime,
    expires: Option<SystemTime>,
    status: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl CommandResult for DiskCacheEntry {
    fn created(&self) -> SystemTime {
        self.created
    }

    fn expires(&self) -> Option<SystemTime> {
        self.expires
    }

    fn status(&self) -> i32 {
        self.status
    }

    fn replay_output(&self) {
        let mut stdout = crate::command::OutputReader {
            reader: BufReader::new(self.stdout.as_slice()),
        };

        let mut stderr = crate::command::OutputReader {
            reader: BufReader::new(self.stderr.as_slice()),
        };

        crate::command::replay_output(&mut stdout, &mut stderr);
    }
}

impl Cache<DiskCacheEntry> for DiskCache {
    fn read(&self, hash: &str) -> anyhow::Result<Option<DiskCacheEntry>> {
        let path = self.path(hash);
        debug(format!("looking for path: {}", path.display()));
        if path.exists() {
            let file =
                std::fs::File::open(&path).map_err(|_| unable_to_read_cache_entry_error(&path))?;
            let reader = BufReader::new(file);
            let result: DiskCacheEntry = ron::de::from_reader(reader)?;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn record(&self, command: &mut Command, options: RecordOptions) -> anyhow::Result<i32> {
        let now = SystemTime::now();
        let (status, stdout, stderr) = command.run(Vec::new(), Vec::new())?;
        if options.should_record_status(status) {
            let entry = DiskCacheEntry {
                command: command.clone(),
                created: now,
                expires: options.cache_for.map(|duration| now + duration),
                status,
                stdout,
                stderr,
            };
            self.write(&command.scope.hash, entry)?;
        }
        Ok(status)
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
