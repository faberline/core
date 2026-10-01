use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};

use crate::install_default_crypto_provider;

#[derive(Debug, Clone)]
pub struct PeerTlsConfig {
    cert: PathBuf,
    key: PathBuf,
    ca: PathBuf,
    required: bool,
}

impl PeerTlsConfig {
    /// A config for the given certificate, key and CA paths. `required`
    /// turns on mutual TLS: the server then demands a client certificate
    /// signed by the CA. The paths are not checked here; loading a rustls
    /// config reports a missing or unreadable file.
    pub fn new(
        cert: impl Into<PathBuf>,
        key: impl Into<PathBuf>,
        ca: impl Into<PathBuf>,
        required: bool,
    ) -> Self {
        Self {
            cert: cert.into(),
            key: key.into(),
            ca: ca.into(),
            required,
        }
    }

    /// The PEM certificate chain path.
    pub fn cert(&self) -> &Path {
        &self.cert
    }

    /// The PEM private key path.
    pub fn key(&self) -> &Path {
        &self.key
    }

    /// The PEM CA bundle path.
    pub fn ca(&self) -> &Path {
        &self.ca
    }

    /// Whether peers must present a client certificate (mutual TLS).
    pub fn required(&self) -> bool {
        self.required
    }

    /// Load from env, deriving `<prefix>_TLS_CERT` / `<prefix>_TLS_KEY` /
    /// `<prefix>_TLS_CA` / `<prefix>_MTLS` from `prefix` (lumen passes
    /// `"LUMEN_PEER"`, reproducing its `LUMEN_PEER_TLS_*`/`LUMEN_PEER_MTLS`
    /// env contract byte-for-byte). Returns `Ok(None)` when no TLS material
    /// is configured (plain-HTTP peer transport).
    pub fn from_env(prefix: &str) -> Result<Option<Self>> {
        let cert_var = format!("{prefix}_TLS_CERT");
        let key_var = format!("{prefix}_TLS_KEY");
        let ca_var = format!("{prefix}_TLS_CA");
        let mtls_var = format!("{prefix}_MTLS");
        let cert = std::env::var(&cert_var).ok().map(PathBuf::from);
        let key = std::env::var(&key_var).ok().map(PathBuf::from);
        let ca = std::env::var(&ca_var).ok().map(PathBuf::from);
        let required = std::env::var(&mtls_var)
            .map(|v| v.eq_ignore_ascii_case("on"))
            .unwrap_or(false);
        match (cert, key, ca) {
            (Some(cert), Some(key), Some(ca)) => {
                let cfg = Self::new(cert, key, ca, required);
                cfg.verify_paths()?;
                Ok(Some(cfg))
            }
            (None, None, None) if !required => Ok(None),
            (None, None, None) => Err(anyhow!("{mtls_var}=on but no cert/key/ca paths set")),
            _ => Err(anyhow!(
                "{cert_var} / {key_var} / {ca_var} must all be set together"
            )),
        }
    }

    fn verify_paths(&self) -> Result<()> {
        for (name, p) in [("cert", &self.cert), ("key", &self.key), ("ca", &self.ca)] {
            if !p.exists() {
                return Err(anyhow!("TLS {name} not found at {}", p.display()));
            }
            std::fs::metadata(p).with_context(|| format!("stat {name} {}", p.display()))?;
        }
        Ok(())
    }

    /// Build a rustls server config for the peer transport.
    pub fn rustls_server_config(&self) -> Result<rustls::ServerConfig> {
        install_default_crypto_provider();
        let cert_chain = load_cert_chain(&self.cert)?;
        let key = load_private_key(&self.key)?;
        let builder = rustls::ServerConfig::builder();
        let server = if self.required {
            let client_roots = load_root_store(&self.ca)?;
            let verifier =
                rustls::server::WebPkiClientVerifier::builder(Arc::new(client_roots)).build()?;
            builder.with_client_cert_verifier(verifier)
        } else {
            builder.with_no_client_auth()
        };
        let mut config = server
            .with_single_cert(cert_chain, key)
            .context("build peer rustls server config")?;
        config.alpn_protocols = vec![b"h2".to_vec()];
        Ok(config)
    }

    /// Build a rustls client config for dialing peer transports.
    pub fn rustls_client_config(&self) -> Result<rustls::ClientConfig> {
        install_default_crypto_provider();
        let roots = load_root_store(&self.ca)?;
        let cert_chain = load_cert_chain(&self.cert)?;
        let key = load_private_key(&self.key)?;
        let mut config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_client_auth_cert(cert_chain, key)
            .context("build peer rustls client config")?;
        config.alpn_protocols = vec![b"h2".to_vec()];
        Ok(config)
    }
}

fn load_cert_chain(path: &Path) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let file =
        std::fs::File::open(path).with_context(|| format!("open cert {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("parse cert {}", path.display()))?;
    if certs.is_empty() {
        return Err(anyhow!("no certificates found at {}", path.display()));
    }
    Ok(certs)
}

fn load_private_key(path: &Path) -> Result<rustls::pki_types::PrivateKeyDer<'static>> {
    let file = std::fs::File::open(path).with_context(|| format!("open key {}", path.display()))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .with_context(|| format!("parse key {}", path.display()))?
        .ok_or_else(|| anyhow!("no private key found at {}", path.display()))
}

fn load_root_store(path: &Path) -> Result<rustls::RootCertStore> {
    let mut roots = rustls::RootCertStore::empty();
    for cert in load_cert_chain(path)? {
        roots
            .add(cert)
            .with_context(|| format!("add CA cert from {}", path.display()))?;
    }
    Ok(roots)
}

#[cfg(test)]
mod tests;
