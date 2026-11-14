#[allow(clippy::missing_safety_doc)]

use crate::conv::map_surface_configuration;
use std::{
    mem::{self},
    sync::{atomic, Arc},
};

use crate::conv;
use crate::follow_chain;
use crate::error::*;
use crate::native;
use crate::objects::*;

// Surface methods

#[no_mangle]
pub unsafe extern "C" fn wgpuSurfaceConfigure(
    surface: native::WGPUSurface,
    config: Option<&native::WGPUSurfaceConfiguration>,
) {
    let surface = surface.as_ref().expect("invalid surface");
    let config = config.expect("invalid config");
    let device = config
        .device
        .as_ref()
        .expect("invalid device for surface configuration");
    let context = &device.context;

    let surface_config = follow_chain!(map_surface_configuration(
        (config),
        WGPUSType_SurfaceConfigurationExtras => native::WGPUSurfaceConfigurationExtras
    ));

    match context.surface_configure(surface.id, device.id, &surface_config) {
        Some(cause) => handle_error_fatal(cause, "wgpuSurfaceConfigure"),
        None => {
            let mut surface_data_guard = surface.data.lock();
            *surface_data_guard = Some(SurfaceData {
                error_sink: device.error_sink.clone(),
                texture_data: TextureData {
                    usage: config.usage,
                    dimension: native::WGPUTextureDimension_2D,
                    format: config.format,
                    mip_level_count: 1,
                    size: native::WGPUExtent3D {
                        width: config.width,
                        height: config.height,
                        depthOrArrayLayers: 1,
                    },
                    sample_count: 1,
                    texture_binding_view_dimension: conv::resolve_texture_binding_view_dimension(
                        native::WGPUTextureDimension_2D,
                        1,
                        1,
                        None,
                    ),
                },
            });
            surface
                .has_surface_presented
                .store(false, atomic::Ordering::SeqCst);
        }
    };
}

#[no_mangle]
pub unsafe extern "C" fn wgpuSurfaceGetCapabilities(
    surface: native::WGPUSurface,
    adapter: native::WGPUAdapter,
    capabilities: Option<&mut native::WGPUSurfaceCapabilities>,
) -> native::WGPUStatus {
    let (adapter_id, context) = {
        let adapter = adapter.as_ref().expect("invalid adapter");
        (adapter.id, &adapter.context)
    };
    let surface_id = surface.as_ref().expect("invalid surface").id;
    let capabilities = capabilities.expect("invalid return pointer \"capabilities\"");

    let caps = match context.surface_get_capabilities(surface_id, adapter_id) {
        Ok(caps) => caps,
        Err(
            wgc::instance::GetSurfaceSupportError::FailedToRetrieveSurfaceCapabilitiesForAdapter,
        ) => wgt::SurfaceCapabilities::default(),
        Err(cause) => {
            log::warn!("Surface Capabilities error: {}", cause);
            return native::WGPUStatus_Error;
        }
    };

    capabilities.usages =
        conv::to_native_texture_usage_flags(caps.usages) as native::WGPUTextureUsage;

    let formats = caps
        .formats
        .iter()
        // some texture formats are not in webgpu.h and
        // conv::to_native_texture_format returns None for them.
        // so, filter them out.
        .filter_map(|f| conv::to_native_texture_format(*f))
        .collect::<Vec<_>>();

    if !formats.is_empty() {
        let mut array = formats.into_boxed_slice();
        capabilities.formats = array.as_mut_ptr();
        capabilities.formatCount = array.len();
        mem::forget(array);
    } else {
        capabilities.formats = std::ptr::null_mut();
        capabilities.formatCount = 0;
    }

    let present_modes = caps
        .present_modes
        .iter()
        .filter_map(|f| conv::to_native_present_mode(*f))
        .collect::<Vec<_>>();

    if !present_modes.is_empty() {
        let mut array = present_modes.into_boxed_slice();
        capabilities.presentModes = array.as_mut_ptr();
        capabilities.presentModeCount = array.len();
        mem::forget(array);
    } else {
        capabilities.presentModes = std::ptr::null_mut();
        capabilities.presentModeCount = 0;
    }

    let alpha_modes = caps
        .alpha_modes
        .iter()
        .map(|f| conv::to_native_composite_alpha_mode(*f))
        .collect::<Vec<_>>();

    if !alpha_modes.is_empty() {
        let mut array = alpha_modes.into_boxed_slice();
        capabilities.alphaModes = array.as_mut_ptr();
        capabilities.alphaModeCount = array.len();
        mem::forget(array);
    } else {
        capabilities.alphaModes = std::ptr::null_mut();
        capabilities.alphaModeCount = 0;
    }

    native::WGPUStatus_Success
}

#[no_mangle]
pub unsafe extern "C" fn wgpuSurfaceGetCurrentTexture(
    surface: native::WGPUSurface,
    surface_texture: Option<&mut native::WGPUSurfaceTexture>,
) {
    let surface = surface.as_ref().expect("invalid surface");
    let context = &surface.context;
    let surface_texture = surface_texture.expect("invalid return pointer \"surface_texture\"");

    let surface_data_guard = surface.data.lock();
    let surface_data = match surface_data_guard.as_ref() {
        Some(surface_data) => surface_data,
        None => handle_error_fatal(
            wgc::present::SurfaceError::NotConfigured,
            "wgpuSurfaceGetCurrentTexture",
        ),
    };

    match context.surface_get_current_texture(surface.id, None) {
        Ok(wgc::present::SurfaceOutput { status, texture }) => {
            surface
                .has_surface_presented
                .store(false, atomic::Ordering::SeqCst);
            surface_texture.status = match status {
                wgt::SurfaceStatus::Good => {
                    native::WGPUSurfaceGetCurrentTextureStatus_SuccessOptimal
                }
                wgt::SurfaceStatus::Suboptimal => {
                    native::WGPUSurfaceGetCurrentTextureStatus_SuccessSuboptimal
                }
                wgt::SurfaceStatus::Timeout => native::WGPUSurfaceGetCurrentTextureStatus_Timeout,
                wgt::SurfaceStatus::Outdated => native::WGPUSurfaceGetCurrentTextureStatus_Outdated,
                wgt::SurfaceStatus::Lost => native::WGPUSurfaceGetCurrentTextureStatus_Lost,
                wgt::SurfaceStatus::Occluded => native::WGPUSurfaceGetCurrentTextureStatus_Occluded,
                wgt::SurfaceStatus::Validation => {
                    log::error!("Surface validation error");
                    native::WGPUSurfaceGetCurrentTextureStatus_Error
                }
            };
            surface_texture.texture = match texture {
                Some(texture_id) => Arc::into_raw(Arc::new(WGPUTextureImpl {
                    context: context.clone(),
                    id: texture_id,
                    error_sink: surface_data.error_sink.clone(),
                    data: surface_data.texture_data,
                    surface_id: Some(surface.id),
                    has_surface_presented: surface.has_surface_presented.clone(),
                })),
                None => std::ptr::null_mut(),
            };
        }
        Err(cause) => handle_error_fatal(cause, "wgpuSurfaceGetCurrentTexture"),
    };
}

#[no_mangle]
pub unsafe extern "C" fn wgpuSurfacePresent(surface: native::WGPUSurface) -> native::WGPUStatus {
    let surface = surface.as_ref().expect("invalid surface");
    let context = &surface.context;

    let _status = match context.surface_present(surface.id) {
        Ok(status) => status,
        Err(cause) => {
            log::warn!("Presentation error: {}", cause);
            return native::WGPUStatus_Error;
        }
    };

    surface
        .has_surface_presented
        .store(true, atomic::Ordering::SeqCst);

    native::WGPUStatus_Success
}

#[no_mangle]
pub unsafe extern "C" fn wgpuSurfaceUnconfigure(surface: native::WGPUSurface) {
    let surface = surface.as_ref().expect("invalid surface");
    let mut surface_data_guard = surface.data.lock();
    let _ = surface_data_guard.take(); // drop SurfaceData
    surface
        .has_surface_presented
        .store(false, atomic::Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn wgpuSurfaceAddRef(surface: native::WGPUSurface) {
    assert!(!surface.is_null(), "invalid surface");
    Arc::increment_strong_count(surface);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuSurfaceRelease(surface: native::WGPUSurface) {
    assert!(!surface.is_null(), "invalid surface");
    Arc::decrement_strong_count(surface);
}

// SurfaceCapabilities methods

#[no_mangle]
pub unsafe extern "C" fn wgpuSurfaceCapabilitiesFreeMembers(
    capabilities: native::WGPUSurfaceCapabilities,
) {
    if !capabilities.formats.is_null() && capabilities.formatCount > 0 {
        drop(Vec::from_raw_parts(
            capabilities.formats as *mut native::WGPUTextureFormat,
            capabilities.formatCount,
            capabilities.formatCount,
        ));
    }
    if !capabilities.presentModes.is_null() && capabilities.presentModeCount > 0 {
        drop(Vec::from_raw_parts(
            capabilities.presentModes as *mut native::WGPUPresentMode,
            capabilities.presentModeCount,
            capabilities.presentModeCount,
        ));
    }
    if !capabilities.alphaModes.is_null() && capabilities.alphaModeCount > 0 {
        drop(Vec::from_raw_parts(
            capabilities.alphaModes as *mut native::WGPUCompositeAlphaMode,
            capabilities.alphaModeCount,
            capabilities.alphaModeCount,
        ));
    }
}
