use super::*;

#[test]
fn test_socket_path_generation() {
    let root1 = PathBuf::from("/home/user/project1");
    let root2 = PathBuf::from("/home/user/project2");

    let path1 = DaemonConfig::default_socket_path(&root1);
    let path2 = DaemonConfig::default_socket_path(&root2);

    // Different roots should produce different socket paths
    assert_ne!(path1, path2);

    // Same root should produce same path
    let path1_again = DaemonConfig::default_socket_path(&root1);
    assert_eq!(path1, path1_again);
}

#[test]
fn test_daemon_config() {
    let root = PathBuf::from("/test/project");
    let config = DaemonConfig::new(root.clone());

    assert_eq!(config.root, root);
    assert!(config.watch);
    assert_eq!(config.debounce, Duration::from_millis(300));
}
