//! Pure ordered-concurrency planner for a Fleet rollout.

/// Rollout state of one Fleet entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RolloutState {
    /// The entry is up to date.
    Ready,
    /// The entry is being updated and uses one slot.
    Updating,
    /// The entry waits for a slot.
    Pending,
}

/// Return the indexes of the pending entries that may start now, in ascending order.
///
/// `max_concurrent` limits the entries in flight. Each `Updating` entry uses one slot.
/// `max_concurrent == 0` returns an empty vector.
pub fn plan_rollout_order(states: &[RolloutState], max_concurrent: usize) -> Vec<usize> {
    let updating = states
        .iter()
        .filter(|s| **s == RolloutState::Updating)
        .count();
    let mut slots = max_concurrent.saturating_sub(updating);
    let mut started = Vec::new();
    for (index, state) in states.iter().enumerate() {
        if slots == 0 {
            break;
        }
        if *state == RolloutState::Pending {
            started.push(index);
            slots -= 1;
        }
    }
    started
}
