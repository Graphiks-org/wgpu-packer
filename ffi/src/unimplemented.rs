//! Stubs volontairement non implémentés : chaque appel lève `unimplemented!()`
//! (abort côté consommateur, GUIDE §6). Raisons, vérifiées dans les sources
//! de wgpu-core/wgpu-types/naga 29 :
//!
//! - `wgpuInstanceGetWGSLLanguageFeatures` / `wgpuInstanceHasWGSLLanguageFeature`
//!   / `wgpuSupportedWGSLLanguageFeaturesFreeMembers` : aucune API wgt/naga 29
//!   listant les features de langage WGSL supportées.
//! - `wgpuShaderModuleGetCompilationInfo` : wgc 29 ne conserve pas les
//!   messages de compilation des shader modules (absent aussi de 29.0.3).
//! - `wgpuExternalTextureAddRef` / `wgpuExternalTextureRelease` : type
//!   incréable via l'ABI — webgpu.h : "Creation of WGPUExternalTexture is
//!   extremely implementation-dependent and not defined in this header"
//!   (voie vidéo différée, spec §5).

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
