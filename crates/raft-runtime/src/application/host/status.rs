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

pub(crate) async fn host_status(s: &Shared) -> RaftStatus {
    let n = s.node.lock().await;
    let durability_error = s
        .latched_failure
        .lock()
        .unwrap()
        .as_ref()
        .map(|e| e.to_string());
    let conf = n.conf_state();
    let (committed_voters, incoming_voters, membership_phase) = match &conf.outgoing {
        Some(outgoing) => (
            outgoing.clone(),
            Some(conf.membership.voters().to_vec()),
            MembershipPhase::Joint,
        ),
        None => (
            conf.membership.voters().to_vec(),
            None,
            MembershipPhase::Stable,
        ),
    };
    let learners = conf.membership.learners().to_vec();
    let role = if !n.is_voter() {
        "Learner".to_string()
    } else {
        format!("{:?}", n.role())
    };
    RaftStatus {
        group_id: s.group_id.as_str().to_owned(),
        id: s.id,
        role,
        term: n.current_term().get(),
        commit_index: n.commit_index().get(),
        last_index: n.last_index().get(),
        snapshot_index: n.snapshot_index().get(),
        applied_index: s.completed_applied_index().get(),
        leader: n.leader(),
        is_leader: n.is_leader(),
        durability_error,
        committed_voters,
        incoming_voters,
        learners,
        membership_phase,
        undeliverable_never_addressed: s.undeliverable_never_addressed.load(Ordering::Relaxed),
        undeliverable_withdrawn_address: s.undeliverable_withdrawn_address(),
        proposal_rejected_before_routing: s
            .proposal_rejected_before_routing
            .load(Ordering::Relaxed),
        proposal_rejected_before_append: s.proposal_rejected_before_append.load(Ordering::Relaxed),
        proposal_admission_closed: s.lifecycle_generation.load(Ordering::Acquire) > 0,
        lifecycle_generation: s.lifecycle_generation.load(Ordering::Acquire),
        snapshot_capability: s.sm.snapshot_capability().map(str::to_string),
        resident_log_bytes: n.resident_log_bytes() as u64,
        max_resident_log_bytes: s.max_resident_log_bytes as u64,
    }
}
