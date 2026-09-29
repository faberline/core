//! Durable storage for a raft node's hard state.
//!
//! Persists [`PersistedState`] (term, votedFor, log, snapshot) to a single file
//! under a data dir, written atomically (temp + rename) and fsynced per
//! [`FsyncPolicy`]. The host calls [`RaftStore::save`] *before* flushing the
//! node's outbox, so no vote or ack is sent before the decision that produced it
//! is durable. (Lifted from lumen/relay's identical `raft_store`.)

use std::collections::BTreeMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use memmap2::{Mmap, MmapOptions};
use raft_core::{
    ConfState, EntryKind, Index, NodeId, PersistedState, PersistedStateRef, RaftEntry, Term,
};
use sha2::{Digest, Sha256};
pub use storage_durable::FsyncPolicy;

use crate::application::{GroupId, LEGACY_GROUP_ID};

mod artifacts;
mod command_frame;
mod command_pin;
mod cursor;
mod fault_injection;
mod load;
mod log_codec;
mod log_plan;
mod migration;
mod open;
mod save;
mod state_codec;

#[cfg(test)]
mod command_lease_tests;

use command_frame::{
    apply_command_frame_update, command_frame_update, command_frames_from_artifact,
    validate_command_frame_update,
};
use cursor::CursorReader;
use log_codec::{decode_log_artifact, encode_log_entries, entry_digest};
use log_plan::{plan_log_write, LogWritePlan};
use state_codec::encode_persisted_state_v4;

const MAGIC_V1: &[u8; 8] = b"RAFTST01";
const MAGIC_V2: &[u8; 8] = b"RAFTST02";
const MAGIC_V3: &[u8; 8] = b"RAFTST03";
const MAGIC_V4: &[u8; 8] = b"RAFTST04";
const LOG_MAGIC_V1: &[u8; 8] = b"RAFTLG01";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LogLayout {
    generation: [u8; 32],
    byte_len: u64,
    entry_count: u64,
    first_index: u64,
    last_index: u64,
    last_term: u64,
    last_entry_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug)]
struct CommandFrame {
    index: Index,
    term: Term,
    kind: EntryKind,
    payload_offset: u64,
    payload_len: u64,
    payload_crc: u32,
    command_offset: u64,
    command_len: u64,
}

#[derive(Clone)]
struct PublishedLog {
    layout: LogLayout,
    commands: BTreeMap<(Index, Term), CommandFrame>,
}

#[derive(Default)]
struct StoreCache {
    hard_digest: Option<[u8; 32]>,
    log: Option<PublishedLog>,
    commit_index: Index,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PersistenceStats {
    pub log_bytes_appended: u64,
    pub log_bytes_rewritten: u64,
}

/// File-backed persistence for one raft node.
pub struct RaftStore {
    path: PathBuf,
    fsync: FsyncPolicy,
    io_serial: Mutex<()>,
    cache: Mutex<StoreCache>,
    pinned_log_generations: Arc<Mutex<BTreeMap<[u8; 32], usize>>>,
    log_bytes_appended: AtomicU64,
    log_bytes_rewritten: AtomicU64,
    injected_save_failure: Mutex<Option<io::ErrorKind>>,
    injected_after_artifact_failure: Mutex<Option<io::ErrorKind>>,
    injected_after_publish_failure: Mutex<Option<io::ErrorKind>>,
    #[cfg(test)]
    load_after_state_read_hook: Mutex<Option<LoadAfterStateReadHook>>,
}

#[cfg(test)]
struct LoadAfterStateReadHook {
    entered: std::sync::mpsc::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

/// A generation-pinned identity for one committed command. The pin is created
/// while a host still holds its Raft-node mutex; a later implementation maps
/// and validates the exact durable frame after that mutex is released.
pub struct CommittedCommandPin {
    generation: GenerationPin,
    frame: CommandFrame,
}

/// Read-only bytes for one durable Raft command.
pub struct CommittedCommandLease {
    map: Mmap,
    command_start: usize,
    command_end: usize,
    _generation: GenerationPin,
}

struct GenerationPin {
    generation: [u8; 32],
    path: PathBuf,
    byte_len: u64,
    pins: Arc<Mutex<BTreeMap<[u8; 32], usize>>>,
}
