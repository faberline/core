# peer-tls

peer-tls owns TLS material for a service's ports: loading a certificate, key
and CA bundle, deciding whether a candidate is fit to serve, building the
rustls server and client configurations, and replacing that material while the
process keeps serving. It has no listener; a listener asks it for the
configuration to use at each handshake. In core, raft-runtime uses it for its
peer transport, and server-http uses it only in tests. Downstream, lumen,
relay, sift, tape, defer and courier use it.

**Form:** whole-src infrastructure · **Depends on:** — · **Crate:** [`crates/peer-tls`](../../crates/peer-tls)

## Model

- **Peer TLS config** — `PeerTlsConfig`: certificate, key and CA paths plus
  whether client certificates are required, read by `from_env(prefix)` from
  `<prefix>_TLS_CERT`, `<prefix>_TLS_KEY`, `<prefix>_TLS_CA` and
  `<prefix>_MTLS`, or built with `new(cert, key, ca, required)`, which does
  not check the paths. It builds the rustls configs with ALPN `h2`.
- **Material** — `MaterialPem`: the three PEM bodies. Its `Debug` prints sizes
  only.
- **Identity expectation** — `IdentityExpectation` (`serving()`, `peer()`): what
  the leaf must prove for a given role.
- **Validated material** — `ValidatedMaterial`: material that passed every
  check, carrying the certificate fingerprint, the lowercase hex sha256 of the
  leaf.
- **Rejection** — `Rejection` with a `RejectionReason` (`Unreadable`,
  `MalformedPem`, `EmptyTrustBundle`, `KeyMismatch`, `NotYetValid`, `Expired`,
  `MissingUsage`, `Untrusted`, `WrongIdentity`), each with a stable spelling.
- **Runtime profile** — `TlsRuntimeProfile`: `serving` offers `h2` and
  `http/1.1` without client certificates; `peer` is mutual TLS with `h2` only.
- **Reloadable TLS** — `ReloadableTls`: the active activation, its TLS reload
  generation, and the trust anchors retiring from the previous one.
- **Reload status** — `TlsReloadStatus`: generation (`0` means nothing ever
  activated), fingerprint, time to expiry, counters and the last refusal.

## Ports

- `MaterialSource` — where a reloadable runtime reads its PEM bytes.
  `FileMaterialSource` reads a mounted Secret's three files;
  `MemoryMaterialSource` holds material set by a caller or a test.

## Invariants

- `PeerTlsConfig::from_env` returns a config only when all three paths are set
  and exist. With none set it returns `None`, or an error when `<prefix>_MTLS`
  requires TLS. A partial set is an error.
- The server config verifies client certificates only when `required` is set.
- `validate` checks structure, then that the key matches the leaf, then the
  validity window, then chain trust through rustls, and checks every DNS and
  SPIFFE URI SAN against the expectation.
- A rejection's detail and the reload status never carry PEM bodies or file
  paths.
- Reload is build-then-swap: a candidate is validated and its configs built
  before the write lock is taken, so any failure leaves the previous generation
  serving.
- The TLS reload generation increases by one on each activation of a new
  certificate. Reloading material with the same fingerprint returns the current
  generation and changes nothing.
- `retire_previous_trust` acts only when given the active generation.
- `server_config` and `client_config` return `None` when nothing is active or
  the active leaf has expired, so the caller refuses the connection.
- `spawn_material_watcher` polls the source (every 30 s by default; a zero
  interval becomes 1 s) and never stops on a rejected candidate.

## Published language

The whole public API; whole-src infrastructure contexts have no application
layer. The root re-exports the `material` and `reload` items; both modules
stay public.

## Exceptions and debts

- **Checker exceptions:** None. Reading files and the clock is the job
  of an infrastructure crate.
- **Debts:** `anyhow` in `from_env` and the rustls config builders (these
  are not ports, so ADR D4 does not cover them). P2 made the
  `PeerTlsConfig` fields private (D2).
