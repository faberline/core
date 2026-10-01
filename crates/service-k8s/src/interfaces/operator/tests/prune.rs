use super::*;
use crate::interfaces::operator::children::prune_object;

/// The happy path: our own child gets deleted, so flipping a toggle off
/// actually stops the enforcement it turned on (#2603).
#[tokio::test]
async fn prune_deletes_a_child_this_cr_controls() {
    let (client, seen) = fake_apiserver(vec![
        (200, live_policy("uid-1234", true)),
        (200, live_policy("uid-1234", true)),
    ]);
    prune_object(&client, "acme", "uid-1234", &np_target())
        .await
        .expect("prune succeeds");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "expected a GET then a DELETE: {seen:?}");
    assert!(seen[1].starts_with("DELETE"), "{seen:?}");
}

/// The guard: a name is not proof of authorship. Another controller, a Helm
/// chart, or a human can own an object at exactly this CR's name, and
/// deleting it would be this operator destroying something it never made.
#[tokio::test]
async fn prune_leaves_an_object_this_cr_does_not_own() {
    let (client, seen) = fake_apiserver(vec![(200, live_policy("uid-somebody-else", true))]);
    prune_object(&client, "acme", "uid-1234", &np_target())
        .await
        .expect("a foreign object is a no-op, not an error");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "must stop after the GET: {seen:?}");
    assert!(seen[0].starts_with("GET"), "{seen:?}");
}

/// A plain (non-controller) owner reference is a weaker link than the one
/// the apiserver writes for a controlled child — Kubernetes' own garbage
/// collector distinguishes them, and so must this.
#[tokio::test]
async fn prune_leaves_a_non_controller_owner_reference_alone() {
    let (client, seen) = fake_apiserver(vec![(200, live_policy("uid-1234", false))]);
    prune_object(&client, "acme", "uid-1234", &np_target())
        .await
        .expect("no-op");
    assert_eq!(seen.lock().unwrap().len(), 1);
}

/// Prune runs on every requeue, so the steady state after it converges is
/// "object already gone" — that has to be a cheap no-op rather than an
/// error that fails the reconcile forever.
#[tokio::test]
async fn prune_of_an_absent_object_is_a_silent_no_op() {
    let (client, seen) = fake_apiserver(vec![not_found()]);
    prune_object(&client, "acme", "uid-1234", &np_target())
        .await
        .expect("absent is success");
    assert_eq!(seen.lock().unwrap().len(), 1);
}

/// Losing the delete race against the CR's own garbage collection must not
/// fail the reconcile: both wanted the object gone and it is gone.
#[tokio::test]
async fn prune_treats_a_404_on_delete_as_success() {
    let (client, _) = fake_apiserver(vec![(200, live_policy("uid-1234", true)), not_found()]);
    prune_object(&client, "acme", "uid-1234", &np_target())
        .await
        .expect("racing GC is success");
}
