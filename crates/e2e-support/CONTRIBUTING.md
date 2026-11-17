# Contributing to e2e-support

## Brief

How to change `crates/e2e-support`. What it promises and the items it exports
live in [README.md](README.md); repository-wide authoring and verification
rules live in the root [CONTRIBUTING.md](../../CONTRIBUTING.md).

There is no phase ladder here. A change to this crate is authored and committed
directly.

## Verification

| Gate | Command |
|---|---|
| unit + colocated tests | `cargo test -p e2e-support` |
