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

fn subcommand(name: &str, about: &str, include_cache_miss_exit_code_param: bool, include_record_exit_codes_param: bool) -> clap::Command {
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

    let mut cache_args = vec![
        watch_path,
        watch_scope,
        watch_env,
        exclude_pwd,
        exclude_user,
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
                .help("Exit code to use when cache miss occurs")
                .default_value("1"),
        );
    }

    if include_record_exit_codes_param {
        cache_args.push(
            Arg::new("record-exit-codes")
                .long("record-exit-codes")
                .value_name("exit-codes")
                .help("Exit codes to record in the cache")
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
            subcommand(
                "run",
                "Return cached result or run and cache command",
                false,
                true
            ),
            subcommand("read", "Return cached result or exit", true, false),
            subcommand("force", "Run and cache command", false, true),
            subcommand("remove", "Remove command from cache", false, false),
            subcommand("test", "Test if command is cached", false, false),
            subcommand("explain", "Explain cache key for command", false, false),
            subcommand(
                "hash",
                "Print hash generated for command and options",
                false,
                false
            ),
        ]))
}

fn parse_exit_codes(
  param: &str
) -> [bool; 256] {
  let parts = param
    .split(',')
    .map(|s| s.trim());

  let mut exit_codes = [false; 256];
  for part in parts {
    if part.ends_with('+') {
      let start = part.trim_end_matches('+').parse::<i32>().unwrap();
      for i in start..=255 {
        exit_codes[i as usize] = true;
      }
    }
    else if part.contains('-') {
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

#[allow(clippy::type_complexity)]
fn collect_matches(
    matches: &clap::ArgMatches,
) -> anyhow::Result<(Command, impl Cache, Option<Duration>, Option<Duration>, [bool;256])> {

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

    let record_exit_codes = if let Some(exit_codes) = matches.get_one::<String>("record-exit-codes") {
      parse_exit_codes(exit_codes)
    }
    else {
      parse_exit_codes("0")
    };

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

    Ok((Command::new(scope.build()?), cache, look_back, cache_for, record_exit_codes))
}

fn run() -> anyhow::Result<i32> {
    let matches = cli()?.get_matches();

    DEBUG.set(matches.get_flag("debug")).unwrap();

    match matches.subcommand() {
        Some(("run", matches)) => {
            let (mut command, cache, look_back, cache_for, record_exit_codes) = collect_matches(matches)?;
            action::run(&mut command, &cache, look_back, cache_for, record_exit_codes)
        }
        Some(("read", matches)) => {
            let (mut command, cache, look_back, _cache_for, _record_exit_codes) = collect_matches(matches)?;
            let exit_code_on_cache_miss = matches.get_one::<i32>("cache-miss-exit-code").unwrap_or(&1);
            action::read(&mut command, &cache, look_back, *exit_code_on_cache_miss)
        }
        Some(("force", matches)) => {
            let (mut command, cache, _look_back, cache_for, record_exit_codes) = collect_matches(matches)?;
            action::force(&mut command, &cache, cache_for, record_exit_codes)
        }
        Some(("remove", matches)) => {
            let (mut command, cache, _look_back, _cache_for, _record_exit_codes) = collect_matches(matches)?;
            action::remove(&mut command, &cache)
        }
        Some(("test", matches)) => {
            let (mut command, cache, look_back, _cache_for, _record_exit_codes) = collect_matches(matches)?;
            action::test(&mut command, &cache, look_back)
        }
        Some(("explain", matches)) => {
            let (mut command, cache, look_back, _cache_for, _record_exit_codes) = collect_matches(matches)?;
            action::explain(&mut command, &cache, look_back)
        }
        Some(("hash", matches)) => {
            let (mut command, cache, _look_back, _cache_for, _record_exit_codes) = collect_matches(matches)?;
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
