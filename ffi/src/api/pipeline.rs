use std::sync::Arc;

use crate::native;

// PipelineLayout methods

#[no_mangle]
pub unsafe extern "C" fn wgpuPipelineLayoutAddRef(pipeline_layout: native::WGPUPipelineLayout) {
    crate::ffi_utils::contain_entry(
        "wgpuPipelineLayoutAddRef",
        (),
        || {

        assert!(!pipeline_layout.is_null(), "invalid pipeline layout");
        Arc::increment_strong_count(pipeline_layout);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuPipelineLayoutRelease(pipeline_layout: native::WGPUPipelineLayout) {
    crate::ffi_utils::contain_entry(
        "wgpuPipelineLayoutRelease",
        (),
        || {

        assert!(!pipeline_layout.is_null(), "invalid pipeline layout");
        Arc::decrement_strong_count(pipeline_layout);
        },
    )
}
