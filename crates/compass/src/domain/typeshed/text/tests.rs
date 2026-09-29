use super::*;

#[test]
fn test_re_stub() {
    let re = create_re_stub();
    assert!(re.exports.contains_key("compile"));
    assert!(re.exports.contains_key("match"));
    assert!(re.exports.contains_key("IGNORECASE"));
}

#[test]
fn test_json_stub() {
    let json = create_json_stub();
    assert!(json.exports.contains_key("dumps"));
    assert!(json.exports.contains_key("loads"));
}
