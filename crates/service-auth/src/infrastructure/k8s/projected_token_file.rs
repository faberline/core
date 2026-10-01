use std::fmt;
use std::path::{Path, PathBuf};

use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;

use crate::domain::k8s::ProjectedToken;

/// Why a projected token cannot be presented. Every variant names the file and
/// the audience expected; no variant carries the material or any part of it.
#[derive(Debug)]
pub enum ProjectedTokenError {
    /// The file is absent or unreadable — almost always a volume that was not
    /// mounted, or a path that disagrees with the rendered manifest.
    Unreadable {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The file exists and holds nothing. A projection mid-write looks like
    /// this, and so does a mount of the wrong key.
    Empty { path: PathBuf },
    /// Not a JWT this client can inspect. Reported without the content: a
    /// malformed credential is still a credential.
    Malformed { path: PathBuf },
    /// A valid token minted for someone else — the default pod token is the
    /// usual culprit. Reported without the audiences actually found, which
    /// are claims of an unverified token and would be attacker-chosen text in
    /// a log line.
    WrongAudience { path: PathBuf, expected: String },
    /// Past its expiry. Names the file, because the fix is at the kubelet or
    /// the projection, not at the callee.
    Expired { path: PathBuf },
}

impl fmt::Display for ProjectedTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, source } => write!(
                f,
                "cannot read the projected ServiceAccount token at {}: {source} — is the \
                 projected volume mounted, and does its mountPath match this path?",
                path.display()
            ),
            Self::Empty { path } => write!(
                f,
                "the projected ServiceAccount token at {} is empty",
                path.display()
            ),
            Self::Malformed { path } => write!(
                f,
                "the file at {} is not a readable ServiceAccount token; its contents are \
                 withheld because they may still be a credential",
                path.display()
            ),
            Self::WrongAudience { path, expected } => write!(
                f,
                "the ServiceAccount token at {} was not issued for `{expected}` — this is what \
                 mounting the pod's default token instead of a projected one looks like; add a \
                 `serviceAccountToken` projection with that audience",
                path.display()
            ),
            Self::Expired { path } => write!(
                f,
                "the ServiceAccount token at {} has expired; the kubelet refreshes a projected \
                 token in place, so a persistently expired one means the projection is not \
                 rotating",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ProjectedTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Claims this client is willing to look at. Everything else — issuer, subject,
/// the `kubernetes.io` block — is the callee's business, verified there
/// against a signature this side does not hold.
#[derive(Deserialize)]
struct InspectedClaims {
    #[allow(dead_code)]
    exp: i64,
}

/// A projected ServiceAccount token file, and the audience it must carry.
///
/// Holds no token: every [`Self::read`] goes to the file. See the module note
/// on why caching one here is a bug rather than an optimisation.
pub struct ProjectedTokenFile {
    path: PathBuf,
    audience: String,
}

impl ProjectedTokenFile {
    pub fn new(path: impl Into<PathBuf>, audience: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            audience: audience.into(),
        }
    }

    /// The file this reads, for diagnostics that want to name it.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The audience every token from this file must carry.
    pub fn audience(&self) -> &str {
        &self.audience
    }

    /// The token as it stands right now, or the reason it cannot be presented.
    ///
    /// Call this per request. It is a `read` on tmpfs, and it is the only
    /// thing that makes rotation work.
    pub fn read(&self) -> Result<ProjectedToken, ProjectedTokenError> {
        let raw = std::fs::read_to_string(&self.path).map_err(|source| {
            ProjectedTokenError::Unreadable {
                path: self.path.clone(),
                source,
            }
        })?;
        // Trailing newline: the kubelet does not add one, but a developer
        // writing the file by hand for a local run always does, and a bearer
        // header with a newline in it is a protocol error rather than a 401.
        let token = raw.trim();
        if token.is_empty() {
            return Err(ProjectedTokenError::Empty {
                path: self.path.clone(),
            });
        }
        self.inspect(token)?;
        Ok(ProjectedToken(token.to_string()))
    }

    /// Audience and expiry, without the signature. The key belongs to the
    /// cluster's token issuer and the audience is the callee — this side is
    /// neither, so signature validation here would be theatre. Refusing a
    /// token whose *own claims* already disqualify it is not.
    fn inspect(&self, token: &str) -> Result<(), ProjectedTokenError> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.insecure_disable_signature_validation();
        validation.set_audience(&[&self.audience]);
        validation.validate_exp = true;
        // The default 60-second grace, kept deliberately. This check exists to
        // catch a projection that stopped rotating, not to second-guess a
        // clock: the callee holds the authoritative one, and a token a second
        // past expiry by *this* pod's reckoning is one it should still send
        // and let the cluster judge.
        validation.leeway = 60;
        validation.required_spec_claims = ["exp", "aud"].into_iter().map(String::from).collect();

        match jsonwebtoken::decode::<InspectedClaims>(
            token,
            // Unused: signature validation is off above. `jsonwebtoken` still
            // requires a key argument.
            &DecodingKey::from_secret(&[]),
            &validation,
        ) {
            Ok(_) => Ok(()),
            Err(err) => Err(match err.kind() {
                jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                    ProjectedTokenError::WrongAudience {
                        path: self.path.clone(),
                        expected: self.audience.clone(),
                    }
                }
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => ProjectedTokenError::Expired {
                    path: self.path.clone(),
                },
                // Missing `aud` reads as a missing required claim, not as a
                // wrong audience — but for this client they are the same
                // mistake, and the actionable message is the audience one.
                jsonwebtoken::errors::ErrorKind::MissingRequiredClaim(claim) if claim == "aud" => {
                    ProjectedTokenError::WrongAudience {
                        path: self.path.clone(),
                        expected: self.audience.clone(),
                    }
                }
                _ => ProjectedTokenError::Malformed {
                    path: self.path.clone(),
                },
            }),
        }
    }
}

#[cfg(test)]
mod tests;
