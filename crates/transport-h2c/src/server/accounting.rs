use http::{Method, Version};
use server_lifecycle::LifecycleSubscription;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::{ConnectionProtocol, ConnectionReport, ConnectionTerminal};
use crate::http_method::is_safe_method;

pub(super) struct Accounting {
    admission: Mutex<AdmissionState>,
    protocol: Mutex<ConnectionProtocol>,
    admitted: AtomicUsize,
    active: AtomicUsize,
    active_at_drain: AtomicUsize,
    completed: AtomicUsize,
    refused: AtomicUsize,
    timed_out: AtomicUsize,
    ambiguous: AtomicUsize,
}

struct AdmissionState {
    open: bool,
    active: usize,
    mutations: usize,
}

impl Accounting {
    pub(super) fn new() -> Self {
        Self {
            admission: Mutex::new(AdmissionState {
                open: true,
                active: 0,
                mutations: 0,
            }),
            protocol: Mutex::new(ConnectionProtocol::Undetermined),
            admitted: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            active_at_drain: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
            refused: AtomicUsize::new(0),
            timed_out: AtomicUsize::new(0),
            ambiguous: AtomicUsize::new(0),
        }
    }

    fn protocol(&self, version: Version) {
        let mut protocol = self.protocol.lock().unwrap();
        *protocol = if version == Version::HTTP_2 {
            ConnectionProtocol::Http2
        } else {
            ConnectionProtocol::Http1
        };
    }

    pub(super) fn begin_drain(&self) {
        let mut admission = self.admission.lock().unwrap();
        if admission.open {
            admission.open = false;
            self.active_at_drain
                .store(admission.active, Ordering::Release);
        }
    }

    pub(super) fn drain_started(&self) -> bool {
        !self.admission.lock().unwrap().open
    }

    pub(super) fn admit(
        self: &Arc<Self>,
        version: Version,
        method: &Method,
    ) -> Option<RequestGuard> {
        self.protocol(version);
        let mutation = !is_safe_method(method);
        let mut admission = self.admission.lock().unwrap();
        if !admission.open {
            self.refused.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        admission.active += 1;
        if mutation {
            admission.mutations += 1;
        }
        self.admitted.fetch_add(1, Ordering::Relaxed);
        self.active.fetch_add(1, Ordering::AcqRel);
        drop(admission);
        Some(RequestGuard {
            accounting: Arc::clone(self),
            mutation,
            done: false,
        })
    }

    pub(super) fn mark_deadline(&self) {
        let admission = self.admission.lock().unwrap();
        let active = admission.active;
        self.timed_out.fetch_add(active, Ordering::Relaxed);
    }

    pub(super) fn report(
        &self,
        terminal: ConnectionTerminal,
        error: Option<String>,
    ) -> ConnectionReport {
        ConnectionReport {
            protocol: *self.protocol.lock().unwrap(),
            admitted: self.admitted.load(Ordering::Acquire),
            active_at_drain: self.active_at_drain.load(Ordering::Acquire),
            completed: self.completed.load(Ordering::Acquire),
            refused: self.refused.load(Ordering::Acquire),
            timed_out: self.timed_out.load(Ordering::Acquire),
            ambiguous: self.ambiguous.load(Ordering::Acquire),
            terminal,
            error,
        }
    }
}

pub(super) struct RequestGuard {
    accounting: Arc<Accounting>,
    mutation: bool,
    done: bool,
}
impl RequestGuard {
    pub(super) fn complete(mut self) {
        self.done = true;
        let mut admission = self.accounting.admission.lock().unwrap();
        admission.active = admission.active.saturating_sub(1);
        if self.mutation {
            admission.mutations = admission.mutations.saturating_sub(1);
        }
        drop(admission);
        self.accounting.active.fetch_sub(1, Ordering::AcqRel);
        self.accounting.completed.fetch_add(1, Ordering::Relaxed);
    }
}
impl Drop for RequestGuard {
    fn drop(&mut self) {
        if !self.done {
            let mut admission = self.accounting.admission.lock().unwrap();
            admission.active = admission.active.saturating_sub(1);
            if self.mutation {
                admission.mutations = admission.mutations.saturating_sub(1);
            }
            drop(admission);
            self.accounting.active.fetch_sub(1, Ordering::AcqRel);
            if self.mutation {
                self.accounting.ambiguous.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

pub(super) fn observe_drain(
    accounting: &Accounting,
    lifecycle: &Mutex<LifecycleSubscription>,
) -> bool {
    if lifecycle
        .lock()
        .unwrap()
        .observation()
        .phase
        .is_draining_or_later()
    {
        accounting.begin_drain();
        return true;
    }
    false
}
