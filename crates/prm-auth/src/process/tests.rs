use super::*;

fn strings(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

/// Killing only the `az` wrapper orphaned its Python child; the kill targets the whole tree
/// (Windows) or the process group (Unix).
#[test]
fn tree_kill_spec_targets_the_tree() {
    let spec = tree_kill_spec(4242);
    if cfg!(windows) {
        assert!(spec.program.ends_with("taskkill.exe"), "{spec:?}");
        assert_eq!(strings(&spec.args), ["/PID", "4242", "/T", "/F"]);
    } else {
        assert_eq!(spec.program, PathBuf::from("/bin/kill"));
        assert_eq!(strings(&spec.args), ["-KILL", "--", "-4242"]);
    }
    assert_eq!(spec.timeout, crate::consts::TREE_KILL_TIMEOUT);
}

#[test]
fn process_output_debug_redacts_stdout() {
    let output = ProcessOutput {
        status: Some(0),
        stdout: b"{\"accessToken\":\"secret-token\"}".to_vec(),
        stderr: String::new(),
    };
    let printed = format!("{output:?}");
    assert!(!printed.contains("secret-token"), "{printed}");
    assert!(printed.contains("stdout_len"), "{printed}");
}
