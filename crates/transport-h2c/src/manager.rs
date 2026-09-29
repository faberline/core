//! `H2cManager` — a self-managing pool of frame-level h2c connections to one
//! authority.
//!
//! Where [`H2cPool`](crate::H2cPool) is a fixed-size, blindly round-robin set of
//! reqwest clients, `H2cManager` actively manages the underlying `h2`
//! connections:
//!
//! - **least-loaded dispatch** — each request goes to the healthy connection
//!   with the fewest in-flight streams (not blind round-robin).
//! - **bounded admission** — a per-origin semaphore caps requests admitted into
//!   the transport; bursts queue up to `pool_timeout` instead of creating
//!   unbounded streams from async task fan-out.
//! - **adaptive sizing** — grows a new connection when the least-loaded one is
//!   saturated, up to the `ln(concurrency)`/cores cap; a supervisor shrinks
//!   connections that sit idle past `idle_timeout` (down to `min_connections`).
//! - **health / failover** — a supervisor PINGs each connection for liveness;
//!   the connection driver flags GOAWAY / I/O death; dead connections are
//!   evicted and replenished to `min_connections`. Safe requests that lose
//!   their connection are retried once; mutation outcomes are ambiguous.
//! - **metrics** — [`H2cManager::stats`] snapshots connection count, health,
//!   in-flight streams, and lifetime request/error totals.
//!
//! Cheap to [`Clone`] (shares one `Arc` of state); clone freely across tasks.

mod config;
mod slots;
mod supervisor;

pub use config::{ManagerConfig, ManagerStats};

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use http::{Method, Request, Response};
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};

use crate::conn::ManagedConn;
use crate::error::{H2cError, Result};
use slots::connect_tracked;
use supervisor::supervise;

struct Inner {
    authority: String,
    cfg: ManagerConfig,
    admission: Arc<Semaphore>,
    conns: RwLock<Vec<Arc<ManagedConn>>>,
    /// Connections-plus-pending-connects, used to enforce `max_connections`
    /// without holding the conns write lock across a (slow) connect. Kept equal
    /// to `conns.len()` once connects settle.
    slots: AtomicUsize,
    next_id: AtomicUsize,
    shutdown: AtomicBool,
    // Totals from connections that have been evicted, so lifetime stats survive.
    retired_requests: AtomicU64,
    retired_errors: AtomicU64,
}

/// A self-managing pool of frame-level h2c connections to one authority.
///
/// ```no_run
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// // Warm a managed pool to a keep node and drive requests through it.
/// let mgr = transport_h2c::H2cManager::connect("keep:7117").await?;
/// let resp = mgr.get("/healthz").await?;
/// assert!(resp.status().is_success());
/// // It grows/shrinks/heals connections on its own; snapshot the live state:
/// let s = mgr.stats().await;
/// println!("{}/{} healthy, {} in-flight", s.healthy, s.connections, s.in_flight);
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct H2cManager {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for H2cManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("H2cManager")
            .field("authority", &self.inner.authority)
            .field("max_connections", &self.inner.cfg.max_connections)
            .finish_non_exhaustive()
    }
}

impl H2cManager {
    /// Connect a manager to `endpoint` (`host:port` or `http://host:port`),
    /// opening `min_connections` eagerly and starting the supervisor.
    pub async fn connect(endpoint: &str) -> Result<Self> {
        Self::with_config(endpoint, ManagerConfig::default()).await
    }

    /// Like [`connect`](Self::connect) with an explicit [`ManagerConfig`].
    pub async fn with_config(endpoint: &str, cfg: ManagerConfig) -> Result<Self> {
        let authority = authority_of(endpoint);
        let inner = Arc::new(Inner {
            authority,
            admission: Arc::new(Semaphore::new(cfg.max_in_flight_per_origin.max(1))),
            cfg,
            conns: RwLock::new(Vec::new()),
            slots: AtomicUsize::new(0),
            next_id: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            retired_requests: AtomicU64::new(0),
            retired_errors: AtomicU64::new(0),
        });
        let mgr = H2cManager { inner };

        // Open the warm minimum eagerly so the first request is fast.
        for _ in 0..mgr.inner.cfg.min_connections.max(1) {
            mgr.grow_one().await?;
        }

        // Supervisor holds a Weak so it stops when every handle is dropped.
        let weak = Arc::downgrade(&mgr.inner);
        tokio::spawn(supervise(weak));
        Ok(mgr)
    }

    /// The `host:port` this manager dials.
    pub fn authority(&self) -> &str {
        &self.inner.authority
    }

    /// GET `path` (e.g. `/healthz`).
    pub async fn get(&self, path: &str) -> Result<Response<Bytes>> {
        self.request(self.build(Method::GET, path, Bytes::new())?)
            .await
    }

    /// PUT `path` with `body`.
    pub async fn put(&self, path: &str, body: Bytes) -> Result<Response<Bytes>> {
        self.request(self.build(Method::PUT, path, body)?).await
    }

    /// POST `path` with `body`.
    pub async fn post(&self, path: &str, body: Bytes) -> Result<Response<Bytes>> {
        self.request(self.build(Method::POST, path, body)?).await
    }

    /// Send a fully-built request. Dispatches to the least-loaded healthy
    /// connection; safe reads retry once on a fresh connection. Mutations are
    /// never replayed after dispatch because their outcome may be ambiguous.
    pub async fn request(&self, req: Request<Bytes>) -> Result<Response<Bytes>> {
        let timeout = self.inner.cfg.request_timeout;
        let mut last_err: Option<H2cError> = None;
        let safe = is_safe_method(req.method());
        for attempt in 0..if safe { 2 } else { 1 } {
            let lease = self.acquire().await?;
            lease.conn.touch();
            let res = match timeout {
                Some(t) => {
                    match tokio::time::timeout(t, lease.conn.send(dup_request(&req))).await {
                        Ok(r) => r,
                        Err(_) => return Err(H2cError::Timeout(t)),
                    }
                }
                None => lease.conn.send(dup_request(&req)).await,
            };
            drop(lease); // release the in-flight slot before retry / return
            match res {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    let retryable = e.is_connection_lost();
                    if !safe && retryable {
                        let method = req.method().clone();
                        if matches!(&e, H2cError::H2(error) if error.reason() == Some(h2::Reason::REFUSED_STREAM))
                        {
                            return Err(H2cError::Refused {
                                method,
                                cause: "REFUSED_STREAM".into(),
                            });
                        }
                        return Err(H2cError::Ambiguous {
                            method,
                            cause: "connection lost after dispatch".into(),
                        });
                    }
                    last_err = Some(e);
                    if attempt == 0 && retryable {
                        continue; // fresh connection on the next loop
                    }
                    break;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| H2cError::NoConnection(self.inner.authority.clone())))
    }

    /// Snapshot connection / health / in-flight / lifetime totals.
    pub async fn stats(&self) -> ManagerStats {
        let conns = self.inner.conns.read().await;
        let mut s = ManagerStats {
            connections: conns.len(),
            ..Default::default()
        };
        for c in conns.iter() {
            if c.is_healthy() {
                s.healthy += 1;
            }
            s.in_flight += c.in_flight();
            s.total_requests += c.total();
            s.total_errors += c.errors();
        }
        s.total_requests += self.inner.retired_requests.load(Ordering::Relaxed);
        s.total_errors += self.inner.retired_errors.load(Ordering::Relaxed);
        s
    }

    /// Stop the supervisor and drop all connections (draining in-flight streams
    /// as their futures complete). Subsequent requests fail with `Shutdown`.
    pub async fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::Release);
        self.inner.admission.close();
        let drained: Vec<Arc<ManagedConn>> = {
            let mut conns = self.inner.conns.write().await;
            for c in conns.iter() {
                self.inner
                    .retired_requests
                    .fetch_add(c.total(), Ordering::Relaxed);
                self.inner
                    .retired_errors
                    .fetch_add(c.errors(), Ordering::Relaxed);
            }
            std::mem::take(&mut conns)
        };
        drop(drained);
    }

    fn build(&self, method: Method, path: &str, body: Bytes) -> Result<Request<Bytes>> {
        let uri = format!("http://{}{}", self.inner.authority, path);
        Ok(Request::builder().method(method).uri(uri).body(body)?)
    }

    /// Lease the least-loaded healthy connection (reserving an in-flight slot on
    /// it), growing a new connection when the best is saturated (and under
    /// `max_connections`) or none is healthy.
    async fn acquire(&self) -> Result<Lease> {
        if self.inner.shutdown.load(Ordering::Acquire) {
            return Err(H2cError::Shutdown);
        }
        let cfg = &self.inner.cfg;
        let admission = match tokio::time::timeout(
            cfg.pool_timeout,
            self.inner.admission.clone().acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => return Err(H2cError::Shutdown),
            Err(_) => return Err(H2cError::Timeout(cfg.pool_timeout)),
        };
        let (best, total) = {
            let conns = self.inner.conns.read().await;
            let best = conns
                .iter()
                .filter(|c| c.is_healthy())
                .min_by_key(|c| c.in_flight())
                .cloned();
            (best, conns.len())
        };

        let should_grow = match &best {
            None => true, // no healthy connection
            Some(c) => c.in_flight() >= cfg.grow_threshold && total < cfg.max_connections,
        };
        let chosen = if should_grow {
            match self.grow_one().await {
                Ok(c) => c,
                Err(e) => match best {
                    Some(b) => {
                        tracing::debug!(error = %e, "grow failed; using least-loaded existing conn");
                        b
                    }
                    None => return Err(e),
                },
            }
        } else {
            best.expect("best is Some when should_grow is false")
        };
        // Reserve the slot now so concurrent acquirers see this connection's load
        // rise — that's what makes adaptive growth track real demand.
        chosen.reserve();
        Ok(Lease {
            conn: chosen,
            _admission: admission,
        })
    }

    /// Open one new connection and add it to the pool (respecting the cap).
    async fn grow_one(&self) -> Result<Arc<ManagedConn>> {
        connect_tracked(&self.inner).await
    }
}

fn is_safe_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

/// An in-flight reservation on a connection. Holds the connection alive for the
/// request and releases the reserved slot on drop (including on cancellation).
struct Lease {
    conn: Arc<ManagedConn>,
    _admission: OwnedSemaphorePermit,
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.conn.release();
    }
}

/// Normalize an endpoint to a bare `host:port` authority for `TcpStream` +
/// the `:authority` pseudo-header (strip a leading `http://`, trailing `/`).
fn authority_of(endpoint: &str) -> String {
    endpoint
        .strip_prefix("http://")
        .unwrap_or(endpoint)
        .trim_end_matches('/')
        .to_string()
}

/// Clone a request (method / uri / version / headers / body) for a retry.
/// Extensions are dropped — the manager never sets any.
fn dup_request(req: &Request<Bytes>) -> Request<Bytes> {
    let mut builder = Request::builder()
        .method(req.method().clone())
        .uri(req.uri().clone())
        .version(req.version());
    if let Some(headers) = builder.headers_mut() {
        *headers = req.headers().clone();
    }
    builder
        .body(req.body().clone())
        .expect("rebuilding a validated request cannot fail")
}
