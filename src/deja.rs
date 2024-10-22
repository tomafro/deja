use crate::cache::Cache;
use crate::cache::CacheResult;
use crate::command::Command;
use crate::command::CommandResult;
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
    E: CommandResult,
{
    Ok(cache.record(cmd, options)?)
}

pub fn run<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    record_options: RecordOptions,
    read_options: ReadOptions,
) -> anyhow::Result<i32>
where
    E: CommandResult,
{
    if let CacheResult::Fresh(result) = cache.find(&cmd.scope.hash, read_options.look_back)? {
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
    E: CommandResult,
{
    if let CacheResult::Fresh(result) = cache.find(&cmd.scope.hash, read_options.look_back)? {
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
    E: CommandResult,
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
    E: CommandResult,
{
    println!("{}", cmd.scope.explanation().explain());

    match cache.find(&cmd.scope.hash, read_options.look_back)? {
        CacheResult::Fresh(_) => {
            println!("Available in cache");
        }
        CacheResult::Stale(created) => {
            println!(
                "Stale: entry in cache created {} seconds ago",
                created.elapsed()?.as_secs()
            );
        }
        CacheResult::Expired(expires_at) => {
            println!(
                "Expired: entry in cache expired {} seconds ago",
                expires_at.elapsed()?.as_secs()
            );
        }
        CacheResult::Missing => {
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
    E: CommandResult,
{
    if let CacheResult::Fresh(_result) = cache.find(&cmd.scope.hash, read_options.look_back)? {
        Ok(0)
    } else {
        Ok(1)
    }
}

pub fn remove<E>(cmd: &mut Command, cache: &impl Cache<E>) -> anyhow::Result<i32>
where
    E: CommandResult,
{
    if cache.remove(&cmd.scope.hash)? {
        Ok(0)
    } else {
        Ok(1)
    }
}

pub fn hash<E>(cmd: &mut Command, _cache: &impl Cache<E>) -> anyhow::Result<i32>
where
    E: CommandResult,
{
    println!("{}", cmd.scope.hash);
    Ok(0)
}
