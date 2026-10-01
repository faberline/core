use super::*;

mod children;
mod statefulset;

fn cx() -> RenderCtx<'static> {
    RenderCtx::new("svc", "svc-operator", "svc.dev/v1", "Svc", "s", "ns")
}
