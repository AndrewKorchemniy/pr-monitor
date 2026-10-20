//! The real binary's command line: exit codes and where text goes. Only modes that print and
//! exit are spawned here; Run mode and `--unregister` open windows or touch the system.
#![expect(
    clippy::unwrap_used,
    reason = "test helpers outside #[test] fns; a setup failure should fail the test"
)]

use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

/// A hung child (a modal dialog instead of console text) fails the test instead of the run.
const CHILD_BUDGET: Duration = Duration::from_secs(60);

/// What the binary did, with output decoded lossily for assertions.
struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl std::fmt::Display for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "exit {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.code, self.stdout, self.stderr
        )
    }
}

fn spawn_reader(
    mut pipe: impl Read + Send + 'static,
    done: mpsc::Sender<()>,
) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = done.send(());
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Runs the binary with an isolated, empty data folder (also its working directory) and none
/// of the app's environment overrides.
fn run_in(home: &std::path::Path, args: &[&str]) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pr-monitor"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // why: a console-subsystem debug build must not flash a console window of its own.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .args(args)
        .current_dir(home)
        .env("PRMONITOR_HOME", home)
        .env_remove("PRMONITOR_LOG")
        .env_remove("PRMONITOR_SECRET_STORE")
        .env_remove("PRMONITOR_REGISTER")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (done, finished) = mpsc::channel();
    let stdout = spawn_reader(child.stdout.take().unwrap(), done.clone());
    let stderr = spawn_reader(child.stderr.take().unwrap(), done);
    let closed = (0..2).all(|_| finished.recv_timeout(CHILD_BUDGET).is_ok());
    if !closed {
        let _ = child.kill();
    }
    let status = child.wait().unwrap();
    let run = Run {
        code: status.code(),
        stdout: stdout.join().unwrap(),
        stderr: stderr.join().unwrap(),
    };
    assert!(closed, "pr-monitor {args:?} hung and was killed\n{run}");
    run
}

fn run(args: &[&str]) -> Run {
    let home = tempfile::tempdir().unwrap();
    run_in(home.path(), args)
}

#[test]
fn version_prints_the_version_and_exits_zero() {
    for flag in ["--version", "-V"] {
        let run = run(&[flag]);
        assert_eq!(run.code, Some(0), "{flag}\n{run}");
        assert!(
            run.stdout.contains(env!("CARGO_PKG_VERSION")),
            "{flag}\n{run}"
        );
    }
}

#[test]
fn help_lists_the_flags_and_exits_zero() {
    for flag in ["--help", "-h"] {
        let run = run(&[flag]);
        assert_eq!(run.code, Some(0), "{flag}\n{run}");
        for listed in [
            "Usage:",
            "--minimized",
            "--home",
            "--self-test",
            "--unregister",
        ] {
            assert!(run.stdout.contains(listed), "{flag} lacks {listed}\n{run}");
        }
    }
}

/// With a console attached (as under `cargo test`), a usage error is printed to stdout rather
/// than shown in a modal dialog, which would hang the test until the budget runs out.
#[test]
fn usage_errors_print_usage_to_stdout_and_exit_two() {
    let cases: [&[&str]; 2] = [&["--no-such-flag"], &["--self-test", "--version"]];
    for args in cases {
        let run = run(args);
        assert_eq!(run.code, Some(2), "{args:?}\n{run}");
        assert!(run.stdout.contains("Usage:"), "{args:?}\n{run}");
    }
}

#[test]
fn self_test_passes_and_writes_nothing() {
    let home = tempfile::tempdir().unwrap();
    let run = run_in(home.path(), &["--self-test"]);
    assert_eq!(run.code, Some(0), "{run}");
    assert!(run.stdout.contains(r#""self_test":"ok""#), "{run}");
    let written: Vec<_> = std::fs::read_dir(home.path())
        .unwrap()
        .map(|entry| entry.map(|e| e.path()))
        .collect();
    assert!(written.is_empty(), "self-test wrote {written:?}\n{run}");
}
