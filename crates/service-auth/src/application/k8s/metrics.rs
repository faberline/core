use metrics_prometheus::{render, Counter, Sample};

use crate::domain::k8s::CacheOutcome;

/// Counters for the delegated path. Every one of them is derived from a
/// classification, never from a caller-supplied string, so no scrape can be
/// made to carry a credential or an unbounded label set.
#[derive(Debug, Default)]
pub struct DelegatedAuthMetrics {
    pub token_reviews: Counter,
    pub token_cache_hits: Counter,
    pub token_cache_misses: Counter,
    pub token_cache_stale: Counter,
    pub authenticated: Counter,
    pub unauthenticated: Counter,
    pub access_reviews: Counter,
    pub access_cache_hits: Counter,
    pub access_cache_misses: Counter,
    pub access_cache_stale: Counter,
    pub allowed: Counter,
    pub denied: Counter,
    pub review_failures: Counter,
    pub unavailable: Counter,
}

impl DelegatedAuthMetrics {
    pub fn samples(&self) -> Vec<Sample<'static>> {
        vec![
            Sample::new(
                "delegated_auth_token_reviews_total",
                "counter",
                "TokenReview calls made to the apiserver",
                self.token_reviews.get(),
            ),
            Sample::new(
                "delegated_auth_token_cache_hits_total",
                "counter",
                "Authentications served from an unexpired cache entry",
                self.token_cache_hits.get(),
            ),
            Sample::new(
                "delegated_auth_token_cache_misses_total",
                "counter",
                "Authentications that required a TokenReview call",
                self.token_cache_misses.get(),
            ),
            Sample::new(
                "delegated_auth_token_cache_stale_total",
                "counter",
                "Authentications served from an expired cache entry during an apiserver outage",
                self.token_cache_stale.get(),
            ),
            Sample::new(
                "delegated_auth_authenticated_total",
                "counter",
                "Callers accepted as Kubernetes ServiceAccounts",
                self.authenticated.get(),
            ),
            Sample::new(
                "delegated_auth_unauthenticated_total",
                "counter",
                "Credentials rejected before authorization",
                self.unauthenticated.get(),
            ),
            Sample::new(
                "delegated_auth_access_reviews_total",
                "counter",
                "SubjectAccessReview calls made to the apiserver",
                self.access_reviews.get(),
            ),
            Sample::new(
                "delegated_auth_access_cache_hits_total",
                "counter",
                "Authorizations served from an unexpired cache entry",
                self.access_cache_hits.get(),
            ),
            Sample::new(
                "delegated_auth_access_cache_misses_total",
                "counter",
                "Authorizations that required a SubjectAccessReview call",
                self.access_cache_misses.get(),
            ),
            Sample::new(
                "delegated_auth_access_cache_stale_total",
                "counter",
                "Authorizations served from an expired cache entry during an apiserver outage",
                self.access_cache_stale.get(),
            ),
            Sample::new(
                "delegated_auth_allowed_total",
                "counter",
                "Authorization decisions that allowed the operation",
                self.allowed.get(),
            ),
            Sample::new(
                "delegated_auth_denied_total",
                "counter",
                "Authorization decisions that denied the operation",
                self.denied.get(),
            ),
            Sample::new(
                "delegated_auth_review_failures_total",
                "counter",
                "Review calls that returned no usable answer",
                self.review_failures.get(),
            ),
            Sample::new(
                "delegated_auth_unavailable_total",
                "counter",
                "Requests failed closed because no decision could be reached",
                self.unavailable.get(),
            ),
        ]
    }

    /// Prometheus text for this metric set, for a service that exposes the
    /// delegated-auth counters on its own scrape endpoint.
    pub fn render(&self) -> String {
        render(&self.samples())
    }

    pub(super) fn record_token_cache(&self, outcome: CacheOutcome) {
        match outcome {
            CacheOutcome::Hit => self.token_cache_hits.incr(),
            CacheOutcome::Miss => self.token_cache_misses.incr(),
            CacheOutcome::Stale => self.token_cache_stale.incr(),
        }
    }

    pub(super) fn record_access_cache(&self, outcome: CacheOutcome) {
        match outcome {
            CacheOutcome::Hit => self.access_cache_hits.incr(),
            CacheOutcome::Miss => self.access_cache_misses.incr(),
            CacheOutcome::Stale => self.access_cache_stale.incr(),
        }
    }
}
