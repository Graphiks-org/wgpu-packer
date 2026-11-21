//! SetLabel: the label hint is accepted and ignored.
//!
//! wgpu-core 30 exposes no post-creation label change (no
//! `*_set_label` on `Global`): the labels of error messages remain
//! the ones passed at creation. An explicit no-op is better than an abort for
//! a purely informational hint — the ABI stays safe for consumers.

use crate::native;

#[no_mangle]
pub unsafe extern "C" fn wgpuBindGroupSetLabel(
    _bind_group: native::WGPUBindGroup,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuBindGroupSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBindGroupLayoutSetLabel(
    _bind_group_layout: native::WGPUBindGroupLayout,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuBindGroupLayoutSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferSetLabel(
    _buffer: native::WGPUBuffer,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuBufferSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandBufferSetLabel(
    _command_buffer: native::WGPUCommandBuffer,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandBufferSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderSetLabel(
    _command_encoder: native::WGPUCommandEncoder,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderSetLabel(
    _compute_pass_encoder: native::WGPUComputePassEncoder,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuComputePassEncoderSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePipelineSetLabel(
    _compute_pipeline: native::WGPUComputePipeline,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuComputePipelineSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceSetLabel(
    _device: native::WGPUDevice,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuExternalTextureSetLabel(
    _external_texture: native::WGPUExternalTexture,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuExternalTextureSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuPipelineLayoutSetLabel(
    _pipeline_layout: native::WGPUPipelineLayout,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuPipelineLayoutSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQuerySetSetLabel(
    _query_set: native::WGPUQuerySet,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuQuerySetSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQueueSetLabel(
    _queue: native::WGPUQueue,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuQueueSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleSetLabel(
    _render_bundle: native::WGPURenderBundle,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderSetLabel(
    _render_bundle_encoder: native::WGPURenderBundleEncoder,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetLabel(
    _render_pass_encoder: native::WGPURenderPassEncoder,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPipelineSetLabel(
    _render_pipeline: native::WGPURenderPipeline,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPipelineSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuSamplerSetLabel(
    _sampler: native::WGPUSampler,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuSamplerSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuShaderModuleSetLabel(
    _shader_module: native::WGPUShaderModule,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuShaderModuleSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuSurfaceSetLabel(
    _surface: native::WGPUSurface,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuSurfaceSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureSetLabel(
    _texture: native::WGPUTexture,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuTextureSetLabel",
        (),
        || {

        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTextureViewSetLabel(
    _texture_view: native::WGPUTextureView,
    _label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuTextureViewSetLabel",
        (),
        || {

        },
    )
}
