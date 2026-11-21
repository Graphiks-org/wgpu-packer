use std::sync::Arc;

use crate::native;

// BindGroup methods

#[no_mangle]
pub unsafe extern "C" fn wgpuBindGroupAddRef(bind_group: native::WGPUBindGroup) {
    crate::ffi_utils::contain_entry(
        "wgpuBindGroupAddRef",
        (),
        || {

        assert!(!bind_group.is_null(), "invalid bind group");
        Arc::increment_strong_count(bind_group);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuBindGroupRelease(bind_group: native::WGPUBindGroup) {
    crate::ffi_utils::contain_entry(
        "wgpuBindGroupRelease",
        (),
        || {

        assert!(!bind_group.is_null(), "invalid bind group");
        Arc::decrement_strong_count(bind_group);
        },
    )
}

// BindGroupLayout methods

#[no_mangle]
pub unsafe extern "C" fn wgpuBindGroupLayoutAddRef(bind_group_layout: native::WGPUBindGroupLayout) {
    crate::ffi_utils::contain_entry(
        "wgpuBindGroupLayoutAddRef",
        (),
        || {

        assert!(!bind_group_layout.is_null(), "invalid bind group layout");
        Arc::increment_strong_count(bind_group_layout);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuBindGroupLayoutRelease(
    bind_group_layout: native::WGPUBindGroupLayout,
) {
    crate::ffi_utils::contain_entry(
        "wgpuBindGroupLayoutRelease",
        (),
        || {

        assert!(!bind_group_layout.is_null(), "invalid bind group layout");
        Arc::decrement_strong_count(bind_group_layout);
        },
    )
}
