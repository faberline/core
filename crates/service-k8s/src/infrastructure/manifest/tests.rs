use super::*;

mod children;
mod statefulset;

fn cx() -> RenderCtx<'static> {
    RenderCtx {
        app: "svc",
        manager: "svc-operator",
        api_version: "svc.dev/v1",
        kind: "Svc",
        name: "s",
        ns: "ns",
        owner: None,
    }
}
