use std::sync::Arc;

use crate::native;

// ShaderModule methods

#[no_mangle]
pub unsafe extern "C" fn wgpuShaderModuleAddRef(shader_module: native::WGPUShaderModule) {
    assert!(!shader_module.is_null(), "invalid shader module");
    Arc::increment_strong_count(shader_module);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuShaderModuleRelease(shader_module: native::WGPUShaderModule) {
    assert!(!shader_module.is_null(), "invalid shader module");
    Arc::decrement_strong_count(shader_module);
}
