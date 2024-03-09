# Deja

`deja` is a shell utility that runs a given command, or if it has seen it before, returns a cached result.

The first time `deja run` is called with a command and arguments, it will always run the command and cache the result. When called again with the same command and arguments it chooses whether to run the command again, or return the previously cached output. Different options control how this choice is made, including if a file or directory has changed, an amount of time has passed, or a user provided scope has changed.

Here's an example, calling `date` to print the current date, waiting, then calling it again. The second call returns the same result, even though time marches inexorably onward:

```bash
$ deja run -- date
Wed 22 Jun 2020 11:00:00 BST
$ sleep 10
# 10 seconds later...
$ deja run -- date
Wed 22 Jun 2020 11:00:00 BST
```

For `date`, a command that returns instantly where you always want the latest result, this is very unhelpful. But for slower commands, or those you only want to run once it can be very useful. Here are some examples:

```bash
# Amazon RDS authentication tokens are valid for 15 minutes, so rather than requesting a new token, re-use the old token for the next 15 minutes
deja run --cache-for 15m -- aws rds generate-db-auth-token…

# Re-use a list of rake tasks until the Rakefile changes (useful for building quick shell completions)
deja run --watch-path Rakefile -- rake --tasks

# Re-run webpack only when git HEAD changes
deja run --watch-scope "$(git rev-parse HEAD)" -- yarn run webpack

# Re-use cargo audit results for the same build
deja run --watch-scope "$BUILD_ID" -- cargo audit

# Play around with a slow API, caching results while you experiment
export DEJA_WATCH_SCOPE=$(uuidgen)
deja run -- http http://slow.com/slow.json | jq '.[] | .date'
deja run -- http http://slow.com/slow.json | jq '.[] | .name'
```

## How deja works

Unless options are provided, `deja run` takes a command, arguments, the current user, and the working directory and generates a hash. If its cache has a match, it replays the result, otherwise it runs the command and caches the new result.

A cached result when available will (by default) be returned forever.

## Options

`--watch-path [path]` returns the cached result as long as the content of the given path remains the same. It uses a content hash to determine if the path has changed. This option can be provided multiple times to watch multiple different paths.

- `--watch-path Gemfile.lock` - Reuse the result as long as the Gemfile doesn't change
- `--watch-path src` - Reuse the result while the content of the src folder doesn't change

`--watch-scope [scope]` returns the cached result while the given scope remains the same. This accepts any string, and combined with shell substitution can be extremely powerful:

- `--watch-scope "$(date +%Y-%m-%d)"` - Reuse the result throughout the day
- `--watch-scope "$(git rev-parse HEAD)"` - Reuse the result for the current git commit

As with `--watch-path`, `--watch-scope` can be provided multiple times to watch multiple scopes.

`--watch-env` returns the cached result as long as the given environment variables remain the same. This option can be provided multiple times to watch multiple different environment variables.

`--exclude-pwd` excludes the working directory from the cache key. Normally `deja` includes the working directory in the cache key, so the cache is used only when called from the same directory. With this flag set, cached results can be returned whatever directory the command is called from, but _only_ if `--exclude-pwd` was originally used. A result generated without `--exclude-pwd` will never be returned from a different directory.

`--exclude-user` excludes the current user from the cache key. Normally `deja` includes the current user in the cache key; cached results are only returned if called by the same user. With this flag set, cached results can be shared by multiple users, but _only_ if `--exclude-user` was originally used. A result generated without `--exclude-user` will _never_ be returned to a different user.

`--cache-for [duration]` limits how long a cached result is valid. It accepts durations in the form `30s`, `5m`, `1h`, `30d`, etc. Any result older than the duration will be discarded.

`--look-back [duration]` limits how far back in time to look for a cached result. It accepts durations in the form `30s`, `5m`, `1h`, `30d`, etc.

- `--look-back 30s` will return any result generated in the last 30 seconds.

`--cache-miss-exit-code` (for `read` subcommand only) returns the given exit status on cache miss.

- `deja read --cache-miss-exit-code 200 -- grep -q needle haystack` will return 200 if the cache is missed, and the exit status of `grep` if the cache is hit.

`test` tests whether a cache result exists

`read` returns a cached result or exits

`force` forces an update of the cache

`remove` removes a cached result

`explain` returns information about the given options including the hash components and the cache result (if any)

`hash` returns just the hash used to cache results

## Tips and tricks

### Caching results for a specific shell session

```bash
export DEJA_WATCH_SCOPE=$(uuidgen)
deja run -- http http://slow.com/slow.json | jq '.[] | .date'
```

## Motivation

This utility was inspired by some code we use at [Farillio](https://farill.io) to speed up our CI builds. We use `rake` as our main build tool, and have a custom `CachedTask` class that caches results. `deja` is an attempt to do this
in a more generic, faster, flexible way.
