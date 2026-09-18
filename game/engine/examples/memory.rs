//! Retained grey-box World allocations: cargo run --release -p exact-game --example memory.
#[path = "../../games/greybox/logic/src/lib.rs"]
mod greybox;
use exact_game::{Game, World};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
struct Counter;
static BYTES: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: Counter = Counter;
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            BYTES.fetch_add(l.size(), Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        BYTES.fetch_sub(l.size(), Relaxed);
        unsafe {
            System.dealloc(p, l);
        }
    }
}
fn main() {
    let before = BYTES.load(Relaxed);
    let mut w = World::new(60, 7);
    greybox::Greybox::setup(
        &mut w,
        &greybox::GreyboxArgs {
            seed: 7,
            paused: false,
        },
    );
    let allocated = BYTES.load(Relaxed) - before;
    println!(
        "GREYBOX_WORLD heap_bytes={allocated} inline_bytes={} total_bytes={} hash={:016x}",
        std::mem::size_of::<World>(),
        allocated + std::mem::size_of::<World>(),
        w.hash()
    );
}
