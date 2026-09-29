use super::*;

#[test]
fn test_datetime_stub() {
    let dt = create_datetime_stub();
    assert!(dt.exports.contains_key("datetime"));
    assert!(dt.exports.contains_key("date"));
    assert!(dt.exports.contains_key("timedelta"));
}
