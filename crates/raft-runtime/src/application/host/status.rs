use super::*;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipPhase {
    Stable,
    Joint,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PeerMatch {
    pub id: NodeId,
    pub match_index: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RaftStatus {
    pub group_id: String,
    pub id: NodeId,
    pub role: String,
    pub term: u64,
    pub commit_index: u64,
    pub last_index: u64,
    pub snapshot_index: u64,
    pub applied_index: u64,
    pub leader: Option<NodeId>,
    pub is_leader: bool,
    pub durability_error: Option<String>,
    pub committed_voters: Vec<NodeId>,
    pub incoming_voters: Option<Vec<NodeId>>,
    pub learners: Vec<NodeId>,
    pub membership_phase: MembershipPhase,
    pub undeliverable_never_addressed: u64,
    pub undeliverable_withdrawn_address: u64,
    /// Proposals refused by `propose_outcome` before route selection.
    pub proposal_rejected_before_routing: u64,
    /// Proposals refused by the leader-side check before log append.
    pub proposal_rejected_before_append: u64,
    pub proposal_admission_closed: bool,
    pub lifecycle_generation: u64,
    #[serde(default)]
    pub snapshot_capability: Option<String>,
    #[serde(default)]
    pub resident_log_bytes: u64,
    #[serde(default)]
    pub max_resident_log_bytes: u64,
    #[serde(default)]
    pub peer_match_index: Vec<PeerMatch>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raft_status_deserializes_without_peer_match_index() {
        let json = concat!(
            r#"{"group_id":"orders","id":1,"role":"Leader","term":3,"commit_index":9,"#,
            r#""last_index":10,"snapshot_index":4,"applied_index":8,"leader":1,"is_leader":true,"#,
            r#""durability_error":null,"committed_voters":[1,2,3],"incoming_voters":[1,2],"#,
            r#""learners":[4],"membership_phase":"Joint","undeliverable_never_addressed":5,"#,
            r#""undeliverable_withdrawn_address":6,"proposal_rejected_before_routing":7,"#,
            r#""proposal_rejected_before_append":8,"proposal_admission_closed":false,"#,
            r#""lifecycle_generation":2,"snapshot_capability":"cap-v1","resident_log_bytes":64,"#,
            r#""max_resident_log_bytes":1024}"#
        );
        let s: RaftStatus =
            serde_json::from_str(json).expect("deserialization without peer_match_index succeeds");
        assert!(s.peer_match_index.is_empty());
    }
}
