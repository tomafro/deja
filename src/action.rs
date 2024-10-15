use crate::cache::Cache;
use crate::cache::CacheResult;
use crate::command::Command;
use std::ops::Add;
use std::time::Duration;
use std::time::SystemTime;

fn record(
    cmd: &mut Command,
    cache: &impl Cache,
    cache_for: Option<Duration>,
    record_exit_codes: [bool; 256],
) -> anyhow::Result<i32> {
    let mut result = cmd.run()?;
    if record_exit_codes[result.status as usize] {
        result.expires = cache_for.map(|d| SystemTime::now().add(d));
        cache.write(&cmd.scope.hash, &result)?;
    }
    Ok(result.status)
}

pub fn run(
    cmd: &mut Command,
    cache: &impl Cache,
    look_back: Option<Duration>,
    cache_for: Option<Duration>,
    record_exit_codes: [bool; 256],
) -> anyhow::Result<i32> {
    if let CacheResult::Fresh(result) = cache.result(&cmd.scope.hash, look_back, None)? {
        Ok(result.replay())
    } else {
        record(cmd, cache, cache_for, record_exit_codes)
    }
}

pub fn read(
    cmd: &mut Command,
    cache: &impl Cache,
    look_back: Option<Duration>,
    cache_miss_exit_code: i32,
) -> anyhow::Result<i32> {
    if let CacheResult::Fresh(result) = cache.result(&cmd.scope.hash, look_back, None)? {
        Ok(result.replay())
    } else {
        Ok(cache_miss_exit_code)
    }
}

pub fn force(
    cmd: &mut Command,
    cache: &impl Cache,
    cache_for: Option<Duration>,
    record_exit_codes: [bool; 256],
) -> anyhow::Result<i32> {
    record(cmd, cache, cache_for, record_exit_codes)?;
    Ok(0)
}

pub fn explain(
    cmd: &mut Command,
    cache: &impl Cache,
    look_back: Option<Duration>,
) -> anyhow::Result<i32> {
    println!("{}", cmd.scope.explanation().explain());

    match cache.result(&cmd.scope.hash, look_back, None)? {
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
    look_back: Option<Duration>,
) -> anyhow::Result<i32> {
    if let CacheResult::Fresh(result) = cache.result(&cmd.scope.hash, look_back, None)? {
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
