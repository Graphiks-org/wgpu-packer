#[allow(clippy::missing_safety_doc)]

use crate::conv::map_query_set_index;
use std::{
    borrow::Cow,
    sync::{atomic, Arc},
};
use crate::utils::{
    make_slice,
    string_view_into_label, string_view_into_str,
};
use wgc::Label;

use crate::conv;
use crate::error::*;
use crate::native;
use crate::objects::*;

// CommandBuffer methods

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandBufferAddRef(command_buffer: native::WGPUCommandBuffer) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandBufferAddRef",
        (),
        || {

        assert!(!command_buffer.is_null(), "invalid command buffer");
        Arc::increment_strong_count(command_buffer);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuCommandBufferRelease(command_buffer: native::WGPUCommandBuffer) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandBufferRelease",
        (),
        || {

        assert!(!command_buffer.is_null(), "invalid command buffer");
        Arc::decrement_strong_count(command_buffer);
        },
    )
}

// CommandEncoder methods

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderBeginComputePass(
    command_encoder: native::WGPUCommandEncoder,
    descriptor: Option<&native::WGPUComputePassDescriptor>,
) -> native::WGPUComputePassEncoder {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderBeginComputePass",
        std::ptr::null(),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };

        let timestamp_writes = descriptor.and_then(|descriptor| {
            descriptor.timestampWrites.as_ref().map(|timestamp_write| {
                wgc::command::PassTimestampWrites {
                    query_set: timestamp_write
                        .querySet
                        .as_ref()
                        .expect("invalid query set in timestamp writes")
                        .id,
                    beginning_of_pass_write_index: map_query_set_index(
                        timestamp_write.beginningOfPassWriteIndex,
                    ),
                    end_of_pass_write_index: map_query_set_index(timestamp_write.endOfPassWriteIndex),
                }
            })
        });

        let desc = match descriptor {
            Some(descriptor) => wgc::command::ComputePassDescriptor {
                label: string_view_into_label(descriptor.label),
                timestamp_writes,
            },
            None => wgc::command::ComputePassDescriptor {
                label: Label::default(),
                timestamp_writes,
            },
        };

        let (pass, err) = context.command_encoder_begin_compute_pass(command_encoder_id, &desc);
        if let Some(cause) = err {
            handle_error(
                error_sink,
                cause,
                desc.label,
                "wgpuCommandEncoderBeginComputePass",
            );
        }
        Arc::into_raw(Arc::new(WGPUComputePassEncoderImpl {
            context: context.clone(),
            encoder: Box::into_raw(Box::new(pass)),
            error_sink: error_sink.clone(),
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderBeginRenderPass(
    command_encoder: native::WGPUCommandEncoder,
    descriptor: Option<&native::WGPURenderPassDescriptor>,
) -> native::WGPURenderPassEncoder {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderBeginRenderPass",
        std::ptr::null(),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let depth_stencil_attachment = descriptor.depthStencilAttachment.as_ref().map(|desc| {
            wgc::command::RenderPassDepthStencilAttachment {
                view: desc
                    .view
                    .as_ref()
                    .expect("invalid texture view for depth stencil attachment")
                    .id,
                depth: wgc::command::PassChannel {
                    load_op: conv::map_load_op(desc.depthLoadOp, Some(desc.depthClearValue)),
                    store_op: conv::map_store_op(desc.depthStoreOp),
                    read_only: desc.depthReadOnly != 0,
                },
                stencil: wgc::command::PassChannel {
                    load_op: conv::map_load_op(desc.stencilLoadOp, Some(desc.stencilClearValue)),
                    store_op: conv::map_store_op(desc.stencilStoreOp),
                    read_only: desc.stencilReadOnly != 0,
                },
            }
        });

        let timestamp_writes = descriptor.timestampWrites.as_ref().map(|timestamp_write| {
            wgc::command::PassTimestampWrites {
                query_set: timestamp_write
                    .querySet
                    .as_ref()
                    .expect("invalid query set in timestamp writes")
                    .id,
                beginning_of_pass_write_index: map_query_set_index(
                    timestamp_write.beginningOfPassWriteIndex,
                ),
                end_of_pass_write_index: map_query_set_index(timestamp_write.endOfPassWriteIndex),
            }
        });

        let desc = wgc::command::RenderPassDescriptor {
            label: string_view_into_label(descriptor.label),
            color_attachments: Cow::Owned(
                make_slice(descriptor.colorAttachments, descriptor.colorAttachmentCount)
                    .iter()
                    .map(|color_attachment| {
                        if color_attachment.depthSlice != native::WGPU_DEPTH_SLICE_UNDEFINED {
                            log::warn!("Depth slice on color attachments is not implemented");
                        }

                        color_attachment.view.as_ref().map(|view| {
                            wgc::command::RenderPassColorAttachment {
                                view: view.id,
                                resolve_target: color_attachment.resolveTarget.as_ref().map(|v| v.id),
                                load_op: conv::map_load_op(
                                    color_attachment.loadOp,
                                    conv::map_color(&color_attachment.clearValue),
                                )
                                .expect("invalid load op for render pass color attachment"),
                                store_op: conv::map_store_op(color_attachment.storeOp)
                                    .expect("invalid store op for render pass color attachment"),
                                depth_slice: (color_attachment.depthSlice
                                    != native::WGPU_DEPTH_SLICE_UNDEFINED)
                                    .then_some(color_attachment.depthSlice),
                            }
                        })
                    })
                    .collect(),
            ),
            depth_stencil_attachment: depth_stencil_attachment,
            timestamp_writes: timestamp_writes,
            occlusion_query_set: descriptor.occlusionQuerySet.as_ref().map(|v| v.id),
            multiview_mask: None,
        };

        // maxDrawCount (C contract): WGPURenderPassMaxDrawCount chain. wgpu-core
        // 30 no longer has this cap — the packer counts it itself (see
        // wgpuRenderPassEncoderDraw). A descriptor without the chain takes the webgpu.h
        // default (50 000 000); an explicit 0 also counts as the default.
        const WGPU_RENDER_PASS_MAX_DRAW_COUNT_DEFAULT: u64 = 50_000_000;
        let max_draw_count = {
            let mut from_chain: Option<u64> = None;
            let mut chain = descriptor.nextInChain;
            while let Some(link) = unsafe { chain.as_ref() } {
                if link.sType == native::WGPUSType_RenderPassMaxDrawCount {
                    let payload = unsafe {
                        &*(chain as *const native::WGPUChainedStruct
                            as *const native::WGPURenderPassMaxDrawCount)
                    };
                    from_chain = Some(payload.maxDrawCount);
                    break;
                }
                chain = link.next;
            }
            match from_chain {
                Some(0) | None => WGPU_RENDER_PASS_MAX_DRAW_COUNT_DEFAULT,
                Some(count) => count,
            }
        };

        let (pass, err) = context.command_encoder_begin_render_pass(command_encoder_id, &desc);
        if let Some(cause) = err {
            handle_error(
                error_sink,
                cause,
                desc.label,
                "wgpuCommandEncoderBeginRenderPass",
            );
        }
        Arc::into_raw(Arc::new(WGPURenderPassEncoderImpl {
            context: context.clone(),
            encoder: Box::into_raw(Box::new(pass)),
            error_sink: error_sink.clone(),
            max_draw_count,
            draw_count: atomic::AtomicU64::new(0),
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderClearBuffer(
    command_encoder: native::WGPUCommandEncoder,
    buffer: native::WGPUBuffer,
    offset: u64,
    size: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderClearBuffer",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };
        let buffer_id = buffer.as_ref().expect("invalid buffer").id;

        if let Err(cause) = context.command_encoder_clear_buffer(
            command_encoder_id,
            buffer_id,
            offset,
            match size {
                0 => panic!("invalid size"),
                conv::WGPU_WHOLE_SIZE => None,
                _ => Some(size),
            },
        ) {
            handle_error(error_sink, cause, None, "wgpuCommandEncoderClearBuffer");
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderClearTexture(
    command_encoder: native::WGPUCommandEncoder,
    texture: native::WGPUTexture,
    range: Option<&native::WGPUImageSubresourceRange>,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderClearTexture",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };
        let texture_id = texture.as_ref().expect("invalid texture").id;

        let subresource_range = match range {
            Some(range) => wgt::ImageSubresourceRange {
                aspect: conv::map_texture_aspect(range.aspect).unwrap_or(wgt::TextureAspect::All),
                base_mip_level: range.baseMipLevel,
                mip_level_count: match range.mipLevelCount {
                    0 => panic!("invalid mipLevelCount"),
                    native::WGPU_MIP_LEVEL_COUNT_UNDEFINED => None,
                    _ => Some(range.mipLevelCount),
                },
                base_array_layer: range.baseArrayLayer,
                array_layer_count: match range.arrayLayerCount {
                    0 => panic!("invalid arrayLayerCount"),
                    native::WGPU_ARRAY_LAYER_COUNT_UNDEFINED => None,
                    _ => Some(range.arrayLayerCount),
                },
            },
            None => wgt::ImageSubresourceRange::default(),
        };

        if let Err(cause) =
            context.command_encoder_clear_texture(command_encoder_id, texture_id, &subresource_range)
        {
            handle_error(error_sink, cause, None, "wgpuCommandEncoderClearTexture");
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderCopyBufferToBuffer(
    command_encoder: native::WGPUCommandEncoder,
    source: native::WGPUBuffer,
    source_offset: u64,
    destination: native::WGPUBuffer,
    destination_offset: u64,
    size: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderCopyBufferToBuffer",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };
        let source_buffer_id = source.as_ref().expect("invalid source").id;
        let destination_buffer_id = destination.as_ref().expect("invalid destination").id;

        if let Err(cause) = context.command_encoder_copy_buffer_to_buffer(
            command_encoder_id,
            source_buffer_id,
            source_offset,
            destination_buffer_id,
            destination_offset,
            Some(size),
        ) {
            handle_error(
                error_sink,
                cause,
                None,
                "wgpuCommandEncoderCopyBufferToBuffer",
            );
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderCopyBufferToTexture(
    command_encoder: native::WGPUCommandEncoder,
    source: Option<&native::WGPUTexelCopyBufferInfo>,
    destination: Option<&native::WGPUTexelCopyTextureInfo>,
    copy_size: Option<&native::WGPUExtent3D>,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderCopyBufferToTexture",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };

        if let Err(cause) = context.command_encoder_copy_buffer_to_texture(
            command_encoder_id,
            &conv::map_image_copy_buffer(source.expect("invalid source")),
            &conv::map_image_copy_texture(destination.expect("invalid destination")),
            &conv::map_extent3d(copy_size.expect("invalid copy size")),
        ) {
            handle_error(
                error_sink,
                cause,
                None,
                "wgpuCommandEncoderCopyBufferToTexture",
            );
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderCopyTextureToBuffer(
    command_encoder: native::WGPUCommandEncoder,
    source: Option<&native::WGPUTexelCopyTextureInfo>,
    destination: Option<&native::WGPUTexelCopyBufferInfo>,
    copy_size: Option<&native::WGPUExtent3D>,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderCopyTextureToBuffer",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };

        if let Err(cause) = context.command_encoder_copy_texture_to_buffer(
            command_encoder_id,
            &conv::map_image_copy_texture(source.expect("invalid source")),
            &conv::map_image_copy_buffer(destination.expect("invalid destination")),
            &conv::map_extent3d(copy_size.expect("invalid copy size")),
        ) {
            handle_error(
                error_sink,
                cause,
                None,
                "wgpuCommandEncoderCopyTextureToBuffer",
            );
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderCopyTextureToTexture(
    command_encoder: native::WGPUCommandEncoder,
    source: Option<&native::WGPUTexelCopyTextureInfo>,
    destination: Option<&native::WGPUTexelCopyTextureInfo>,
    copy_size: Option<&native::WGPUExtent3D>,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderCopyTextureToTexture",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };

        if let Err(cause) = context.command_encoder_copy_texture_to_texture(
            command_encoder_id,
            &conv::map_image_copy_texture(source.expect("invalid source")),
            &conv::map_image_copy_texture(destination.expect("invalid destination")),
            &conv::map_extent3d(copy_size.expect("invalid copy size")),
        ) {
            handle_error(
                error_sink,
                cause,
                None,
                "wgpuCommandEncoderCopyTextureToTexture",
            );
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderFinish(
    command_encoder: native::WGPUCommandEncoder,
    descriptor: Option<&native::WGPUCommandBufferDescriptor>,
) -> native::WGPUCommandBuffer {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderFinish",
        std::ptr::null(),
        || {

        let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
        let (command_encoder_id, context, error_sink) = (
            command_encoder.id,
            &command_encoder.context,
            &command_encoder.error_sink,
        );
        command_encoder.open.store(false, atomic::Ordering::SeqCst);

        let desc = match descriptor {
            Some(descriptor) => wgt::CommandBufferDescriptor {
                label: string_view_into_label(descriptor.label),
            },
            None => wgt::CommandBufferDescriptor::default(),
        };

        let (command_buffer_id, error) =
            context.command_encoder_finish(command_encoder_id, &desc, None);
        if let Some((_label, cause)) = error {
            handle_error(error_sink, cause, None, "wgpuCommandEncoderFinish");
        }

        Arc::into_raw(Arc::new(WGPUCommandBufferImpl {
            context: context.clone(),
            id: command_buffer_id,
            open: atomic::AtomicBool::new(false),
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderInsertDebugMarker(
    command_encoder: native::WGPUCommandEncoder,
    marker_label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderInsertDebugMarker",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };

        if let Err(cause) = context.command_encoder_insert_debug_marker(
            command_encoder_id,
            string_view_into_str(marker_label).unwrap_or(""),
        ) {
            handle_error(
                error_sink,
                cause,
                None,
                "wgpuCommandEncoderInsertDebugMarker",
            );
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderPopDebugGroup(
    command_encoder: native::WGPUCommandEncoder,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderPopDebugGroup",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };

        if let Err(cause) = context.command_encoder_pop_debug_group(command_encoder_id) {
            handle_error(error_sink, cause, None, "wgpuCommandEncoderPopDebugGroup");
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderPushDebugGroup(
    command_encoder: native::WGPUCommandEncoder,
    group_label: native::WGPUStringView,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderPushDebugGroup",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };

        if let Err(cause) = context.command_encoder_push_debug_group(
            command_encoder_id,
            string_view_into_str(group_label).unwrap_or(""),
        ) {
            handle_error(error_sink, cause, None, "wgpuCommandEncoderPushDebugGroup");
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderResolveQuerySet(
    command_encoder: native::WGPUCommandEncoder,
    query_set: native::WGPUQuerySet,
    first_query: u32,
    query_count: u32,
    destination: native::WGPUBuffer,
    destination_offset: u64,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderResolveQuerySet",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };
        let query_set_id = query_set.as_ref().expect("invalid query set").id;
        let destination_buffer_id = destination.as_ref().expect("invalid destination").id;

        if let Err(cause) = context.command_encoder_resolve_query_set(
            command_encoder_id,
            query_set_id,
            first_query,
            query_count,
            destination_buffer_id,
            destination_offset,
        ) {
            handle_error(error_sink, cause, None, "wgpuCommandEncoderResolveQuerySet");
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderWriteTimestamp(
    command_encoder: native::WGPUCommandEncoder,
    query_set: native::WGPUQuerySet,
    query_index: u32,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderWriteTimestamp",
        (),
        || {

        let (command_encoder_id, context, error_sink) = {
            let command_encoder = command_encoder.as_ref().expect("invalid command encoder");
            (
                command_encoder.id,
                &command_encoder.context,
                &command_encoder.error_sink,
            )
        };
        let query_set_id = query_set.as_ref().expect("invalid query set").id;

        if let Err(cause) =
            context.command_encoder_write_timestamp(command_encoder_id, query_set_id, query_index)
        {
            handle_error(error_sink, cause, None, "wgpuCommandEncoderWriteTimestamp");
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderAddRef(command_encoder: native::WGPUCommandEncoder) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderAddRef",
        (),
        || {

        assert!(!command_encoder.is_null(), "invalid command encoder");
        Arc::increment_strong_count(command_encoder);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderRelease(command_encoder: native::WGPUCommandEncoder) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderRelease",
        (),
        || {

        assert!(!command_encoder.is_null(), "invalid command encoder");
        Arc::decrement_strong_count(command_encoder);
        },
    )
}
