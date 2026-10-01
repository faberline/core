use std::sync::Mutex;

use super::*;

/// A Lease that only records which election it was asked to keep current.
#[derive(Default)]
struct RecordingLease {
    started: Mutex<Vec<Arc<Election>>>,
}

impl LeaderLease for RecordingLease {
    fn start(&self, election: Arc<Election>) {
        self.started.lock().unwrap().push(election);
    }
}

#[test]
fn a_campaign_starts_the_lease_on_its_own_election_and_waits_for_it() {
    let lease = RecordingLease::default();
    let leadership = Leadership::campaign("replica-0".to_string(), &lease);

    let started = lease.started.lock().unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(leadership.identity(), "replica-0");
    // Nobody leads until the Lease says so.
    assert!(!leadership.is_leader());
    assert_eq!(leadership.follower_requeue(), Some(FOLLOWER_REQUEUE));

    // The Lease flips the same election the gate reads.
    started[0].is_leader.store(true, Ordering::Relaxed);
    assert!(leadership.is_leader());
    assert_eq!(leadership.follower_requeue(), None);
}

#[test]
fn a_held_follower_is_never_promoted() {
    let election = Election::new("one-shot".to_string());
    let leadership = Leadership::held(election.clone());

    assert_eq!(leadership.follower_requeue(), Some(FOLLOWER_REQUEUE));
    assert!(!election.is_leader.load(Ordering::Relaxed));
}

#[test]
fn a_held_leader_may_act() {
    let election = Election::new("one-shot".to_string());
    election.is_leader.store(true, Ordering::Relaxed);

    assert_eq!(Leadership::held(election).follower_requeue(), None);
}
