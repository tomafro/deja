use anyhow::anyhow;
use core::str;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Formatter;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::{
    io::{BufRead, BufReader},
    process::Stdio,
    thread,
    time::{Instant, SystemTime},
};

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
        let mut line = &mut String::new();
        while let Ok(count) = reader.read_line(&mut line) {
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
    pub format: String,
    pub cmd: String,
    pub args: Vec<String>,
    pub shared: bool,
    pub user: Option<String>,
    pub pwd: Option<OsString>,
    pub watch_paths: Vec<PathBuf>,
    pub watch_scope: Vec<String>,
    pub watch_env: HashMap<String, String>,
}

impl ScopeBuilder {
    pub fn new() -> Self {
        ScopeBuilder {
            format: env!("CARGO_PKG_VERSION").to_string(),
            shared: false,
            ..Default::default()
        }
    }

    pub fn cmd(mut self, cmd: String) -> Self {
        self.cmd = cmd;
        self
    }

    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn shared(mut self, shared: bool) -> Self {
        self.shared = shared;
        self
    }

    pub fn user(mut self, user: String) -> Self {
        self.user = Some(user);
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

    pub fn watch_scope(mut self, watch_scope: Vec<String>) -> Self {
        self.watch_scope = watch_scope;
        self
    }

    pub fn watch_env(mut self, watch_env: HashMap<String, String>) -> Self {
        self.watch_env = watch_env;
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
    pub format: String,
    pub cmd: String,
    pub args: Vec<String>,
    pub user: Option<String>,
    pub pwd: Option<OsString>,
    pub watch_paths: Vec<PathBuf>,
    pub watch_scope: Vec<String>,
    pub watch_env: HashMap<String, String>,
    pub hash: String,
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
    pub scope: Scope,
}

impl Command {
    pub fn new(scope: Scope) -> Self {
        Command { scope }
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

pub(crate) fn replay_output(stdout: &mut OutputReader, stderr: &mut OutputReader) {
    let mut stdout = stdout.peekable();
    let mut stderr = stderr.peekable();

    loop {
        match (stdout.peek(), stderr.peek()) {
            (Some((ot, ol)), Some((et, el))) => {
                if ot < et {
                    print!("{}", ol);
                    stdout.next();
                } else {
                    eprint!("{}", el);
                    stderr.next();
                }
            }
            (Some((_, ol)), None) => {
                print!("{}", ol);
                stdout.next();
            }
            (None, Some((_, el))) => {
                eprint!("{}", el);
                stderr.next();
            }
            (None, None) => break,
        }
    }
}

pub trait CommandResult {
    fn created(&self) -> SystemTime;
    fn expires(&self) -> Option<SystemTime>;
    fn status(&self) -> i32;
    fn replay_output(&self);

    fn has_expired(&self) -> bool {
        self.expires()
            .map_or(false, |expires| expires < SystemTime::now())
    }

    fn is_older_than(&self, look_back: std::time::Duration) -> bool {
        self.created().elapsed().unwrap() > look_back
    }

    fn replay(&self) -> i32 {
        self.replay_output();
        self.status()
    }
}

pub struct OutputReader<'a> {
    pub reader: BufReader<&'a [u8]>,
}

impl<'a> Iterator for OutputReader<'a> {
    type Item = (u128, String);

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        let mut bytes: [u8; 16] = [0; 16];

        // First 16 bytes are the timestamp

        match self.reader.read_exact(&mut bytes) {
            Ok(()) => (),
            Err(_) => return None,
        }

        // Following the timestamp is the line contents

        match self.reader.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some((u128::from_be_bytes(bytes), line.to_string())),
            Err(_) => None,
        }
    }
}
