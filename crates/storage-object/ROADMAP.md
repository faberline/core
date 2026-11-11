# storage-object roadmap

## Purpose

This document records future storage-object outcomes and explicit non-goals
only. It does not describe current support. This crate has not adopted the
project document set (README and STATUS), so this file stands alone.

The issue tracker owns assignees, work state, schedules, and delivery history.

## Near-term outcomes

No items.

## Later outcomes

### Scratch object store

- ID: `scratch-object-store`
- Outcome: A non-durable `ObjectStore` for scratch data. Today
  `LocalObjectStore` always uses `FsyncPolicy::Always`
  (`src/local.rs:180,189-194`).
- Boundary: Core owns the type and never claims durability; its name and docs
  say so. The app chooses it only for data it can rebuild. Independent: may
  land at any time.
- Completion evidence: The type passes the same `ObjectStore` conformance
  cases as `LocalObjectStore` (see `crates/storage-object/tests/it/local_object_store.rs`),
  and a test proves that no `fsync` call is made.
- Tracking: Not assigned.

## Non-goals

No items.
