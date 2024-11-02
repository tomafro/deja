use crate::cache::Cache;
use crate::cache::CacheEntry;
use crate::command::Command;
use std::time::Duration;
use std::time::SystemTime;

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

fn find_any_result_from_cache<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    read_options: ReadOptions,
) -> anyhow::Result<CacheResultType<E>>
where
    E: CacheEntry,
{
    if let Some(result) = cache.read(&cmd.scope.hash)? {
        if !result.is_fresh() {
            return Ok(CacheResultType::Expired(result.expires_at().unwrap()));
        }

        if !read_options
            .look_back
            .is_none_or(|duration| result.is_younger_than(duration))
        {
            return Ok(CacheResultType::TooOld(result.created_at()));
        }

        Ok(CacheResultType::Fresh(Box::new(result)))
    } else {
        Ok(CacheResultType::Missing)
    }
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

    match find_any_result_from_cache(cmd, cache, read_options)? {
        CacheResultType::Fresh(_) => {
            println!("Available in cache");
        }
        CacheResultType::TooOld(created) => {
            println!(
                "Stale: entry in cache created {} seconds ago",
                created.elapsed()?.as_secs()
            );
        }
        CacheResultType::Expired(expires_at) => {
            println!(
                "Expired: entry in cache expired {} seconds ago",
                expires_at.elapsed()?.as_secs()
            );
        }
        CacheResultType::Missing => {
            println!("Missing from cache");
        }
    }

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
    if let CacheResultType::Fresh(_result) = find_any_result_from_cache(cmd, cache, read_options)? {
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

pub enum CacheResultType<T> {
    Fresh(Box<T>),
    TooOld(SystemTime),
    Expired(SystemTime),
    Missing,
}
