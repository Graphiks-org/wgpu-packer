use std::sync::Arc;

use crate::native;

// Sampler methods

#[no_mangle]
pub unsafe extern "C" fn wgpuSamplerAddRef(sampler: native::WGPUSampler) {
    crate::ffi_utils::contain_entry(
        "wgpuSamplerAddRef",
        (),
        || {

        assert!(!sampler.is_null(), "invalid sampler");
        Arc::increment_strong_count(sampler);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuSamplerRelease(sampler: native::WGPUSampler) {
    crate::ffi_utils::contain_entry(
        "wgpuSamplerRelease",
        (),
        || {

        assert!(!sampler.is_null(), "invalid sampler");
        Arc::decrement_strong_count(sampler);
        },
    )
}
