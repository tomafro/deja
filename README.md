# Deja

deja is a utility to cache CLI commands. It captures stdout, stderr, and the exit code of a given command, and replays them when the command is run again. This is particularly useful for commands that are slow and idempotent, or return results with a known lifespan.

Here's a quick example:

```bash
$ deja run --cache-for 15m -- aws rds generate-db-auth-token …args
db.example.com:5432/?Action=connect&DBUser=user&X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=ABCDEFG%2F20201010%2Feu-west-2%2Frds-db%2Faws4_request&X-Amz-Date=20201010T010101Z&X-Amz-Expires=900&X-Amz-SignedHeaders=host&X-Amz-Signature=abcdefg1234567890
```

For the next 15 minutes, invoking the same command will re-print the result to stdout. After 15 minutes, the command will be run again, and the new result stored in the cache.

This uses the `--cache-for` option to limit the cached lifespan, and there are a wide range of other options to control caching, including when environment variables change, if the contents of a file or folder change, or based on arbitary strings.

## Examples of how you might use deja:

* Run a command only once a day:
  ```bash
  deja run --watch-scope "$(date +%Y-%m-%d)" -- command-that-should-only-run-once-a-day
  ```
* Run a command only when a file changes:
  ```bash
  deja run --watch-file /path/to/file -- command-that-should-only-run-when-file-changes
  ```
* Play around with a slow API:
  ```bash
  alias deja-http="deja run --watch-scope $(uuidgen) -- http "
  deja-http https://api.example.com/slow-endpoint | jq '.[] | .date'
  deja-http https://api.example.com/slow-endpoint | jq '.[] | .name'
  ```
* Use the same RDS token for its full 15 minute lifespan:
  ```bash
  deja run --cache-for 15m -- aws rds generate-db-auth-token…
  ```
* Reuse a command throughout a build:
  ```bash
  deja run --watch-env BUILD_ID -- cargo audit
  ```
* Run tests only when the git HEAD changes:
  ```bash
  deja run --watch-scope "$(git rev-parse HEAD)" -- rake test
  ```
* Pre-record CLI output for a demo:
  ```bash
  deja run --record-exit-codes 0,1 -- ./demo.sh
  ```

## Installation

Deja is written in rust. You can install it easily with [`cargo`](https://doc.rust-lang.org/cargo/), using `cargo install deja`.

## How deja works

For each command, deja creates a hash from the command, arguments, and other options (by default the user and working directory). If a fresh result for this hash is found in the cache, it's replayed. If not, the command is run, and when the exit code is 0, the result stored in the cache.  When replaying a command, both stdout and stderr are rewritten to the terminal in the same order as recorded. Deja will then exit with the original exit code.

Deja stores cached results in a dedicated directory (by default `$HOME/Library/Caches/deja` on macOS, or either `$XDG_CACHE_HOME/deja` or `$HOME/.cache/deja` on Linux). Stored results are not encrypted, but _are_ stored with permissions so only the user who created the entry can read or write to it.

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

`--exclude-pwd` removes the working directory from the cache key. Without this flag deja includes the working directory; cached results are only returned when called from the same directory. With this flag, cached results can be returned whatever directory the command is called from, but _only_ if `--exclude-pwd` was originally used. A result generated without `--exclude-pwd` will never be returned from a different directory.

`--cache-for [duration]` limits for how long a cached result is valid. It accepts durations in the form `30s`, `5m`, `1h`, `30d`, etc. If a result is stored with `--cache-for`, it will never be returned after the duration has passed.

`--record-exit-codes [codes]` expands the list of exit codes deja will cache. It accepts a comma separated list of either individual codes like `0,1`, inclusive ranges like `100-200`, or open-ended ranges like `0+`. By default, deja only caches the result of a command if the exit code is `0`. In some cases you may want other exit codes to be cached, for example if grepping a huge file for a string that may or may not be present.

- `--record-exit-codes 0,1` will cache the result if the exit code is `0` or `1`.
- `--record-exit-codes 0,10-12,100+` will cache the result if the exit code is `0`, `10`, `11`, or `12`, or `100` or greater.

`--look-back [duration]` limits how far back in time to look for a cached result. It accepts durations in the form `30s`, `5m`, `1h`, `30d`, etc. When `--look-back` is used, deja will only reuse a result if it was generated within the given duration. If no result is found within the period, the command will be run and the result cached.

- `--look-back 30s` will return any result generated in the last 30 seconds.

`--cache-miss-exit-code` (for `read` subcommand only) returns the given exit status on cache miss.

- `deja read --cache-miss-exit-code 200 -- grep -q needle haystack` will return 200 if the cache is missed, and the exit status of `grep` if the cache is hit.

## Subcommands

`run` is the main subcommand, used to run a command and cache the result.

`test` takes the same options as `run`, but never runs the command. Instead it exits with a status code of 0 if a cached result is found, or 1 if not.

`read` never runs the given command, but will replay a cached result if one exists. If no result is found, deja will exit with a status of 1 (though this can be changed with `--cache-miss-exit-code`).

`force` always runs the given command and caches the result.

`remove` removes any cached result that would have been returned.

`explain` returns information about the given options including the hash components and the cache result (if any)

`hash` returns the hash used to cache results

## Motivation

This utility was inspired by some code we use at [Farillio](https://farill.io) to speed up our CI builds. We use `rake` as our main build tool, and have a custom `CachedTask` class that caches results. deja is an attempt to do this
in a more generic, faster, flexible way.
