# Supply-chain advisories of the workspace

Measured with `cargo deny check advisories` (cargo-deny 0.20.2, RustSec database of 2026-11-14) on `Cargo.lock` of main. Issue #383. No `deny.toml` change here.

## Advisories

| Advisory | Crate | Locked | Enters through | Fixed in | Recommendation |
|---|---|---|---|---|---|
| RUSTSEC-2026-0386 | tokio | 1.52.3 | workspace dependency (`Cargo.toml` `tokio`), most crates | 1.53.3 (or 1.51.6) | Upgrade: group A |
| RUSTSEC-2026-0190 | anyhow | 1.0.102 | workspace dependency, `cli-std` and others | 1.0.103 | Upgrade: group A |
| RUSTSEC-2026-0258 | h2 | 0.4.14 | hyper, axum, tonic, reqwest | 0.4.16 | Upgrade: group A |
| RUSTSEC-2026-0186 | memmap2 | 0.9.10 | `raft-runtime`, `storage-durable` | 0.9.11 | Upgrade: group A |
| RUSTSEC-2026-0204 | crossbeam-epoch | 0.9.18 | `compass` via `jwalk` and `rayon` | 0.9.20 | Upgrade: group A |
| RUSTSEC-2026-0285 | rustls | 0.23.40 | `peer-tls`, `raft-runtime`, `server-http`, hyper-rustls (reqwest, kube) | 0.23.45 | Upgrade: group A |
| RUSTSEC-2026-0346 | rustls | 0.23.40 | same as above | 0.23.46 | Upgrade: group A |
| RUSTSEC-2026-0348 | rustls | 0.23.40 | same as above | 0.23.46 | Upgrade: group A |
| RUSTSEC-2026-0349 | rustls | 0.23.40 | same as above | 0.23.46 | Upgrade: group A |
| RUSTSEC-2026-0382 | rustls-webpki | 0.103.13 | rustls | 0.103.16 | Upgrade: group A |
| RUSTSEC-2026-0381 | webpki-roots | 1.0.7 | hyper-rustls (reqwest, kube-client) | 1.0.10 | Upgrade: group A |
| RUSTSEC-2026-0457 | rmcp | 3.1.4 | `service-mcp` (direct, `rmcp = "3.1.4"`) | 3.7.0 | Upgrade: group A |
| RUSTSEC-2026-0461 | rmcp | 3.1.4 | `service-mcp` | 3.6.0 | Upgrade: group A |
| RUSTSEC-2026-0462 | rmcp | 3.1.4 | `service-mcp` | 3.9.0 | Upgrade: group A |
| RUSTSEC-2025-0134 | rustls-pemfile | 2.2.0 | `peer-tls` (direct), kube-client | none (unmaintained) | Replace in `peer-tls` with the PEM parser of `rustls-pki-types`; ignore until kube moves: group B |
| RUSTSEC-2025-0141 | bincode | 1.3.3 | `compass` (direct, `bincode = "1.3"`) | none (unmaintained) | Replace in `compass` (desk picks the crate): group B |
| RUSTSEC-2024-0449 | unsafe-libyaml | 0.2.11 | serde_yaml 0.9 (`compass` direct, kube-client) | none (unmaintained) | Reasoned ignore: serde_yaml is deprecated, replace it with the kube upgrade: group B |
| RUSTSEC-2025-0012 | backoff | 0.4.0 | kube-runtime 0.98 (`service-k8s`) | none (unmaintained) | Reasoned ignore; leaves with a kube upgrade: group B |
| RUSTSEC-2024-0384 | instant | 0.1.13 | backoff, and notify-types (notify 7, `compass`) | none (unmaintained) | Reasoned ignore; leaves with kube and notify upgrades: group B |
| RUSTSEC-2024-0370 | proc-macro-error | 1.0.4 | utoipa-gen 4.3.1 (utoipa 4, `service-http`, `service-projection`) | none (unmaintained) | Reasoned ignore; leaves with the utoipa 4 to 5 upgrade: group B |

## Group A: upgrade (14 advisories)

One lock-file-only change fixes all of them. Command and result measured with `cargo update --dry-run` on main: `cargo update -p tokio -p anyhow -p h2 -p memmap2 -p crossbeam-epoch -p rustls -p rustls-webpki -p webpki-roots -p rmcp`. It moves tokio 1.52.3 to 1.54.1, anyhow to 1.0.104, h2 to 0.4.20, memmap2 to 0.9.12, crossbeam-epoch to 0.9.21, rustls to 0.23.46, rustls-webpki to 0.103.17, webpki-roots to 1.0.12, rmcp to 3.10.0 (and rmcp-macros, aws-lc-rs, aws-lc-sys). The `rmcp` requirement `3.1.4` in `crates/service-mcp/Cargo.toml` already allows 3.10.0, so no manifest change. Follow-up issue (for the desk): `chore(deps): update the 9 crates that carry 14 RustSec advisories`, one pull request; acceptance: `cargo deny check advisories` shows only the 6 group B ids.

## Group B: unmaintained, no fixed version (6 advisories)

For each of the 6 ids, the reason to ignore until the owner crate is replaced is in the "Enters through" column. Recommendation: a reasoned ignore `{ id = "RUSTSEC-...", reason = "<one line>" }` in `deny.toml` `[advisories] ignore`, only after the desk approves, with a follow-up per owner:

- (B1) `compass`: replace `bincode` 1.3 and `serde_yaml` 0.9;
- (B2) `peer-tls`: replace `rustls-pemfile` with `rustls-pki-types` PEM parsing;
- (B3) `service-k8s`: upgrade `kube` 0.98. The registry has 5.1.0. Whether it drops backoff, instant, rustls-pemfile and unsafe-libyaml is NOT checked; the follow-up must check it;
- (B4) `service-http`, `service-projection`: upgrade `utoipa` 4 to 5. Whether this drops proc-macro-error is NOT checked.

These four points are unchecked claims.

## Licence: borrow-or-share

`borrow-or-share` 0.2.4 (MIT-0) enters as `compass` -> `jsonschema` 0.29.1 -> `referencing` 0.29.1 -> `fluent-uri` 0.3.2 -> `borrow-or-share`. MIT-0 is not in `deny.toml` `[licenses] allow`, so it needs an allow entry. This is a desk decision (a new licence): it is reported here and not added. The 22 missing `license` fields are #364.

## RUSTSEC-2020-0071

RUSTSEC-2020-0071 is NOT in the 20 and not in the workspace lock. It is the seeded test fixture of #353: `time 0.1.43` in `ci/fixtures/supply-chain-advisory` on branch `claude/issue-353`.

## What this means for #353

#353 continues from `claude/issue-353` after #364, this issue and the group A upgrade. Group B needs the desk's ignores (or the replacements) before `cargo deny check` is green.
