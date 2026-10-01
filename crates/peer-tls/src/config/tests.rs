use super::*;
use std::io::Write;

const TEST_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIC5zCCAc+gAwIBAgIJAPl6HZTX5LElMA0GCSqGSIb3DQEBCwUAMBUxEzARBgNV
BAMMCmx1bWVuLXBlZXIwHhcNMjYwNjE4MTQwODA4WhcNMjYwNzE4MTQwODA4WjAV
MRMwEQYDVQQDDApsdW1lbi1wZWVyMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIB
CgKCAQEAuwEAFs8xYsY9GDDbefwoV4FMiD9f49hs7iLVijVhUM7z5W0Xj9mXFCFS
Sn/DNb/bF9UtUoGJ0cdpjlevd6BjaXbjm2gMIDod1yKBZ2BXwT/elwRzjEIcTgR5
+GTu355VsWqugBYob8cYn2kGAMvVFUZeRBbC1IO02xbp9zABNaBHOWVdRTXODxiU
jbtB4gioNJOG1A71sto61lMmLMp4IL02k+BbuwekhCkkRGGNuqMHVAehkJwTmmxF
aPHK3LMifWgUXn51JWEhU2OiWe3Ja8/XQU5LZDvbc3vmMaJSuIMheOIkM5AXHyo4
LX62YgtuUpouYYOHkOqNRWRQfLrvywIDAQABozowODAVBgNVHREEDjAMggpsdW1l
bi1wZWVyMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgKkMA0GCSqGSIb3
DQEBCwUAA4IBAQAavsvsmN/zKL0TVx7FLEnDRbD6L4KNg3ndPrZDKncl0Df1W5kl
4jZTujiZ2CqH7CQakra3kV51EIUuKSbc0kQBsvsCIw0Fxb/JUmsui/z9uCqrqhrT
ODlcV6pETppce5JozMAZCKUyx9460/+flP7VTqHnLt1oMrM/mmaKeZ0ImSBnx8xF
0JpJN0HyX+vlbrT/9J3xxe53v7glRPZIgBlOT1eTaroXjIk6ZzOBS8bCBpNYVec5
wN93qI3ZQWwNUMB3TXJ7IBpgIrtD+z/ZhliDnk6NOLqKPXJrch0cVwlljT0Uu+DP
Qd9/aITxkqX7P0phj2cYmALL/aBJJaWRuAfw
-----END CERTIFICATE-----
"#;

const TEST_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC7AQAWzzFixj0Y
MNt5/ChXgUyIP1/j2GzuItWKNWFQzvPlbReP2ZcUIVJKf8M1v9sX1S1SgYnRx2mO
V693oGNpduObaAwgOh3XIoFnYFfBP96XBHOMQhxOBHn4ZO7fnlWxaq6AFihvxxif
aQYAy9UVRl5EFsLUg7TbFun3MAE1oEc5ZV1FNc4PGJSNu0HiCKg0k4bUDvWy2jrW
UyYsynggvTaT4Fu7B6SEKSREYY26owdUB6GQnBOabEVo8crcsyJ9aBRefnUlYSFT
Y6JZ7clrz9dBTktkO9tze+YxolK4gyF44iQzkBcfKjgtfrZiC25Smi5hg4eQ6o1F
ZFB8uu/LAgMBAAECggEAF23pp/HvmxOBRg2hAeiQ2V3Oy+c8yVwtUay1mmpTtf8n
2Z/Qaup1HkWKfOEDATH3bkX8NrEaJllYpUfhKRjEO8t0et0PX95ILNMa6WvNst2g
ssURAQqrZy7yZSeoMgYxcFgQYuXjzRVhxV8wLFtdaBv35YoAgQW7XBPD3n96N1CV
nRpk/tPeIaqmGnC6xhtrd9zaRy1qZ3aX5Np27ZrwsMghmJyLNI6OfsS0FRK3dKQx
dkx5L5iMD0wmqC5FsR4nc+pkFsSgdv2uxtS95JDX2jOHuJj4qm5moh0Z6eXQ8lCD
Nhr+JN1TQXHVAL696tQPnQNJtdnQYshNpsC+R2Sg+QKBgQDjsyACww2OLLIO/QBJ
rzbuAgx0n4cRR7mSCZhrgO3xX+sKU1yGPNRtwj0R/dQsVClld0KB2Oa9brR2dzcE
QWSLGcRhAmpjgmYLFn6T2Odbyb5YfTMVF6ka53w4CELKQPm7cm5QsXimTqSYPwZb
Jth2e7bkEdVemzDS8C33WYio3QKBgQDSPweWZNLC+EtHlNHg4fguww6wjkUPcoxG
C8prGovcSEMuIXUnrJmRkWdKTxHud9ofvhfauB86Daf4tkaklGPuJ5CepxnMXyos
I8fSEnIyTPD6sYC37GNUMDhMU3iyxV+CsH077TwSGpjw4cntf8pqYklP8zjctjnq
wAPG6O2cxwKBgQCTEQnW3tatgo7LAXwjG2k2FtqmpLbfYV0pRstMfCyzHwm3VJpJ
FZb7AV7idPiKXR2TrJCnP0nhBlTGwz8kn3vqIA1nvuCqPvnbpX7BzXG5Jjer/cl1
kR+nAeaIZkWFTqw99q3rroTHnbnPn71iOFfNRyCcdCxE+6VwSLLXtNuAfQKBgQDA
05QW6FOxA96vQRuY4EcqRDXV0jYeq9VhbPDyeD9sAk6zIXZ8s72JF82fBpQQnZXN
ZSAltpbVPK8g2bRCv+JDC8CE8gckPOfF4e8jiU15Or4NfvzqMwEKtsr7ndbmR0WI
7Gt/qd5dUE2TJ9J2Y6z3Ezvf+tfc/bhyyDbumLVNAwKBgHj74ZKxCKE21mv1azYk
EF1sOEisJVtdtSq2PZN7hiGgvaMTSfKegRgM+12lGDvabf93LSoYX1pEHY7qIs2f
pky/zqjmfLFtvyP+vQvAL3F+5B/1XpFj2dRnAOJaWpq62Ebe2L9k4ff7EYNTL7oq
LkjT2UdpFBDZGWHwqDRhXX8k
-----END PRIVATE KEY-----
"#;

// The prefix a hypothetical caller (not lumen) would use — proves
// `from_env` is genuinely parameterized, not hardcoded to lumen's names.
const TEST_PREFIX: &str = "TEST_PEER";

// env vars are process-global, so the three scenarios share a mutex to
// keep them from racing under `cargo test`'s default parallel runner.
use std::sync::Mutex;
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn clear_env() {
    unsafe {
        std::env::remove_var("TEST_PEER_TLS_CERT");
        std::env::remove_var("TEST_PEER_TLS_KEY");
        std::env::remove_var("TEST_PEER_TLS_CA");
        std::env::remove_var("TEST_PEER_MTLS");
    }
}

fn write_tls_fixture(name: &str) -> PeerTlsConfig {
    let dir = std::env::temp_dir().join(format!("peer-tls-rustls-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("cert.pem"), TEST_CERT).unwrap();
    std::fs::write(dir.join("key.pem"), TEST_KEY).unwrap();
    std::fs::write(dir.join("ca.pem"), TEST_CERT).unwrap();
    PeerTlsConfig::new(
        dir.join("cert.pem"),
        dir.join("key.pem"),
        dir.join("ca.pem"),
        true,
    )
}

#[test]
fn from_env_returns_none_when_nothing_set() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_env();
    let cfg = PeerTlsConfig::from_env(TEST_PREFIX).unwrap();
    assert!(cfg.is_none());
}

#[test]
fn from_env_loads_when_all_set() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_env();
    let dir = std::env::temp_dir().join(format!("peer-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["cert.pem", "key.pem", "ca.pem"] {
        let mut f = std::fs::File::create(dir.join(name)).unwrap();
        f.write_all(b"DUMMY").unwrap();
    }
    unsafe {
        std::env::set_var("TEST_PEER_TLS_CERT", dir.join("cert.pem"));
        std::env::set_var("TEST_PEER_TLS_KEY", dir.join("key.pem"));
        std::env::set_var("TEST_PEER_TLS_CA", dir.join("ca.pem"));
        std::env::set_var("TEST_PEER_MTLS", "on");
    }
    let cfg = PeerTlsConfig::from_env(TEST_PREFIX).unwrap().expect("Some");
    assert!(cfg.required());
    assert_eq!(cfg.cert(), dir.join("cert.pem"));
    assert_eq!(cfg.key(), dir.join("key.pem"));
    assert_eq!(cfg.ca(), dir.join("ca.pem"));
    std::fs::remove_dir_all(&dir).ok();
    clear_env();
}

#[test]
fn from_env_errors_on_partial_config() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_env();
    unsafe {
        std::env::set_var("TEST_PEER_TLS_CERT", "/tmp/dummy-cert");
    }
    let err = PeerTlsConfig::from_env(TEST_PREFIX).unwrap_err();
    assert!(err.to_string().contains("must all be set together"));
    clear_env();
}

#[test]
fn builds_rustls_peer_configs_from_pem_material() {
    let cfg = write_tls_fixture("builder");
    cfg.rustls_server_config()
        .expect("server config should build");
    cfg.rustls_client_config()
        .expect("client config should build");
    std::fs::remove_dir_all(cfg.cert().parent().unwrap()).ok();
}
