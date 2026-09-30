use super::*;

#[test]
fn now_rfc3339_has_no_subsecond_component() {
    let now = now_rfc3339();
    assert!(now.ends_with('Z'), "{now} is not UTC-suffixed");
    assert!(!now.contains('.'), "{now} carries sub-second precision");
}
