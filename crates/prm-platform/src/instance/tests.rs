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
