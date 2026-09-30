use super::*;

/// Pending work for one peer lane.
///
/// Only consecutive, unsent `Append` requests may coalesce. Every other Raft
/// message is a FIFO barrier: a later heartbeat must neither replace nor pass
/// a pending control message such as `TimeoutNow`.
#[derive(Default)]
pub(crate) struct PeerLaneQueue {
    messages: VecDeque<RaftMsg>,
}

impl PeerLaneQueue {
    pub(crate) fn enqueue(&mut self, message: RaftMsg) {
        match message {
            RaftMsg::Append(next) => {
                if let Some(RaftMsg::Append(pending)) = self.messages.back_mut() {
                    *pending = next;
                } else {
                    self.messages.push_back(RaftMsg::Append(next));
                }
            }
            control => self.messages.push_back(control),
        }
    }

    pub(crate) fn dequeue(&mut self) -> Option<RaftMsg> {
        self.messages.pop_front()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.messages.len()
    }
}

#[derive(Default)]
pub(crate) struct PeerLane {
    pending: Mutex<PeerLaneQueue>,
    running: AtomicBool,
}

impl Shared {
    /// Drain the outbox and deliver each request to its peer through the
    /// delivery port, one task per peer lane (fire-and-forget). Replies feed back into the node + drive apply.
    pub(super) async fn flush(self: &Arc<Self>) {
        let outs = {
            let mut n = self.node.lock().await;
            n.take_outgoing()
        };
        for o in outs {
            let lane = {
                let lanes = self.peer_lanes.read().unwrap_or_else(|p| p.into_inner());
                if let Some(l) = lanes.get(&o.to).cloned() {
                    Some(l)
                } else if self.peer_client.address(&o.to).is_some() {
                    drop(lanes);
                    let mut lanes = self.peer_lanes.write().unwrap_or_else(|p| p.into_inner());
                    Some(
                        lanes
                            .entry(o.to)
                            .or_insert_with(|| Arc::new(PeerLane::default()))
                            .clone(),
                    )
                } else {
                    None
                }
            };
            let Some(lane) = lane else {
                self.undeliverable_never_addressed
                    .fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    target = o.to.get(),
                    group = %self.group_id.0,
                    "raft: discarded message to peer with no registered address"
                );
                continue;
            };
            let spawn_worker = {
                let mut pending = lane.pending.lock().await;
                pending.enqueue(o.msg);
                !lane.running.swap(true, Ordering::AcqRel)
            };
            if spawn_worker {
                let s = Arc::clone(self);
                let tracker = Arc::clone(&self.rpc_tracker);
                tracker.active.fetch_add(1, Ordering::AcqRel);
                tokio::spawn(async move {
                    let _guard = RpcGuard { tracker };
                    loop {
                        let next = {
                            let mut pending = lane.pending.lock().await;
                            match pending.dequeue() {
                                Some(msg) => Some(msg),
                                None => {
                                    // Producers update the queue and observe
                                    // `running` under this same lock, closing
                                    // the empty-lane/spawn race.
                                    lane.running.store(false, Ordering::Release);
                                    None
                                }
                            }
                        };
                        let Some(msg) = next else {
                            break;
                        };
                        Arc::clone(&s).send_request(o.to, msg).await;
                    }
                });
            }
        }
    }
}
