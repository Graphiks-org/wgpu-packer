use smallvec::SmallVec;
use std::{
    num::NonZeroU64,
    sync::{atomic::Ordering, Arc},
};
use crate::utils::{
    make_slice,
    string_view_into_label, string_view_into_str,
};
use wgc::command::bundle_ffi;

use crate::conv;
use crate::error::*;
use crate::native;
use crate::objects::*;

/// Counts a draw call of the pass against its `maxDrawCount` (C contract) and
/// delivers the WebGPU validation error when the cap is exceeded —
/// wgpu-core 30 no longer has this field, the packer carries the semantics: each
/// `draw*` beyond the cap is rejected (encoding continues, like a wgc
/// encoding validation) and the error joins the current error scope.
fn check_draw_count(pass: &WGPURenderPassEncoderImpl, operation: &'static str) {
    let drawn = pass.draw_count.fetch_add(1, Ordering::SeqCst) + 1;
    if drawn > pass.max_draw_count {
        handle_error(
            &pass.error_sink,
            MaxDrawCountExceeded {
                drawn,
                allowed: pass.max_draw_count,
            },
            None,
            operation,
        );
    }
}

/// Exceeding the `maxDrawCount` of a render pass (webgpu.h C contract).
#[derive(Debug)]
struct MaxDrawCountExceeded {
    drawn: u64,
    allowed: u64,
}
impl std::fmt::Display for MaxDrawCountExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "draw count {} exceeds the pass maxDrawCount {}",
            self.drawn, self.allowed,
        )
    }
}
impl std::error::Error for MaxDrawCountExceeded {}

// RenderBundle methods

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleAddRef(render_bundle: native::WGPURenderBundle) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleAddRef",
        (),
        || {

        assert!(!render_bundle.is_null(), "invalid render bundle");
        Arc::increment_strong_count(render_bundle);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleRelease(render_bundle: native::WGPURenderBundle) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleRelease",
        (),
        || {

        assert!(!render_bundle.is_null(), "invalid render bundle");
        Arc::decrement_strong_count(render_bundle);
        },
    )
}

// RenderBundleEncoder methods

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderDraw(
    bundle: native::WGPURenderBundleEncoder,
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderDraw",
        (),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.expect("invalid render bundle");
        let encoder = encoder.as_mut().unwrap();

        bundle_ffi::wgpu_render_bundle_draw(
            encoder,
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        );
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderDrawIndexed(
    bundle: native::WGPURenderBundleEncoder,
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderDrawIndexed",
        (),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.expect("invalid render bundle");
        let encoder = encoder.as_mut().unwrap();

        bundle_ffi::wgpu_render_bundle_draw_indexed(
            encoder,
            index_count,
            instance_count,
            first_index,
            base_vertex,
            first_instance,
        );
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderDrawIndexedIndirect(
    bundle: native::WGPURenderBundleEncoder,
    indirect_buffer: native::WGPUBuffer,
    indirect_offset: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderDrawIndexedIndirect",
        (),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let indirect_buffer_id = indirect_buffer
            .as_ref()
            .expect("invalid indirect buffer")
            .id;
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.expect("invalid render bundle");
        let encoder = encoder.as_mut().unwrap();

        bundle_ffi::wgpu_render_bundle_draw_indexed_indirect(
            encoder,
            indirect_buffer_id,
            indirect_offset,
        );
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderDrawIndirect(
    bundle: native::WGPURenderBundleEncoder,
    indirect_buffer: native::WGPUBuffer,
    indirect_offset: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderDrawIndirect",
        (),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let indirect_buffer_id = indirect_buffer
            .as_ref()
            .expect("invalid indirect buffer")
            .id;
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.expect("invalid render bundle");
        let encoder = encoder.as_mut().unwrap();

        bundle_ffi::wgpu_render_bundle_draw_indirect(encoder, indirect_buffer_id, indirect_offset);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderFinish(
    bundle: native::WGPURenderBundleEncoder,
    descriptor: Option<&native::WGPURenderBundleDescriptor>,
) -> native::WGPURenderBundle {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderFinish",
        std::ptr::null(),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let context = &bundle.context;
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.take().expect("invalid render bundle");
        let mut encoder = Box::from_raw(encoder);

        let desc = match descriptor {
            Some(descriptor) => wgt::RenderBundleDescriptor {
                label: string_view_into_label(descriptor.label),
            },
            None => wgt::RenderBundleDescriptor::default(),
        };

        let (render_bundle_id, error) = context.render_bundle_encoder_finish(&mut *encoder, &desc, None);
        if let Some(cause) = error {
            // WebGPU semantics: content validation of a render bundle is
            // a record-time error, delivered to the sink (current error scope or
            // uncaptured error handler) — never fatal. wgc assigns an id
            // `Fallible::Invalid`: finish returns a valid handle for invalid
            // content, and a later execution (executeBundles) will report
            // the error normally.
            handle_error(
                &bundle.error_sink,
                cause,
                None,
                "wgpuRenderBundleEncoderFinish",
            );
        }

        Arc::into_raw(Arc::new(WGPURenderBundleImpl {
            context: context.clone(),
            id: render_bundle_id,
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderInsertDebugMarker(
    _bundle: native::WGPURenderBundleEncoder,
    _marker_label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderInsertDebugMarker",
        (),
        || {

        // These functions are not implemented in wgpu-core, and the API is incompatible with the new WGPUStringView.
        // Commenting out until it's actually implemented.

        // let bundle = bundle.as_ref().expect("invalid render bundle");
        // let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        // let encoder = encoder.expect("invalid render bundle");
        // let encoder = encoder.as_mut().unwrap();

        // bundle_ffi::wgpu_render_bundle_insert_debug_marker(encoder, marker_label);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderPopDebugGroup(
    _bundle: native::WGPURenderBundleEncoder,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderPopDebugGroup",
        (),
        || {

        // These functions are not implemented in wgpu-core, and the API is incompatible with the new WGPUStringView.
        // Commenting out until it's actually implemented.

        // let bundle = bundle.as_ref().expect("invalid render bundle");
        // let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        // let encoder = encoder.expect("invalid render bundle");
        // let encoder = encoder.as_mut().unwrap();

        // bundle_ffi::wgpu_render_bundle_pop_debug_group(encoder);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderPushDebugGroup(
    _bundle: native::WGPURenderBundleEncoder,
    _group_label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderPushDebugGroup",
        (),
        || {

        // These functions are not implemented in wgpu-core, and the API is incompatible with the new WGPUStringView.
        // Commenting out until it's actually implemented.

        // let bundle = bundle.as_ref().expect("invalid render bundle");
        // let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        // let encoder = encoder.expect("invalid render bundle");
        // let encoder = encoder.as_mut().unwrap();

        // bundle_ffi::wgpu_render_bundle_push_debug_group(encoder, group_label);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderSetBindGroup(
    bundle: native::WGPURenderBundleEncoder,
    group_index: u32,
    group: native::WGPUBindGroup,
    dynamic_offset_count: usize,
    dynamic_offsets: *const u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderSetBindGroup",
        (),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let bind_group_id = group.as_ref().map(|bg| bg.id);
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.expect("invalid render bundle");
        let encoder = encoder.as_mut().unwrap();

        bundle_ffi::wgpu_render_bundle_set_bind_group(
            encoder,
            group_index,
            bind_group_id,
            dynamic_offsets,
            dynamic_offset_count,
        );
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderSetIndexBuffer(
    bundle: native::WGPURenderBundleEncoder,
    buffer: native::WGPUBuffer,
    format: native::WGPUIndexFormat,
    offset: u64,
    size: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderSetIndexBuffer",
        (),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let buffer_id = buffer.as_ref().expect("invalid buffer").id;
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.expect("invalid render bundle");
        let encoder = encoder.as_mut().unwrap();

        bundle_ffi::wgpu_render_bundle_set_index_buffer(
            encoder,
            buffer_id,
            conv::map_index_format(format).expect("invalid index format"),
            offset,
            match size {
                0 => panic!("invalid size"),
                conv::WGPU_WHOLE_SIZE => None,
                _ => Some(NonZeroU64::new_unchecked(size)),
            },
        );
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderSetPipeline(
    bundle: native::WGPURenderBundleEncoder,
    pipeline: native::WGPURenderPipeline,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderSetPipeline",
        (),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let pipeline_id = pipeline.as_ref().expect("invalid render pipeline").id;
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.expect("invalid render bundle");
        let encoder = encoder.as_mut().unwrap();

        bundle_ffi::wgpu_render_bundle_set_pipeline(encoder, pipeline_id);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderSetVertexBuffer(
    bundle: native::WGPURenderBundleEncoder,
    slot: u32,
    buffer: native::WGPUBuffer,
    offset: u64,
    size: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderSetVertexBuffer",
        (),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let buffer_id = buffer.as_ref().map(|buffer| buffer.id);
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.expect("invalid render bundle");
        let encoder = encoder.as_mut().unwrap();

        bundle_ffi::wgpu_render_bundle_set_vertex_buffer(
            encoder,
            slot,
            buffer_id,
            offset,
            match size {
                0 => panic!("invalid size"),
                conv::WGPU_WHOLE_SIZE => None,
                _ => Some(NonZeroU64::new_unchecked(size)),
            },
        );
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderAddRef(
    render_bundle_encoder: native::WGPURenderBundleEncoder,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderAddRef",
        (),
        || {

        assert!(
            !render_bundle_encoder.is_null(),
            "invalid render bundle encoder"
        );
        Arc::increment_strong_count(render_bundle_encoder);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderRelease(
    render_bundle_encoder: native::WGPURenderBundleEncoder,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderRelease",
        (),
        || {

        assert!(
            !render_bundle_encoder.is_null(),
            "invalid render bundle encoder"
        );
        Arc::decrement_strong_count(render_bundle_encoder);
        },
    )
}

// RenderPassEncoder methods

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderBeginOcclusionQuery(
    pass: native::WGPURenderPassEncoder,
    query_index: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderBeginOcclusionQuery",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_begin_occlusion_query(encoder, query_index)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderBeginOcclusionQuery",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderDraw(
    pass: native::WGPURenderPassEncoder,
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderDraw",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        check_draw_count(pass, "wgpuRenderPassEncoderDraw");
        match pass
            .context
            .render_pass_draw(
                encoder,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            )
        {
            Ok(()) => (),
            Err(cause) => handle_error(&pass.error_sink, cause, None, "wgpuRenderPassEncoderDraw"),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderDrawIndexed(
    pass: native::WGPURenderPassEncoder,
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderDrawIndexed",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        check_draw_count(pass, "wgpuRenderPassEncoderDrawIndexed");
        match pass.context.render_pass_draw_indexed(
            encoder,
            index_count,
            instance_count,
            first_index,
            base_vertex,
            first_instance,
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderDrawIndexed",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderDrawIndexedIndirect(
    pass: native::WGPURenderPassEncoder,
    indirect_buffer: native::WGPUBuffer,
    indirect_offset: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderDrawIndexedIndirect",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let indirect_buffer_id = indirect_buffer
            .as_ref()
            .expect("invalid indirect buffer")
            .id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        check_draw_count(pass, "wgpuRenderPassEncoderDrawIndexedIndirect");
        match pass.context.render_pass_draw_indexed_indirect(
            encoder,
            indirect_buffer_id,
            indirect_offset,
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderDrawIndexedIndirect",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderDrawIndirect(
    pass: native::WGPURenderPassEncoder,
    indirect_buffer: native::WGPUBuffer,
    indirect_offset: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderDrawIndirect",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let indirect_buffer_id = indirect_buffer
            .as_ref()
            .expect("invalid indirect buffer")
            .id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        check_draw_count(pass, "wgpuRenderPassEncoderDrawIndirect");
        match pass
            .context
            .render_pass_draw_indirect(encoder, indirect_buffer_id, indirect_offset)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderDrawIndirect",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderEnd(pass: native::WGPURenderPassEncoder) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderEnd",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_end(encoder) {
            Ok(()) => (),
            Err(cause) => handle_error(&pass.error_sink, cause, None, "wgpuRenderPassEncoderEnd"),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderEndOcclusionQuery(
    pass: native::WGPURenderPassEncoder,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderEndOcclusionQuery",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_end_occlusion_query(encoder) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderEndOcclusionQuery",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderExecuteBundles(
    pass: native::WGPURenderPassEncoder,
    bundle_count: usize,
    bundles: *const native::WGPURenderBundle,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderExecuteBundles",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let bundle_ids = make_slice(bundles, bundle_count)
            .iter()
            .map(|v| v.as_ref().expect("invalid render bundle").id)
            .collect::<SmallVec<[_; 4]>>();
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_execute_bundles(encoder, &bundle_ids)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderExecuteBundles",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderInsertDebugMarker(
    pass: native::WGPURenderPassEncoder,
    marker_label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderInsertDebugMarker",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_insert_debug_marker(
            encoder,
            string_view_into_str(marker_label).unwrap_or(""),
            0,
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderInsertDebugMarker",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderPopDebugGroup(pass: native::WGPURenderPassEncoder) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderPopDebugGroup",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_pop_debug_group(encoder) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderPopDebugGroup",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderPushDebugGroup(
    pass: native::WGPURenderPassEncoder,
    group_label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderPushDebugGroup",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_push_debug_group(
            encoder,
            string_view_into_str(group_label).unwrap_or(""),
            0,
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderPushDebugGroup",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetBindGroup(
    pass: native::WGPURenderPassEncoder,
    group_index: u32,
    bind_group: native::WGPUBindGroup,
    dynamic_offset_count: usize,
    dynamic_offsets: *const u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetBindGroup",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let bind_group_id = bind_group.as_ref().map(|bg| bg.id);
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_set_bind_group(
            encoder,
            group_index,
            bind_group_id,
            make_slice(dynamic_offsets, dynamic_offset_count),
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderSetBindGroup",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetBlendConstant(
    pass: native::WGPURenderPassEncoder,
    color: Option<&native::WGPUColor>,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetBlendConstant",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_set_blend_constant(encoder, conv::map_color(color.expect("invalid color")))
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderSetBlendConstant",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetIndexBuffer(
    pass: native::WGPURenderPassEncoder,
    buffer: native::WGPUBuffer,
    index_format: native::WGPUIndexFormat,
    offset: u64,
    size: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetIndexBuffer",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let buffer_id = buffer.as_ref().expect("invalid buffer").id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_set_index_buffer(
            encoder,
            buffer_id,
            conv::map_index_format(index_format).expect("invalid index format"),
            offset,
            match size {
                0 => panic!("invalid size"),
                conv::WGPU_WHOLE_SIZE => None,
                _ => Some(NonZeroU64::new_unchecked(size)),
            },
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderSetIndexBuffer",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetPipeline(
    pass: native::WGPURenderPassEncoder,
    render_pipeline: native::WGPURenderPipeline,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetPipeline",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let render_pipeline_id = render_pipeline
            .as_ref()
            .expect("invalid render pipeline")
            .id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_set_pipeline(encoder, render_pipeline_id)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderSetPipeline",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetScissorRect(
    pass: native::WGPURenderPassEncoder,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetScissorRect",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_set_scissor_rect(encoder, x, y, width, height)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderSetScissorRect",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetStencilReference(
    pass: native::WGPURenderPassEncoder,
    reference: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetStencilReference",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_set_stencil_reference(encoder, reference)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderSetStencilReference",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetVertexBuffer(
    pass: native::WGPURenderPassEncoder,
    slot: u32,
    buffer: native::WGPUBuffer,
    offset: u64,
    size: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetVertexBuffer",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let buffer_id = buffer.as_ref().map(|buffer| buffer.id);
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_set_vertex_buffer(
            encoder,
            slot,
            buffer_id,
            offset,
            match size {
                0 => panic!("invalid size"),
                conv::WGPU_WHOLE_SIZE => None,
                _ => Some(NonZeroU64::new_unchecked(size)),
            },
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderSetVertexBuffer",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetViewport(
    pass: native::WGPURenderPassEncoder,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    min_depth: f32,
    max_depth: f32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetViewport",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_set_viewport(encoder, x, y, width, height, min_depth, max_depth)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderSetViewport",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderAddRef(
    render_pass_encoder: native::WGPURenderPassEncoder,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderAddRef",
        (),
        || {

        assert!(
            !render_pass_encoder.is_null(),
            "invalid render pass encoder"
        );
        Arc::increment_strong_count(render_pass_encoder);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderRelease(
    render_pass_encoder: native::WGPURenderPassEncoder,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderRelease",
        (),
        || {

        assert!(
            !render_pass_encoder.is_null(),
            "invalid render pass encoder"
        );
        Arc::decrement_strong_count(render_pass_encoder);
        },
    )
}

// RenderPipeline methods

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPipelineGetBindGroupLayout(
    render_pipeline: native::WGPURenderPipeline,
    group_index: u32,
) -> native::WGPUBindGroupLayout {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPipelineGetBindGroupLayout",
        std::ptr::null(),
        || {

        let (render_pipeline_id, context, error_sink) = {
            let render_pipeline = render_pipeline.as_ref().expect("invalid render pipeline");
            (
                render_pipeline.id,
                &render_pipeline.context,
                &render_pipeline.error_sink,
            )
        };
        let (bind_group_layout_id, error) =
            context.render_pipeline_get_bind_group_layout(render_pipeline_id, group_index, None);
        if let Some(cause) = error {
            handle_error(
                error_sink,
                cause,
                None,
                "wgpuRenderPipelineGetBindGroupLayout",
            );
        }

        Arc::into_raw(Arc::new(WGPUBindGroupLayoutImpl {
            context: context.clone(),
            id: bind_group_layout_id,
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPipelineAddRef(render_pipeline: native::WGPURenderPipeline) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPipelineAddRef",
        (),
        || {

        assert!(!render_pipeline.is_null(), "invalid render pipeline");
        Arc::increment_strong_count(render_pipeline);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPipelineRelease(render_pipeline: native::WGPURenderPipeline) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPipelineRelease",
        (),
        || {

        assert!(!render_pipeline.is_null(), "invalid render pipeline");
        Arc::decrement_strong_count(render_pipeline);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderSetImmediates(
    pass: native::WGPURenderPassEncoder,
    offset: u32,
    data: *const u8,
    size: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderSetImmediates",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_set_immediates(encoder, offset, make_slice(data, size as usize))
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderSetImmediates",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderBundleEncoderSetImmediates(
    bundle: native::WGPURenderBundleEncoder,
    offset: u32,
    data: *const u8,
    size: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderBundleEncoderSetImmediates",
        (),
        || {

        let bundle = bundle.as_ref().expect("invalid render bundle");
        let encoder = bundle.encoder.as_mut().expect("invalid render bundle");
        let encoder = encoder.expect("invalid render bundle");
        let encoder = encoder.as_mut().unwrap();

        bundle_ffi::wgpu_render_bundle_set_immediates(encoder, offset, size, data);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderMultiDrawIndirect(
    pass: native::WGPURenderPassEncoder,
    buffer: native::WGPUBuffer,
    offset: u64,
    count: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderMultiDrawIndirect",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let buffer_id = buffer.as_ref().expect("invalid buffer").id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_multi_draw_indirect(encoder, buffer_id, offset, count)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderMultiDrawIndirect",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderMultiDrawIndexedIndirect(
    pass: native::WGPURenderPassEncoder,
    buffer: native::WGPUBuffer,
    offset: u64,
    count: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderMultiDrawIndexedIndirect",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let buffer_id = buffer.as_ref().expect("invalid buffer").id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_multi_draw_indexed_indirect(encoder, buffer_id, offset, count)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderMultiDrawIndexedIndirect",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderMultiDrawIndirectCount(
    pass: native::WGPURenderPassEncoder,
    buffer: native::WGPUBuffer,
    offset: u64,
    count_buffer: native::WGPUBuffer,
    count_buffer_offset: u64,
    max_count: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderMultiDrawIndirectCount",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let buffer_id = buffer.as_ref().expect("invalid buffer").id;
        let count_buffer_id = count_buffer.as_ref().expect("invalid count buffer").id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_multi_draw_indirect_count(
            encoder,
            buffer_id,
            offset,
            count_buffer_id,
            count_buffer_offset,
            max_count,
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderMultiDrawIndirectCount",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderMultiDrawIndexedIndirectCount(
    pass: native::WGPURenderPassEncoder,
    buffer: native::WGPUBuffer,
    offset: u64,
    count_buffer: native::WGPUBuffer,
    count_buffer_offset: u64,
    max_count: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderMultiDrawIndexedIndirectCount",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let buffer_id = buffer.as_ref().expect("invalid buffer").id;
        let count_buffer_id = count_buffer.as_ref().expect("invalid count buffer").id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_multi_draw_indexed_indirect_count(
            encoder,
            buffer_id,
            offset,
            count_buffer_id,
            count_buffer_offset,
            max_count,
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderMultiDrawIndexedIndirectCount",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderBeginPipelineStatisticsQuery(
    pass: native::WGPURenderPassEncoder,
    query_set: native::WGPUQuerySet,
    query_index: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderBeginPipelineStatisticsQuery",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let query_set_id = query_set.as_ref().expect("invalid query set").id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass.context.render_pass_begin_pipeline_statistics_query(
            encoder,
            query_set_id,
            query_index,
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderBeginPipelineStatisticsQuery",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderEndPipelineStatisticsQuery(
    pass: native::WGPURenderPassEncoder,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderEndPipelineStatisticsQuery",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_end_pipeline_statistics_query(encoder)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderEndPipelineStatisticsQuery",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderWriteTimestamp(
    pass: native::WGPURenderPassEncoder,
    query_set: native::WGPUQuerySet,
    query_index: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderWriteTimestamp",
        (),
        || {

        let pass = pass.as_ref().expect("invalid render pass");
        let query_set_id = query_set.as_ref().expect("invalid query set").id;
        let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

        match pass
            .context
            .render_pass_write_timestamp(encoder, query_set_id, query_index)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderWriteTimestamp",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderDrawMeshTasks(
    pass: native::WGPURenderPassEncoder,
    group_count_x: u32,
    group_count_y: u32,
    group_count_z: u32,
) {
    crate::ffi_utils::contain_entry("wgpuRenderPassEncoderDrawMeshTasks", (), || {
        let pass = pass.as_ref().expect("invalid render pass");
        let encoder = pass.encoder.as_mut().expect("invalid render pass encoder");

        check_draw_count(pass, "wgpuRenderPassEncoderDrawMeshTasks");
        match pass
            .context
            .render_pass_draw_mesh_tasks(encoder, group_count_x, group_count_y, group_count_z)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderDrawMeshTasks",
            ),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderDrawMeshTasksIndirect(
    pass: native::WGPURenderPassEncoder,
    indirect_buffer: native::WGPUBuffer,
    indirect_offset: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderDrawMeshTasksIndirect",
        (),
        || {
        let pass = pass.as_ref().expect("invalid render pass");
        let indirect_buffer_id = indirect_buffer
            .as_ref()
            .expect("invalid indirect buffer")
            .id;
        let encoder = pass.encoder.as_mut().expect("invalid render pass encoder");

        check_draw_count(pass, "wgpuRenderPassEncoderDrawMeshTasksIndirect");
        match pass
            .context
            .render_pass_draw_mesh_tasks_indirect(encoder, indirect_buffer_id, indirect_offset)
        {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderDrawMeshTasksIndirect",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderMultiDrawMeshTasksIndirect(
    pass: native::WGPURenderPassEncoder,
    indirect_buffer: native::WGPUBuffer,
    indirect_offset: u64,
    max_count: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderMultiDrawMeshTasksIndirect",
        (),
        || {
        let pass = pass.as_ref().expect("invalid render pass");
        let indirect_buffer_id = indirect_buffer
            .as_ref()
            .expect("invalid indirect buffer")
            .id;
        let encoder = pass.encoder.as_mut().expect("invalid render pass encoder");

        check_draw_count(pass, "wgpuRenderPassEncoderMultiDrawMeshTasksIndirect");
        match pass.context.render_pass_multi_draw_mesh_tasks_indirect(
            encoder,
            indirect_buffer_id,
            indirect_offset,
            max_count,
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderMultiDrawMeshTasksIndirect",
            ),
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuRenderPassEncoderMultiDrawMeshTasksIndirectCount(
    pass: native::WGPURenderPassEncoder,
    indirect_buffer: native::WGPUBuffer,
    indirect_offset: u64,
    count_buffer: native::WGPUBuffer,
    count_buffer_offset: u64,
    max_count: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuRenderPassEncoderMultiDrawMeshTasksIndirectCount",
        (),
        || {
        let pass = pass.as_ref().expect("invalid render pass");
        let indirect_buffer_id = indirect_buffer
            .as_ref()
            .expect("invalid indirect buffer")
            .id;
        let count_buffer_id = count_buffer.as_ref().expect("invalid count buffer").id;
        let encoder = pass.encoder.as_mut().expect("invalid render pass encoder");

        check_draw_count(pass, "wgpuRenderPassEncoderMultiDrawMeshTasksIndirectCount");
        match pass.context.render_pass_multi_draw_mesh_tasks_indirect_count(
            encoder,
            indirect_buffer_id,
            indirect_offset,
            count_buffer_id,
            count_buffer_offset,
            max_count,
        ) {
            Ok(()) => (),
            Err(cause) => handle_error(
                &pass.error_sink,
                cause,
                None,
                "wgpuRenderPassEncoderMultiDrawMeshTasksIndirectCount",
            ),
        }
        },
    )
}
