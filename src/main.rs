mod cache;
mod command;
mod deja;
mod hash;

use crate::cache::{DiskCache, FindOptions, RecordOptions};
use crate::command::Command;
use anyhow::anyhow;
use clap::value_parser;
use clap::Arg;
use clap::ValueHint;
use command::ScopeBuilder;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;

use std::sync::OnceLock;

pub static DEBUG: OnceLock<bool> = OnceLock::new();

pub fn debug(string: String) {
    if DEBUG.get_or_init(|| false).to_owned() {
        eprintln!("- {}", string);
    };
}

fn subcommand(
    name: &str,
    about: &str,
    include_cache_miss_exit_code_param: bool,
    include_record_exit_codes_param: bool,
) -> clap::Command {
    let env = "DEJA_CACHE";
    let mut cache = Arg::new("cache")
        .long("cache")
        .value_name("path")
        .help("Path used as cache")
        .env(env)
        .value_parser(value_parser!(PathBuf));

    cache = if let Some(cache_dir) = dirs::cache_dir() {
        let default_cache = cache_dir.join("deja").into_os_string();
        let default_cache_string = default_cache.to_string_lossy();
        let long_help = format!(r#"
Directory to store cache files (default: {default_cache_string}). Can also be set via the {env} variable. Files are stored in this directory with the hash as the filename, only readable by the current user.
"#).trim().to_owned();
        cache
            .long_help(long_help)
            .default_value(&default_cache)
            .hide_default_value(true)
            .hide_env(true)
    } else {
        cache
    };

    let watch_path = Arg::new("watch-path")
        .long("watch-path")
        .help_heading("Caching options")
        .value_name("path")
        .value_hint(ValueHint::AnyPath)
        .help("Include path contents in cache key")
        .long_help(r#"
Include path contents in cache key. Watching a path generates a hash of the contents and includes it in the cache key. The path can point to either a file or a directory. When given a directory, all files and subdirectories are included in the hash.

This option can be given multiple times to watch multiple paths.
"#.trim())
        .value_parser(value_parser!(PathBuf))
        .action(clap::ArgAction::Append);

    let watch_scope = Arg::new("watch-scope")
        .long("watch-scope")
        .value_name("scope")
        .help_heading("Caching options")
        .help("Include scope string in cache key")
        .long_help(r#"
Include scope string in cache key. Any string can be given as a scope, which when combined with shell substitution can be extremely flexible. For example `--watch-scope "$(date +%Y-%m-%d)"` will include the current date.

This option can be given multiple times to watch multiple scopes.
"#.trim())
        .env("DEJA_WATCH_SCOPE")
        .hide_env(true)
        .action(clap::ArgAction::Append);

    let watch_env = Arg::new("watch-env")
        .long("watch-env")
        .value_name("env")
        .help_heading("Caching options")
        .help("Include variable value in cache key")
        .long_help(r#"
Include variable value in cache key. Any environment variable can be given. For example `--watch-env MY_VAR` will include the value of the `MY_VAR` environment variable.

This option can be given multiple times to watch multiple variables.
"#.trim())
        .action(clap::ArgAction::Append);

    let exclude_pwd = Arg::new("exclude-pwd")
        .long("exclude-pwd")
        .help("Remove current directory from cache key")
        .help_heading("Caching options")
        .long_help(r#"
Remove the current working directory from the cache key. By default, the current directory is always included in the cache key. Running the same command from a different directory will result in a cache miss. This flag changes this behaviour, so commands can be run from any location and hit the cache.
"#.trim())
        .env("DEJA_IGNORE_PWD")
        .hide_env(true)
        .action(clap::ArgAction::SetTrue);

    let share_cache = Arg::new("share-cache")
        .long("share-cache")
        .help("Use a shared cache")
        .help_heading("Caching options")
        .long_help(r#"Use a shared cache. By default, each user has their own cache. This flag changes this behaviour, so all users share the same cache. This can be useful when running the same command as different users, as the cache will be shared between them."#.trim())
        .action(clap::ArgAction::SetTrue);

    let look_back = Arg::new("look-back")
        .long("look-back")
        .value_name("duration")
        .help("How far back in time to look for cached results")
        .env("DEJA_LOOK_BACK")
        .help_heading("Retrieval options")
        .hide_env(true)
        .long_help(r#"
How far back in time to look for cached results. When this option is set, deja will only look back into the cache the given amount of time. Any cache hit before this will be ignored. The duration should be provided in a format like 5s, 30m, 2h, 1d, etc.
"#.trim());

    let cache_for = Arg::new("cache-for")
        .long("cache-for")
        .value_name("duration")
        .help("How long a cached result should be valid")
        .help_heading("Caching options")
        .env("DEJA_CACHE_FOR")
        .hide_env(true)
        .long_help(r#"
How long a cached result should be valid. When this option is set, any cached result will only ever be used for the given duration. After the duration has passed, the result will be considered stale and never returned. The duration should be provided in a format like 5s, 30m, 2h, 1d, etc.
"#.trim());

    let command = Arg::new("command")
        .value_name("COMMAND")
        .value_hint(ValueHint::CommandName)
        .required(true)
        .help("Command to run");

    let arguments = Arg::new("arguments")
        .value_name("ARGUMENTS")
        .help("Arguments to pass to command")
        .action(clap::ArgAction::Append);

    let mut cache_args = vec![
        watch_path,
        watch_scope,
        watch_env,
        share_cache,
        exclude_pwd,
        look_back,
        cache_for,
        cache,
    ];

    if include_cache_miss_exit_code_param {
        cache_args.push(
            Arg::new("cache-miss-exit-code")
                .long("cache-miss-exit-code")
                .value_name("code")
                .value_parser(clap::value_parser!(i32).range(1..256))
                .help("Exit code when a cache miss occurs (default: 1)")
                .default_value("1")
                .hide_default_value(true),
        );
    }

    if include_record_exit_codes_param {
        cache_args.push(
            Arg::new("record-exit-codes")
                .long("record-exit-codes")
                .value_name("exit-codes")
                .env("DEJA_RECORD_EXIT_CODES")
                .hide_env(true)
                .help("Exit codes to record in the cache (default: 0)")
                .help_heading("Caching options")
                .hide_default_value(true)
                .default_value("0"),
        );
    }

    cache_args.push(command);
    cache_args.push(arguments);

    clap::Command::new(name.to_string())
        .about(about.to_string())
        .args(cache_args)
}

pub fn styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(
            anstyle::Style::new()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::BrightYellow))),
        )
        .header(
            anstyle::Style::new()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::BrightYellow))),
        )
        .literal(
            anstyle::Style::new()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::BrightGreen))),
        )
        .invalid(
            anstyle::Style::new()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::BrightRed))),
        )
        .error(
            anstyle::Style::new()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::BrightRed))),
        )
        .valid(
            anstyle::Style::new()
                .underline()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::BrightGreen))),
        )
        .placeholder(
            anstyle::Style::new()
                .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::BrightWhite))),
        )
}

fn cli() -> anyhow::Result<clap::Command> {
    let run = subcommand(
        "run",
        "Return cached result or run and cache command",
        false,
        true,
    );

    let read = subcommand("read", "Return cached result or exit", true, false);
    let force = subcommand("force", "Run and cache command", false, true);
    let remove = subcommand("remove", "Remove command from cache", false, false);
    let test = subcommand("test", "Test if command is cached", false, false);
    let explain = subcommand("explain", "Explain cache key for command", false, false).hide(true);
    let hash = subcommand(
        "hash",
        "Print hash generated for command and options",
        false,
        false,
    );

    let completions = clap::command!()
        .name("completions")
        .args(vec![Arg::new("shell")
            .long("shell")
            .value_name("SHELL")
            .value_parser(["bash", "fish", "zsh", "powershell"])
            .required(true)
            .help("Shell to generate completions for")]);

    Ok(clap::command!()
        .name("deja")
        .arg_required_else_help(true)
        .styles(styles())
        .arg(
            Arg::new("debug")
                .long("debug")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .hide(true),
        )
        .subcommands(vec![
            run,
            read,
            force,
            remove,
            test,
            explain,
            hash,
            completions,
        ]))
}

fn parse_exit_codes(param: &str) -> [bool; 256] {
    let parts = param.split(',').map(|s| s.trim());

    let mut exit_codes = [false; 256];
    for part in parts {
        if part.ends_with('+') {
            let start = part.trim_end_matches('+').parse::<i32>().unwrap();
            for i in start..=255 {
                exit_codes[i as usize] = true;
            }
        } else if part.contains('-') {
            let mut parts = part.split('-');
            let start = parts.next().unwrap().parse::<i32>().unwrap();
            let end = parts.next().unwrap().parse::<i32>().unwrap();
            for i in start..=end {
                exit_codes[i as usize] = true;
            }
        } else {
            let code = part.parse::<i32>().unwrap();
            exit_codes[code as usize] = true;
        }
    }
    exit_codes
}

fn command(matches: &clap::ArgMatches) -> anyhow::Result<Command> {
    let cmd = matches
        .get_one::<String>("command")
        .ok_or(anyhow!("unexpected failure to parse arguments"))?;
    let args = matches
        .get_many::<String>("arguments")
        .unwrap_or_default()
        .map(|s| s.into())
        .collect::<Vec<String>>();
    let watch_path_bufs = matches
        .get_many::<PathBuf>("watch-path")
        .unwrap_or_default()
        .map(|s| s.into())
        .collect::<Vec<PathBuf>>();

    let watch_paths = watch_path_bufs
        .iter()
        .map(|path| {
            std::fs::canonicalize(path)
                .map_err(|_| anyhow!("watch path '{}' not found", path.display()))
        })
        .collect::<Result<Vec<PathBuf>, anyhow::Error>>()?;

    let watch_scope = matches
        .get_many::<String>("watch-scope")
        .unwrap_or_default()
        .map(|s| s.into())
        .collect::<Vec<String>>();

    let watch_env_names = matches
        .get_many::<String>("watch-env")
        .unwrap_or_default()
        .map(|s| s.into())
        .collect::<Vec<String>>();

    let watch_env: HashMap<String, String> = HashMap::from_iter(
        watch_env_names
            .iter()
            .map(|name| (name.clone(), std::env::var(name).unwrap_or_default())),
    );

    let exclude_pwd = matches.get_flag("exclude-pwd");

    let share_cache = matches.get_flag("share-cache");

    let mut scope = ScopeBuilder::new()
        .cmd(cmd.to_string())
        .args(args)
        .watch_paths(watch_paths)
        .watch_scope(watch_scope)
        .watch_env(watch_env);

    if !exclude_pwd {
        scope = scope.pwd(std::env::current_dir().unwrap());
    }

    if share_cache {
        scope = scope.shared(true);
    } else {
        scope = scope.user(whoami::username());
    }

    Ok(Command::new(scope.build()?))
}

fn cache(matches: &clap::ArgMatches) -> anyhow::Result<DiskCache> {
    let share_cache = matches.get_flag("share-cache");
    let cache = matches.get_one::<PathBuf>("cache").unwrap();
    let cache_dir = cache.clone();

    let cache = cache::DiskCache::new(cache_dir, share_cache);

    Ok(cache)
}

fn record_options(matches: &clap::ArgMatches) -> anyhow::Result<RecordOptions> {
    let record_exit_codes = if let Some(exit_codes) = matches.get_one::<String>("record-exit-codes")
    {
        parse_exit_codes(exit_codes)
    } else {
        parse_exit_codes("0")
    };

    let cache_for = if let Some(s) = matches.get_one::<String>("cache-for") {
        Some(humantime::parse_duration(s).map_err(|_| {
            anyhow!(
                "invalid duration '{}', use values like 15s, 30m, 3h, 4d etc",
                s
            )
        })?)
    } else {
        None
    };

    Ok(RecordOptions::new(record_exit_codes, cache_for))
}

fn read_options(matches: &clap::ArgMatches) -> anyhow::Result<FindOptions> {
    let look_back = if let Some(s) = matches.get_one::<String>("look-back") {
        Some(humantime::parse_duration(s).map_err(|_| {
            anyhow!(
                "invalid duration '{}', use values like 15s, 30m, 3h, 4d etc",
                s
            )
        })?)
    } else {
        None
    };

    Ok(FindOptions::new(look_back))
}

fn run() -> anyhow::Result<i32> {
    let matches = cli()?.get_matches();

    DEBUG.set(matches.get_flag("debug")).unwrap();

    match matches.subcommand() {
        Some(("run", matches)) => deja::run(
            &mut command(matches)?,
            &cache(matches)?,
            record_options(matches)?,
            read_options(matches)?,
        ),
        Some(("read", matches)) => deja::read(
            &mut command(matches)?,
            &cache(matches)?,
            read_options(matches)?,
            *matches.get_one::<i32>("cache-miss-exit-code").unwrap_or(&1),
        ),
        Some(("force", matches)) => deja::force(
            &mut command(matches)?,
            &cache(matches)?,
            record_options(matches)?,
        ),
        Some(("remove", matches)) => deja::remove(&mut command(matches)?, &cache(matches)?),
        Some(("test", matches)) => deja::test(
            &mut command(matches)?,
            &cache(matches)?,
            read_options(matches)?,
        ),
        Some(("explain", matches)) => deja::explain(
            &mut command(matches)?,
            &cache(matches)?,
            read_options(matches)?,
        ),
        Some(("hash", matches)) => deja::hash(&mut command(matches)?, &cache(matches)?),
        Some(("completions", matches)) => {
            let shell_name = matches.get_one::<String>("shell").unwrap();
            let shell = clap_complete::Shell::from_str(shell_name).unwrap();
            clap_complete::generate(shell, &mut cli().unwrap(), "deja", &mut io::stdout());
            Ok(0)
        }
        _ => unreachable!("unknown subcommand not caught by clap"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match run() {
        Ok(status) => {
            std::process::exit(status);
        }
        Err(e) => {
            eprintln!("deja: {:?}", e);
            std::process::exit(1);
        }
    }
}
