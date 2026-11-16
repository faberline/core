//! Prints the objects that this build of service-k8s renders for the upgrade-rollback
//! harness (#372). ci/upgrade-rollback-old-render.sh copies this file into a worktree of
//! the old release tag and runs it there, so it may only use API that old tags have.
use service_k8s::render::{client_service, service_account, RenderCtx};

fn main() {
    let cx = RenderCtx::new(
        "probe",
        "upgrade-rollback-probe",
        "service-k8s.e2e/v1",
        "RollbackProbe",
        "probe",
        "upgrade-rollback",
    );
    let objects = vec![
        service_account(&cx, "serving"),
        client_service(&cx, "probe-client", "serving", 8080),
    ];
    println!(
        "{}",
        serde_json::to_string_pretty(&objects).expect("objects serialize")
    );
}
