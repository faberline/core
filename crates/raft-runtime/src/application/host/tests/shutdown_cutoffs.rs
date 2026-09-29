use super::*;
use crate::application::host::shutdown::{shutdown_phase_cutoff_elapsed, shutdown_phase_cutoffs};

#[test]
fn shutdown_phase_cutoffs_are_cumulative_and_ordered() {
    let started_at = tokio::time::Instant::now();
    let deadline = ShutdownDeadline {
        expires_at: started_at + Duration::from_millis(100),
        total: Duration::from_millis(100),
        reserve: Duration::ZERO,
    };

    let cutoffs = shutdown_phase_cutoffs(deadline, started_at);

    assert_eq!(cutoffs[0], started_at + Duration::from_millis(25));
    assert_eq!(cutoffs[1], started_at + Duration::from_millis(50));
    assert_eq!(cutoffs[2], started_at + Duration::from_millis(75));
    assert_eq!(cutoffs[3], deadline.expires_at);
    assert!(cutoffs.windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
fn shutdown_phase_cutoffs_handle_zero_and_very_short_usable_intervals() {
    let started_at = tokio::time::Instant::now();
    let zero = ShutdownDeadline {
        expires_at: started_at,
        total: Duration::ZERO,
        reserve: Duration::ZERO,
    };
    assert_eq!(shutdown_phase_cutoffs(zero, started_at), [started_at; 4]);

    let short = ShutdownDeadline {
        expires_at: started_at + Duration::from_nanos(3),
        total: Duration::from_nanos(3),
        reserve: Duration::ZERO,
    };
    assert_eq!(
        shutdown_phase_cutoffs(short, started_at),
        [
            started_at,
            started_at,
            started_at,
            started_at + Duration::from_nanos(3),
        ]
    );
}

#[test]
fn shutdown_phase_cutoffs_keep_remainder_for_final_cutoff_and_preserve_reserve() {
    let started_at = tokio::time::Instant::now();
    let deadline = ShutdownDeadline {
        expires_at: started_at + Duration::from_nanos(11),
        total: Duration::from_nanos(11),
        reserve: Duration::from_nanos(2),
    };

    let cutoffs = shutdown_phase_cutoffs(deadline, started_at);
    let usable_end = deadline.expires_at - deadline.reserve;

    assert_eq!(cutoffs[0], started_at + Duration::from_nanos(2));
    assert_eq!(cutoffs[1], started_at + Duration::from_nanos(4));
    assert_eq!(cutoffs[2], started_at + Duration::from_nanos(6));
    assert_eq!(cutoffs[3], usable_end);
    assert_eq!(
        deadline.expires_at.duration_since(cutoffs[3]),
        deadline.reserve
    );
}

#[test]
fn expired_background_cutoff_is_detected_before_absent_tasks_are_skipped() {
    let cutoff = tokio::time::Instant::now() - Duration::from_millis(1);

    assert!(shutdown_phase_cutoff_elapsed(cutoff));
}
