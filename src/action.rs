use crate::cache::Cache;
use crate::cache::CacheResult;
use crate::command::Command;
use crate::error;
use std::ops::Add;
use std::time::Duration;
use std::time::SystemTime;

fn record(
    cmd: &mut Command,
    cache: &impl Cache,
    cache_for: Option<Duration>,
) -> Result<i32, error::Error> {
    let mut result = cmd.run()?;
    result.expires = cache_for.map(|d| SystemTime::now().add(d));
    cache.write(&cmd.hash, &result)?;
    Ok(result.status)
}

pub fn run(
    cmd: &mut Command,
    cache: &impl Cache,
    look_back: Option<Duration>,
    cache_for: Option<Duration>,
) -> Result<i32, error::Error> {
    if let CacheResult::Fresh(result) = cache.result(&cmd.hash, look_back, None) {
        Ok(result.replay())
    } else {
        record(cmd, cache, cache_for)
    }
}

pub fn read(
    cmd: &mut Command,
    cache: &impl Cache,
    look_back: Option<Duration>,
) -> Result<i32, error::Error> {
    if let CacheResult::Fresh(result) = cache.result(&cmd.hash, look_back, None) {
        Ok(result.replay())
    } else {
        Ok(1)
    }
}

pub fn force(
    cmd: &mut Command,
    cache: &impl Cache,
    cache_for: Option<Duration>,
) -> Result<i32, error::Error> {
    record(cmd, cache, cache_for)?;
    Ok(0)
}

pub fn explain(
    cmd: &mut Command,
    cache: &impl Cache,
    look_back: Option<Duration>,
) -> Result<i32, error::Error> {
    println!("{}", cmd.scope.explanation().explain());

    match cache.result(&cmd.hash, look_back, None) {
        CacheResult::Fresh(_) => {
            println!("Available in cache");
        }
        CacheResult::Stale(created) => {
            println!(
                "Stale: entry in cache created {} seconds ago",
                created.elapsed().unwrap().as_secs()
            );
        }
        CacheResult::Expired(expires_at) => {
            println!(
                "Expired: entry in cache expired {} seconds ago",
                expires_at.elapsed().unwrap().as_secs()
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
) -> Result<i32, error::Error> {
    if let CacheResult::Fresh(result) = cache.result(&cmd.hash, look_back, None) {
        println!("{:?}", result);
        Ok(0)
    } else {
        Ok(1)
    }
}

pub fn remove(cmd: &mut Command, cache: &impl Cache) -> Result<i32, error::Error> {
    if cache.remove(&cmd.hash) {
        Ok(0)
    } else {
        Ok(1)
    }
}
