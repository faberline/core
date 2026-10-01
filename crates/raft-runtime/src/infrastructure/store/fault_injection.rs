use super::*;

impl RaftStore {
    /// Fault-injection seam for testing durable persistence failures before save.
    pub fn inject_next_save_failure_with_kind(&self, kind: io::ErrorKind) {
        *self
            .injected_save_failure
            .lock()
            .expect("injected_save_failure mutex poisoned") = Some(kind);
    }

    /// Fault-injection seam armed after the snapshot artifact is written and before the hard-state reference is published.
    pub fn inject_next_after_artifact_failure_with_kind(&self, kind: io::ErrorKind) {
        *self
            .injected_after_artifact_failure
            .lock()
            .expect("injected_after_artifact_failure mutex poisoned") = Some(kind);
    }

    /// Fault-injection seam armed after the hard-state reference is published and before superseded artifact collection.
    pub fn inject_next_after_publish_failure_with_kind(&self, kind: io::ErrorKind) {
        *self
            .injected_after_publish_failure
            .lock()
            .expect("injected_after_publish_failure mutex poisoned") = Some(kind);
    }

    #[cfg(test)]
    pub(super) fn pause_next_load_after_state_read(
        &self,
        entered: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
    ) {
        *self
            .load_after_state_read_hook
            .lock()
            .expect("load_after_state_read_hook mutex poisoned") =
            Some(LoadAfterStateReadHook { entered, release });
    }

    #[cfg(test)]
    pub(super) fn wait_after_load_state_read(&self) {
        let hook = self
            .load_after_state_read_hook
            .lock()
            .expect("load_after_state_read_hook mutex poisoned")
            .take();
        if let Some(hook) = hook {
            let _ = hook.entered.send(());
            let _ = hook.release.recv();
        }
    }
}
