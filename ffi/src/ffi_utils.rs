use std::panic::{catch_unwind, AssertUnwindSafe};

/// Runs `f` while containing any panic. Returns `None` if a panic was
/// caught — no Rust panic crosses the C ABI.
pub fn catch_unwind_contained<T>(name: &str, f: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(f))
        .map_err(|payload| {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<panique non-textuelle>".into());
            eprintln!("[wgpu_packer] panique contenue dans {name}: {msg}");
        })
        .ok()
}

/// Test probe: panic deliberately contained by the FFI wrapper.
/// Debug only. Returns 1 if the panic was contained, 0 otherwise.
#[cfg(debug_assertions)]
#[no_mangle]
pub unsafe extern "C" fn wgpuDebugTriggerPanic() -> u32 {
    match catch_unwind_contained("wgpuDebugTriggerPanic", || panic!("sonde orchestree par l'oracle")) {
        Some(_) => 0, // no panic: the probe is broken
        None => 1,    // panic properly contained
    }
}
