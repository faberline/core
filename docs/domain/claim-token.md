# claim-token

claim-token models a scoped claim-check access token. loom's schema layer
signs a token scoped to one task's keep keys; keep verifies it. A worker can
then read its input from keep and write its result to keep directly, so the
bytes never pass through loom, but only within that scope and only until the
token expires. The signer and the verifier live in one crate so they cannot
drift. No core crate depends on it; downstream, keep and loom use it.

**Form:** domain only · **Depends on:** — · **Crate:** [`crates/claim-token`](../../crates/claim-token)

## Model

- **Scope** — `Scope`: what a token authorizes. `r` is the readable input key
  (`GET /v1/inputs/{r}`), `w` is the writable result key
  (`PUT /v1/results/{w}`), and `exp` is the expiry in Unix seconds.
- **Claim token** — the text `b64url(json(scope)) "." hex(hmac)`: the scope as
  unpadded URL-safe base64 of its JSON, then the hex HMAC-SHA256 of that
  payload.
- **Secret** — the key both sides share. It is distributed out of band; the
  crate never loads or stores it.
- **Signing** — `sign(secret, &scope)` returns a claim token.
- **Verification** — `verify(secret, token, now)` returns the `Scope` of a
  valid token, else `None`.

## Ports

None. The caller passes the current time, so the crate reads no clock.

## Invariants

- `verify` checks the signature before it decodes the payload, and compares
  signatures in constant time.
- A token without a `.`, with a wrong secret, with a changed payload or
  signature, or with an undecodable payload yields `None`.
- A token is valid while `exp >= now`; at `now = exp + 1` it is rejected.
- HMAC-SHA256 follows the standard construction over `sha2`: a key longer
  than the 64-byte block is hashed first.

## Published language

The whole public API; domain-only contexts have no application layer. That
API is `Scope`, `sign` and `verify` at the crate root; there is no old module
path to keep. The behaviour contract is in
[`docs/contracts/behavior/scoped-claim-tokens-contract.md`](../../crates/claim-token/docs/contracts/behavior/scoped-claim-tokens-contract.md)
(ADR D10).

## Exceptions and debts

- **Checker exceptions (P1):** B2 (`base64`): the token format is base64url,
  and `base64` is not on the domain allowlist. The encoding is part of the
  token's wire form, so this stays with a long-term reason.
- **Tracked for P2:**
  - `Scope` public fields, built with struct literals by keep and loom
    (ADR D2).
  - Bare ids: the input and result keys are `String`s and the expiry a `u64`.
