use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::material::{MaterialPem, Rejection, RejectionReason};

/// Where a reloadable runtime reads its material from.
///
/// A trait rather than a path pair because the interesting deployments differ:
/// a mounted Secret is three files, an operator-driven runtime may hold the PEM
/// in memory, and a test wants neither. Everything downstream sees bytes.
pub trait MaterialSource: Send + Sync + 'static {
    fn load(&self) -> Result<MaterialPem, Rejection>;
}

/// The mounted-Secret case: `tls.crt`, `tls.key`, `ca.crt`.
#[derive(Debug, Clone)]
pub struct FileMaterialSource {
    pub cert: PathBuf,
    pub key: PathBuf,
    pub trust_bundle: PathBuf,
}

impl FileMaterialSource {
    pub fn new(
        cert: impl Into<PathBuf>,
        key: impl Into<PathBuf>,
        trust_bundle: impl Into<PathBuf>,
    ) -> Self {
        Self {
            cert: cert.into(),
            key: key.into(),
            trust_bundle: trust_bundle.into(),
        }
    }

    /// The three files, in the order the reloader reads them. Callers watching
    /// for changes should watch exactly these.
    pub fn paths(&self) -> [&Path; 3] {
        [&self.cert, &self.key, &self.trust_bundle]
    }
}

impl MaterialSource for FileMaterialSource {
    fn load(&self) -> Result<MaterialPem, Rejection> {
        // The path is not in the error. A projection error reaches request logs
        // and status conditions, and a private-key path is a hint nobody outside
        // the process needs (R6). The `what` is enough to act on.
        let read = |path: &Path, what: &str| {
            std::fs::read(path).map_err(|err| {
                Rejection::new(
                    RejectionReason::Unreadable,
                    format!("{what} could not be read: {}", err.kind()),
                )
            })
        };
        Ok(MaterialPem {
            cert_chain: read(&self.cert, "certificate")?,
            key: read(&self.key, "private key")?,
            trust_bundle: read(&self.trust_bundle, "trust bundle")?,
        })
    }
}

/// An in-memory source, for callers driven by a watch rather than a mount.
pub struct MemoryMaterialSource(RwLock<Option<MaterialPem>>);

impl MemoryMaterialSource {
    pub fn new(pem: MaterialPem) -> Self {
        Self(RwLock::new(Some(pem)))
    }

    pub fn empty() -> Self {
        Self(RwLock::new(None))
    }

    /// Replace what the next reload will see.
    pub fn set(&self, pem: MaterialPem) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = Some(pem);
    }

    /// Make the next reload fail the way a missing mount does.
    pub fn clear(&self) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

impl MaterialSource for MemoryMaterialSource {
    fn load(&self) -> Result<MaterialPem, Rejection> {
        self.0
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| Rejection::new(RejectionReason::Unreadable, "no material available"))
    }
}
