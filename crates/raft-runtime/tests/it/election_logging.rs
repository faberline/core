//! A three-voter cluster writes the election log lines at `info`, under the
//! `raft_runtime::election` target, when it elects its first leader.

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

use tracing_subscriber::fmt::MakeWriter;

use crate::support::cluster::{await_leader, cluster};

/// A shared in-memory writer for the global test subscriber.
#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Buffer {
    type Writer = Buffer;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Installs one global `info` subscriber for this test binary and returns its
/// buffer. No other test in this binary installs a subscriber.
fn capture() -> &'static Buffer {
    static CAPTURE: OnceLock<Buffer> = OnceLock::new();
    CAPTURE.get_or_init(|| {
        let buf = Buffer::default();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(buf.clone())
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("subscriber installs once");
        buf
    })
}

fn captured() -> String {
    String::from_utf8_lossy(&capture().0.lock().unwrap()).into_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn forced_election_logs_start_vote_and_leader_lines() {
    capture();
    let nodes = cluster(3).await;
    await_leader(&nodes).await.expect("a leader is elected");
    let log = captured();
    eprintln!("{log}");
    for needle in [
        "raft: election started",
        "old_term=0",
        "new_term=1",
        "raft: vote answered",
        "granted=true",
        "raft: vote result received",
        "raft: leader changed",
        "new_leader=Some(",
    ] {
        assert!(log.contains(needle), "missing {needle:?} in:\n{log}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lines_follow_the_raft_runtime_election_target() {
    capture();
    let nodes = cluster(3).await;
    await_leader(&nodes).await.expect("a leader is elected");
    let log = captured();
    let messages = [
        "raft: election started",
        "raft: vote answered",
        "raft: vote result received",
        "raft: term changed",
        "raft: leader changed",
    ];
    let election: Vec<&str> = log
        .lines()
        .filter(|line| messages.iter().any(|m| line.contains(m)))
        .collect();
    assert!(!election.is_empty(), "no election lines in:\n{log}");
    for line in election {
        assert!(line.contains("raft_runtime::election"), "{line}");
    }
}
