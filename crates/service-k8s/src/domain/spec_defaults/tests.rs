use super::*;

#[test]
fn defaults_equal_the_values_the_operators_copy() {
    assert_eq!(DEFAULT_STORAGE_SIZE, "10Gi");
    assert_eq!(DEFAULT_GRACE_SECS, 10);
    assert_eq!(DEFAULT_BODY_LIMIT_BYTES, 8_388_608);
    assert_eq!(DEFAULT_AUTH_MODE, "off");
}
