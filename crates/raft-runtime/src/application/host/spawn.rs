use super::*;

const DEFAULT_MAX_RESIDENT_LOG_BYTES: usize = 2 * 1024 * 1024 * 1024;

impl RaftHost {
    /// Build a host for node `id`, recovering persisted state + replaying the
    /// resident committed log into the state machine, and start the tick + pump.
    /// `peers` maps the other members to base URLs (empty ⇒ single-node).
    pub fn spawn(
        id: NodeId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
    ) -> RaftHost {
        Self::spawn_group(
            id,
            GroupId(LEGACY_GROUP_ID.to_string()),
            membership,
            peers,
            store,
            sm,
            cfg,
        )
    }

    pub fn spawn_group(
        id: NodeId,
        group_id: GroupId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
    ) -> RaftHost {
        Self::spawn_inner(id, group_id, membership, peers, store, sm, cfg, None)
    }

    /// Access the group identity of this host.
    pub fn group_id(&self) -> &GroupId {
        &self.shared.group_id
    }

    /// Spawn a host whose outgoing peer RPCs use the current generation of a
    /// shared mutually authenticated HTTPS transport. Callers serve
    /// [`Self::router`] on [`PeerTransport::serve`] using the same clone.
    pub fn spawn_with_peer_transport(
        id: NodeId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
        peer_transport: PeerTransport,
    ) -> RaftHost {
        Self::spawn_with_peer_transport_group(
            id,
            GroupId(LEGACY_GROUP_ID.to_string()),
            membership,
            peers,
            store,
            sm,
            cfg,
            peer_transport,
        )
    }

    pub fn spawn_with_peer_transport_group(
        id: NodeId,
        group_id: GroupId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
        peer_transport: PeerTransport,
    ) -> RaftHost {
        Self::spawn_inner(
            id,
            group_id,
            membership,
            peers,
            store,
            sm,
            cfg,
            Some(peer_transport),
        )
    }

    fn spawn_inner(
        id: NodeId,
        group_id: GroupId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
        peer_transport: Option<PeerTransport>,
    ) -> RaftHost {
        let loaded = store.load().unwrap_or_else(|error| {
            panic!(
                "raft: refuse to start node {id}; durable state {} is invalid: {error}",
                store.path().display()
            )
        });
        let mut node = match loaded {
            Some(state) => RaftNode::from_persisted(id, &membership, state),
            None => RaftNode::new(id, &membership),
        };
        cold_start(&mut node, sm.as_ref(), true).unwrap_or_else(|error| {
            panic!("raft: refuse to start node {id}; committed replay failed: {error}")
        });

        let client =
            transport_h2c::h2c_client_with(Some(cfg.rpc_timeout), None).expect("h2c client");
        let (applied_tx, _rx) = watch::channel(sm.applied_index());
        let (shutdown_tx, _shutdown_rx) = watch::channel(None);
        let peer_lanes = peers
            .keys()
            .copied()
            .map(|peer| (peer, Arc::new(PeerLane::default())))
            .collect();
        let max_resident_log_bytes = std::env::var("RAFT_RUNTIME_MAX_RESIDENT_LOG_BYTES")
            .ok()
            .map(|value| {
                value.parse::<usize>().unwrap_or_else(|error| {
                    panic!(
                        "RAFT_RUNTIME_MAX_RESIDENT_LOG_BYTES must be a positive integer: {error}"
                    )
                })
            })
            .unwrap_or(DEFAULT_MAX_RESIDENT_LOG_BYTES);
        let snapshot_rpc_timeout = std::env::var("RAFT_RUNTIME_SNAPSHOT_RPC_TIMEOUT_SECONDS")
            .ok()
            .map(|value| {
                value.parse::<u64>().unwrap_or_else(|error| {
                    panic!(
                        "RAFT_RUNTIME_SNAPSHOT_RPC_TIMEOUT_SECONDS must be a positive integer: {error}"
                    )
                })
            })
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(600))
            .max(cfg.rpc_timeout);
        assert!(
            max_resident_log_bytes > 0,
            "RAFT_RUNTIME_MAX_RESIDENT_LOG_BYTES must be greater than zero"
        );
        let shared = Arc::new(Shared {
            id,
            group_id,
            node: Mutex::new(node),
            store,
            sm,
            pending_admission: StdMutex::new(BTreeMap::new()),
            peers: StdRwLock::new(peers),
            peer_lanes: StdRwLock::new(peer_lanes),
            client,
            peer_transport,
            applied_tx,
            cfg,
            rpc_tracker: Arc::new(RpcTracker::default()),
            latched_failure: StdMutex::new(None),
            undeliverable_never_addressed: AtomicU64::new(0),
            undeliverable_withdrawn_address: AtomicU64::new(0),
            proposal_rejected_before_routing: AtomicU64::new(0),
            proposal_rejected_before_append: AtomicU64::new(0),
            lifecycle_generation: AtomicU64::new(0),
            snapshot_nonce: AtomicU64::new(1),
            snapshot_rpc_timeout,
            snapshot_install: Arc::new(Mutex::new(())),
            apply_running: AtomicBool::new(false),
            apply_stopped: AtomicBool::new(false),
            apply_tracker: Arc::new(RpcTracker::default()),
            max_resident_log_bytes,
            shutdown_started: AtomicBool::new(false),
            shutdown_tx,
        });

        let s = Arc::clone(&shared);
        let tick = tokio::spawn(async move {
            let mut first_tick = true;
            loop {
                tokio::time::sleep(s.cfg.tick).await;
                {
                    let mut n = s.node.lock().await;
                    if tick_then_maybe_persist(
                        &mut n,
                        &mut first_tick,
                        |node| s.persist(node).is_ok(),
                        || s.latched_failure.lock().unwrap().is_some(),
                    ) {
                        s.apply_ready(&mut n);
                    }
                }
                s.flush().await;
            }
        });
        let p = Arc::clone(&shared);
        let pump = tokio::spawn(async move {
            loop {
                tokio::time::sleep(p.cfg.pump).await;
                p.flush().await;
            }
        });
        RaftHost {
            shared,
            tasks: StdMutex::new(Some((tick, pump))),
        }
    }
}
