use std::panic::{catch_unwind, AssertUnwindSafe};

/// Exécute `f` en contenant toute panique. Retourne `None` si une panique a été
/// interceptée — aucune panique Rust ne franchit l'ABI C.
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

/// Sonde de test : panique volontairement contenue par le wrapper FFI.
/// Debug uniquement. Retourne 1 si la panique a été contenue, 0 sinon.
#[cfg(debug_assertions)]
#[no_mangle]
pub unsafe extern "C" fn wgpuDebugTriggerPanic() -> u32 {
    match catch_unwind_contained("wgpuDebugTriggerPanic", || panic!("sonde orchestree par l'oracle")) {
        Some(_) => 0, // pas de panique : la sonde est cassée
        None => 1,    // panique bien contenue
    }
}
