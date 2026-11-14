use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use raft_core::NodeId;

use crate::consensus::Cluster;

// Swaps the first two commands. It is a deliberately broken apply, used only
// by the negative test below.
fn broken_apply(list: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut out = list.to_vec();
    if out.len() >= 2 {
        out.swap(0, 1);
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn replicas_apply_the_same_commands_in_the_same_order(
        cmds in prop::collection::vec(prop::collection::vec(any::<u8>(), 1..8), 1..12)
    ) {
        let mut c = Cluster::new(3);
        c.run_until_leader();
        for cmd in &cmds {
            c.propose(cmd.clone());
        }
        for id in (0..3).map(NodeId::new) {
            prop_assert_eq!(&c.applied[&id], &cmds);
        }
    }
}

#[test]
fn seeded_bug_is_found() {
    let mut runner = TestRunner::default();
    let result = runner.run(
        &prop::collection::vec(prop::collection::vec(any::<u8>(), 1..8), 2..6),
        |cmds| {
            prop_assert_eq!(broken_apply(&cmds), cmds);
            Ok(())
        },
    );
    assert!(result.is_err());
}
