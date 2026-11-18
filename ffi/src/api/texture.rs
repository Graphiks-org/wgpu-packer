use std::{
    ffi::c_void,
    sync::Arc,
};
use crate::utils::string_view_into_label;

use crate::conv;
use crate::error::*;
use crate::native;
use crate::objects::*;

// Texture methods

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureCreateView(
    texture: native::WGPUTexture,
    descriptor: Option<&native::WGPUTextureViewDescriptor>,
) -> native::WGPUTextureView {
    crate::ffi_utils::contain_entry(
        "wgpuTextureCreateView",
        std::ptr::null(),
        || {

        let (texture_id, context, error_sink) = {
            let texture = texture.as_ref().expect("invalid texture");
            (texture.id, &texture.context, &texture.error_sink)
        };

        let desc = match descriptor {
            Some(descriptor) => wgc::resource::TextureViewDescriptor {
                usage: Some(conv::map_texture_usage_flags(descriptor.usage)),
                label: string_view_into_label(descriptor.label),
                format: conv::map_texture_format(descriptor.format),
                dimension: conv::map_texture_view_dimension(descriptor.dimension),
                range: wgt::ImageSubresourceRange {
                    aspect: conv::map_texture_aspect_native(descriptor.aspect)
                        .unwrap_or(wgt::TextureAspect::All),
                    base_mip_level: descriptor.baseMipLevel,
                    mip_level_count: match descriptor.mipLevelCount {
                        0 => panic!("invalid mipLevelCount"),
                        native::WGPU_MIP_LEVEL_COUNT_UNDEFINED => None,
                        _ => Some(descriptor.mipLevelCount),
                    },
                    base_array_layer: descriptor.baseArrayLayer,
                    array_layer_count: match descriptor.arrayLayerCount {
                        0 => panic!("invalid arrayLayerCount"),
                        native::WGPU_ARRAY_LAYER_COUNT_UNDEFINED => None,
                        _ => Some(descriptor.arrayLayerCount),
                    },
                },
            },
            None => wgc::resource::TextureViewDescriptor::default(),
        };

        let (texture_view_id, error) = context.texture_create_view(texture_id, &desc, None);
        if let Some(cause) = error {
            handle_error(error_sink, cause, None, "wgpuTextureCreateView");
        }

        Arc::into_raw(Arc::new(WGPUTextureViewImpl {
            context: context.clone(),
            id: texture_view_id,
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureDestroy(texture: native::WGPUTexture) {
    crate::ffi_utils::contain_entry(
        "wgpuTextureDestroy",
        (),
        || {

        let (texture_id, context) = {
            let texture = texture.as_ref().expect("invalid texture");
            (texture.id, &texture.context)
        };

        // Per spec, no error to report. Even calling destroy multiple times is valid.
        context.texture_destroy(texture_id);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetDepthOrArrayLayers(texture: native::WGPUTexture) -> u32 {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetDepthOrArrayLayers",
        0,
        || {

        let texture = texture.as_ref().expect("invalid texture");
        texture.data.size.depthOrArrayLayers
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetDimension(
    texture: native::WGPUTexture,
) -> native::WGPUTextureDimension {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetDimension",
        0,
        || {

        let texture = texture.as_ref().expect("invalid texture");
        texture.data.dimension
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetFormat(
    texture: native::WGPUTexture,
) -> native::WGPUTextureFormat {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetFormat",
        0,
        || {

        let texture = texture.as_ref().expect("invalid texture");
        texture.data.format
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetHeight(texture: native::WGPUTexture) -> u32 {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetHeight",
        0,
        || {

        let texture = texture.as_ref().expect("invalid texture");
        texture.data.size.height
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetMipLevelCount(texture: native::WGPUTexture) -> u32 {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetMipLevelCount",
        0,
        || {

        let texture = texture.as_ref().expect("invalid texture");
        texture.data.mip_level_count
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetSampleCount(texture: native::WGPUTexture) -> u32 {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetSampleCount",
        0,
        || {

        let texture = texture.as_ref().expect("invalid texture");
        texture.data.sample_count
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetTextureBindingViewDimension(
    texture: native::WGPUTexture,
) -> native::WGPUTextureViewDimension {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetTextureBindingViewDimension",
        0,
        || {

        // Resolved at creation packer-side (explicit chained value or derivation
        // rule): wgc 30 does not expose this concept (gfx-rs/wgpu#7428).
        let texture = texture.as_ref().expect("invalid texture");
        texture.data.texture_binding_view_dimension
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetUsage(
    texture: native::WGPUTexture,
) -> native::WGPUTextureUsage {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetUsage",
        0,
        || {

        let texture = texture.as_ref().expect("invalid texture");
        texture.data.usage
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetWidth(texture: native::WGPUTexture) -> u32 {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetWidth",
        0,
        || {

        let texture = texture.as_ref().expect("invalid texture");
        texture.data.size.width
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureGetNativeMetalTexture(
    texture: native::WGPUTexture,
) -> *mut c_void {
    crate::ffi_utils::contain_entry(
        "wgpuTextureGetNativeMetalTexture",
        std::ptr::null_mut(),
        || {

        #[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "metal"))]
        {
            let texture = texture.as_ref().expect("invalid texture");
            let hal_texture = texture
                .context
                .texture_as_hal::<hal::api::Metal>(texture.id);
            if let Some(hal_texture) = hal_texture {
                return hal_texture.raw_handle() as *const _ as *mut c_void;
            }
            std::ptr::null_mut()
        }
        #[cfg(not(all(any(target_os = "ios", target_os = "macos"), feature = "metal")))]
        {
            let _ = texture;
            std::ptr::null_mut()
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureAddRef(texture: native::WGPUTexture) {
    crate::ffi_utils::contain_entry(
        "wgpuTextureAddRef",
        (),
        || {

        assert!(!texture.is_null(), "invalid texture");
        Arc::increment_strong_count(texture);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuTextureRelease(texture: native::WGPUTexture) {
    crate::ffi_utils::contain_entry(
        "wgpuTextureRelease",
        (),
        || {

        assert!(!texture.is_null(), "invalid texture");
        Arc::decrement_strong_count(texture);
        },
    )
}

// TextureView methods

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureViewAddRef(texture_view: native::WGPUTextureView) {
    crate::ffi_utils::contain_entry(
        "wgpuTextureViewAddRef",
        (),
        || {

        assert!(!texture_view.is_null(), "invalid texture");
        Arc::increment_strong_count(texture_view);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuTextureViewRelease(texture_view: native::WGPUTextureView) {
    crate::ffi_utils::contain_entry(
        "wgpuTextureViewRelease",
        (),
        || {

        assert!(!texture_view.is_null(), "invalid texture");
        Arc::decrement_strong_count(texture_view);
        },
    )
}
