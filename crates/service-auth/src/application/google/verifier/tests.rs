use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{encode, EncodingKey, Header};

use super::*;
use crate::gcp::*;
use crate::{Registry, Role, TokenClaims};

/// A throwaway RSA key generated for this test suite alone. It signs
/// nothing real; the matching JWKS below is what makes offline
/// verification assertable without a live Google token.
const SIGNING_KEY: &str = include_str!("../../../../fixtures/gcp/throwaway-signing-key.pem");
const JWKS: &str = include_str!("../../../../fixtures/gcp/throwaway-jwks.json");
const KID: &str = "test-key-1";
const AUDIENCE: &str = "lumen";
const IDENTITY: &str = "lumen-dev@axiom-502607.iam.gserviceaccount.com";

// -- fakes ------------------------------------------------------------

struct FakeClock(AtomicU64);
impl FakeClock {
    fn new(at: u64) -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(at)))
    }
    fn advance(&self, secs: u64) {
        self.0.fetch_add(secs, Ordering::SeqCst);
    }
}
impl Clock for FakeClock {
    fn now_unix(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct CountingJwks {
    calls: AtomicUsize,
    body: Option<String>,
}
impl CountingJwks {
    fn serving() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            body: Some(JWKS.to_string()),
        })
    }
    fn unavailable() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            body: None,
        })
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
#[async_trait]
impl JwksSource for CountingJwks {
    async fn fetch(&self) -> Result<JwkSet, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.body {
            Some(body) => serde_json::from_str(body).map_err(|e| e.to_string()),
            None => Err("connection refused".to_string()),
        }
    }
}

enum IntrospectionBehaviour {
    Live { email: String, expires_in: u64 },
    Invalid,
    Unreachable,
}
struct CountingIntrospection {
    calls: AtomicUsize,
    behaviour: IntrospectionBehaviour,
}
impl CountingIntrospection {
    fn live(email: &str, expires_in: u64) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            behaviour: IntrospectionBehaviour::Live {
                email: email.to_string(),
                expires_in,
            },
        })
    }
    fn invalid() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            behaviour: IntrospectionBehaviour::Invalid,
        })
    }
    fn unreachable() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            behaviour: IntrospectionBehaviour::Unreachable,
        })
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
#[async_trait]
impl AccessTokenIntrospection for CountingIntrospection {
    async fn introspect(&self, _token: &str) -> Result<Option<IntrospectedToken>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.behaviour {
            IntrospectionBehaviour::Live { email, expires_in } => Ok(Some(IntrospectedToken {
                email: Some(email.clone()),
                email_verified: true,
                expires_in: *expires_in,
            })),
            IntrospectionBehaviour::Invalid => Ok(None),
            IntrospectionBehaviour::Unreachable => Err("connection refused".to_string()),
        }
    }
}

// -- helpers ----------------------------------------------------------

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn mint_with_kid(kid: &str, claims: serde_json::Value) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_string());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).unwrap(),
    )
    .unwrap()
}

fn id_token() -> String {
    mint_with_kid(
        KID,
        serde_json::json!({
            "iss": "https://accounts.google.com",
            "aud": AUDIENCE,
            "sub": "1234567890",
            "email": IDENTITY,
            "email_verified": true,
            "exp": now_unix() + 3600,
        }),
    )
}

fn dev_claims() -> TokenClaims {
    TokenClaims {
        subject: "dev:lumen-dev".to_string(),
        roles: HashMap::from([("products".to_string(), Role::Read)]),
    }
}

/// The registry keyed by IAM identity instead of by secret. The key is a
/// public email, so it can live in git, in a CR, in a code review — none
/// of which is true of a bearer secret. It lives in the `identities`
/// namespace, which is the only one a verified Google email may reach.
fn registry() -> Arc<ReloadableRoleMapVerifier> {
    Arc::new(ReloadableRoleMapVerifier::with_registry(
        true,
        Registry {
            tokens: HashMap::new(),
            identities: HashMap::from([(IDENTITY.to_string(), dev_claims())]),
        },
    ))
}

fn verifier_with(
    jwks: Arc<dyn JwksSource>,
    introspection: Option<Arc<dyn AccessTokenIntrospection>>,
    clock: Arc<dyn Clock>,
) -> GoogleVerifier {
    GoogleVerifier::with_sources(
        true,
        registry(),
        GoogleAuthConfig::new([AUDIENCE]),
        jwks,
        introspection,
        clock,
    )
    .expect("one audience is configured")
}

fn offline_verifier() -> GoogleVerifier {
    verifier_with(CountingJwks::serving(), None, FakeClock::new(1_700_000_000))
}

mod credentials;
mod jwks_and_introspection;
mod offline;
