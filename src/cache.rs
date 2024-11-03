use anyhow::{anyhow, Error};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::command::Command;
use crate::debug;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub struct RecordOptions {
    /// The duration to cache a recorded result for.
    cache_for: Option<Duration>,
    /// Array of exit codes to record, where the index is the exit code (so when `exit_codes[0] == true` we record the result for exit code 0).
    exit_codes: [bool; 256],
}

impl RecordOptions {
    pub fn set_exit_codes(&mut self, exit_codes: [bool; 256]) {
        self.exit_codes = exit_codes;
    }

    pub fn set_cache_for(&mut self, cache_for: Option<Duration>) {
        self.cache_for = cache_for;
    }

    pub fn should_record(&self, exit_code: i32) -> bool {
        self.exit_codes[exit_code as usize]
    }
}

impl Default for RecordOptions {
    fn default() -> Self {
        let mut exit_codes = [false; 256];
        exit_codes[0] = true;

        RecordOptions {
            exit_codes,
            cache_for: None,
        }
    }
}

pub struct FindOptions {
    /// The maximum age of a cached result to consider. Results older than this will be ignored.
    pub max_age: Option<Duration>,
}

impl FindOptions {
    pub fn set_max_age(&mut self, s: Option<Duration>) {
        self.max_age = s;
    }
}

impl Default for FindOptions {
    fn default() -> Self {
        FindOptions { max_age: None }
    }
}

pub trait Cache<T: CacheEntry> {
    fn remove(&self, hash: &str) -> anyhow::Result<bool>;
    fn record(&self, command: &mut Command, options: &RecordOptions) -> anyhow::Result<i32>;
    fn read(&self, hash: &str) -> anyhow::Result<Option<T>>;
    fn find(&self, hash: &str, options: &FindOptions) -> anyhow::Result<Option<T>> {
        self.read(hash).map(|result| {
            result.filter(|result| result.is_fresh()).filter(|result| {
                options
                    .max_age
                    .is_none_or(|duration| result.is_younger_than(duration))
            })
        })
    }
}

pub struct DiskCache {
    root: std::path::PathBuf,
    shared: bool,
}

impl DiskCache {
    pub fn new(root: PathBuf, shared: bool) -> anyhow::Result<DiskCache> {
        create_cache_dir(root.as_path(), shared)
            .map_err(|_| unable_to_write_to_cache_error(&root))?;
        Ok(DiskCache { root, shared })
    }

    fn path(&self, hash: &str, suffix: &str) -> std::path::PathBuf {
        self.root.join(format!("{hash}.{suffix}"))
    }

    fn create_file(&self, path: &PathBuf) -> anyhow::Result<File> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .map_err(|_| unable_to_write_to_cache_error(&path))?;

        let mode = if self.shared { 0o666 } else { 0o600 };
        let mut file_permissions = file.metadata()?.permissions();
        file_permissions.set_mode(mode);
        std::fs::set_permissions(&path, file_permissions)?;
        Ok(file)
    }

    fn write(&self, hash: &str, entry: DiskCacheEntry) -> anyhow::Result<()> {
        let path = self.path(hash, "ron");
        let file = self.create_file(&path)?;
        ron::ser::to_writer_pretty(file, &entry, PrettyConfig::default())
            .map_err(|_| unable_to_write_to_cache_error(&path))?;
        Ok(())
    }
}

pub fn unable_to_write_to_cache_error(path: &Path) -> Error {
    anyhow!("unable to write file to cache {}", path.display())
}

pub fn unable_to_read_cache_entry_error(path: &Path) -> Error {
    anyhow!("unable to read file from cache {}", path.display())
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
pub struct DiskCacheEntryMeta {
    command: Command,
    created: SystemTime,
    expires: Option<SystemTime>,
    status: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DiskCacheEntry {
    meta: DiskCacheEntryMeta,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl CacheEntry for DiskCacheEntry {
    fn created_at(&self) -> SystemTime {
        self.meta.created
    }

    fn expires_at(&self) -> Option<SystemTime> {
        self.meta.expires
    }

    fn command_status(&self) -> i32 {
        self.meta.status
    }

    fn replay_command_output(&self) -> anyhow::Result<()> {
        replay_output(File::open(&self.stdout)?, File::open(&self.stderr)?);
        Ok(())
    }
}

impl Cache<DiskCacheEntry> for DiskCache {
    fn read(&self, hash: &str) -> anyhow::Result<Option<DiskCacheEntry>> {
        let path = self.path(hash, "ron");
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

    fn record(&self, command: &mut Command, options: &RecordOptions) -> anyhow::Result<i32> {
        let now = SystemTime::now();
        let ulid: String = Ulid::new().to_string();

        let out = self.path(&command.scope.hash, &format!("{ulid}.out"));
        let err = self.path(&command.scope.hash, &format!("{ulid}.err"));

        let out_file = self.create_file(&out)?;
        let err_file = self.create_file(&err)?;

        let (status, _, _) = command.run(out_file, err_file)?;

        if options.should_record(status) {
            let meta = DiskCacheEntryMeta {
                command: command.clone(),
                created: now,
                expires: options.cache_for.map(|duration| now + duration),
                status,
            };

            let entry = DiskCacheEntry {
                meta,
                stdout: out,
                stderr: err,
            };

            if let Some(existing) = self.read(&command.scope.hash)? {
                std::fs::remove_file(existing.stdout)?;
                std::fs::remove_file(existing.stderr)?;
            }

            self.write(&command.scope.hash, entry)?;
        } else {
            std::fs::remove_file(&out)?;
            std::fs::remove_file(&err)?;
        }
        Ok(status)
    }

    fn remove(&self, hash: &str) -> anyhow::Result<bool> {
        let path = self.path(hash, "ron");
        debug(format!("cache remove: {}, {}", hash, path.display()));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|_| unable_to_write_to_cache_error(&path))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

pub(crate) fn replay_output<O>(stdout: O, stderr: O)
where
    O: Read,
{
    let mut stdout = OutputReader {
        reader: BufReader::new(stdout),
    }
    .peekable();

    let mut stderr = OutputReader {
        reader: BufReader::new(stderr),
    }
    .peekable();

    loop {
        match (stdout.peek(), stderr.peek()) {
            (Some((ot, ol)), Some((et, el))) => {
                if ot < et {
                    print!("{}", ol);
                    stdout.next();
                } else {
                    eprint!("{}", el);
                    stderr.next();
                }
            }
            (Some((_, ol)), None) => {
                print!("{}", ol);
                stdout.next();
            }
            (None, Some((_, el))) => {
                eprint!("{}", el);
                stderr.next();
            }
            (None, None) => break,
        }
    }
}

pub trait CacheEntry {
    fn created_at(&self) -> SystemTime;
    fn expires_at(&self) -> Option<SystemTime>;
    fn command_status(&self) -> i32;
    fn replay_command_output(&self) -> anyhow::Result<()>;

    fn is_fresh(&self) -> bool {
        self.expires_at()
            .map_or(true, |expires| SystemTime::now() < expires)
    }

    fn is_younger_than(&self, duration: Duration) -> bool {
        self.created_at().elapsed().unwrap() < duration
    }

    fn replay(&self) -> i32 {
        self.replay_command_output().unwrap();
        self.command_status()
    }
}

pub struct OutputReader<R>
where
    R: Read,
{
    pub reader: BufReader<R>,
}

impl<R> Iterator for OutputReader<R>
where
    R: Read,
{
    type Item = (u128, String);

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        let mut bytes: [u8; 16] = [0; 16];

        // First 16 bytes are the timestamp

        match self.reader.read_exact(&mut bytes) {
            Ok(()) => (),
            Err(_) => return None,
        }

        // Following the timestamp is the line contents

        match self.reader.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some((u128::from_be_bytes(bytes), line.to_string())),
            Err(_) => None,
        }
    }
}
