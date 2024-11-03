use crate::cache::Cache;
use crate::cache::CacheEntry;
use crate::cache::FindOptions;
use crate::cache::RecordOptions;
use crate::command::Command;

fn record<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    options: RecordOptions,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    let result = cache.record(cmd, &options)?;
    Ok(result)
}

pub fn run<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    record_options: RecordOptions,
    read_options: FindOptions,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    if let Some(result) = cache.find(&cmd.scope.hash, &read_options)? {
        Ok(result.replay())
    } else {
        record(cmd, cache, record_options)
    }
}

pub fn read<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    read_options: FindOptions,
    cache_miss_exit_code: i32,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    if let Some(result) = cache.find(&cmd.scope.hash, &read_options)? {
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
    read_options: FindOptions,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    println!("{}", cmd.scope.explanation().explain());

    let hash = &cmd.scope.hash;

    let description = if let Some(result) = cache.read(hash)? {
        if !result.is_fresh() {
            let expires_at_ago = result.expires_at().unwrap().elapsed()?.as_secs();
            format!("Expired: entry in cache expired {expires_at_ago} seconds ago")
        } else if !read_options
            .max_age
            .is_none_or(|duration| result.is_younger_than(duration))
        {
            let max_age = read_options.max_age.unwrap().as_secs();
            format!("Stale: entry in cache created longer than {max_age} seconds ago")
        } else {
            format!("Fresh: entry for {hash} available in cache")
        }
    } else {
        format!("Missing: no entry found in cache for {hash}")
    };

    println!("{}", description);

    Ok(0)
}

pub fn test<E>(
    cmd: &mut Command,
    cache: &impl Cache<E>,
    read_options: FindOptions,
) -> anyhow::Result<i32>
where
    E: CacheEntry,
{
    if let Some(_result) = cache.find(&cmd.scope.hash, &read_options)? {
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
