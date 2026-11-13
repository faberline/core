use super::*;

#[test]
fn publish_path_is_the_canonical_peer_write_route() {
    assert_eq!(RaftHost::PUBLISH_PATH, "/raft/publish");
}

#[test]
fn read_index_path_is_the_canonical_peer_route() {
    assert_eq!(RaftHost::READ_INDEX_PATH, "/raft/read-index");
}
