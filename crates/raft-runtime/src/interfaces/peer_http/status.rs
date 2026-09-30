use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::application::{MembershipPhase, RaftStatus, Shared};

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
        group_id: s.group_id.0.clone(),
        id: s.id,
        role,
        term: n.current_term(),
        commit_index: n.commit_index(),
        last_index: n.last_index(),
        snapshot_index: n.snapshot_index(),
        applied_index: s.completed_applied_index(),
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

pub(crate) async fn raftz(State(s): State<Arc<Shared>>) -> Json<RaftStatus> {
    Json(host_status(&s).await)
}
