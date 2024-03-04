mod action;
mod cache;
mod command;
mod hash;

use crate::cache::Cache;
use crate::command::Command;
use anyhow::anyhow;
use clap::value_parser;
use clap::Arg;
use command::ScopeBuilder;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use std::sync::OnceLock;

pub static DEBUG: OnceLock<bool> = OnceLock::new();

pub fn debug(string: String) {
    if DEBUG.get_or_init(|| false).to_owned() {
        eprintln!("- {}", string);
    };
}

fn subcommand(name: &str, about: &str) -> clap::Command {
    let mut cache = Arg::new("cache")
        .long("cache")
        .value_name("path")
        .help("Cache directory")
        .env("DEJA_CACHE")
        .hide_env(true)
        .value_parser(value_parser!(PathBuf));

    cache = if let Some(cache_dir) = dirs::cache_dir() {
        cache.default_value(cache_dir.join("deja").into_os_string())
    } else {
        cache
    };

    let watch_path = Arg::new("watch-path")
        .long("watch-path")
        .value_name("path")
        .help("Include path contents in cache key (any file or directory)")
        .value_parser(value_parser!(PathBuf))
        .action(clap::ArgAction::Append);

    let watch_scope = Arg::new("watch-scope")
        .long("watch-scope")
        .value_name("scope")
        .help("Include given scope in cache key (any string)")
        .env("DEJA_WATCH_SCOPE")
        .action(clap::ArgAction::Append);

    let watch_env = Arg::new("watch-env")
        .long("watch-env")
        .value_name("env")
        .help("Include environment variable value in cache key")
        .action(clap::ArgAction::Append);

    let exclude_pwd = Arg::new("exclude-pwd")
        .long("exclude-pwd")
        .help("Remove current directory from cache key (default: false)")
        .env("DEJA_IGNORE_PWD")
        .action(clap::ArgAction::SetTrue);

    let exclude_user = Arg::new("exclude-user")
        .long("exclude-user")
        .help("Remove current user from cache key (default: false)")
        .env("DEJA_IGNORE_USER")
        .action(clap::ArgAction::SetTrue);

    let look_back = Arg::new("look-back")
        .long("look-back")
        .value_name("duration")
        .env("DEJA_LOOK_BACK")
        .help("When reading from the cache, how far back in time to look (e.g. 30s, 15m, 1h, 5d)")
        .long_help("When reading from the cache, only consider results created in the given time period (e.g. 30s, 15m, 1h, 5d)\n\nThis can be useful to ensure the result is still fresh.");

    let cache_for = Arg::new("cache-for")
        .long("cache-for")
        .value_name("duration")
        .env("DEJA_CACHE_FOR")
        .help("When writing to the cache, how long a result should be valid (e.g. 30s, 15m, 1h, 5d)")
        .long_help("When writing to the cache, only store results for the given time period (e.g. 30s, 15m, 1h, 5d)\n\nThis can be useful to ensure the result is still fresh.");

    let command = Arg::new("command")
        .value_name("COMMAND")
        .required(true)
        .help("Command to run");

    let arguments = Arg::new("arguments")
        .value_name("ARGUMENTS")
        .help("Arguments to pass to command")
        .action(clap::ArgAction::Append);

    let cache_args = vec![
        watch_path,
        watch_scope,
        watch_env,
        exclude_pwd,
        exclude_user,
        look_back,
        cache_for,
        cache,
        command,
        arguments,
    ];

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
    Ok(clap::command!()
        .name("deja")
        .arg_required_else_help(true)
        .styles(styles())
        .arg(
            Arg::new("debug")
                .long("debug")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .hide(false),
        )
        .subcommands(vec![
            subcommand("run", "Return cached result or run and cache command"),
            subcommand("read", "Return cached result or exit"),
            subcommand("force", "Run and cache command"),
            subcommand("remove", "Remove command from cache"),
            subcommand("test", "Test if command is cached"),
            subcommand("explain", "Explain cache key for command"),
            subcommand("hash", "Print hash generated for command and options"),
        ]))
}

fn collect_matches(
    matches: &clap::ArgMatches,
) -> anyhow::Result<(Command, impl Cache, Option<Duration>, Option<Duration>)> {
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
    let exclude_user = matches.get_flag("exclude-user");

    let mut scope = ScopeBuilder::new()
        .cmd(cmd.to_string())
        .args(args)
        .watch_paths(watch_paths)
        .watch_scope(watch_scope)
        .watch_env(watch_env);

    if !exclude_pwd {
        scope = scope.pwd(std::env::current_dir().unwrap());
    }

    if !exclude_user {
        scope = scope.user(whoami::username());
    }

    let look_back_arg = matches.get_one::<String>("look-back");

    let look_back = if let Some(s) = look_back_arg {
        Some(humantime::parse_duration(s).map_err(|_| {
            anyhow!(
                "invalid duration '{}', use values like 15s, 30m, 3h, 4d etc",
                s
            )
        })?)
    } else {
        None
    };

    let cache_for_arg = matches.get_one::<String>("cache-for");

    let cache_for = if let Some(s) = cache_for_arg {
        Some(humantime::parse_duration(s).map_err(|_| {
            anyhow!(
                "invalid duration '{}', use values like 15s, 30m, 3h, 4d etc",
                s
            )
        })?)
    } else {
        None
    };

    let cache_dir = matches.get_one::<PathBuf>("cache").unwrap();

    let cache = cache::DiskCache::new(cache_dir.clone());

    Ok((Command::new(scope.build()?), cache, look_back, cache_for))
}

fn run() -> anyhow::Result<i32> {
    let matches = cli()?.get_matches();

    DEBUG.set(matches.get_flag("debug")).unwrap();

    match matches.subcommand() {
        Some(("run", matches)) => {
            let (mut command, cache, look_back, cache_for) = collect_matches(matches)?;
            action::run(&mut command, &cache, look_back, cache_for)
        }
        Some(("read", matches)) => {
            let (mut command, cache, look_back, _cache_for) = collect_matches(matches)?;
            action::read(&mut command, &cache, look_back)
        }
        Some(("force", matches)) => {
            let (mut command, cache, _look_back, cache_for) = collect_matches(matches)?;
            action::force(&mut command, &cache, cache_for)
        }
        Some(("remove", matches)) => {
            let (mut command, cache, _look_back, _cache_for) = collect_matches(matches)?;
            action::remove(&mut command, &cache)
        }
        Some(("test", matches)) => {
            let (mut command, cache, look_back, _cache_for) = collect_matches(matches)?;
            action::test(&mut command, &cache, look_back)
        }
        Some(("explain", matches)) => {
            let (mut command, cache, look_back, _cache_for) = collect_matches(matches)?;
            action::explain(&mut command, &cache, look_back)
        }
        Some(("hash", matches)) => {
            let (mut command, cache, _look_back, _cache_for) = collect_matches(matches)?;
            action::hash(&mut command, &cache)
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
