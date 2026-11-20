use super::*;

mod children;
mod extra_workloads;
mod monitoring;
mod statefulset;

fn cx() -> RenderCtx<'static> {
    RenderCtx::new("svc", "svc-operator", "svc.dev/v1", "Svc", "s", "ns")
}
