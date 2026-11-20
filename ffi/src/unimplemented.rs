//! Deliberately unimplemented stubs: every call raises `unimplemented!()`
//! (abort on the consumer side, GUIDE §6). Reasons, verified in the sources
//! of wgpu-core/wgpu-types/naga 30:
//!
//! - `wgpuInstanceGetWGSLLanguageFeatures` / `wgpuInstanceHasWGSLLanguageFeature`
//!   / `wgpuSupportedWGSLLanguageFeaturesFreeMembers`: no wgt/naga 29 API
//!   listing the supported WGSL language features.

use crate::native;

#[no_mangle]
pub extern "C" fn wgpuInstanceGetWGSLLanguageFeatures(
    _instance: native::WGPUInstance,
    _features: *mut native::WGPUSupportedWGSLLanguageFeatures,
) -> native::WGPUStatus {
    crate::ffi_utils::contain_entry(
        "wgpuInstanceGetWGSLLanguageFeatures",
        native::WGPUStatus_Error,
        || {

        unimplemented!();
        },
    )
}

#[no_mangle]
pub extern "C" fn wgpuInstanceHasWGSLLanguageFeature(
    _instance: native::WGPUInstance,
    _feature: native::WGPUWGSLLanguageFeatureName,
) -> native::WGPUBool {
    // webgpu.h declares WGPUBool (c_uint): the stub used to return a Rust
    // bool — a 1-byte value where the caller reads 4 (ABI mismatch).
    crate::ffi_utils::contain_entry(
        "wgpuInstanceHasWGSLLanguageFeature",
        0,
        || {

        unimplemented!();
        },
    )
}

#[no_mangle]
pub extern "C" fn wgpuSupportedWGSLLanguageFeaturesFreeMembers(
    _supported_wgsl_language_features: native::WGPUSupportedWGSLLanguageFeatures,
) {
    crate::ffi_utils::contain_entry(
        "wgpuSupportedWGSLLanguageFeaturesFreeMembers",
        (),
        || {

        unimplemented!();
        },
    )
}


