use crate::cache::Cache;
use crate::cache::CacheEntry;
use crate::command::Command;
use std::time::Duration;

pub struct RecordOptions {
    pub cache_for: Option<Duration>,
    record_status_codes: [bool; 256],
}

impl RecordOptions {
    pub fn new(record_status_codes: [bool; 256], cache_for: Option<Duration>) -> RecordOptions {
        RecordOptions {
            record_status_codes,
            cache_for,
        }
    }

    pub fn should_record_status(&self, status: i32) -> bool {
        self.record_status_codes[status as usize]
    }
}

pub struct ReadOptions {
    look_back: Option<Duration>,
}

impl ReadOptions {
    pub fn new(look_back: Option<Duration>) -> ReadOptions {
        ReadOptions { look_back }
    }
}

fn record<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    options: RecordOptions,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    let result = cache.record(cmd, options)?;
    Ok(result)
}

pub fn run<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    record_options: RecordOptions,
    read_options: ReadOptions,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    if let Some(result) = cache.read_fresh(&cmd.scope.hash, read_options.look_back)? {
        Ok(result.replay())
    } else {
        record(cmd, cache, record_options)
    }
}

pub fn read<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    read_options: ReadOptions,
    cache_miss_exit_code: i32,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    if let Some(result) = cache.read_fresh(&cmd.scope.hash, read_options.look_back)? {
        Ok(result.replay())
    } else {
        Ok(cache_miss_exit_code)
    }
}

pub fn force<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    record_options: RecordOptions,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    record(cmd, cache, record_options)?;
    Ok(0)
}

pub fn explain<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    read_options: ReadOptions,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    println!("{}", cmd.scope.explanation().explain());

    let description = if let Some(result) = cache.read(&cmd.scope.hash)? {
        if !result.is_fresh() {
            let expires_at_ago = result.expires_at().unwrap().elapsed()?.as_secs();
            format!("Expired: entry in cache expired {expires_at_ago} seconds ago")
        } else if !read_options
            .look_back
            .is_none_or(|duration| result.is_younger_than(duration))
        {
            let look_back_ago = read_options.look_back.unwrap().as_secs();
            format!("Stale: entry in cache created {look_back_ago} seconds ago")
        } else {
            format!("Available in cache")
        }
    } else {
        format!("Expired: ")
    };

    println!("{}", description);

    Ok(0)
}

pub fn test<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    read_options: ReadOptions,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    if let Some(_result) = cache.read_fresh(&cmd.scope.hash, read_options.look_back)? {
        Ok(0)
    } else {
        Ok(1)
    }
}

pub fn remove<E>(cmd: &mut Command, cache: &impl Cache<E>) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    if cache.remove(&cmd.scope.hash)? {
        Ok(0)
    } else {
        Ok(1)
    }
}

pub fn hash<E>(cmd: &mut Command, _cache: &impl Cache<E>) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    println!("{}", cmd.scope.hash);
    Ok(0)
}
