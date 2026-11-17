use super::*;
use http::header::HeaderName;

fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    map
}

fn map() -> VirtualBucketShardMap {
    VirtualBucketShardMap::balanced(5, 4096, 3).unwrap()
}

#[test]
fn the_header_names_are_the_neutral_and_legacy_names() {
    assert_eq!(FORWARDED_HEADER, "x-faberline-forwarded");
    assert_eq!(MAP_VERSION_HEADER, "x-faberline-map-version");
    assert_eq!(LEGACY_FORWARDED_HEADER, "x-lumen-forwarded");
    assert_eq!(LEGACY_MAP_VERSION_HEADER, "x-lumen-map-version");
}

#[test]
fn a_stale_map_version_is_refused() {
    let stale = headers(&[(MAP_VERSION_HEADER, "4")]);
    let err = check_forwarded_map_version(&stale, &map()).unwrap_err();
    assert_eq!(
        err.downcast_ref::<ShardMapVersionMismatch>(),
        Some(&ShardMapVersionMismatch {
            local: 5,
            forwarded: 4
        })
    );
    assert_eq!(
        err.to_string(),
        "shard map version mismatch: local 5, forwarded 4"
    );
    let current = headers(&[(MAP_VERSION_HEADER, "5")]);
    assert!(check_forwarded_map_version(&current, &map()).is_ok());
}

#[test]
fn a_forwarded_request_for_a_bucket_the_shard_does_not_own_is_refused() {
    let h = headers(&[(FORWARDED_HEADER, "1")]);
    let err = assert_owns(&h, &map(), 1, "catalog", "tenant-a").unwrap_err();
    assert_eq!(
        err.downcast_ref::<ShardForwardMisrouted>(),
        Some(&ShardForwardMisrouted {
            bucket: 2226,
            local_shard: 1,
            owner_shard: 0
        })
    );
    assert_eq!(
        err.to_string(),
        "forwarded request for bucket 2226 belongs to shard 0, not shard 1"
    );
    assert!(assert_owns(&h, &map(), 0, "catalog", "tenant-a").is_ok());
    assert!(assert_owns(&h, &map(), 1, "catalog", "doc-1").is_ok());
}

#[test]
fn a_request_that_was_not_forwarded_is_not_refused_by_the_owner_guard() {
    assert!(assert_owns(&HeaderMap::new(), &map(), 1, "catalog", "tenant-a").is_ok());
    let zero = headers(&[(FORWARDED_HEADER, "0")]);
    assert!(assert_owns(&zero, &map(), 1, "catalog", "tenant-a").is_ok());
}

#[test]
fn both_header_names_are_accepted_for_the_marker_and_the_version() {
    assert!(is_forwarded(&headers(&[(FORWARDED_HEADER, "1")])));
    assert!(is_forwarded(&headers(&[(LEGACY_FORWARDED_HEADER, "1")])));
    assert!(!is_forwarded(&HeaderMap::new()));

    assert_eq!(
        forwarded_map_version(&headers(&[(MAP_VERSION_HEADER, "7")])).unwrap(),
        Some(7)
    );
    assert_eq!(
        forwarded_map_version(&headers(&[(LEGACY_MAP_VERSION_HEADER, "7")])).unwrap(),
        Some(7)
    );
    let both = headers(&[(MAP_VERSION_HEADER, "8"), (LEGACY_MAP_VERSION_HEADER, "7")]);
    assert_eq!(forwarded_map_version(&both).unwrap(), Some(8));

    let legacy_stale = headers(&[
        (LEGACY_FORWARDED_HEADER, "1"),
        (LEGACY_MAP_VERSION_HEADER, "4"),
    ]);
    let err = check_forwarded_map_version(&legacy_stale, &map()).unwrap_err();
    assert!(err.downcast_ref::<ShardMapVersionMismatch>().is_some());

    let legacy_marked = headers(&[(LEGACY_FORWARDED_HEADER, "1")]);
    let err = assert_owns(&legacy_marked, &map(), 1, "catalog", "tenant-a").unwrap_err();
    assert!(err.downcast_ref::<ShardForwardMisrouted>().is_some());
}

#[test]
fn a_request_without_a_map_version_header_passes() {
    assert_eq!(forwarded_map_version(&HeaderMap::new()).unwrap(), None);
    assert!(check_forwarded_map_version(&HeaderMap::new(), &map()).is_ok());
    let marked = headers(&[(FORWARDED_HEADER, "1")]);
    assert!(check_forwarded_map_version(&marked, &map()).is_ok());
}

#[test]
fn a_map_version_that_is_not_a_number_is_an_error() {
    let bad = headers(&[(MAP_VERSION_HEADER, "abc")]);
    let err = forwarded_map_version(&bad).unwrap_err();
    assert_eq!(
        err.to_string(),
        "header x-faberline-map-version is not a map version: abc"
    );
    assert!(check_forwarded_map_version(&bad, &map()).is_err());

    let negative = headers(&[(MAP_VERSION_HEADER, "-1")]);
    assert!(forwarded_map_version(&negative).is_err());
}

#[test]
fn the_sender_writes_only_the_neutral_names() {
    let h = forward_headers(9);
    assert_eq!(h.len(), 2);
    assert_eq!(h.get(FORWARDED_HEADER).unwrap(), "1");
    assert_eq!(h.get(MAP_VERSION_HEADER).unwrap(), "9");
    assert!(h.get(LEGACY_FORWARDED_HEADER).is_none());
    assert!(h.get(LEGACY_MAP_VERSION_HEADER).is_none());
    assert!(is_forwarded(&h));
    assert_eq!(forwarded_map_version(&h).unwrap(), Some(9));
}

#[test]
fn the_forward_errors_have_exact_messages_and_are_std_errors() {
    assert_eq!(
        ShardForwardUnavailable {
            shard: 2,
            detail: "timeout".to_string()
        }
        .to_string(),
        "shard 2 is unavailable: timeout"
    );
    assert_eq!(
        ShardForwardRemoteError {
            shard: 2,
            status: 503,
            body: "busy".to_string()
        }
        .to_string(),
        "shard 2 answered 503: busy"
    );
    let err = anyhow::Error::new(ShardForwardUnavailable {
        shard: 2,
        detail: "x".to_string(),
    });
    assert!(err.downcast_ref::<ShardForwardUnavailable>().is_some());
}
