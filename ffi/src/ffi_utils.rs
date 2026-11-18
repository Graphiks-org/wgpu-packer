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
                .unwrap_or_else(|| "<non-textual panic>".into());
            log::error!("panic contained in {name}: {msg}");
        })
        .ok()
}

/// The uniform FFI boundary: runs the entry point's body with any panic
/// contained (no Rust panic crosses `extern "C"` — that is a process
/// abort). On a contained panic the call logs and returns `fallback`:
/// the safest possible answer for its return type (NULL handle, error
/// status, 0, false, the null future...).
///
/// `catch_unwind` is free when nothing panics; `panic = "unwind"` is the
/// release profile of this workspace.
pub fn contain_entry<T>(name: &str, fallback: T, f: impl FnOnce() -> T) -> T {
    match catch_unwind_contained(name, f) {
        Some(value) => value,
        None => fallback,
    }
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
