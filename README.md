# Deja

`deja` is a CLI utility to cache the output of commands, re-using previous output on subsequent calls. It's used like this:

```bash
$ deja run -- some-slow-command --with --arguments
The meaning of life is 42
```

## Installation

At present, the easiest way to install `deja` is with [`cargo`](https://doc.rust-lang.org/cargo/), using `cargo install deja`.

## Usage

The first time `deja run` is called, the given command is executed and the output both displayed and cached. If called again (with the same arguments), the cached result is displayed. By passing various options, you can control under what conditions to return the cached result, or when to re-run the command. These include checking if a file or folder have changed, a period of time has passed, or a provided scope is different.

Here's an example, calling `date` to print the current date, waiting, then calling it again. The second call returns the same result, even though time marches inexorably onward:

```bash
# Run the `date` command to output the date and time
$ deja run -- date
Wed 22 Jun 2025 11:00:00 BST

# Sleep ten seconds, then run `date` again
$ sleep 10
$ deja run -- date
Wed 22 Jun 2025 11:00:00 BST
```

For a command like `date` this is pretty much useless. But for slower commands, or those you only want to run once it can be much more helpful. Here are some examples:

```bash
# Re-use a generated RDS token for its 15 minute lifespan
deja run --cache-for 15m -- aws rds generate-db-auth-token…

# Re-use a list of rake tasks until the Rakefile changes (for quick shell completions)
deja run --watch-path Rakefile -- rake --tasks

# Run webpack only when git HEAD changes
deja run --watch-scope "$(git rev-parse HEAD)" -- yarn run webpack

# Re-use build audit results for the same build
deja run --watch-env BUILD_ID -- cargo audit

# Play around with a slow API, caching results while you experiment
export DEJA_WATCH_SCOPE=$(uuidgen)
deja run -- http http://example.com/slow.json | jq '.[] | .date'
deja run -- http http://example.com/slow.json | jq '.[] | .name'
unset DEJA_WATCH_SCOPE
```

## How deja works

For each command, deja creates a hash from the command, arguments, user and working directory. If a result for this hash is found in the cache, its replayed. If not, the command is run, and if the exit code is 0, the result stored in the cache.  When replaying a command, the output to stdout and stderr is rewritten to the terminal in the same order as recorded. deja will then exit with the original exit code.

## Options

`--cache [path]` sets the path to the cache directory. If the directory does not exist, it will be created. By default deja will use `$XDG_CACHE_HOME/deja or $HOME/.cache/deja` on Linux, or `$HOME/Library/Caches/deja` on macOS.

`--share-cache` sets the cache to shared. By default the cache is per-user, and only the user who created the cache can read or write to it. When `--share-cache` is used, the cache is created with group read/write permissions, allowing other users to read and write to it.

`--watch-path [path]` returns the cached result until the path contents change (detected via a content hash). Multiple paths can be watched by providing the option multiple times.

- `--watch-path Gemfile.lock` - Reuse the result until `Gemfile.lock` changes
- `--watch-path src` - Reuse the result until the contents `src` changes

`--watch-scope [scope]` returns the cached result until the scope changes. This accepts any string, and combined with shell substitution can be extremely powerful:

- `--watch-scope "$(date +%Y-%m-%d)"` - Reuse the result throughout the day
- `--watch-scope "$(git rev-parse HEAD)"` - Reuse the result for the current git commit

As with `--watch-path`, `--watch-scope` can be provided multiple times to watch multiple scopes.

`--watch-env` returns the cached result until the given environment variables change. This option can be provided multiple times to watch multiple different environment variables.

`--exclude-pwd` removes the working directory from the cache key. Without this flag `deja` includes the working directory; cached results are only returned when called from the same directory. With this flag, cached results can be returned whatever directory the command is called from, but _only_ if `--exclude-pwd` was originally used. A result generated without `--exclude-pwd` will never be returned from a different directory.

`--cache-for [duration]` limits for how long a cached result is valid. It accepts durations in the form `30s`, `5m`, `1h`, `30d`, etc. If a result is stored with `--cache-for`, it will never be returned after the duration has passed.

`--record-exit-codes [codes]` expands the list of exit codes `deja` will cache. It accepts a comma separated list of either individual codes like `0,1`, inclusive ranges like `100-200`, or open-ended ranges like `0+`. By default, `deja` only caches the result of a command if the exit code is `0`. In some cases you may want other exit codes to be cached, for example if grepping a huge file for a string that may or may not be present.

- `--record-exit-codes 0,1` will cache the result if the exit code is `0` or `1`.
- `--record-exit-codes 0,10-12,100+` will cache the result if the exit code is `0`, `10`, `11`, or `12`, or `100` or greater.

`--look-back [duration]` limits how far back in time to look for a cached result. It accepts durations in the form `30s`, `5m`, `1h`, `30d`, etc. When `--look-back` is used, `deja` will only reuse a result if it was generated within the given duration. If no result is found within the period, the command will be run and the result cached.

- `--look-back 30s` will return any result generated in the last 30 seconds.

`--cache-miss-exit-code` (for `read` subcommand only) returns the given exit status on cache miss.

- `deja read --cache-miss-exit-code 200 -- grep -q needle haystack` will return 200 if the cache is missed, and the exit status of `grep` if the cache is hit.

## Subcommands

`run` is the main subcommand, used to run a command and cache the result.

`test` takes the same options as `run`, but never runs the command. Instead it exits with a status code of 0 if a cached result is found, or 1 if not.

`read` never runs the given command, but will replay a cached result if one exists. If no result is found, `deja` will exit with a status of 1 (though this can be changed with `--cache-miss-exit-code`).

`force` always runs the given command and caches the result.

`remove` removes any cached result that would have been returned.

`explain` returns information about the given options including the hash components and the cache result (if any)

`hash` returns the hash used to cache results

## Motivation

This utility was inspired by some code we use at [Farillio](https://farill.io) to speed up our CI builds. We use `rake` as our main build tool, and have a custom `CachedTask` class that caches results. `deja` is an attempt to do this
in a more generic, faster, flexible way.
