use super::*;

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::FsyncPolicy;

use super::codec::HEADER_LEN;
#[cfg(unix)]
use super::trim_plan::TrimFaultPoint;

mod cursor;
mod large_frame;
mod mapped_trim;
mod mapped_trim_failures;
mod writer;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct FrameAllocationObserver;

#[global_allocator]
static FRAME_ALLOCATION_OBSERVER: FrameAllocationObserver = FrameAllocationObserver;

thread_local! {
    static OBSERVE_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static LARGEST_ALLOCATION: Cell<usize> = const { Cell::new(0) };
}

impl FrameAllocationObserver {
    fn observe(size: usize) {
        let _ = OBSERVE_ALLOCATIONS.try_with(|enabled| {
            if enabled.get() {
                LARGEST_ALLOCATION.with(|largest| largest.set(largest.get().max(size)));
            }
        });
    }
}

unsafe impl GlobalAlloc for FrameAllocationObserver {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let allocation = unsafe { System.alloc(layout) };
        Self::observe(layout.size());
        allocation
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let allocation = unsafe { System.alloc_zeroed(layout) };
        Self::observe(layout.size());
        allocation
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let allocation = unsafe { System.realloc(pointer, layout, size) };
        Self::observe(size);
        allocation
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}

struct AllocationObservation;

impl AllocationObservation {
    fn start() -> Self {
        OBSERVE_ALLOCATIONS.with(|enabled| enabled.set(true));
        LARGEST_ALLOCATION.with(|largest| largest.set(0));
        Self
    }

    fn reset(&self) {
        LARGEST_ALLOCATION.with(|largest| largest.set(0));
    }

    fn largest_request(&self) -> usize {
        LARGEST_ALLOCATION.with(Cell::get)
    }
}

impl Drop for AllocationObservation {
    fn drop(&mut self) {
        OBSERVE_ALLOCATIONS.with(|enabled| enabled.set(false));
    }
}

const LARGE_PAYLOAD_BYTES: usize = MAX_FRAME_PAYLOAD_BYTES + 1;

fn replay_sequences(path: &Path) -> Vec<u64> {
    FramedLogReader::read_frames(path, 0)
        .unwrap()
        .into_iter()
        .map(|frame| frame.seq())
        .collect()
}
