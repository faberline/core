use super::*;

/// What the composition root needs to connect the host's peer client.
pub(crate) struct PeerWiring {
    pub(crate) id: NodeId,
    pub(crate) group_id: GroupId,
    pub(crate) peers: HashMap<NodeId, String>,
    pub(crate) rpc_timeout: Duration,
    pub(crate) propose_timeout: Duration,
    pub(crate) snapshot_rpc_timeout: Duration,
}

/// The durable storage a host is spawned over, and the path that names it in
/// `StorageFailed` and in the refusal to start.
pub(crate) struct HostStore {
    pub(crate) storage: Box<dyn HostStorage>,
    pub(crate) path: PathBuf,
}

const DEFAULT_MAX_RESIDENT_LOG_BYTES: usize = 2 * 1024 * 1024 * 1024;

impl RaftHost {
    /// Access the group identity of this host.
    pub fn group_id(&self) -> &GroupId {
        &self.shared.group_id
    }

    /// Build a host for node `id`, recovering persisted state + replaying the
    /// resident committed log into the state machine, connect its peer client
    /// with `connect`, and start the tick + pump.
    pub(crate) fn spawn_with_ports<P>(
        id: NodeId,
        group_id: GroupId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: HostStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
        connect: impl FnOnce(PeerWiring) -> Arc<P>,
    ) -> RaftHost
    where
        P: RaftDelivery + PeerClient<NodeId> + 'static,
    {
        let HostStore {
            storage,
            path: store_path,
        } = store;
        let loaded = storage.load().unwrap_or_else(|error| {
            panic!(
                "raft: refuse to start node {id}; durable state {} is invalid: {error}",
                store_path.display()
            )
        });
        let mut node = match loaded {
            Some(state) => RaftNode::from_persisted(id, &membership, state),
            None => RaftNode::new(id, &membership),
        };
        cold_start(&mut node, sm.as_ref(), true).unwrap_or_else(|error| {
            panic!("raft: refuse to start node {id}; committed replay failed: {error}")
        });

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
            .max(cfg.rpc_timeout());
        assert!(
            max_resident_log_bytes > 0,
            "RAFT_RUNTIME_MAX_RESIDENT_LOG_BYTES must be greater than zero"
        );
        let peer_client = connect(PeerWiring {
            id,
            group_id: group_id.clone(),
            peers,
            rpc_timeout: cfg.rpc_timeout(),
            propose_timeout: cfg.propose_timeout(),
            snapshot_rpc_timeout,
        });
        let shared = Arc::new(Shared {
            id,
            group_id,
            node: Mutex::new(node),
            storage,
            store_path,
            sm,
            pending_admission: StdMutex::new(BTreeMap::new()),
            delivery: Arc::clone(&peer_client) as Arc<dyn RaftDelivery>,
            peer_client,
            peer_lanes: StdRwLock::new(peer_lanes),
            applied_tx,
            cfg,
            rpc_tracker: Arc::new(RpcTracker::default()),
            latched_failure: StdMutex::new(None),
            undeliverable_never_addressed: AtomicU64::new(0),
            proposal_rejected_before_routing: AtomicU64::new(0),
            proposal_rejected_before_append: AtomicU64::new(0),
            lifecycle_generation: AtomicU64::new(0),
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
                tokio::time::sleep(s.cfg.tick()).await;
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
                tokio::time::sleep(p.cfg.pump()).await;
                p.flush().await;
            }
        });
        RaftHost {
            shared,
            tasks: StdMutex::new(Some((tick, pump))),
        }
    }
}
