use super::*;
use crate::application::host::snapshot_compaction::preflight_snapshot_with_serial;

struct PermitPreparation {
    released: StdMutex<Option<oneshot::Sender<()>>>,
}

impl Drop for PermitPreparation {
    fn drop(&mut self) {
        if let Some(released) = self
            .released
            .lock()
            .expect("test preparation release mutex poisoned")
            .take()
        {
            let _ = released.send(());
        }
    }
}

struct PermitCapture;

impl crate::PreparedSnapshot for PermitCapture {
    fn write_to(self: Box<Self>, _writer: &mut dyn Write) -> Result<()> {
        Ok(())
    }
}

impl crate::SnapshotPreparation for PermitPreparation {
    fn capture_at(self: Box<Self>, _index: Index) -> Result<Box<dyn crate::PreparedSnapshot>> {
        Ok(Box::new(PermitCapture))
    }
}

struct PermitPreflightSm {
    preflight_entered: AtomicBool,
    released: StdMutex<Option<oneshot::Sender<()>>>,
}

impl RaftStateMachine for PermitPreflightSm {
    fn apply(&self, _index: Index, _command: &[u8]) -> Result<()> {
        Ok(())
    }

    fn snapshot(&self, _writer: &mut dyn Write) -> Result<()> {
        Ok(())
    }

    fn preflight_snapshot(&self) -> Result<Option<Box<dyn SnapshotPreparation>>> {
        self.preflight_entered.store(true, Ordering::Release);
        let released = self
            .released
            .lock()
            .expect("test preflight release mutex poisoned")
            .take();
        Ok(Some(Box::new(PermitPreparation {
            released: StdMutex::new(released),
        })))
    }

    fn restore(&self, _reader: &mut dyn Read) -> Result<()> {
        Ok(())
    }

    fn applied_index(&self) -> Index {
        0
    }
}

#[tokio::test]
async fn preflight_preparation_releases_product_permit_before_waiting_for_snapshot_lease() {
    let (released_tx, mut released_rx) = oneshot::channel();
    let state_machine = Arc::new(PermitPreflightSm {
        preflight_entered: AtomicBool::new(false),
        released: StdMutex::new(Some(released_tx)),
    });
    let snapshot_install = Arc::new(Mutex::new(()));
    let held_lease = Arc::clone(&snapshot_install).lock_owned().await;
    let tracker = Arc::new(RpcTracker::default());
    tracker.active.fetch_add(1, Ordering::SeqCst);
    let operation = Arc::new(RpcGuard { tracker });
    let waiting = tokio::spawn(preflight_snapshot_with_serial(
        state_machine.clone() as Arc<dyn RaftStateMachine>,
        Arc::clone(&snapshot_install),
        operation,
    ));

    let entered = tokio::time::timeout(Duration::from_secs(1), async {
        while !state_machine.preflight_entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let released_while_host_lease_is_held =
        tokio::time::timeout(Duration::from_millis(100), &mut released_rx)
            .await
            .is_ok();

    waiting.abort();
    let _ = waiting.await;
    drop(held_lease);
    if !released_while_host_lease_is_held {
        let _ = released_rx.await;
    }

    assert!(
        entered.is_ok(),
        "preflight must start before lease acquisition"
    );
    assert!(
        released_while_host_lease_is_held,
        "preflight preparation must release its product permit before awaiting snapshot_install"
    );
}
