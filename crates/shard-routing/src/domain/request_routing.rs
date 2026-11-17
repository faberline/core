//! The hook an app implements to say where a request goes and how scattered answers merge.

/// Where one request is served.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteDecision {
    /// This shard serves the request itself.
    Local,
    /// The named shard owns the request; forward it there.
    Owner(u32),
    /// Every shard takes part; the answers are merged.
    Scatter,
}

/// App hook for routing. The crate knows no app type: the app picks `Request` and `Response`.
pub trait RequestRouting {
    /// The app's request type.
    type Request;
    /// The app's answer type. One answer comes from one shard; `merge` makes one from many.
    type Response;

    /// Decide where `request` is served.
    fn classify(&self, request: &Self::Request) -> RouteDecision;

    /// Merge the answers of the shards that took part, in shard order.
    fn merge(&self, parts: Vec<Self::Response>) -> Self::Response;
}
