//! Deliberately unimplemented stubs: every call raises `unimplemented!()`
//! (abort on the consumer side, GUIDE §6). Reasons, verified in the sources
//! of wgpu-core/wgpu-types/naga 29:
//!
//! - `wgpuInstanceGetWGSLLanguageFeatures` / `wgpuInstanceHasWGSLLanguageFeature`
//!   / `wgpuSupportedWGSLLanguageFeaturesFreeMembers`: no wgt/naga 29 API
//!   listing the supported WGSL language features.
//! - `wgpuShaderModuleGetCompilationInfo`: wgc 29 does not keep the
//!   compilation messages of shader modules (also absent from 29.0.3).
//! - `wgpuExternalTextureAddRef` / `wgpuExternalTextureRelease`: type
//!   that cannot be ref-counted through the ABI — webgpu.h: "Creation of WGPUExternalTexture is
//!   extremely implementation-dependent and not defined in this header"
//!   (deferred video path, spec §5).

use crate::native;

#[no_mangle]
pub extern "C" fn wgpuInstanceGetWGSLLanguageFeatures(
    _instance: native::WGPUInstance,
    _features: *mut native::WGPUSupportedWGSLLanguageFeatures,
) -> native::WGPUStatus {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn wgpuInstanceHasWGSLLanguageFeature(
    _instance: native::WGPUInstance,
    _feature: native::WGPUWGSLLanguageFeatureName,
) -> bool {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn wgpuSupportedWGSLLanguageFeaturesFreeMembers(
    _supported_wgsl_language_features: native::WGPUSupportedWGSLLanguageFeatures,
) {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn wgpuShaderModuleGetCompilationInfo(
    _shader_module: native::WGPUShaderModule,
    _callback: native::WGPUCompilationInfoCallbackInfo,
) -> native::WGPUFuture {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn wgpuExternalTextureAddRef(_external_texture: native::WGPUExternalTexture) {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn wgpuExternalTextureRelease(_external_texture: native::WGPUExternalTexture) {
    unimplemented!();
}
