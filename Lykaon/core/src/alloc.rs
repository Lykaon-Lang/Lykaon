//! M5 (Appendix L 7.6, M5-S45): the allocation counter behind
//! `Limit memory N megabytes.`
//!
//! A counting global allocator wraps the system allocator. `Limit memory`
//! stores a byte cap and resets the counter; every allocation after that
//! adds to the live-byte count. When the cap is exceeded, `exceeded()` turns
//! true and the interpreter stops at the next sentence with T0611. The
//! allocation itself is allowed to finish (an allocator must not allocate to
//! report), so the stop is clean and observable at a sentence boundary.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub static LIVE: AtomicUsize = AtomicUsize::new(0);
static LIMIT: AtomicUsize = AtomicUsize::new(0);
static EXCEEDED: AtomicBool = AtomicBool::new(false);
static PEAK: AtomicUsize = AtomicUsize::new(0);

pub struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
            let limit = LIMIT.load(Ordering::Relaxed);
            if limit > 0 && now > limit {
                EXCEEDED.store(true, Ordering::Relaxed);
            }
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(layout);
        if !p.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
            let limit = LIMIT.load(Ordering::Relaxed);
            if limit > 0 && now > limit {
                EXCEEDED.store(true, Ordering::Relaxed);
            }
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = System.realloc(ptr, layout, new_size);
        if !p.is_null() {
            let old = layout.size();
            if new_size >= old {
                let now = LIVE.fetch_add(new_size - old, Ordering::Relaxed) + (new_size - old);
                PEAK.fetch_max(now, Ordering::Relaxed);
                let limit = LIMIT.load(Ordering::Relaxed);
                if limit > 0 && now > limit {
                    EXCEEDED.store(true, Ordering::Relaxed);
                }
            } else {
                LIVE.fetch_sub(old - new_size, Ordering::Relaxed);
            }
        }
        p
    }
}

/// Set the cap (0 = unlimited) and restart the count from this sentence.
pub fn set_limit(bytes: u64) {
    LIMIT.store(bytes as usize, Ordering::Relaxed);
    LIVE.store(0, Ordering::Relaxed);
    EXCEEDED.store(false, Ordering::Relaxed);
}

pub fn exceeded() -> bool {
    EXCEEDED.load(Ordering::Relaxed)
}

pub fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

pub fn peak() -> usize {
    PEAK.load(Ordering::Relaxed)
}

/// M5g item f: restart the peak watermark at the current live count and
/// return that baseline, so a report can describe one run instead of the
/// whole process. `peak()` minus this value is the run's own high-water
/// mark (0 if the run never rose above the baseline).
pub fn reset_peak() -> usize {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    base
}
