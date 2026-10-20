//! Running external commands (`az`) hidden, with a timeout and cancellation that kill the whole
//! process tree. Behind a trait so tests never spawn anything.
//!
//! Commands always run in `%SystemRoot%` on Windows, so a planted `az.cmd` in the working
//! directory is ignored, and never open a console window (`CREATE_NO_WINDOW`).

use std::{ffi::OsString, path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use tokio_util::sync::CancellationToken;

/// What to run. Built by pure functions so its exact shape is unit-tested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    pub timeout: Duration,
}

/// `stdout` may contain a token: never log it. `Debug` shows only its length.
#[derive(Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl std::fmt::Debug for ProcessOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessOutput")
            .field("status", &self.status)
            .field("stdout_len", &self.stdout.len())
            .field("stderr", &self.stderr)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    #[error("not found")]
    NotFound,
    #[error("timed out")]
    Timeout,
    #[error("cancelled")]
    Cancelled,
    #[error("io: {0}")]
    Io(Arc<str>),
    #[error("invalid argument: {0}")]
    InvalidInput(Arc<str>),
}

#[async_trait::async_trait]
pub trait CommandRunner: Send + Sync + 'static {
    async fn run(
        &self,
        spec: ProcessSpec,
        cancel: CancellationToken,
    ) -> Result<ProcessOutput, RunError>;
}

/// Spawns real processes with tokio.
#[derive(Copy, Clone, Debug, Default)]
pub struct TokioCommandRunner;

#[async_trait::async_trait]
impl CommandRunner for TokioCommandRunner {
    async fn run(
        &self,
        spec: ProcessSpec,
        cancel: CancellationToken,
    ) -> Result<ProcessOutput, RunError> {
        let mut command = build_command(&spec);
        let child = command.spawn().map_err(spawn_error)?;
        let pid = child.id();
        tokio::select! {
            output = child.wait_with_output() => {
                let output = output.map_err(|e| RunError::Io(e.to_string().into()))?;
                Ok(ProcessOutput {
                    status: output.status.code(),
                    stdout: output.stdout,
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                })
            }
            () = tokio::time::sleep(spec.timeout) => {
                kill_tree(pid).await;
                Err(RunError::Timeout)
            }
            () = cancel.cancelled() => {
                kill_tree(pid).await;
                Err(RunError::Cancelled)
            }
        }
    }
}

fn build_command(spec: &ProcessSpec) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(&spec.program);
    command
        .args(&spec.args)
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(cwd) = system_root() {
        command.current_dir(cwd);
    }
    #[cfg(windows)]
    command.creation_flags(crate::consts::CREATE_NO_WINDOW);
    // why: `az` on Unix is a shell wrapper that runs Python as a child; its own process group
    // lets `kill_tree` kill both instead of orphaning Python.
    #[cfg(unix)]
    command.process_group(0);
    command
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "io::ErrorKind is non_exhaustive; only two kinds need their own mapping"
)]
fn spawn_error(e: std::io::Error) -> RunError {
    match e.kind() {
        std::io::ErrorKind::NotFound => RunError::NotFound,
        std::io::ErrorKind::InvalidInput => RunError::InvalidInput(e.to_string().into()),
        _ => RunError::Io(e.to_string().into()),
    }
}

/// `%SystemRoot%` on Windows; `None` elsewhere (inherit).
fn system_root() -> Option<PathBuf> {
    if cfg!(windows) {
        Some(
            std::env::var_os("SystemRoot")
                .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from),
        )
    } else {
        None
    }
}

/// `az` is a wrapper (`cmd.exe` on Windows, a shell script elsewhere) running Python: killing
/// the wrapper alone orphans Python, so kill the whole tree or process group.
async fn kill_tree(pid: Option<u32>) {
    let Some(pid) = pid else {
        return;
    };
    let spec = tree_kill_spec(pid);
    let mut command = build_command(&spec);
    let wait = async {
        if let Ok(mut child) = command.spawn() {
            let _ = child.wait().await;
        }
    };
    if tokio::time::timeout(spec.timeout, wait).await.is_err() {
        tracing::warn!(pid, "process tree kill did not finish in time");
    }
}

/// `taskkill /T /F` on Windows; elsewhere `kill -KILL` on the process group that
/// `build_command` put the child in.
fn tree_kill_spec(pid: u32) -> ProcessSpec {
    let (program, args) = if cfg!(windows) {
        let taskkill = system_root().map_or_else(
            || PathBuf::from("taskkill.exe"),
            |r| r.join("System32").join("taskkill.exe"),
        );
        (
            taskkill,
            vec![
                "/PID".to_owned(),
                pid.to_string(),
                "/T".to_owned(),
                "/F".to_owned(),
            ],
        )
    } else {
        let group = format!("-{pid}");
        (
            PathBuf::from("/bin/kill"),
            vec!["-KILL".to_owned(), "--".to_owned(), group],
        )
    };
    ProcessSpec {
        program,
        args: args.into_iter().map(OsString::from).collect(),
        env: Vec::new(),
        timeout: crate::consts::TREE_KILL_TIMEOUT,
    }
}

#[cfg(test)]
mod tests;
