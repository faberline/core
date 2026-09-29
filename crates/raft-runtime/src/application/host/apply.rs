use super::*;

/// Apply the durable Raft state into a caller supplied state machine.  The
/// production host and the deterministic conformance host intentionally share
/// this primitive so cold replay, installed snapshots, and command ordering do
/// not have two subtly different implementations.
pub(crate) fn apply_ready(
    node: &mut RaftNode,
    sm: &dyn RaftStateMachine,
    applied_tx: Option<&watch::Sender<Index>>,
    snapshot_policy: SnapshotPolicy,
    strict: bool,
) -> anyhow::Result<()> {
    apply_ready_with_admission(node, sm, applied_tx, snapshot_policy, strict, None)
}

pub(super) fn apply_ready_with_admission(
    node: &mut RaftNode,
    sm: &dyn RaftStateMachine,
    applied_tx: Option<&watch::Sender<Index>>,
    snapshot_policy: SnapshotPolicy,
    strict: bool,
    pending_admission: Option<&StdMutex<BTreeMap<(Index, u64), AdmissionPermit>>>,
) -> anyhow::Result<()> {
    if let Some(bytes) = node.take_installed_snapshot() {
        sm.restore(&mut std::io::Cursor::new(bytes))?;
    }
    let mut advanced = false;
    while let Some((index, term, kind)) = node.peek_next_committed_identity() {
        let permit = pending_admission.and_then(|pending| {
            let mut pending = pending.lock().unwrap_or_else(|p| p.into_inner());
            pending.retain(|(at, entry_term), _| *at != index || *entry_term == term);
            pending.remove(&(index, term))
        });
        if kind == raft_core::EntryKind::Command && index > sm.applied_index() {
            // Cold start and deterministic conformance own the node directly.
            // Borrow exactly this command; never materialize a committed batch.
            let persisted = node.persisted_ref();
            let offset = (index - persisted.snapshot_index - 1) as usize;
            let entry = &persisted.log[offset];
            sm.apply_admitted(index, &entry.command, permit)?;
            if sm.applied_index() < index {
                anyhow::bail!("state machine returned success without applying index {index}");
            }
            advanced = true;
        }
        if !node.finish_committed_identity(index, term) {
            anyhow::bail!("committed identity changed while applying index {index}");
        }
    }
    if advanced {
        if let Some(tx) = applied_tx {
            tx.send_replace(sm.applied_index());
        }
    }
    let SnapshotPolicy::EveryEntries(every) = snapshot_policy else {
        return Ok(());
    };
    let applied = sm.applied_index();
    if applied == 0 || applied.saturating_sub(node.snapshot_index()) < every {
        return Ok(());
    }
    let mut sink = ChunkSink::new(SNAPSHOT_CHUNK_SIZE);
    match sm.snapshot_at(applied, &mut sink) {
        Ok(()) => node.compact(applied, sink.into_bytes()),
        Err(e) if strict => return Err(e),
        Err(e) => tracing::warn!(error = %e, "raft: snapshot capture failed; skip compaction"),
    }
    Ok(())
}

/// Cold-start uses the same applier as every ordinary step.  It intentionally
/// leaves store-load policy to the caller: the production host retains its
/// historical best-effort load behavior, while conformance opening returns a
/// load error to its caller.
pub(crate) fn cold_start(
    node: &mut RaftNode,
    sm: &dyn RaftStateMachine,
    strict: bool,
) -> anyhow::Result<()> {
    apply_ready(node, sm, None, SnapshotPolicy::Disabled, strict)
}

/// Persist exactly the core durable image.  The production wrapper adds its
/// latched-failure policy; deterministic conformance returns this error to the
/// scheduler.  Both therefore save identical bytes at identical step points.
pub(crate) fn persist_node(store: &RaftStore, node: &RaftNode) -> std::io::Result<()> {
    store.save_ref(&node.persisted_ref())
}

/// Advance one periodic tick without revalidating an unchanged durable image.
/// `RaftNode::tick` currently changes durable state only when it advances the
/// term to start an election. The first tick persists a fresh node because a
/// learner may otherwise never elect and would leave its membership unsaved.
pub(super) fn tick_then_maybe_persist<P, L>(
    node: &mut RaftNode,
    first_tick: &mut bool,
    persist: P,
    is_latched: L,
) -> bool
where
    P: FnOnce(&RaftNode) -> bool,
    L: FnOnce() -> bool,
{
    let term_before = node.current_term();
    node.tick();
    let must_persist = std::mem::replace(first_tick, false) || node.current_term() != term_before;
    if must_persist && !persist(node) {
        return false;
    }
    !is_latched()
}
