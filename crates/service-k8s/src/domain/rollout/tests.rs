use std::collections::BTreeMap;

use super::*;

#[test]
fn selection_starts_at_n_and_lowers_one_member_at_a_time() {
    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true), (2, true)]));

    // Initial state: N=3, partition=3, all members on rev1.
    let mut obs = ObservedRollout::new(3, 1, 1, "rev1", "rev2", 3)
        .with_member(0, "rev1", true)
        .with_member(1, "rev1", true)
        .with_member(2, "rev1", true);

    // Step 1: Release member 2 by setting partition to 2.
    let step1 = plan_rollout(&obs, &gate);
    assert_eq!(
        step1,
        RolloutStep::SetPartition {
            partition: 2,
            member: 2
        }
    );

    // Member 2 released, but still on rev1.
    obs.partition = 2;
    obs.members.insert(2, ObservedMember::new("rev1", false));
    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MemberNotUpdated
        }
    );

    // Member 2 finishes updating, becomes ready and healthy.
    obs.members.insert(2, ObservedMember::new("rev2", true));

    // Step 2: Release member 1 by setting partition to 1.
    let step2 = plan_rollout(&obs, &gate);
    assert_eq!(
        step2,
        RolloutStep::SetPartition {
            partition: 1,
            member: 1
        }
    );

    // Member 1 released and finishes updating.
    obs.partition = 1;
    obs.members.insert(1, ObservedMember::new("rev2", true));

    // Step 3: Release member 0 by setting partition to 0.
    let step3 = plan_rollout(&obs, &gate);
    assert_eq!(
        step3,
        RolloutStep::SetPartition {
            partition: 0,
            member: 0
        }
    );

    // Member 0 released, updating.
    obs.partition = 0;
    obs.members.insert(0, ObservedMember::new("rev1", false));
    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MemberNotUpdated
        }
    );

    // Member 0 finishes updating, ready, healthy.
    obs.members.insert(0, ObservedMember::new("rev2", true));
    assert_eq!(plan_rollout(&obs, &gate), RolloutStep::Complete);
}

#[test]
fn generation_mismatch_holds() {
    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true)]));
    let obs = ObservedRollout::new(2, 2, 1, "rev1", "rev2", 2)
        .with_member(0, "rev1", true)
        .with_member(1, "rev1", true);

    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::GenerationMismatch
        }
    );
}

#[test]
fn member_not_updated_holds() {
    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true)]));
    let obs = ObservedRollout::new(2, 1, 1, "rev1", "rev2", 1)
        .with_member(0, "rev1", true)
        .with_member(1, "rev1", true);

    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MemberNotUpdated
        }
    );
}

#[test]
fn member_not_ready_holds() {
    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true)]));
    let obs = ObservedRollout::new(2, 1, 1, "rev1", "rev2", 1)
        .with_member(0, "rev1", true)
        .with_member(1, "rev2", false);

    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MemberNotReady
        }
    );
}

#[test]
fn second_member_not_released_while_the_first_is_unhealthy() {
    // Negative control: first member released (partition = 2 for N=3, member 2)
    // is on update_revision and ready, but app_healthy reports unhealthy.
    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true), (2, false)]));
    let obs = ObservedRollout::new(3, 1, 1, "rev1", "rev2", 2)
        .with_member(0, "rev1", true)
        .with_member(1, "rev1", true)
        .with_member(2, "rev2", true);

    let step = plan_rollout(&obs, &gate);
    assert_eq!(
        step,
        RolloutStep::Hold {
            reason: HoldReason::MemberUnhealthy
        }
    );
}

#[test]
fn missing_app_healthy_ordinal_counts_as_unhealthy() {
    // Member 2 is missing from app_healthy map.
    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true)]));
    let obs = ObservedRollout::new(3, 1, 1, "rev1", "rev2", 2)
        .with_member(0, "rev1", true)
        .with_member(1, "rev1", true)
        .with_member(2, "rev2", true);

    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MemberUnhealthy
        }
    );
}

#[test]
fn paused_always_yields_hold_paused_and_never_lowers_partition() {
    let gate = RolloutGate::paused();
    let obs = ObservedRollout::new(3, 1, 1, "rev1", "rev2", 3)
        .with_member(0, "rev1", true)
        .with_member(1, "rev1", true)
        .with_member(2, "rev1", true);

    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::Paused
        }
    );

    let obs_mid = ObservedRollout::new(3, 1, 1, "rev1", "rev2", 1)
        .with_member(0, "rev1", true)
        .with_member(1, "rev2", true)
        .with_member(2, "rev2", true);

    assert_eq!(
        plan_rollout(&obs_mid, &gate),
        RolloutStep::Hold {
            reason: HoldReason::Paused
        }
    );
}

#[test]
fn resume_continues_from_observed_partition_not_from_n() {
    // When paused = false, planner resumes from observed partition (1),
    // releasing member 0 next, rather than resetting to N=3.
    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true), (2, true)]));
    let obs = ObservedRollout::new(3, 1, 1, "rev1", "rev2", 1)
        .with_member(0, "rev1", true)
        .with_member(1, "rev2", true)
        .with_member(2, "rev2", true);

    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::SetPartition {
            partition: 0,
            member: 0
        }
    );
}

#[test]
fn interrupted_reconcile_recovery_holds_on_unupdated_or_unhealthy_member() {
    // Leader failover / restarted planner with only observed state (no in-memory history).
    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, false), (2, true)]));
    let obs = ObservedRollout::new(3, 1, 1, "rev1", "rev2", 1)
        .with_member(0, "rev1", true)
        .with_member(1, "rev2", true)
        .with_member(2, "rev2", true);

    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MemberUnhealthy
        }
    );
}

#[test]
fn multiple_members_in_flight_holds() {
    // Observed partition is 1, but member 2 is still on rev1 while member 1 is released on rev1.
    // Both 1 and 2 are in flight.
    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true), (2, true)]));
    let obs = ObservedRollout::new(3, 1, 1, "rev1", "rev2", 1)
        .with_member(0, "rev1", true)
        .with_member(1, "rev1", false)
        .with_member(2, "rev1", true);

    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MultipleMembersInFlight
        }
    );
}

#[test]
fn complete_only_when_partition_zero_and_all_members_updated_ready_and_healthy() {
    let mut gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true)]));
    let mut obs = ObservedRollout::new(2, 1, 1, "rev1", "rev2", 0)
        .with_member(0, "rev2", true)
        .with_member(1, "rev2", true);

    // All complete
    assert_eq!(plan_rollout(&obs, &gate), RolloutStep::Complete);

    // Not complete if one member is not healthy
    gate.app_healthy.insert(0, false);
    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MemberUnhealthy
        }
    );

    // Not complete if one member is not ready
    gate.app_healthy.insert(0, true);
    obs.members.insert(0, ObservedMember::new("rev2", false));
    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MemberNotReady
        }
    );

    // Not complete if one member is on old revision
    obs.members.insert(0, ObservedMember::new("rev1", true));
    assert_eq!(
        plan_rollout(&obs, &gate),
        RolloutStep::Hold {
            reason: HoldReason::MemberNotUpdated
        }
    );
}

#[test]
fn rollout_gate_ignores_disruption_budget_policy() {
    // No disruption budget on input types.
    let obs = ObservedRollout::new(3, 1, 1, "rev1", "rev2", 3)
        .with_member(0, "rev1", true)
        .with_member(1, "rev1", true)
        .with_member(2, "rev1", true);

    let gate = RolloutGate::unpaused(BTreeMap::from([(0, true), (1, true), (2, true)]));

    let step_without_pdb = plan_rollout(&obs, &gate);

    // Create a PDB-bearing WorkloadPlan alongside.
    let cx = crate::api::render::RenderCtx::new(
        "demo",
        "demo-operator",
        "demo.axiom.dev/v1",
        "Demo",
        "sample",
        "observability",
    );
    let mut workload_plan = crate::api::render::WorkloadPlan::new(&cx);
    let selector = BTreeMap::from([("demo.axiom.dev/role".to_string(), "query".to_string())]);
    workload_plan.add_pod_disruption_budget(
        crate::api::render::PodDisruptionBudgetPlan::min_available(
            "sample-pdb",
            "query",
            selector,
            1,
        ),
    );

    // Calling the planner with the same facts alongside the PDB-bearing plan produces the identical result.
    let step_with_pdb = plan_rollout(&obs, &gate);

    assert_eq!(step_without_pdb, step_with_pdb);
    assert_eq!(
        step_with_pdb,
        RolloutStep::SetPartition {
            partition: 2,
            member: 2
        }
    );
}
