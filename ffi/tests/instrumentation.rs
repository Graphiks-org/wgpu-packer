mod common;

#[test]
#[cfg(debug_assertions)] // sonde disponible en debug uniquement (cf. ffi/ffi/wgpu.h)
fn panic_probe_is_contained() {
    assert_eq!(unsafe { wgpu_packer::ffi_utils::wgpuDebugTriggerPanic() }, 1);
}
