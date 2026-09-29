use super::*;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipPhase {
    Stable,
    Joint,
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
}
