use crate::cache::Cache;
use crate::command::Command;
use crate::command::CommandResult;
use crate::error;
use std::ops::Add;
use std::time::Duration;
use std::time::SystemTime;

use crate::debug;

fn find_command_result(
    cmd: &Command,
    cache: &impl Cache,
    look_back: Option<Duration>,
) -> Option<CommandResult> {
    let now = SystemTime::now();

    debug(format!("looking for {} in cache", &cmd.hash));
    if let Some(cache_entry) = cache.read(&cmd.hash) {
        debug(format!("found {} in cache, checking freshness", &cmd.hash));
        if cache_entry.expires.unwrap_or(now) >= now {
            if look_back.is_none() || cache_entry.created.add(look_back.unwrap()) >= now {
                debug(format!("{} is fresh, returning", &cmd.hash));
                return Some(cache_entry);
            }
            debug(format!(
                "{} isn't recent enough for --look-back, discarding",
                &cmd.hash
            ));
        } else {
            debug(format!("{} has expired, discarding", &cmd.hash));
        }
    } else {
        debug(format!("{} not found in cache", &cmd.hash));
    }

    None
}

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
    if let Some(result) = find_command_result(cmd, cache, look_back) {
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
    if let Some(result) = find_command_result(cmd, cache, look_back) {
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

    if let Some(result) = find_command_result(cmd, cache, look_back) {
        println!("{:?}", result);
    }
    Ok(0)
}

pub fn test(
    cmd: &mut Command,
    cache: &impl Cache,
    look_back: Option<Duration>,
) -> Result<i32, error::Error> {
    if let Some(result) = find_command_result(cmd, cache, look_back) {
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
