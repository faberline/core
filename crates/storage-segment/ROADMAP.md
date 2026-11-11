# storage-segment roadmap

## Purpose

This document records future storage-segment outcomes and explicit non-goals
only. It does not describe current support. This crate has not adopted the
project document set (README and STATUS), so this file stands alone.

The issue tracker owns assignees, work state, schedules, and delivery history.

## Near-term outcomes

No items.

## Later outcomes

### Archive lifecycle primitives

- ID: `archive-lifecycle-primitives`
- Outcome: Immutable archive segments get an upload intent, a GC-pending
  staging state, and a retention fence. Sift keeps about 1,500 lines of its
  own for this.
- Boundary: Core owns the intent record, the staging state machine, and the
  fence. The app supplies the retention policy values and the segment
  contents. Order: after the `multi-workload-service-profile` adoption by Sift
  is scheduled; no earlier than keep.
- Completion evidence: Tests prove recovery from a crash between intent and
  upload, GC of staged objects past the fence, and refusal to delete a segment
  inside the retention window.
- Tracking: Not assigned.

## Non-goals

No items.
