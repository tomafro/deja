use anyhow::anyhow;
use core::str;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fmt::Formatter;
use std::io::Write;
use std::path::PathBuf;
use std::{
    io::{BufRead, BufReader},
    process::Stdio,
    thread,
    time::Instant,
};
use ulid::Ulid;

use crate::hash::{self, Hash};

fn capture_output<R, W, O>(
    start: Instant,
    mut reader: R,
    mut writer: W,
    mut output: O,
) -> thread::JoinHandle<W>
where
    R: BufRead + Send + 'static,
    W: Write + Send + 'static,
    O: Write + Send + 'static,
{
    thread::spawn(move || {
        let line = &mut String::new();
        while let Ok(count) = reader.read_line(line) {
            if count == 0 {
                break;
            }
            let bytes = line.as_bytes();

            output.write_all(bytes).unwrap();

            let elapsed = start.elapsed().as_nanos().to_be_bytes();

            writer.write_all(&elapsed).unwrap();
            writer.write_all(bytes).unwrap();

            line.clear();
        }
        writer
    })
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct ScopeBuilder {
    format: String,
    cmd: String,
    args: Vec<String>,
    shared: bool,
    user: Option<String>,
    pwd: Option<OsString>,
    watch_paths: Vec<PathBuf>,
    watch_scope: HashSet<String>,
    watch_env: HashMap<String, String>,
}

impl ScopeBuilder {
    pub fn new() -> Self {
        ScopeBuilder {
            format: env!("CARGO_PKG_VERSION").to_string(),
            shared: false,
            ..Default::default()
        }
    }

    pub fn cmd(mut self, cmd: impl Into<String>) -> Self {
        self.cmd = cmd.into();
        self
    }

    pub fn args<T>(mut self, args: impl IntoArgs<T>) -> Self {
        self.args = args.into_args();
        self
    }

    pub fn shared(mut self, shared: bool) -> Self {
        self.shared = shared;
        self
    }

    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn pwd(mut self, pwd: PathBuf) -> Self {
        self.pwd = Some(pwd.as_os_str().to_os_string());
        self
    }

    pub fn watch_paths(mut self, watch_paths: Vec<PathBuf>) -> Self {
        self.watch_paths = watch_paths;
        self
    }

    pub fn watch_scope(mut self, watch_scope: impl IntoWatchScope) -> Self {
        self.watch_scope = watch_scope.into_watch_scope();
        self
    }

    pub fn watch_env<T>(mut self, watch_env: impl IntoEnv<T>) -> Self {
        self.watch_env = watch_env.into_env();
        self
    }

    pub fn hash(&self) -> anyhow::Result<String> {
        let format_hash = hash::Hash::from(&self.format);
        let cmd_hash = hash::Hash::from(&self.cmd);
        let args_hash = hash::Hash::from(&self.args);
        let shared_hash = hash::Hash::from(self.shared);
        let user_hash = hash::Hash::from(&self.user);
        let pwd_hash = hash::Hash::from(&self.pwd);
        let watch_scope_hash = hash::Hash::from(&self.watch_scope);
        let watch_env_hash = hash::Hash::from(&self.watch_env);
        let watch_paths_hash = hash::Hash::try_from(&self.watch_paths)?;
        let hash = hash::Hash::from(&vec![
            format_hash,
            cmd_hash,
            args_hash,
            shared_hash,
            user_hash,
            pwd_hash,
            watch_scope_hash,
            watch_env_hash,
            watch_paths_hash,
        ]);
        Ok(hash.hex())
    }

    pub fn build(self) -> anyhow::Result<Scope> {
        Ok(Scope {
            hash: self.hash()?,
            format: self.format,
            cmd: self.cmd,
            args: self.args,
            user: self.user,
            pwd: self.pwd,
            watch_paths: self.watch_paths,
            watch_scope: self.watch_scope,
            watch_env: self.watch_env,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Scope {
    format: String,
    cmd: String,
    args: Vec<String>,
    user: Option<String>,
    pwd: Option<OsString>,
    watch_paths: Vec<PathBuf>,
    watch_scope: HashSet<String>,
    watch_env: HashMap<String, String>,
    hash: String,
}

pub trait IntoArgs<T> {
    fn into_args(self) -> Vec<String>;
}

impl IntoArgs<Vec<String>> for Vec<String> {
    fn into_args(self) -> Vec<String> {
        self
    }
}

impl IntoArgs<String> for String {
    fn into_args(self) -> Vec<String> {
        self.split_whitespace().map(|s| s.to_string()).collect()
    }
}

impl IntoArgs<String> for &str {
    fn into_args(self) -> Vec<String> {
        self.split_whitespace().map(|s| s.to_string()).collect()
    }
}

pub trait IntoEnv<T> {
    fn into_env(self) -> HashMap<String, String>;
}

impl IntoEnv<HashMap<String, String>> for HashMap<String, String> {
    fn into_env(self) -> HashMap<String, String> {
        self
    }
}

impl IntoEnv<HashMap<String, String>> for String {
    fn into_env(self) -> HashMap<String, String> {
        self.split_whitespace()
            .filter_map(|pair| {
                pair.split_once('=')
                    .map(|(key, value)| (key.to_string(), value.to_string()))
            })
            .collect()
    }
}

impl IntoEnv<HashMap<String, String>> for &str {
    fn into_env(self) -> HashMap<String, String> {
        self.to_string().into_env()
    }
}

pub trait IntoWatchScope {
    fn into_watch_scope(self) -> HashSet<String>;
}

impl IntoWatchScope for HashSet<String> {
    fn into_watch_scope(self) -> HashSet<String> {
        self
    }
}

impl IntoWatchScope for Vec<String> {
    fn into_watch_scope(self) -> HashSet<String> {
        self.into_iter().collect()
    }
}

impl Scope {
    pub fn explanation(&self) -> ScopeExplanation {
        ScopeExplanation { scope: self }
    }
}

pub struct ScopeExplanation<'a> {
    scope: &'a Scope,
}

impl<'a> ScopeExplanation<'a> {
    fn explain_cmd_and_args(&self, result: &mut String) {
        result.push_str(format!("cmd: {}", self.scope.cmd).as_str());
        for arg in &self.scope.args {
            result.push_str(format!(" {}", arg).as_str());
        }
        result.push('\n');
    }

    fn explain_user(&self, result: &mut String) {
        if let Some(user) = &self.scope.user {
            result.push_str(format!("user: {}\n", user).as_str());
        }
    }

    fn explain_pwd(&self, result: &mut String) {
        if let Some(pwd) = &self.scope.pwd {
            result.push_str(format!("pwd: {}\n", pwd.to_string_lossy()).as_str());
        }
    }

    fn explain_watch_scope(&self, result: &mut String) {
        if !self.scope.watch_scope.is_empty() {
            result.push_str("scope:");
            for scope in &self.scope.watch_scope {
                result.push_str(format!(" \"{}\"", scope).as_str());
            }
            result.push('\n');
        }
    }

    fn explain_watch_paths(&self, result: &mut String) {
        if !self.scope.watch_paths.is_empty() {
            result.push_str("paths:\n");
            for path in &self.scope.watch_paths {
                result.push_str(
                    format!(
                        "  {}: {}\n",
                        path.to_string_lossy(),
                        Hash::try_from(path).unwrap()
                    )
                    .as_str(),
                );
            }
        }
    }

    fn explain_watch_env(&self, result: &mut String) {
        if !self.scope.watch_env.is_empty() {
            result.push_str("env:\n");
            for (key, value) in &self.scope.watch_env {
                result.push_str(format!("  {}: {}\n", key, value).as_str());
            }
        }
    }

    pub fn explain(&self) -> String {
        let mut result = String::new();
        self.explain_cmd_and_args(&mut result);
        self.explain_user(&mut result);
        self.explain_pwd(&mut result);
        self.explain_watch_scope(&mut result);
        self.explain_watch_paths(&mut result);
        self.explain_watch_env(&mut result);
        result
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Command {
    pub ulid: String,
    pub scope: Scope,
}

impl Command {
    pub fn new(scope: Scope) -> Self {
        let ulid = Ulid::new().to_string();
        Command { ulid, scope }
    }

    pub fn hash(&self) -> &str {
        &self.scope.hash
    }

    pub fn run<O, E>(&mut self, stdout_capture: O, stderr_capture: E) -> anyhow::Result<(i32, O, E)>
    where
        O: Write + Send + 'static,
        E: Write + Send + 'static,
    {
        let mut child = std::process::Command::new(&self.scope.cmd)
            .args(&self.scope.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                let message = match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        format!("command not found: {}", self.scope.cmd)
                    }
                    std::io::ErrorKind::PermissionDenied => {
                        format!("permission denied running command: {}", self.scope.cmd)
                    }
                    _ => format!("error running command: {}", self.scope.cmd),
                };

                anyhow!("{}", message)
            })?;

        let start = Instant::now();

        let child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("unable to capture stdout"))?;
        let child_stdout_handle = capture_output(
            start,
            BufReader::new(child_stdout),
            stdout_capture,
            std::io::stdout(),
        );

        let child_stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("unable to capture stderr"))?;
        let child_stderr_handle = capture_output(
            start,
            BufReader::new(child_stderr),
            stderr_capture,
            std::io::stderr(),
        );

        let status = child
            .wait()
            .map_err(|e| anyhow!("error waiting for command to finish: {}", e))?
            .code()
            .unwrap_or(1);

        let stdout = child_stdout_handle.join().unwrap();
        let stderr = child_stderr_handle.join().unwrap();

        Ok((status, stdout, stderr))
    }
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.scope.cmd)?;
        for arg in &self.scope.args {
            write!(f, " {}", arg)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn assert_unique<T>(elements: Vec<T>)
    where
        T: Eq + Ord + Clone,
    {
        let mut sorted = elements.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(elements.len(), sorted.len(), "elements are unique");
    }

    fn scope() -> ScopeBuilder {
        ScopeBuilder::new()
    }

    #[test]
    fn test_scope() {
        let cmds = vec!["echo", "cat", "ls"];
        let mut hashes = cmds
            .iter()
            .map(|cmd| ScopeBuilder::new().cmd(cmd.to_string()).hash().unwrap())
            .collect::<Vec<_>>();

        hashes.sort();
        hashes.dedup();

        assert_eq!(
            cmds.len(),
            hashes.len(),
            "hashes for each command are unique"
        );
    }

    #[test]
    fn test_scope_empty() -> anyhow::Result<()> {
        assert_eq!(scope().hash()?, scope().hash()?, "empty scopes are equal");

        Ok(())
    }

    #[test]
    fn test_scope_shared() -> anyhow::Result<()> {
        assert_eq!(
            scope().shared(true).hash()?,
            scope().shared(true).hash()?,
            "hashes are equal when shared"
        );

        assert_eq!(
            scope().shared(false).hash()?,
            scope().shared(false).hash()?,
            "hashes are equal when not shared"
        );

        assert_ne!(
            scope().shared(false).hash()?,
            scope().shared(true).hash()?,
            "hashes are not equal when sharing status is different"
        );

        Ok(())
    }

    #[test]
    fn test_scopes() -> anyhow::Result<()> {
        assert_unique(vec![
            scope().cmd("echo").hash()?,
            scope().cmd("echo").args("--arg").hash()?,
            scope().cmd("echo").args("--one").hash()?,
            scope().cmd("echo").args("--one --two").hash()?,
            scope().cmd("echo").args("--two --one").hash()?,
            scope().cmd("echo").watch_env("A=1").hash()?,
            scope().cmd("echo").watch_env("B=1").hash()?,
            scope().cmd("echo").watch_env("A=1 B=1").hash()?,
        ]);

        Ok(())
    }

    #[test]
    fn test_scope_env() -> anyhow::Result<()> {
        assert_eq!(
            scope().watch_env("A=1 B=2").hash()?,
            scope().watch_env("B=2 A=1").hash()?,
            "hashes are equal regardless of order of env vars"
        );

        assert_ne!(
            scope().watch_env("A=2 B=2").hash()?,
            scope().watch_env("B=2 A=1").hash()?,
            "hashes are different when env vars are different"
        );

        Ok(())
    }

    #[test]
    fn test_scope_args() -> anyhow::Result<()> {
        assert_ne!(
            scope().args("--one").hash()?,
            scope().args("--two").hash()?,
            "hashes are different when args are different"
        );

        assert_ne!(
            scope().args("--one --two").hash()?,
            scope().args("--two --one").hash()?,
            "hashes are different when args are in different order"
        );

        Ok(())
    }

    #[test]
    fn test_scope_scope() -> anyhow::Result<()> {
        assert_ne!(
            scope().watch_scope(vec!["a".into(), "b".into()]).hash()?,
            scope().watch_scope(vec!["a".into(), "c".into()]).hash()?,
            "hashes are different when scopes are different"
        );

        assert_eq!(
            scope().watch_scope(vec!["a".into(), "b".into()]).hash()?,
            scope().watch_scope(vec!["b".into(), "a".into()]).hash()?,
            "hashes are equal regardless of order of scopes"
        );

        Ok(())
    }
}
