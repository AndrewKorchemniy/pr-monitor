use super::*;

#[test]
fn mutex_second_create_reports_exists() {
    let dir = tempfile::tempdir().unwrap();

    let (first, key) = acquire_instance(dir.path(), InstanceScope::Isolated).unwrap();
    let (second, again) = acquire_instance(dir.path(), InstanceScope::Isolated).unwrap();

    assert!(matches!(first, Acquired::Primary(_)));
    assert!(matches!(second, Acquired::Secondary));
    assert_eq!(key, again);
    drop(first);
    let (third, _) = acquire_instance(dir.path(), InstanceScope::Isolated).unwrap();
    assert!(matches!(third, Acquired::Primary(_)), "released on drop");
}

#[test]
fn instance_primary_holds_the_data_folder_lock() {
    let dir = tempfile::tempdir().unwrap();

    let (primary, key) = acquire_instance(dir.path(), InstanceScope::Isolated).unwrap();

    assert!(matches!(primary, Acquired::Primary(_)));
    assert!(matches!(
        lock_file(dir.path(), &key).unwrap(),
        FileLock::Busy
    ));
    drop(primary);
    assert!(matches!(
        lock_file(dir.path(), &key).unwrap(),
        FileLock::Held(_)
    ));
}

/// An elevated primary (or one started under another spelling of the folder) holds the data
/// folder's lock under a mutex we do not see: a second primary on the same files would poll,
/// notify and write `state.json` alongside it.
#[cfg(windows)]
#[test]
fn instance_refuses_a_data_folder_locked_by_another_primary() {
    let dir = tempfile::tempdir().unwrap();
    let key = derive(
        dir.path(),
        InstanceScope::Isolated,
        crate::process::session_id(),
    );
    let FileLock::Held(_held) = lock_file(dir.path(), &key).unwrap() else {
        panic!("the lock file should be free");
    };

    let result = acquire_instance(dir.path(), InstanceScope::Isolated);

    let Err(error) = result else {
        panic!("a locked data folder must not get a second primary");
    };
    assert_eq!(error.kind, PortErrorKind::Denied);
}

/// `--unregister --purge` deletes the data folder while it holds the instance lock.
#[test]
fn instance_data_folder_can_be_deleted_while_locked() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("prm");
    let (primary, _) = acquire_instance(&root, InstanceScope::Isolated).unwrap();

    std::fs::remove_dir_all(&root).unwrap();

    assert!(!root.exists());
    drop(primary);
}
