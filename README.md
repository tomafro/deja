# Deja

`deja` is a cli utility to cache the output of commands, and re-use the previous output when called again. It's used like this:

```bash
$ deja run -- some-slow-command --with --arguments
The meaning of life is 42
```

The first time `deja run` is called (with a given command and arguments), it runs the command and caches the output. If called again, it decides whether to repeat the previous output, or re-run the command. Different options control how this decision is made, including if a file or directory has changed, time has passed, or a user provided scope has changed.

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
# Re-use a generated RDS token for 15 minutes
deja run --cache-for 15m -- aws rds generate-db-auth-token…

# Re-use a list of rake tasks until the Rakefile changes (for quick shell completions)
deja run --watch-path Rakefile -- rake --tasks

# Run webpack only when git HEAD changes
deja run --watch-scope "$(git rev-parse HEAD)" -- yarn run webpack

# Re-use build audit results for the same build
deja run --watch-env BUILD_ID -- cargo audit

# Play with a slow API, caching results while you experiment
export DEJA_WATCH_SCOPE=$(uuidgen)
deja run -- http http://slow.example.com/slow.json | jq '.[] | .date'
deja run -- http http://slow.example.com/slow.json | jq '.[] | .name'
unset DEJA_WATCH_SCOPE
```

## How deja works

When deja is called with no options, it generates a hash from the command, arguments, current user, and working directory. If a result matching this hash is found in the cache its output is replayed, otherwise the command is run and if it is successful, the result is cached alongside some metadata. A matching result will be returned forever.

## Options

`--watch-path [path]` returns the cached result while the content of the path remains the same (detected via a content hash). It can be used multiple times to watch multiple paths.

- `--watch-path Gemfile.lock` - Reuse the result while `Gemfile.lock` doesn't change
- `--watch-path src` - Reuse the result while the the contents or `src` folder doesn't change

`--watch-scope [scope]` returns the cached result while the given scope remains the same. This accepts any string, and combined with shell substitution can be extremely powerful:

- `--watch-scope "$(date +%Y-%m-%d)"` - Reuse the result throughout the day
- `--watch-scope "$(git rev-parse HEAD)"` - Reuse the result for the current git commit

As with `--watch-path`, `--watch-scope` can be provided multiple times to watch multiple scopes.

`--watch-env` returns the cached result while given environment variables remain unchanged. This option can be provided multiple times to watch multiple different environment variables.

`--exclude-pwd` removes the working directory from the cache key. Without this flag `deja` includes the working directory; cached results are only returned when called from the same directory. With this flag, cached results can be returned whatever directory the command is called from, but _only_ if `--exclude-pwd` was originally used. A result generated without `--exclude-pwd` will never be returned from a different directory.

`--exclude-user` removes the current user from the cache key. Normally `deja` includes the current user; cached results are only returned if called by the same user. With this flag, cached results can be shared by multiple users, but _only_ if `--exclude-user` was originally used. A result generated without `--exclude-user` will _never_ be returned to a different user.

`--cache-for [duration]` limits for how long a cached result is valid. It accepts durations in the form `30s`, `5m`, `1h`, `30d`, etc. If a result is stored with `--cache-for`, it will never be returned after the duration has passed.

`--look-back [duration]` limits how far back in time to look for a cached result. It accepts durations in the form `30s`, `5m`, `1h`, `30d`, etc. When `--look-back` is used, `deja` will only re-use a result if it was generated within the given duration. If no result is found within the look-back period, the command will be run and the result cached.

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

## Tips and tricks

### Caching results for a specific shell session

```bash
export DEJA_WATCH_SCOPE=$(uuidgen)
deja run -- http http://slow.com/slow.json | jq '.[] | .date'
```

## Motivation

This utility was inspired by some code we use at [Farillio](https://farill.io) to speed up our CI builds. We use `rake` as our main build tool, and have a custom `CachedTask` class that caches results. `deja` is an attempt to do this
in a more generic, faster, flexible way.
