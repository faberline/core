//! Append log whose tail is allowed to be garbage.
//!
//! A frame is a 16-byte header -- `seq: u64 LE`, `len: u32 LE`, `crc32: u32 LE`
//! -- followed by `len` payload bytes. **The CRC covers the payload only, never
//! the header.** A corrupted header cannot announce itself as corrupt: it is
//! read as some plausible `(seq, len)` pair and rejected when `len` exceeds the
//! fixed payload limit, runs past EOF, or points at a payload that fails its
//! CRC. That is the whole torn-tail rule -- the first frame failing any check
//! ends the log, and every byte after it is discarded, unexamined.
//!
//! So reading is not read-only. [`FramedLogWriter::open`] scans for the last
//! good frame end and `set_len`s the file down to it before appending, which
//! means opening a torn log **truncates it**. Recovery happens on open, not on
//! replay.
//!
//! Both sequence arguments are exclusive, in opposite-looking APIs:
//! `read_frames(path, from_seq)` and `replay` keep `seq > from_seq`, and
//! `truncate_through(through)` keeps `seq > through`. `replay` returns the
//! highest seq it actually applied, so replaying a range with nothing in it
//! returns `0` rather than the log's real maximum.
//!
//! `truncate_through` is a full rewrite -- read every frame, write the survivors
//! to `<path>.compact.tmp`, fsync, rename, fsync the parent, re-open in append
//! mode. It is not an in-place hole punch, and its cost is the whole log.
//!
//! The `EverySec` interval is fixed at one second in `open` and is not
//! configurable by any public method.

mod codec;
mod cursor;
mod frame;
mod observer;
mod reader;
mod sync_plan;
mod trim;
mod trim_plan;
mod trim_stream;
mod writer;

pub use codec::MAX_FRAME_PAYLOAD_BYTES;
pub use cursor::FramedLogCursor;
pub use frame::{LogFrame, MappedLogFrame};
pub use observer::FramedLogTrimObserver;
pub use reader::FramedLogReader;
pub use trim_plan::FramedLogTrimPlan;
pub use writer::FramedLogWriter;

#[cfg(test)]
mod tests;
