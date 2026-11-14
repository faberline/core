# Threat model

This threat model covers the core library workspace: the crates in `crates/`, their tests and the way downstream repositories take core. It names each asset, the threats to it, the control that exists in this repository and the risk that remains. The independent review of this threat model is a person's task and is not done here.

## Auth

**Assets:** the bearer credential that a caller presents, and the authorization decision that follows from it.

**Threats:** a forged, expired or wrong-audience token is accepted; a denied or unanswered decision is treated as allowed.

**Control:** `service-auth` verifies the bearer token and fails closed. See the section "Authenticate a Kubernetes caller" in `crates/service-auth/README.md` (TokenReview plus SubjectAccessReview, with an explicit audience). The code is in `crates/service-auth/src/api/k8s/delegated.rs` and `crates/service-auth/src/application/role_map.rs`.

**Residual risk:** the application owns the resource mapping and the access policy. Core does not check them.

## TLS

**Assets:** the peer traffic between nodes.

**Threats:** an unauthenticated peer connects; a plaintext window opens during certificate rotation; material with a wrong identity is accepted.

**Control:** `peer-tls` validates PEM material and builds mutual-TLS configurations. The validation is in `crates/peer-tls/src/material/validation.rs`, and rotation with trust overlap is in `crates/peer-tls/src/reload/reloadable.rs`. The tests are `crates/peer-tls/tests/it/material_rejection.rs`, `crates/peer-tls/tests/it/trust_overlap.rs` and `crates/peer-tls/tests/it/tls_reload.rs`. The behavior of an open connection across a reload is covered by a separate test (issue 355).

**Residual risk:** an open connection across a reload is covered by a separate test (issue 355), not by the tests named here.

## Audit

**Assets:** the record of who was accepted or refused.

**Threats:** a credential is written to a log; an authorization event is lost.

**Control:** `AuthEvent` cannot hold a bearer credential, and `TracingAuthEventSink` writes events under the tracing target `service_auth.audit`. See `crates/service-auth/src/infrastructure/tracing_sink.rs` and `crates/service-auth/src/domain.rs`.

**Residual risk:** where the tracing output goes and how long it is kept is a decision of the service that runs it.

## Secrets

**Assets:** certificate secrets and ServiceAccount tokens.

**Threats:** a token is stored in a Secret or a ConfigMap; a secret is written by another field manager.

**Control:** certificate secrets are applied with Server-Side Apply by one field manager, in `crates/service-k8s/src/infrastructure/certificate/kubernetes_store.rs`. The callee token is a projected, audience-bound `serviceAccountToken` volume and never a Secret; see `crates/service-k8s/src/infrastructure/manifest/projected_token.rs`.

**Residual risk:** cluster RBAC and etcd encryption belong to the cluster operator.

## Supply chain of the git-tag dependency model

**Assets:** the code that a service builds from a core tag.

**Threats:** a tag is moved or reused; a service pins two different tags; a vendored crate changes without notice.

**Control:** tags are never moved or reused, and each workspace uses one tag. The pinning rules are in `docs/operations/README.md` (sections "Releasing" and "How downstream repos consume core") and in `docs/operations/release-pipeline.md`. Dependencies are locked in `Cargo.lock`, the dependency policy is in `deny.toml`, and the vendored crate lives under `vendor/`.

**Residual risk:** a git tag is not signed. A service that does not check the tag trusts GitHub.

## Not applicable to core

Reproducible images, SBOM and signing do not apply to core, because core is a library and builds no image or release artifact. They apply to the service repositories that build images from a core tag.

The independent review of this threat model is a person's task and is not done here.
