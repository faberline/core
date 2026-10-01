use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::client::h2c_builder;
use crate::sizing::recommended_h2c_connections;

/// A round-robin pool of h2c clients. Each underlying [`reqwest::Client`] owns
/// one connection that multiplexes many streams; requests are dispatched across
/// them round-robin so framing spreads over multiple cores.
///
/// Size it from a target concurrency with [`H2cPool::for_concurrency`] (uses
/// [`recommended_h2c_connections`]) or pin the count with
/// [`H2cPool::with_connections`]. Cheap to [`Clone`] (shares the clients and the
/// cursor); clone freely across tasks.
///
/// ```no_run
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = transport_h2c::H2cPool::for_concurrency(256)?; // ~6 connections
/// let resp = pool.get("http://keep:7117/healthz").send().await?;
/// # let _ = resp; Ok(()) }
/// ```
#[derive(Clone)]
pub struct H2cPool {
    clients: Arc<Vec<reqwest::Client>>,
    next: Arc<AtomicUsize>,
}

impl H2cPool {
    /// Build a pool sized by [`recommended_h2c_connections`] for `concurrency`.
    pub fn for_concurrency(concurrency: usize) -> reqwest::Result<Self> {
        Self::with_connections(recommended_h2c_connections(concurrency))
    }

    /// Build a pool of exactly `n` connections (clamped to at least 1).
    pub fn with_connections(n: usize) -> reqwest::Result<Self> {
        Self::with_connections_and(n, None, None)
    }

    /// Build a pool of `n` connections, each with the given `timeout`/`user_agent`.
    pub fn with_connections_and(
        n: usize,
        timeout: Option<Duration>,
        user_agent: Option<&str>,
    ) -> reqwest::Result<Self> {
        let clients = (0..n.max(1))
            .map(|_| h2c_builder(timeout, user_agent).build())
            .collect::<reqwest::Result<Vec<_>>>()?;
        Ok(Self {
            clients: Arc::new(clients),
            next: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Number of underlying connections.
    pub fn connections(&self) -> usize {
        self.clients.len()
    }

    /// The next client in round-robin order.
    pub fn client(&self) -> &reqwest::Client {
        let i = self.next.fetch_add(1, Ordering::Relaxed) % self.clients.len();
        &self.clients[i]
    }

    /// Round-robin `GET`.
    pub fn get<U: reqwest::IntoUrl>(&self, url: U) -> reqwest::RequestBuilder {
        self.client().get(url)
    }

    /// Round-robin `POST`.
    pub fn post<U: reqwest::IntoUrl>(&self, url: U) -> reqwest::RequestBuilder {
        self.client().post(url)
    }
}

#[cfg(test)]
mod tests;
