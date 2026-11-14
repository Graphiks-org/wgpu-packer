use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use wgpu_packer::native::{WGPUInstance, WGPUInstanceDescriptor};

/// The memory oracles share a global allocator: serializes the tests of the
/// binary (otherwise the allocation baseline is measured while neighboring
/// tests allocate on other threads — intermittent failure).
pub static TEST_SERIAL: Mutex<()> = Mutex::new(());

/// Counting allocator: declare it as `#[global_allocator]` in the test binary.
pub struct CountingAllocator;
pub static LIVE_ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
pub static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            LIVE_ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
            LIVE_BYTES.fetch_add(layout.size() as u64, Ordering::SeqCst);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        LIVE_ALLOCATIONS.fetch_sub(1, Ordering::SeqCst);
        LIVE_BYTES.fetch_sub(layout.size() as u64, Ordering::SeqCst);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = System.realloc(ptr, layout, new_size);
        if !p.is_null() {
            LIVE_BYTES.fetch_add(new_size as u64, Ordering::SeqCst);
            LIVE_BYTES.fetch_sub(layout.size() as u64, Ordering::SeqCst);
        }
        p
    }
}

/// (live allocations, live bytes) at the time of the call.
pub fn live() -> (u64, u64) {
    (LIVE_ALLOCATIONS.load(Ordering::SeqCst), LIVE_BYTES.load(Ordering::SeqCst))
}

/// Headless instance with a zeroed descriptor (like `wgpuCreateInstance(NULL)` in C).
pub unsafe fn make_instance() -> WGPUInstance {
    let desc = std::mem::zeroed::<WGPUInstanceDescriptor>();
    wgpu_packer::wgpuCreateInstance(Some(&desc))
}

pub unsafe fn release_instance(instance: WGPUInstance) {
    wgpu_packer::wgpuInstanceRelease(instance);
}

/// Poisoning-insensitive serialization lock: a test that panics must not
/// cascade onto the following ones (isolation still holds).
pub fn TEST_SERIAL_LOCKED() -> std::sync::MutexGuard<'static, ()> {
    match TEST_SERIAL.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}
