use std::sync::Arc;
use std::time::Duration;

use super::delegated_error::DelegatedAuthError;
use super::metrics::DelegatedAuthMetrics;
use crate::domain::k8s::{
    digest, AuthRejection, CacheOutcome, Clock, DelegatedAuthConfig, PrincipalRejection,
    ResourceAttributes, ReviewBackend, ReviewError, ServiceAccountPrincipal, TokenDigest,
    TokenReviewOutcome, TtlCache,
};

/// The cache key for one authorization decision.
///
/// It carries the *whole* reviewed identity, not just the username: RBAC binds
/// by group and policy may read `extra`, so two callers with the same username
/// and different groups are genuinely different questions. Keying on the
/// username alone would let one caller's allow answer another caller's request.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DecisionKey {
    identity: super::principal::ReviewedIdentity,
    attributes: ResourceAttributes,
}

/// Authenticates bearer tokens through `TokenReview` and authorizes operations
/// through `SubjectAccessReview`.
pub struct DelegatedAuthenticator {
    backend: Arc<dyn ReviewBackend>,
    config: DelegatedAuthConfig,
    /// Cached authentications. The error arm is cached too, so a stream of bad
    /// tokens cannot be used to generate apiserver load.
    tokens: TtlCache<TokenDigest, Result<ServiceAccountPrincipal, AuthRejection>>,
    decisions: TtlCache<DecisionKey, bool>,
    metrics: Arc<DelegatedAuthMetrics>,
}

impl DelegatedAuthenticator {
    /// The authenticator on an injectable clock, so a caller can prove its own
    /// revocation bound without waiting for one. `new` is the same on the
    /// system clock.
    pub fn with_clock(
        backend: Arc<dyn ReviewBackend>,
        config: DelegatedAuthConfig,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let policy = config.cache;
        Self {
            backend,
            config,
            tokens: TtlCache::new(policy, clock.clone()),
            decisions: TtlCache::new(policy, clock),
            metrics: Arc::new(DelegatedAuthMetrics::default()),
        }
    }

    pub fn metrics(&self) -> &Arc<DelegatedAuthMetrics> {
        &self.metrics
    }

    pub fn config(&self) -> &DelegatedAuthConfig {
        &self.config
    }

    /// Drop every cached answer. For a service that learns out-of-band that
    /// its policy changed; correctness never depends on it being called.
    pub fn invalidate(&self) {
        self.tokens.clear();
        self.decisions.clear();
    }

    /// Resolve a bearer token to the ServiceAccount that presented it.
    pub async fn authenticate(
        &self,
        token: &str,
    ) -> Result<ServiceAccountPrincipal, DelegatedAuthError> {
        if token.is_empty() {
            self.metrics.unauthenticated.incr();
            return Err(DelegatedAuthError::Unauthenticated(
                AuthRejection::MissingCredential,
            ));
        }
        let key = digest(token);

        if let Some(cached) = self.tokens.get(&key) {
            self.metrics.record_token_cache(CacheOutcome::Hit);
            return self.finish_authentication(cached);
        }
        self.metrics.record_token_cache(CacheOutcome::Miss);

        self.metrics.token_reviews.incr();
        let outcome = match self
            .backend
            .review_token(token, self.config.audiences())
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                self.metrics.review_failures.incr();
                // The only fallback is an answer the apiserver already gave,
                // and only inside the stale window.
                if let Some(cached) = self.tokens.get_stale(&key) {
                    self.metrics.record_token_cache(CacheOutcome::Stale);
                    return self.finish_authentication(cached);
                }
                self.metrics.unavailable.incr();
                return Err(DelegatedAuthError::Unavailable(error));
            }
        };

        let resolved = self.judge(outcome);
        let ttl = match &resolved {
            Ok(_) => self.config.cache.allow_ttl(),
            Err(_) => self.config.cache.deny_ttl(),
        };
        self.tokens.insert(key, resolved.clone(), ttl);
        self.finish_authentication(resolved)
    }

    /// Ask whether this identity may perform this operation.
    pub async fn authorize(
        &self,
        principal: &ServiceAccountPrincipal,
        attributes: &ResourceAttributes,
    ) -> Result<(), DelegatedAuthError> {
        let key = DecisionKey {
            identity: principal.identity.clone(),
            attributes: attributes.clone(),
        };

        if let Some(allowed) = self.decisions.get(&key) {
            self.metrics.record_access_cache(CacheOutcome::Hit);
            return self.finish_authorization(allowed, attributes);
        }
        self.metrics.record_access_cache(CacheOutcome::Miss);

        self.metrics.access_reviews.incr();
        let reviewed = self
            .backend
            .review_access(&principal.identity, attributes)
            .await
            .and_then(|outcome| match outcome.evaluation_error {
                // A partial authorizer failure is not a deny and is certainly
                // not an allow — it is an unanswered question.
                Some(detail) => Err(ReviewError::Malformed(format!(
                    "authorizer reported an evaluation error: {detail}"
                ))),
                None => Ok(outcome),
            });

        let outcome = match reviewed {
            Ok(outcome) => outcome,
            Err(error) => {
                self.metrics.review_failures.incr();
                if let Some(allowed) = self.decisions.get_stale(&key) {
                    self.metrics.record_access_cache(CacheOutcome::Stale);
                    return self.finish_authorization(allowed, attributes);
                }
                self.metrics.unavailable.incr();
                return Err(DelegatedAuthError::Unavailable(error));
            }
        };

        let allowed = outcome.is_allowed();
        let ttl = if allowed {
            self.config.cache.allow_ttl()
        } else {
            self.config.cache.deny_ttl()
        };
        self.decisions.insert(key, allowed, ttl);
        self.finish_authorization(allowed, attributes)
    }

    /// The worst-case delay between a revocation in Kubernetes and this
    /// process refusing the caller.
    pub fn revocation_bound(&self) -> Duration {
        self.config.cache.revocation_bound()
    }

    /// Turn one `TokenReview` response into a caller or a rejection.
    ///
    /// The order is deliberate: the authentication flag, then the audience,
    /// then the identity shape. Reading the identity of a token that was not
    /// minted for this service would be treating an unrelated credential as an
    /// attempt to log in here.
    fn judge(&self, outcome: TokenReviewOutcome) -> Result<ServiceAccountPrincipal, AuthRejection> {
        if !outcome.is_authenticated() {
            return Err(AuthRejection::Principal(
                PrincipalRejection::NotAuthenticated,
            ));
        }
        let audience_accepted = self.config.kubernetes_default
            || outcome
                .audiences()
                .iter()
                .any(|granted| self.config.audiences.iter().any(|want| want == granted));
        if !audience_accepted {
            return Err(AuthRejection::AudienceMismatch);
        }
        ServiceAccountPrincipal::from_review(true, outcome.identity().clone())
            .map_err(AuthRejection::Principal)
    }

    fn finish_authentication(
        &self,
        resolved: Result<ServiceAccountPrincipal, AuthRejection>,
    ) -> Result<ServiceAccountPrincipal, DelegatedAuthError> {
        match resolved {
            Ok(principal) => {
                self.metrics.authenticated.incr();
                Ok(principal)
            }
            Err(rejection) => {
                self.metrics.unauthenticated.incr();
                Err(DelegatedAuthError::Unauthenticated(rejection))
            }
        }
    }

    fn finish_authorization(
        &self,
        allowed: bool,
        attributes: &ResourceAttributes,
    ) -> Result<(), DelegatedAuthError> {
        if allowed {
            self.metrics.allowed.incr();
            Ok(())
        } else {
            self.metrics.denied.incr();
            Err(DelegatedAuthError::Denied(attributes.clone()))
        }
    }
}

#[cfg(test)]
mod tests;
