use super::*;
use crate::config::TcpSocketOptions;
use anyhow::Result;
use server_lifecycle::BindConfig;
use server_lifecycle::{ConnectionBudget, ConnectionMetrics};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

#[tokio::test]
async fn bind_uses_configured_socket_options() {
    // @spec apps/agentic-workflow/tech-design/logic/shared-server-substrate-performance-layers.md#unit-test
    let cfg =
        TcpServerConfig::new(BindConfig::localhost(0)).with_socket_options(TcpSocketOptions {
            backlog: 128,
            reuse_addr: true,
            nodelay: true,
        });
    let listener = bind(&cfg).await.expect("bind");
    assert!(listener.local_addr().unwrap().port() > 0);
}

#[tokio::test]
async fn serve_accepts_closure_handler_without_async_trait_boxing() {
    // @spec apps/agentic-workflow/tech-design/logic/shared-server-substrate-performance-layers.md#unit-test
    let cfg = TcpServerConfig::new(BindConfig::localhost(0));
    let listener = bind(&cfg).await.expect("bind");
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = tokio::spawn(serve(
        listener,
        cfg,
        |mut stream: TcpStream, _cx: ConnectionContext| async move {
            let mut buf = [0_u8; 4];
            stream.read_exact(&mut buf).await?;
            stream.write_all(&buf).await?;
            Ok(())
        },
        async move {
            let _ = shutdown_rx.await;
        },
    ));

    let mut client = TcpStream::connect(addr).await.expect("connect");
    client.write_all(b"ping").await.expect("write");
    let mut out = [0_u8; 4];
    client.read_exact(&mut out).await.expect("read");
    assert_eq!(&out, b"ping");

    let _ = shutdown_tx.send(());
    server.await.expect("server task");
}

#[tokio::test]
async fn connection_budget_releases_after_handler_finishes() {
    // @spec apps/agentic-workflow/tech-design/logic/shared-server-substrate-performance-layers.md#unit-test
    let budget = ConnectionBudget::new(1);
    let cfg = TcpServerConfig::new(BindConfig::localhost(0)).with_connection_budget(budget.clone());
    let listener = bind(&cfg).await.expect("bind");
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = tokio::spawn(serve(
        listener,
        cfg,
        |stream: TcpStream, _cx: ConnectionContext| async move {
            drop(stream);
            Result::<()>::Ok(())
        },
        async move {
            let _ = shutdown_rx.await;
        },
    ));

    let _client = TcpStream::connect(addr).await.expect("connect");
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if budget.active() == 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("permit released");

    let _ = shutdown_tx.send(());
    server.await.expect("server task");
}

#[derive(Default)]
struct CountingMetrics {
    accepted: AtomicUsize,
    rejected: AtomicUsize,
    closed: AtomicUsize,
}

impl ConnectionMetrics for CountingMetrics {
    fn connection_accepted(&self) {
        self.accepted.fetch_add(1, Ordering::SeqCst);
    }

    fn connection_rejected(&self) {
        self.rejected.fetch_add(1, Ordering::SeqCst);
    }

    fn connection_closed(&self) {
        self.closed.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn metrics_cover_admission_rejection_and_completion_once() {
    let budget = ConnectionBudget::new(1);
    let metrics = Arc::new(CountingMetrics::default());
    let cfg = TcpServerConfig::new(BindConfig::localhost(0))
        .with_connection_budget(budget)
        .with_connection_metrics(metrics.clone());
    let listener = bind(&cfg).await.expect("bind");
    let addr = listener.local_addr().unwrap();
    let (release_tx, release_rx) = oneshot::channel();
    let release = Arc::new(tokio::sync::Mutex::new(Some(release_rx)));
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let server = tokio::spawn(serve(
        listener,
        cfg,
        move |stream: TcpStream, _cx: ConnectionContext| {
            let release = release.clone();
            async move {
                let mut release = release.lock().await;
                if let Some(rx) = release.take() {
                    let _ = rx.await;
                }
                drop(stream);
                Result::<()>::Ok(())
            }
        },
        async move {
            let _ = shutdown_rx.await;
        },
    ));

    let first = TcpStream::connect(addr).await.expect("first connect");
    tokio::time::timeout(Duration::from_secs(2), async {
        while metrics.accepted.load(Ordering::SeqCst) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first connection admitted");
    let second = TcpStream::connect(addr).await.expect("second connect");
    tokio::time::timeout(Duration::from_secs(2), async {
        while metrics.rejected.load(Ordering::SeqCst) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("second connection rejected");

    let _ = release_tx.send(());
    tokio::time::timeout(Duration::from_secs(2), async {
        while metrics.closed.load(Ordering::SeqCst) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("admitted connection closed");
    assert_eq!(metrics.accepted.load(Ordering::SeqCst), 1);
    assert_eq!(metrics.rejected.load(Ordering::SeqCst), 1);
    assert_eq!(metrics.closed.load(Ordering::SeqCst), 1);

    drop(first);
    drop(second);
    let _ = shutdown_tx.send(());
    server.await.expect("server task");
}
