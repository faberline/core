use super::*;

#[test]
fn publish_path_is_the_canonical_peer_write_route() {
    assert_eq!(RaftHost::PUBLISH_PATH, "/raft/publish");
}
