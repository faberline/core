//! The leader wait counts poll steps, not wall time (#554).

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tokio::time::Instant;

use crate::support::cluster::{
    poll_with_step_budget, LeaderWaitBudget, LEADER_WAIT_HARD_CAP_FACTOR,
};

fn timing(budget_ms: u64) -> LeaderWaitBudget {
    LeaderWaitBudget {
        max_election_ticks: 0,
        wait_budget: Duration::from_millis(budget_ms),
        poll_interval: Duration::from_millis(10),
    }
}

#[tokio::test]
async fn slow_polls_do_not_use_up_the_budget() {
    let calls = AtomicU32::new(0);
    let result = poll_with_step_budget(timing(200), || async {
        let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
        tokio::time::sleep(Duration::from_millis(30)).await;
        if n == 15 {
            Some(7u32)
        } else {
            None
        }
    })
    .await;

    assert_eq!(result, Ok(7));
}

#[tokio::test]
async fn no_leader_returns_err_after_the_budget_in_poll_steps() {
    let calls = AtomicU32::new(0);
    let result = poll_with_step_budget(timing(200), || async {
        calls.fetch_add(1, Ordering::SeqCst);
        None::<u32>
    })
    .await;

    assert!(result.is_err(), "a wait with no leader must time out");
    assert_eq!(calls.load(Ordering::SeqCst), 21);
    let elapsed = result.unwrap_err();
    assert!(
        elapsed >= Duration::from_millis(200),
        "elapsed {elapsed:?} is below the budget"
    );
}

#[tokio::test]
async fn the_hard_cap_ends_a_wait_that_polls_too_slowly() {
    let calls = AtomicU32::new(0);
    let result = poll_with_step_budget(timing(100), || async {
        calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(150)).await;
        None::<u32>
    })
    .await;

    let elapsed = result.expect_err("the hard cap must end the wait");
    assert!(
        elapsed >= Duration::from_millis(100) * LEADER_WAIT_HARD_CAP_FACTOR,
        "elapsed {elapsed:?} is below the hard cap"
    );
    assert!(
        calls.load(Ordering::SeqCst) < 11,
        "the wait polled {} times, so the cap did not end it",
        calls.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn the_first_poll_runs_before_any_sleep() {
    let started = Instant::now();
    let result = poll_with_step_budget(timing(200), || async { Some(1u32) }).await;

    assert_eq!(result, Ok(1));
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "the first poll waited before it ran"
    );
}
