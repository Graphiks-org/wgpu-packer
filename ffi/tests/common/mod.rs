use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use wgpu_packer::native::{WGPUInstance, WGPUInstanceDescriptor};

/// Les oracles mémoire partagent un allocateur global : sérialise les tests du
/// binaire (sinon la baseline d'allocations est mesurée pendant que les tests
/// voisins allouent sur d'autres threads — échec intermittent).
pub static TEST_SERIAL: Mutex<()> = Mutex::new(());

/// Allocateur compteur : à déclarer `#[global_allocator]` dans le binaire de test.
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

/// (allocations vivantes, octets vivants) au moment de l'appel.
pub fn live() -> (u64, u64) {
    (LIVE_ALLOCATIONS.load(Ordering::SeqCst), LIVE_BYTES.load(Ordering::SeqCst))
}

/// Instance headless avec descripteur zéro (comme `wgpuCreateInstance(NULL)` en C).
pub unsafe fn make_instance() -> WGPUInstance {
    let desc = std::mem::zeroed::<WGPUInstanceDescriptor>();
    wgpu_packer::wgpuCreateInstance(Some(&desc))
}

pub unsafe fn release_instance(instance: WGPUInstance) {
    wgpu_packer::wgpuInstanceRelease(instance);
}

/// Verrou de sérialisation insensible à l'empoisonnement : un test qui panique
/// ne doit pas cascader sur les suivants (l'isolitude reste garantie).
pub fn TEST_SERIAL_LOCKED() -> std::sync::MutexGuard<'static, ()> {
    match TEST_SERIAL.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}
