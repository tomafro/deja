use crate::cache::Cache;
use crate::cache::CacheResult;
use crate::command::Command;
use crate::command::CommandResult;
use std::ops::Add;
use std::time::Duration;
use std::time::SystemTime;

pub struct RecordOptions {
    cache_for: Option<Duration>,
    record_exit_codes: [bool; 256],
}

impl RecordOptions {
    pub fn new(cache_for: Option<Duration>, record_exit_codes: [bool; 256]) -> RecordOptions {
        RecordOptions {
            cache_for,
            record_exit_codes,
        }
    }

    fn should_record_result(&self, result: &CommandResult) -> bool {
        self.record_exit_codes[result.status as usize]
    }

    fn expires_at(&self) -> Option<SystemTime> {
        self.cache_for.map(|d| SystemTime::now().add(d))
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

fn record(
    cmd: &mut Command,
    cache: &impl Cache,
    record_options: RecordOptions,
) -> anyhow::Result<i32> {
    let mut result = cmd.run()?;
    if record_options.should_record_result(&result) {
        result.expires = record_options.expires_at();
        cache.write(&cmd.scope.hash, &result)?;
    }
    Ok(result.status)
}

pub fn run(
    cmd: &mut Command,
    cache: &impl Cache,
    record_options: RecordOptions,
    read_options: ReadOptions,
) -> anyhow::Result<i32> {
    if let CacheResult::Fresh(result) =
        cache.result(&cmd.scope.hash, read_options.look_back, None)?
    {
        Ok(result.replay())
    } else {
        record(cmd, cache, record_options)
    }
}

pub fn read(
    cmd: &mut Command,
    cache: &impl Cache,
    read_options: ReadOptions,
    cache_miss_exit_code: i32,
) -> anyhow::Result<i32> {
    if let CacheResult::Fresh(result) =
        cache.result(&cmd.scope.hash, read_options.look_back, None)?
    {
        Ok(result.replay())
    } else {
        Ok(cache_miss_exit_code)
    }
}

pub fn force(
    cmd: &mut Command,
    cache: &impl Cache,
    record_options: RecordOptions,
) -> anyhow::Result<i32> {
    record(cmd, cache, record_options)?;
    Ok(0)
}

pub fn explain(
    cmd: &mut Command,
    cache: &impl Cache,
    read_options: ReadOptions,
) -> anyhow::Result<i32> {
    println!("{}", cmd.scope.explanation().explain());

    match cache.result(&cmd.scope.hash, read_options.look_back, None)? {
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

pub fn test(
    cmd: &mut Command,
    cache: &impl Cache,
    read_options: ReadOptions,
) -> anyhow::Result<i32> {
    if let CacheResult::Fresh(result) =
        cache.result(&cmd.scope.hash, read_options.look_back, None)?
    {
        println!("{:?}", result);
        Ok(0)
    } else {
        Ok(1)
    }
}

pub fn remove(cmd: &mut Command, cache: &impl Cache) -> anyhow::Result<i32> {
    if cache.remove(&cmd.scope.hash)? {
        Ok(0)
    } else {
        Ok(1)
    }
}

pub fn hash(cmd: &mut Command, _cache: &impl Cache) -> anyhow::Result<i32> {
    println!("{}", cmd.scope.hash);
    Ok(0)
}
