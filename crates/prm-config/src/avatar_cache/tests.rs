use prm_domain::{Vsid, testing::org};

use super::*;

fn key(n: u128) -> AvatarKey {
    AvatarKey::new(org("contoso"), Vsid::from_u128(n))
}

fn at(secs: i64) -> Timestamp {
    Timestamp::from_unix(secs).unwrap()
}

fn cache() -> (tempfile::TempDir, FileAvatarCache) {
    let dir = tempfile::tempdir().unwrap();
    let cache = FileAvatarCache::new(dir.path().join("avatars"));
    (dir, cache)
}

#[test]
fn avatar_cache_round_trips_bytes_and_stored_time() {
    let (_dir, cache) = cache();
    assert_eq!(cache.read(&key(1)), None);
    let avatar = CachedAvatar {
        png: vec![1, 2, 3],
        stored_at: at(1_700_000_000),
    };
    cache.write(&key(1), &avatar).unwrap();
    assert_eq!(cache.read(&key(1)), Some(avatar));
    assert_eq!(cache.read(&key(2)), None);
}

#[test]
fn avatar_cache_keeps_an_empty_marker() {
    let (_dir, cache) = cache();
    let marker = CachedAvatar {
        png: Vec::new(),
        stored_at: at(1_700_000_000),
    };
    cache.write(&key(1), &marker).unwrap();
    assert_eq!(cache.read(&key(1)), Some(marker));
}

#[test]
fn avatar_cache_overwrite_replaces_time_too() {
    let (_dir, cache) = cache();
    for (png, secs) in [(vec![1], 1_700_000_000), (vec![2], 1_700_999_000)] {
        let avatar = CachedAvatar {
            png,
            stored_at: at(secs),
        };
        cache.write(&key(1), &avatar).unwrap();
        assert_eq!(cache.read(&key(1)), Some(avatar));
    }
}

#[test]
fn avatar_cache_prune_drops_only_older_entries() {
    let (_dir, cache) = cache();
    for (n, secs) in [(1, 1_000), (2, 5_000), (3, 9_000)] {
        let avatar = CachedAvatar {
            png: vec![0],
            stored_at: at(secs),
        };
        cache.write(&key(n), &avatar).unwrap();
    }
    cache.prune(at(5_000));
    assert_eq!(cache.read(&key(1)), None);
    assert!(cache.read(&key(2)).is_some());
    assert!(cache.read(&key(3)).is_some());
}

#[test]
fn avatar_cache_prune_of_a_missing_folder_is_quiet() {
    let (_dir, cache) = cache();
    cache.prune(at(1));
}
