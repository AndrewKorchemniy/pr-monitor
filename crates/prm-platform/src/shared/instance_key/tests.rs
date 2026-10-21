use super::*;

fn root() -> PathBuf {
    PathBuf::from(r"C:\Users\ann\AppData\Local\PRMonitor")
}

#[test]
fn instance_key_default_exact_mutex_name() {
    let key = derive(&root(), InstanceScope::Default, Some(3));
    assert_eq!(key.mutex_name, r"Local\AndrewKorchemniy.PRMonitor");
    assert_eq!(key.pipe_name, "AndrewKorchemniy.PRMonitor.3");
    assert_eq!(key.lock_file, root().join("instance.lock"));
}

#[test]
fn instance_key_isolated_appends_root_hash() {
    let key = derive(&root(), InstanceScope::Isolated, Some(3));
    assert_eq!(key.suffix.len(), 8);
    assert!(key.suffix.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        key.mutex_name,
        format!(r"Local\AndrewKorchemniy.PRMonitor.{}", key.suffix)
    );
    assert_eq!(
        key.pipe_name,
        format!("AndrewKorchemniy.PRMonitor.3.{}", key.suffix)
    );
    let other = derive(Path::new(r"D:\other"), InstanceScope::Isolated, Some(3));
    assert_ne!(other.suffix, key.suffix);
}

#[test]
fn instance_key_pipe_includes_session() {
    let one = derive(&root(), InstanceScope::Default, Some(1));
    let two = derive(&root(), InstanceScope::Default, Some(2));
    assert_ne!(one.pipe_name, two.pipe_name);
    assert_eq!(
        one.mutex_name, two.mutex_name,
        "Local\\ is per session already"
    );
}

#[cfg(windows)]
#[test]
fn instance_key_root_hash_case_insensitive_on_windows() {
    let upper = derive(Path::new(r"C:\DATA\PRM"), InstanceScope::Isolated, Some(1));
    let lower = derive(Path::new(r"c:\data\prm"), InstanceScope::Isolated, Some(1));
    assert_eq!(upper.suffix, lower.suffix);
}

#[test]
fn instance_key_unix_socket_name() {
    let key = derive(
        Path::new("/home/ann/.local/share/PRMonitor"),
        InstanceScope::Default,
        None,
    );
    assert_eq!(
        key.pipe_name,
        format!("AndrewKorchemniy.PRMonitor.{}.sock", key.suffix)
    );
}

/// `--home D:\prm` and `--home D:\prm\` used to get different mutexes, so two primaries ran
/// over one folder.
#[test]
fn instance_key_root_hash_ignores_spelling() {
    let base = Path::new("/data/prm");
    let spellings = [
        PathBuf::from("/data/prm/"),
        PathBuf::from("/data/./prm"),
        PathBuf::from("/data/prm/."),
        PathBuf::from("/data/x/../prm"),
    ];
    let expected = derive(base, InstanceScope::Isolated, None).suffix;
    for spelled in spellings {
        assert_eq!(
            derive(&spelled, InstanceScope::Isolated, None).suffix,
            expected,
            "{}",
            spelled.display()
        );
    }
}

#[cfg(windows)]
#[test]
fn instance_key_root_hash_ignores_separator_style_on_windows() {
    let back = derive(Path::new(r"D:\prm\data"), InstanceScope::Isolated, Some(1));
    let forward = derive(Path::new("D:/prm/data/"), InstanceScope::Isolated, Some(1));
    assert_eq!(back.suffix, forward.suffix);
}

#[test]
fn normalize_root_table() {
    let cases = [
        ("/a/b/", "/a/b"),
        ("/a/./b", "/a/b"),
        ("/a/b/..", "/a"),
        ("/..", "/"),
        ("../a", "../a"),
        ("a/../..", ".."),
    ];
    for (input, expected) in cases {
        assert_eq!(
            normalize_root(Path::new(input)),
            PathBuf::from(expected),
            "{input}"
        );
    }
}

#[test]
fn is_same_root_compares_normalized_paths() {
    assert!(is_same_root(
        Path::new("/data/prm/"),
        Path::new("/data/./prm")
    ));
    assert!(!is_same_root(
        Path::new("/data/prm"),
        Path::new("/data/prm2")
    ));
}
