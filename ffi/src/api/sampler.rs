use std::sync::Arc;

use crate::native;

// Sampler methods

#[no_mangle]
pub unsafe extern "C" fn wgpuSamplerAddRef(sampler: native::WGPUSampler) {
    assert!(!sampler.is_null(), "invalid sampler");
    Arc::increment_strong_count(sampler);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuSamplerRelease(sampler: native::WGPUSampler) {
    assert!(!sampler.is_null(), "invalid sampler");
    Arc::decrement_strong_count(sampler);
}
