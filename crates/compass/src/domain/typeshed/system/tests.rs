use super::*;

#[test]
fn test_os_stub() {
    let os = create_os_stub();
    assert!(os.exports.contains_key("getcwd"));
    assert!(os.exports.contains_key("environ"));
    assert!(os.exports.contains_key("sep"));
}

#[test]
fn test_sys_stub() {
    let sys = create_sys_stub();
    assert!(sys.exports.contains_key("argv"));
    assert!(sys.exports.contains_key("path"));
    assert!(sys.exports.contains_key("exit"));
}
