#[allow(clippy::missing_safety_doc)]

use crate::conv::{
    from_u64_bits, map_bind_group_entry,
    map_bind_group_layout_entry, map_pipeline_layout_descriptor, map_query_set_descriptor, map_sampler_border_color_extras, map_shader_module,
    map_shader_runtime_checks,
};
use parking_lot::Mutex;
use std::{
    borrow::Cow,
    ffi::c_void,
    sync::{atomic, Arc},
};
use crate::utils::{
    make_slice, str_into_string_view,
    string_view_into_label, string_view_into_str, texture_format_has_depth,
};

use crate::conv::map_primitive_state;

use crate::callbacks::*;
use crate::conv;
use crate::follow_chain;
use crate::new_userdata;
use crate::error::*;
use crate::native;
use crate::objects::*;
use super::return_features;

// Device methods

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateBindGroup(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUBindGroupDescriptor>,
) -> native::WGPUBindGroup {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateBindGroup",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");
        let bind_group_layout_id = descriptor
            .layout
            .as_ref()
            .expect("invalid bind group layout for bind group descriptor")
            .id;

        let entries = make_slice(descriptor.entries, descriptor.entryCount)
            .iter()
            .map(|entry| {
                follow_chain!(map_bind_group_entry((entry),
                    WGPUSType_BindGroupEntryExtras => native::WGPUBindGroupEntryExtras,
                    WGPUSType_ExternalTextureBindingEntry => native::WGPUExternalTextureBindingEntry)
                )
            })
            .collect::<Vec<_>>();

        let desc = wgc::binding_model::BindGroupDescriptor {
            label: string_view_into_label(descriptor.label),
            layout: bind_group_layout_id,
            entries: Cow::Borrowed(&entries),
        };
        let (bind_group_id, error) = context.device_create_bind_group(device_id, &desc, None);
        if let Some(cause) = error {
            handle_error(error_sink, cause, desc.label, "wgpuDeviceCreateBindGroup");
        }

        Arc::into_raw(Arc::new(WGPUBindGroupImpl {
            context: context.clone(),
            id: bind_group_id,
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateBindGroupLayout(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUBindGroupLayoutDescriptor>,
) -> native::WGPUBindGroupLayout {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateBindGroupLayout",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let entries = make_slice(descriptor.entries, descriptor.entryCount)
            .iter()
            .map(|entry| {
                follow_chain!(map_bind_group_layout_entry((entry),
                    WGPUSType_BindGroupLayoutEntryExtras => native::WGPUBindGroupLayoutEntryExtras,
                    WGPUSType_ExternalTextureBindingLayout => native::WGPUExternalTextureBindingLayout)
                )
            })
            .collect::<Vec<_>>();

        let desc = wgc::binding_model::BindGroupLayoutDescriptor {
            label: string_view_into_label(descriptor.label),
            entries: Cow::Borrowed(&entries),
        };
        let (bind_group_layout_id, error) =
            context.device_create_bind_group_layout(device_id, &desc, None);
        if let Some(cause) = error {
            handle_error(
                error_sink,
                cause,
                desc.label,
                "wgpuDeviceCreateBindGroupLayout",
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
pub unsafe extern "C" fn wgpuDeviceCreateBuffer(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUBufferDescriptor>,
) -> native::WGPUBuffer {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateBuffer",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink, queue_id) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink, device.queue.id)
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let desc = wgt::BufferDescriptor {
            label: string_view_into_label(descriptor.label),
            size: descriptor.size,
            usage: from_u64_bits(descriptor.usage).expect("invalid buffer usage"),
            mapped_at_creation: descriptor.mappedAtCreation != 0,
        };

        let (buffer_id, error) = context.device_create_buffer(device_id, &desc, None);
        if let Some(cause) = error {
            handle_error(error_sink, cause, desc.label, "wgpuDeviceCreateBuffer");
        }

        Arc::into_raw(Arc::new(WGPUBufferImpl {
            context: context.clone(),
            id: buffer_id,
            error_sink: error_sink.clone(),
            data: BufferData {
                usage: descriptor.usage,
                size: descriptor.size,
                queue: queue_id,
                map_state: Arc::new(Mutex::new(if descriptor.mappedAtCreation != 0 {
                    native::WGPUBufferMapState_Mapped
                } else {
                    native::WGPUBufferMapState_Unmapped
                })),
            },
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateCommandEncoder(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUCommandEncoderDescriptor>,
) -> native::WGPUCommandEncoder {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateCommandEncoder",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let desc = match descriptor {
            Some(descriptor) => wgt::CommandEncoderDescriptor {
                label: string_view_into_label(descriptor.label),
            },
            None => wgt::CommandEncoderDescriptor::default(),
        };
        let (command_encoder_id, error) = context.device_create_command_encoder(device_id, &desc, None);
        if let Some(cause) = error {
            handle_error(
                error_sink,
                cause,
                desc.label,
                "wgpuDeviceCreateCommandEncoder",
            );
        }

        Arc::into_raw(Arc::new(WGPUCommandEncoderImpl {
            context: context.clone(),
            id: command_encoder_id,
            error_sink: error_sink.clone(),
            open: atomic::AtomicBool::new(true),
        }))
        },
    )
}

/// C -> wgc mapping of the compute pipeline descriptor, shared by the
/// synchronous and async variants (the C chains are only valid during
/// the call: the mapping stays on the calling thread).
/// Reads the WGPUProgrammableStageDescriptorExtras chain (also honoured on
/// VertexState / FragmentState chains - same sType) and returns its
/// zero-init flag. Absent chain = false (the WGSL-spec default).
fn zero_init_from_chain(chain: *const native::WGPUChainedStruct) -> bool {
    unsafe {
        crate::utils::chain_entry::<native::WGPUProgrammableStageDescriptorExtras>(
            chain,
            native::WGPUSType_ProgrammableStageDescriptorExtras,
        )
    }
    .is_some_and(|extras| extras.zeroInitializeWorkgroupMemory != 0)
}
unsafe fn map_compute_pipeline_descriptor(
    descriptor: &native::WGPUComputePipelineDescriptor,
) -> wgc::pipeline::ComputePipelineDescriptor {
    wgc::pipeline::ComputePipelineDescriptor {
        label: string_view_into_label(descriptor.label),
        layout: descriptor.layout.as_ref().map(|v| v.id),
        stage: wgc::pipeline::ProgrammableStageDescriptor {
            module: descriptor
                .compute
                .module
                .as_ref()
                .expect("invalid compute shader module for compute pipeline descriptor")
                .id
                .expect("invalid compute shader module for compute pipeline descriptor"),
            entry_point: string_view_into_label(descriptor.compute.entryPoint),
            constants: make_slice(
                descriptor.compute.constants,
                descriptor.compute.constantCount,
            )
            .iter()
            .map(|entry| {
                (
                    string_view_into_str(entry.key).unwrap_or("").to_string(),
                    entry.value,
                )
            })
            .collect(),
            // No C knob for this yet: the WGSL spec default is false.
            zero_initialize_workgroup_memory: zero_init_from_chain(descriptor.compute.nextInChain),
        },
        // No C knob for a pipeline cache yet: every device starts without one.
        cache: crate::utils::chain_entry::<native::WGPUComputePipelineDescriptorExtras>(
            descriptor.nextInChain,
            native::WGPUSType_ComputePipelineDescriptorExtras,
        )
        .and_then(|extras| unsafe { extras.cache.as_ref() })
        .map(|cache| cache.id),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateComputePipeline(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUComputePipelineDescriptor>,
) -> native::WGPUComputePipeline {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateComputePipeline",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let desc = map_compute_pipeline_descriptor(descriptor);

        let (compute_pipeline_id, error) =
            context.device_create_compute_pipeline(device_id, &desc, None);
        if let Some(cause) = error {
            if let wgc::pipeline::CreateComputePipelineError::Internal(ref error) = cause {
                log::warn!(
                    "Shader translation error for stage {:?}: {}",
                    wgt::ShaderStages::COMPUTE,
                    error
                );
                log::warn!("Please report it to https://github.com/gfx-rs/wgpu");
            }
            handle_error(
                error_sink,
                cause,
                desc.label,
                "wgpuDeviceCreateComputePipeline",
            );
        }

        Arc::into_raw(Arc::new(WGPUComputePipelineImpl {
            context: context.clone(),
            id: compute_pipeline_id,
            error_sink: error_sink.clone(),
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateComputePipelineAsync(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUComputePipelineDescriptor>,
    mut callback_info: native::WGPUCreateComputePipelineAsyncCallbackInfo,
) -> native::WGPUFuture {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateComputePipelineAsync",
        native::WGPUFuture { id: 0 },
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");
        crate::future::normalize_callback_mode(&mut callback_info.mode);
        crate::future::validate_callback_mode(callback_info.mode);
        let callback = callback_info.callback.expect("invalid callback");
        let userdata = new_userdata!(callback_info);

        // wgc 30 has no async creation: mapping and creation are synchronous
        // (the C chains do not outlive the call), the future completes before
        // the return — the callback is delivered according to the mode (AllowSpontaneous →
        // before return, same convention as requestDevice; GUIDE §5).
        let future_id =
            crate::future::register(context, callback_info.mode, crate::future::Source::Cpu);

        let desc = map_compute_pipeline_descriptor(descriptor);
        let (compute_pipeline_id, error) =
            context.device_create_compute_pipeline(device_id, &desc, None);
        match error {
            None => {
                let mut pipeline = crate::future::OwnedHandle::new(Arc::into_raw(Arc::new(
                    WGPUComputePipelineImpl {
                        context: context.clone(),
                        id: compute_pipeline_id,
                        error_sink: error_sink.clone(),
                    },
                )) as *mut WGPUComputePipelineImpl);
                crate::future::complete(
                    future_id,
                    Box::new(move || {
                        callback(
                            native::WGPUCreatePipelineAsyncStatus_Success,
                            pipeline.take(),
                            EMPTY_STRING,
                            userdata.get_1(),
                            userdata.get_2(),
                        );
                    }),
                );
            }
            Some(cause) => {
                let (status, message) = route_async_pipeline_error(
                    error_sink,
                    cause,
                    desc.label,
                    "wgpuDeviceCreateComputePipelineAsync",
                );
                crate::future::complete(
                    future_id,
                    Box::new(move || {
                        callback(
                            status,
                            std::ptr::null_mut(),
                            str_into_string_view(&message),
                            userdata.get_1(),
                            userdata.get_2(),
                        );
                    }),
                );
            }
        }

        native::WGPUFuture { id: future_id }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreatePipelineLayout(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUPipelineLayoutDescriptor>,
) -> native::WGPUPipelineLayout {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreatePipelineLayout",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let desc = map_pipeline_layout_descriptor(descriptor);
        let (pipeline_layout_id, error) = context.device_create_pipeline_layout(device_id, &desc, None);
        if let Some(cause) = error {
            handle_error(
                error_sink,
                cause,
                desc.label,
                "wgpuDeviceCreatePipelineLayout",
            );
        }

        Arc::into_raw(Arc::new(WGPUPipelineLayoutImpl {
            context: context.clone(),
            id: pipeline_layout_id,
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateQuerySet(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUQuerySetDescriptor>,
) -> native::WGPUQuerySet {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateQuerySet",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid query set descriptor");

        let desc = follow_chain!(
            map_query_set_descriptor(
                (descriptor),
                WGPUSType_QuerySetDescriptorExtras => native::WGPUQuerySetDescriptorExtras)
        );

        let (query_set_id, error) = context.device_create_query_set(device_id, &desc, None);
        if let Some(cause) = error {
            handle_error(error_sink, cause, desc.label, "wgpuDeviceCreateQuerySet");
        }

        Arc::into_raw(Arc::new(WGPUQuerySetImpl {
            context: context.clone(),
            id: query_set_id,
            data: QuerySetData {
                query_type: descriptor.type_,
                query_count: descriptor.count,
            },
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateRenderBundleEncoder(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPURenderBundleEncoderDescriptor>,
) -> native::WGPURenderBundleEncoder {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateRenderBundleEncoder",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let desc = wgc::command::RenderBundleEncoderDescriptor {
            label: string_view_into_label(descriptor.label),
            color_formats: make_slice(descriptor.colorFormats, descriptor.colorFormatCount)
                .iter()
                .map(|format| conv::map_texture_format(*format))
                .collect(),
            depth_stencil: conv::map_texture_format(descriptor.depthStencilFormat).map(|format| {
                wgt::RenderBundleDepthStencil {
                    format,
                    depth_read_only: descriptor.depthReadOnly != 0,
                    stencil_read_only: descriptor.stencilReadOnly != 0,
                }
            }),
            sample_count: descriptor.sampleCount,
            // WGPURenderBundleEncoderDescriptorExtras: 0 = none.
            multiview: crate::utils::chain_entry::<native::WGPURenderBundleEncoderDescriptorExtras>(
                descriptor.nextInChain,
                native::WGPUSType_RenderBundleEncoderDescriptorExtras,
            )
            .and_then(|extras| std::num::NonZeroU32::new(extras.multiview)),
        };

        // device=None: device-dependent validation is deferred to finish()
        // (wgc 30 behavior preserved). Descriptor validation still runs here, and
        // its errors go to the device sink like every other wgpuDeviceCreate*:
        // the C contract returns a handle, it does not abort.
        let encoder = match wgc::command::RenderBundleEncoder::new(&desc, None, device_id) {
            Ok(encoder) => Some(Box::into_raw(Box::new(encoder))),
            Err(cause) => {
                handle_error(
                    error_sink,
                    cause,
                    desc.label,
                    "wgpuDeviceCreateRenderBundleEncoder",
                );
                None
            }
        };

        Arc::into_raw(Arc::new(WGPURenderBundleEncoderImpl {
            context: context.clone(),
            encoder: Box::into_raw(Box::new(encoder)),
            error_sink: error_sink.clone(),
        }))
        },
    )
}

/// C -> wgc mapping of the render pipeline descriptor, shared by the
/// synchronous and async variants (cf. map_compute_pipeline_descriptor).
unsafe fn map_render_pipeline_descriptor(
    descriptor: &native::WGPURenderPipelineDescriptor,
) -> wgc::pipeline::RenderPipelineDescriptor {
    wgc::pipeline::RenderPipelineDescriptor {
        label: string_view_into_label(descriptor.label),
        layout: descriptor.layout.as_ref().map(|v| v.id),
        vertex: wgc::pipeline::VertexState {
            stage: wgc::pipeline::ProgrammableStageDescriptor {
                module: descriptor
                    .vertex
                    .module
                    .as_ref()
                    .expect("invalid vertex shader module for vertex state")
                    .id
                    .expect("invalid vertex shader module for vertex state"),
                entry_point: string_view_into_label(descriptor.vertex.entryPoint),
                constants: make_slice(descriptor.vertex.constants, descriptor.vertex.constantCount)
                    .iter()
                    .map(|entry| {
                        (
                            string_view_into_str(entry.key).unwrap_or("").to_string(),
                            entry.value,
                        )
                    })
                    .collect(),
                // No C knob for this yet: the WGSL spec default is false.
                zero_initialize_workgroup_memory: zero_init_from_chain(descriptor.vertex.nextInChain),
            },
            buffers: Cow::Owned(
                make_slice(descriptor.vertex.buffers, descriptor.vertex.bufferCount)
                    .iter()
                    .map(|buffer| {
                        // webgpu.h: stepMode Undefined + no attribute = "hole"
                        // in the buffer array (equivalent of null on the
                        // JS side, wgc 30: Option<VertexBufferLayout>).
                        if buffer.stepMode == native::WGPUVertexStepMode_Undefined
                            && buffer.attributeCount == 0
                        {
                            return None;
                        }
                        Some(wgc::pipeline::VertexBufferLayout {
                            array_stride: buffer.arrayStride,
                            step_mode: match buffer.stepMode {
                                native::WGPUVertexStepMode_Undefined
                                | native::WGPUVertexStepMode_Vertex => {
                                    wgt::VertexStepMode::Vertex
                                }
                                native::WGPUVertexStepMode_Instance => {
                                    wgt::VertexStepMode::Instance
                                }
                                _ => panic!("invalid vertex step mode for vertex buffer layout"),
                            },
                            attributes: Cow::Owned(
                                make_slice(buffer.attributes, buffer.attributeCount)
                                    .iter()
                                    .map(|attribute| wgt::VertexAttribute {
                                        format: conv::map_vertex_format(attribute.format)
                                            .expect("invalid vertex format for vertex attribute"),
                                        offset: attribute.offset,
                                        shader_location: attribute.shaderLocation,
                                    })
                                    .collect(),
                            ),
                        })
                    })
                    .collect(),
            ),
        },
        primitive: {
            follow_chain!(map_primitive_state((descriptor.primitive), WGPUSType_PrimitiveStateExtras => native::WGPUPrimitiveStateExtras))
        },
        depth_stencil: descriptor.depthStencil.as_ref().map(|desc| {
            let format = conv::map_texture_format(desc.format)
                .expect("invalid texture format for depth stencil state");

            // Validation per spec.
            if texture_format_has_depth(format) {
                if desc.depthWriteEnabled == native::WGPUOptionalBool_Undefined {
                    panic!("Depth write not specified for depth format")
                }
            } else if desc.depthWriteEnabled == native::WGPUOptionalBool_True {
                panic!("Depth write enabled for non-depth format")
            }

            wgt::DepthStencilState {
                format,
                depth_write_enabled: Some(desc.depthWriteEnabled == native::WGPUOptionalBool_True),
                // `WGPUCompareFunction_Undefined` is legal in webgpu.h: the
                // spec requires depthCompare only when depthWriteEnabled is
                // true or a stencil depthFailOp is not "keep". wgpu-core has
                // no undefined variant, so it maps to Always, which is the
                // identity of the comparison (every fragment passes).
                depth_compare: Some(
                    conv::map_compare_function(desc.depthCompare)
                        .expect("invalid depth compare function for depth stencil state")
                        .unwrap_or(wgt::CompareFunction::Always),
                ),
                stencil: wgt::StencilState {
                    front: conv::map_stencil_face_state(desc.stencilFront, "front"),
                    back: conv::map_stencil_face_state(desc.stencilBack, "back"),
                    read_mask: desc.stencilReadMask,
                    write_mask: desc.stencilWriteMask,
                },
                bias: wgt::DepthBiasState {
                    constant: desc.depthBias,
                    slope_scale: desc.depthBiasSlopeScale,
                    clamp: desc.depthBiasClamp,
                },
            }
        }),
        multisample: wgt::MultisampleState {
            count: descriptor.multisample.count,
            mask: descriptor.multisample.mask as u64,
            alpha_to_coverage_enabled: descriptor.multisample.alphaToCoverageEnabled != 0,
        },
        fragment: descriptor
            .fragment
            .as_ref()
            .map(|fragment| wgc::pipeline::FragmentState {
                stage: wgc::pipeline::ProgrammableStageDescriptor {
                    module: fragment
                        .module
                        .as_ref()
                        .expect("invalid fragment shader module for fragment state")
                        .id
                        .expect("invalid fragment shader module for fragment state"),
                    entry_point: string_view_into_label(fragment.entryPoint),
                    constants: make_slice(fragment.constants, fragment.constantCount)
                        .iter()
                        .map(|entry| {
                            (
                                string_view_into_str(entry.key).unwrap_or("").to_string(),
                                entry.value,
                            )
                        })
                        .collect(),
                    // No C knob for this yet: the WGSL spec default is false.
                    zero_initialize_workgroup_memory: zero_init_from_chain(fragment.nextInChain),
                },
                targets: Cow::Owned(
                    make_slice(fragment.targets, fragment.targetCount)
                        .iter()
                        .map(|color_target| {
                            conv::map_texture_format(color_target.format).map(|format| {
                                wgt::ColorTargetState {
                                    format,
                                    blend: color_target.blend.as_ref().map(|blend| {
                                        wgt::BlendState {
                                            color: conv::map_blend_component(blend.color),
                                            alpha: conv::map_blend_component(blend.alpha),
                                        }
                                    }),
                                    write_mask: from_u64_bits(color_target.writeMask).unwrap(),
                                }
                            })
                        })
                        .collect(),
                ),
            }),
        // WGPURenderPipelineDescriptorExtras: 0 = all views (the old
        // behaviour), a nonzero mask selects the multiview layers.
        multiview_mask: std::num::NonZeroU32::new(
            crate::utils::chain_entry::<native::WGPURenderPipelineDescriptorExtras>(
                descriptor.nextInChain,
                native::WGPUSType_RenderPipelineDescriptorExtras,
            )
            .map_or(0, |extras| extras.multiviewMask),
        ),
        // No C knob for a pipeline cache yet: every device starts without one.
        cache: crate::utils::chain_entry::<native::WGPURenderPipelineDescriptorExtras>(
            descriptor.nextInChain,
            native::WGPUSType_RenderPipelineDescriptorExtras,
        )
        .and_then(|extras| unsafe { extras.cache.as_ref() })
        .map(|cache| cache.id),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateRenderPipeline(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPURenderPipelineDescriptor>,
) -> native::WGPURenderPipeline {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateRenderPipeline",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let desc = map_render_pipeline_descriptor(descriptor);

        let (render_pipeline_id, error) = context.device_create_render_pipeline(device_id, &desc, None);
        if let Some(cause) = error {
            if let wgc::pipeline::CreateRenderPipelineError::Internal { stage, ref error } = cause {
                log::error!("Shader translation error for stage {:?}: {}", stage, error);
                log::error!("Please report it to https://github.com/gfx-rs/wgpu");
            }
            handle_error(
                error_sink,
                cause,
                desc.label,
                "wgpuDeviceCreateRenderPipeline",
            );
        }

        Arc::into_raw(Arc::new(WGPURenderPipelineImpl {
            context: context.clone(),
            id: render_pipeline_id,
            error_sink: error_sink.clone(),
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateRenderPipelineAsync(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPURenderPipelineDescriptor>,
    mut callback_info: native::WGPUCreateRenderPipelineAsyncCallbackInfo,
) -> native::WGPUFuture {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateRenderPipelineAsync",
        native::WGPUFuture { id: 0 },
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");
        crate::future::normalize_callback_mode(&mut callback_info.mode);
        crate::future::validate_callback_mode(callback_info.mode);
        let callback = callback_info.callback.expect("invalid callback");
        let userdata = new_userdata!(callback_info);

        // wgc 30 has no async creation (cf. wgpuDeviceCreateComputePipelineAsync).
        let future_id =
            crate::future::register(context, callback_info.mode, crate::future::Source::Cpu);

        let desc = map_render_pipeline_descriptor(descriptor);
        let (render_pipeline_id, error) = context.device_create_render_pipeline(device_id, &desc, None);
        match error {
            None => {
                let mut pipeline = crate::future::OwnedHandle::new(Arc::into_raw(Arc::new(
                    WGPURenderPipelineImpl {
                        context: context.clone(),
                        id: render_pipeline_id,
                        error_sink: error_sink.clone(),
                    },
                )) as *mut WGPURenderPipelineImpl);
                crate::future::complete(
                    future_id,
                    Box::new(move || {
                        callback(
                            native::WGPUCreatePipelineAsyncStatus_Success,
                            pipeline.take(),
                            EMPTY_STRING,
                            userdata.get_1(),
                            userdata.get_2(),
                        );
                    }),
                );
            }
            Some(cause) => {
                let (status, message) = route_async_pipeline_error(
                    error_sink,
                    cause,
                    desc.label,
                    "wgpuDeviceCreateRenderPipelineAsync",
                );
                crate::future::complete(
                    future_id,
                    Box::new(move || {
                        callback(
                            status,
                            std::ptr::null_mut(),
                            str_into_string_view(&message),
                            userdata.get_1(),
                            userdata.get_2(),
                        );
                    }),
                );
            }
        }

        native::WGPUFuture { id: future_id }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateSampler(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUSamplerDescriptor>,
) -> native::WGPUSampler {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateSampler",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };

        let desc = match descriptor {
            Some(descriptor) => wgc::resource::SamplerDescriptor {
                label: string_view_into_label(descriptor.label),
                address_modes: [
                    conv::map_address_mode(descriptor.addressModeU)
                        .unwrap_or(wgt::AddressMode::ClampToEdge),
                    conv::map_address_mode(descriptor.addressModeV)
                        .unwrap_or(wgt::AddressMode::ClampToEdge),
                    conv::map_address_mode(descriptor.addressModeW)
                        .unwrap_or(wgt::AddressMode::ClampToEdge),
                ],
                mag_filter: conv::map_filter_mode(descriptor.magFilter)
                    .unwrap_or(wgt::FilterMode::Nearest),
                min_filter: conv::map_filter_mode(descriptor.minFilter)
                    .unwrap_or(wgt::FilterMode::Nearest),
                mipmap_filter: conv::map_mipmap_filter_mode(descriptor.mipmapFilter)
                    .unwrap_or(wgt::MipmapFilterMode::Nearest),
                lod_min_clamp: descriptor.lodMinClamp,
                lod_max_clamp: descriptor.lodMaxClamp,
                compare: conv::map_compare_function(descriptor.compare)
                    .expect("Invalid compare function"),
                anisotropy_clamp: descriptor.maxAnisotropy,
                border_color: follow_chain!(map_sampler_border_color_extras((*descriptor), WGPUSType_SamplerDescriptorExtras => native::WGPUSamplerDescriptorExtras)),
            },
            // wgpu-core doesn't have Default implementation for SamplerDescriptor,
            // use defaults from spec.
            // ref: https://gpuweb.github.io/gpuweb/#GPUSamplerDescriptor
            None => wgc::resource::SamplerDescriptor {
                label: None,
                address_modes: [
                    wgt::AddressMode::ClampToEdge,
                    wgt::AddressMode::ClampToEdge,
                    wgt::AddressMode::ClampToEdge,
                ],
                mag_filter: wgt::FilterMode::Nearest,
                min_filter: wgt::FilterMode::Nearest,
                mipmap_filter: wgt::MipmapFilterMode::Nearest,
                lod_min_clamp: 0f32,
                lod_max_clamp: 32f32,
                compare: None,
                anisotropy_clamp: 1,
                border_color: None,
            },
        };

        let (sampler_id, error) = context.device_create_sampler(device_id, &desc, None);
        if let Some(cause) = error {
            handle_error(error_sink, cause, desc.label, "wgpuDeviceCreateSampler");
        }

        Arc::into_raw(Arc::new(WGPUSamplerImpl {
            context: context.clone(),
            id: sampler_id,
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreatePipelineCache(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUPipelineCacheDescriptor>,
) -> native::WGPUPipelineCache {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreatePipelineCache",
        std::ptr::null(),
        || {

        let device = device.as_ref().expect("invalid device");
        let label = descriptor
            .map(|descriptor| string_view_into_label(descriptor.label))
            .unwrap_or_default();
        let data: Option<Cow<[u8]>> = descriptor.and_then(|descriptor| {
            (descriptor.dataSize > 0).then(|| {
                Cow::Borrowed(std::slice::from_raw_parts(
                    descriptor.data,
                    descriptor.dataSize,
                ))
            })
        });
        let desc = wgc::pipeline::PipelineCacheDescriptor {
            label,
            data,
            fallback: descriptor.map(|d| d.fallback != 0).unwrap_or(false),
        };
        match device.context.device_create_pipeline_cache(device.id, &desc, None) {
            (id, None) => Arc::into_raw(Arc::new(crate::objects::WGPUPipelineCacheImpl {
                context: device.context.clone(),
                id,
            })),
            (_, Some(cause)) => {
                handle_error(
                    &device.error_sink,
                    cause,
                    desc.label,
                    "wgpuDeviceCreatePipelineCache",
                );
                std::ptr::null()
            }
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuPipelineCacheGetData(
    pipeline_cache: native::WGPUPipelineCache,
    buffer: *mut core::ffi::c_void,
    buffer_size: usize,
) -> usize {
    crate::ffi_utils::contain_entry(
        "wgpuPipelineCacheGetData",
        0,
        || {

        let cache = pipeline_cache.as_ref().expect("invalid pipeline cache");
        // Two-call protocol: a NULL buffer returns the needed size.
        let Some(data) = cache.context.pipeline_cache_get_data(cache.id) else {
            return 0;
        };
        let copied = data.len().min(buffer_size);
        if !buffer.is_null() && copied > 0 {
            std::ptr::copy_nonoverlapping(data.as_ptr(), buffer as *mut u8, copied);
        }
        if buffer.is_null() {
            data.len()
        } else {
            copied
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuPipelineCacheAddRef(pipeline_cache: native::WGPUPipelineCache) {
    crate::ffi_utils::contain_entry("wgpuPipelineCacheAddRef", (), || {
        assert!(!pipeline_cache.is_null(), "invalid pipeline cache");
        Arc::increment_strong_count(pipeline_cache);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuPipelineCacheRelease(pipeline_cache: native::WGPUPipelineCache) {
    crate::ffi_utils::contain_entry("wgpuPipelineCacheRelease", (), || {
        assert!(!pipeline_cache.is_null(), "invalid pipeline cache");
        Arc::decrement_strong_count(pipeline_cache);
    })
}

unsafe fn create_shader_module_impl(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUShaderModuleDescriptor>,
    runtime_checks: wgt::ShaderRuntimeChecks,
    fn_ident: &'static str,
) -> native::WGPUShaderModule {
    let (device_id, context, error_sink) = {
        let device = device.as_ref().expect("invalid device");
        (device.id, &device.context, &device.error_sink)
    };
    let descriptor = descriptor.expect("invalid descriptor");

    let desc_label = string_view_into_label(descriptor.label);

    let source = match follow_chain!(
        map_shader_module((descriptor),
        WGPUSType_ShaderSourceSPIRV => native::WGPUShaderSourceSPIRV,
        WGPUSType_ShaderSourceWGSL => native::WGPUShaderSourceWGSL,
        WGPUSType_ShaderSourceGLSL => native::WGPUShaderSourceGLSL)
    ) {
        Ok(source) => source,
        Err(cause) => {
            handle_error(error_sink, cause, desc_label, fn_ident);

            return Arc::into_raw(Arc::new(WGPUShaderModuleImpl {
                context: context.clone(),
                id: None,
                messages: Vec::new(),
            }));
        }
    };

    let desc = wgc::pipeline::ShaderModuleDescriptor {
        label: desc_label,
        runtime_checks,
    };

    let (shader_module_id, error) =
        context.device_create_shader_module(device_id, &desc, source, None);
    // webgpu.h: the diagnostics of a module are what
    // wgpuShaderModuleGetCompilationInfo reports. wgpu-core hands them out
    // once, at creation, so they are kept on the module.
    let messages = match &error {
        Some(cause) => compilation_messages(cause),
        None => Vec::new(),
    };
    if let Some(cause) = error {
        handle_error(error_sink, cause, desc.label, fn_ident);
    }

    Arc::into_raw(Arc::new(WGPUShaderModuleImpl {
        context: context.clone(),
        id: Some(shader_module_id),
        messages,
    }))
}

/// Turns a wgpu-core shader creation error into the diagnostics webgpu.h
/// reports. naga carries the source and the labels, so line and column come
/// from the span; a variant without spans yields one message at 0:0.
fn compilation_messages(
    cause: &wgc::pipeline::CreateShaderModuleError,
) -> Vec<crate::objects::CompilationMessage> {
    use wgc::pipeline::CreateShaderModuleError as E;

    let error_type = native::WGPUCompilationMessageType_Error;
    let simple = |text: String| {
        vec![crate::objects::CompilationMessage {
            message: text,
            message_type: error_type,
            line_num: 0,
            line_pos: 0,
            offset: 0,
            length: 0,
        }]
    };

    match cause {
        E::Parsing(shader_error) => {
            let source = &shader_error.source;
            shader_error
                .inner
                .labels()
                .map(|(span, label)| {
                    let location = span.location(source);
                    crate::objects::CompilationMessage {
                        message: label.to_string(),
                        message_type: error_type,
                        line_num: location.line_number as u64,
                        line_pos: location.line_position as u64,
                        offset: location.offset as u64,
                        length: location.length as u64,
                    }
                })
                .collect()
        }
        other => simple(other.to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateShaderModule(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUShaderModuleDescriptor>,
) -> native::WGPUShaderModule {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateShaderModule",
        std::ptr::null(),
        || {

        // The standard path honours a chained WGPUShaderRuntimeChecksDescriptor
        // (pass1 finding 1): without it, wgpu-types' safe defaults apply.
        let runtime_checks = descriptor
            .and_then(|descriptor| {
                crate::utils::chain_entry::<native::WGPUShaderRuntimeChecksDescriptor>(
                    descriptor.nextInChain,
                    native::WGPUSType_ShaderRuntimeChecksDescriptor,
                )
            })
            .map(|checks| conv::map_shader_runtime_checks(checks.checks))
            .unwrap_or_default();
        create_shader_module_impl(
            device,
            descriptor,
            runtime_checks,
            "wgpuDeviceCreateShaderModule",
        )
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateShaderModuleTrusted(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUShaderModuleDescriptor>,
    runtime_checks: native::WGPUShaderRuntimeChecks,
) -> native::WGPUShaderModule {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateShaderModuleTrusted",
        std::ptr::null(),
        || {

        create_shader_module_impl(
            device,
            descriptor,
            map_shader_runtime_checks(runtime_checks),
            "wgpuDeviceCreateShaderModuleTrusted",
        )
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateTexture(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUTextureDescriptor>,
) -> native::WGPUTexture {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateTexture",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let desc = wgt::TextureDescriptor {
            label: string_view_into_label(descriptor.label),
            size: conv::map_extent3d(&descriptor.size),
            mip_level_count: descriptor.mipLevelCount,
            sample_count: descriptor.sampleCount,
            dimension: conv::map_texture_dimension(descriptor.dimension)
                .unwrap_or(wgt::TextureDimension::D2),
            format: conv::map_texture_format(descriptor.format)
                .expect("invalid texture format for texture descriptor"),
            usage: from_u64_bits(descriptor.usage)
                .expect("invalid texture usage for texture descriptor"),
            view_formats: make_slice(descriptor.viewFormats, descriptor.viewFormatCount)
                .iter()
                .map(|v| {
                    conv::map_texture_format(*v).expect("invalid view format for texture descriptor")
                })
                .collect(),
        };

        // webgpu.h: the binding view dimension arrives chained
        // (WGPUSType_TextureBindingViewDimension). wgc 30 does not resolve this
        // concept (gfx-rs/wgpu#7428) — resolved packer-side (conv::resolve_...).
        let texture_binding_view_dimension = conv::resolve_texture_binding_view_dimension(
            descriptor.dimension,
            descriptor.sampleCount,
            descriptor.size.depthOrArrayLayers,
            chained_texture_binding_view_dimension(descriptor),
        );

        let (texture_id, error) = context.device_create_texture(device_id, &desc, None);
        if let Some(cause) = error {
            handle_error(error_sink, cause, desc.label, "wgpuDeviceCreateTexture");
        }

        Arc::into_raw(Arc::new(WGPUTextureImpl {
            context: context.clone(),
            id: texture_id,
            error_sink: error_sink.clone(),
            surface_id: None,
            has_surface_presented: Arc::default(),
            data: TextureData {
                usage: descriptor.usage,
                dimension: descriptor.dimension,
                size: descriptor.size,
                format: descriptor.format,
                mip_level_count: descriptor.mipLevelCount,
                sample_count: descriptor.sampleCount,
                texture_binding_view_dimension,
            },
        }))
        },
    )
}

/// Reads WGPUTextureBindingViewDimension in the texture descriptor
/// chain (None if absent).
unsafe fn chained_texture_binding_view_dimension(
    descriptor: &native::WGPUTextureDescriptor,
) -> Option<native::WGPUTextureViewDimension> {
    let mut next = descriptor.nextInChain;
    while let Some(chain) = unsafe { next.as_ref() } {
        if chain.sType == native::WGPUSType_TextureBindingViewDimension {
            let extra = unsafe {
                &*(chain as *const native::WGPUChainedStruct
                    as *const native::WGPUTextureBindingViewDimension)
            };
            return Some(extra.textureBindingViewDimension);
        }
        next = chain.next;
    }
    None
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceDestroy(device: native::WGPUDevice) {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceDestroy",
        (),
        || {

        let (device_id, context) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context)
        };

        context.device_destroy(device_id);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceGetFeatures(
    device: native::WGPUDevice,
    features: *mut native::WGPUSupportedFeatures,
) {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceGetFeatures",
        (),
        || {

        let device = device.as_ref().expect("invalid device");
        let device_features = device.context.device_features(device.id);
        let features = features.as_mut().expect("invalid return pointer \"features\"");

        return_features(features, device_features);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceStartGraphicsDebuggerCapture(
    device: native::WGPUDevice,
) -> bool {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceStartGraphicsDebuggerCapture",
        false,
        || {

        let (device_id, context) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context)
        };

        // wgpu-core's `device_start_graphics_debugger_capture` returns nothing: it
        // discards the backend's `bool` (wgpu-hal reports whether the capture
        // actually started). The C API promises that bool, so the call goes
        // straight to wgpu-hal through `device_as_hal`, the only way to report a
        // real answer.
        #[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "metal"))]
        {
            use hal::Device as _;
            let hal_device = context.device_as_hal::<hal::api::Metal>(device_id);
            if let Some(hal_device) = hal_device {
                return unsafe { hal_device.start_graphics_debugger_capture() };
            }
            return false;
        }
        #[cfg(not(all(any(target_os = "ios", target_os = "macos"), feature = "metal")))]
        {
            context.device_start_graphics_debugger_capture(device_id);
            // No backend exposes a start result here: report that the capture did
            // not start rather than claiming it did.
            false
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceStopGraphicsDebuggerCapture(device: native::WGPUDevice) {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceStopGraphicsDebuggerCapture",
        (),
        || {

        let (device_id, context) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context)
        };

        context.device_stop_graphics_debugger_capture(device_id)
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceGetLimits(
    device: native::WGPUDevice,
    mut limits: Option<&mut native::WGPULimits>,
) -> native::WGPUStatus {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceGetLimits",
        native::WGPUStatus_Error,
        || {

        let device = device.as_ref().expect("invalid device");
        let wgt_limits = device.context.device_limits(device.id);
        let limits = limits.as_mut().expect("invalid return pointer \"limits\"");
        conv::write_limits_struct(wgt_limits, limits);
        // webgpu.h declares WGPUStatus (Success = 1, Error = 2): the limits and
        // any NativeLimits chain entry the caller set up are written above.
        native::WGPUStatus_Success
        },
    )
}

/// Returns the adapter this device was requested from (Dawn's
/// `wgpuDeviceGetAdapter`). The returned handle is a new strong reference the
/// caller releases with `wgpuAdapterRelease`.
#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceGetAdapter(
    device: native::WGPUDevice,
) -> native::WGPUAdapter {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceGetAdapter",
        std::ptr::null(),
        || {

        let device = device.as_ref().expect("invalid device");
        let adapter = Arc::into_raw(device.adapter.clone()) as native::WGPUAdapter;
        adapter
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceGetAdapterInfo(
    device: native::WGPUDevice,
    info: Option<&mut native::WGPUAdapterInfo>,
) -> native::WGPUStatus {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceGetAdapterInfo",
        native::WGPUStatus_Error,
        || {

        let device = device.as_ref().expect("invalid device");
        let info = info.expect("invalid return pointer \"info\"");

        let result = device.context.device_adapter_info(device.id);
        conv::write_adapter_info(info, &result);

        native::WGPUStatus_Success
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceGetLostFuture(device: native::WGPUDevice) -> native::WGPUFuture {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceGetLostFuture",
        native::WGPUFuture { id: 0 },
        || {

        let device = device.as_ref().expect("invalid device");

        // Mode of the deviceLostCallbackInfo: the one of the future created below.
        let mode = device.error_sink.lock().device_lost_mode;

        // Future created lazily, stable id afterwards. Loss is always
        // routed by the sink (error.rs) which registers the future on the first
        // report: here, this branch is only a safeguard — the future
        // resolves empty so as not to block a wait, WITHOUT firing the callback
        // again (the loss event is delivered only once).
        let mut lost = device.lost_future.lock();
        let future_id = match *lost {
            Some(id) => id,
            None => {
                let id =
                    crate::future::register(&device.context, mode, crate::future::Source::Cpu);
                *lost = Some(id);
                if device.device_lost.load(atomic::Ordering::SeqCst) {
                    crate::future::complete(id, Box::new(|| ()));
                }
                id
            }
        };

        native::WGPUFuture { id: future_id }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceGetQueue(device: native::WGPUDevice) -> native::WGPUQueue {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceGetQueue",
        std::ptr::null(),
        || {

        let (queue, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (&device.queue, &device.error_sink)
        };

        Arc::into_raw(Arc::new(WGPUQueueImpl {
            queue: queue.clone(),
            error_sink: error_sink.clone(),
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceGetNativeMetalDevice(device: native::WGPUDevice) -> *mut c_void {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceGetNativeMetalDevice",
        std::ptr::null_mut(),
        || {

        #[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "metal"))]
        {
            let device = device.as_ref().expect("invalid device");
            let hal_device = device.context.device_as_hal::<hal::api::Metal>(device.id);
            if let Some(hal_device) = hal_device {
                return &**hal_device.raw_device() as *const _ as *mut c_void;
            }
            std::ptr::null_mut()
        }
        #[cfg(not(all(any(target_os = "ios", target_os = "macos"), feature = "metal")))]
        {
            let _ = device;
            std::ptr::null_mut()
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceHasFeature(
    device: native::WGPUDevice,
    feature: native::WGPUFeatureName,
) -> native::WGPUBool {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceHasFeature",
        0,
        || {

        let (device_id, context) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context)
        };
        let device_features = context.device_features(device_id);

        let feature = match conv::map_feature(feature) {
            Some(feature) => feature,
            None => return false as native::WGPUBool,
        };

        device_features.contains(feature) as native::WGPUBool
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDevicePopErrorScope(
    device: native::WGPUDevice,
    mut callback_info: native::WGPUPopErrorScopeCallbackInfo,
) -> native::WGPUFuture {
    crate::ffi_utils::contain_entry(
        "wgpuDevicePopErrorScope",
        native::WGPUFuture { id: 0 },
        || {

        let device = device.as_ref().expect("invalid device");
        crate::future::normalize_callback_mode(&mut callback_info.mode);
        crate::future::validate_callback_mode(callback_info.mode);
        let callback = callback_info.callback.expect("invalid callback");
        let userdata = new_userdata!(callback_info);

        // Synchronous resolution (the scope is popped now): the future
        // completes before the return, delivery according to the mode.
        let future_id =
            crate::future::register(&device.context, callback_info.mode, crate::future::Source::Cpu);

        let (typ, message) = {
            let mut error_sink = device.error_sink.lock();
            // webgpu.h: an empty stack is not a fatal error — the callback
            // receives WGPUPopErrorScopeStatus_Error ("stack was empty").
            let Some(scope) = error_sink.scopes.pop() else {
                drop(error_sink);
                crate::future::complete(
                    future_id,
                    Box::new(move || {
                        callback(
                            native::WGPUPopErrorScopeStatus_Error,
                            native::WGPUErrorType_NoError,
                            EMPTY_STRING,
                            userdata.get_1(),
                            userdata.get_2(),
                        );
                    }),
                );
                return native::WGPUFuture { id: future_id };
            };
            match scope.error {
                Some(error) => {
                    let typ = match error {
                        crate::Error::OutOfMemory { .. } => native::WGPUErrorType_OutOfMemory,
                        crate::Error::Validation { .. } => native::WGPUErrorType_Validation,
                        // We handle device lost error early in ErrorSinkRaw::handle_error
                        // so we should never get device lost error here.
                        crate::Error::DeviceLost { .. } => unreachable!(),
                    };
                    (typ, Some(error.to_string()))
                }
                None => (native::WGPUErrorType_NoError, None),
            }
        };

        crate::future::complete(
            future_id,
            Box::new(move || {
                let view = match &message {
                    Some(message) => str_into_string_view(message),
                    None => EMPTY_STRING,
                };
                callback(
                    native::WGPUPopErrorScopeStatus_Success,
                    typ,
                    view,
                    userdata.get_1(),
                    userdata.get_2(),
                );
            }),
        );

        native::WGPUFuture { id: future_id }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDevicePushErrorScope(
    device: native::WGPUDevice,
    filter: native::WGPUErrorFilter,
) {
    crate::ffi_utils::contain_entry(
        "wgpuDevicePushErrorScope",
        (),
        || {

        let device = device.as_ref().expect("invalid device");
        let mut error_sink = device.error_sink.lock();
        error_sink.scopes.push(ErrorScope {
            error: None,
            filter: match filter {
                native::WGPUErrorFilter_Validation => ErrorFilter::Validation,
                native::WGPUErrorFilter_OutOfMemory => ErrorFilter::OutOfMemory,
                _ => panic!("invalid error filter"),
            },
        });
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceAddRef(device: native::WGPUDevice) {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceAddRef",
        (),
        || {

        assert!(!device.is_null(), "invalid device");
        Arc::increment_strong_count(device);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceRelease(device: native::WGPUDevice) {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceRelease",
        (),
        || {

        assert!(!device.is_null(), "invalid device");
        Arc::decrement_strong_count(device);
        },
    )
}

/// Polls the device, `wgpu-native`'s extension to the standard API.
///
/// `wait` selects between a non-blocking poll and a wait; `submissionIndex`
/// narrows the wait to one submission. Returns true when the queue is empty.
///
/// wgpu-core returns three states (`QueueEmpty`, `WaitSucceeded`,
/// `PollFailed`); the C API is a boolean, so `QueueEmpty` maps to true and the
/// other two to false. A poll error is delivered to the error sink (it is how
/// device loss surfaces) and never aborts.
#[no_mangle]
pub unsafe extern "C" fn wgpuDevicePoll(
    device: native::WGPUDevice,
    wait: bool,
    submission_index: Option<&native::WGPUSubmissionIndex>,
) -> bool {
    crate::ffi_utils::contain_entry(
        "wgpuDevicePoll",
        false,
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };

        let maintain = match wait {
            true => match submission_index {
                Some(&index) => wgt::PollType::Wait {
                    submission_index: Some(index),
                    timeout: None,
                },
                None => wgt::PollType::wait_indefinitely(),
            },
            false => wgt::PollType::Poll,
        };

        match context.device_poll(device_id, maintain) {
            Ok(wgt::PollStatus::QueueEmpty) => true,
            Ok(_) => false,
            // A poll error (device loss in particular) is a legitimate
            // error event: delivered to the sink — DeviceLost completes the
            // lost future there — never fatal. We cannot assert that the queue
            // is empty: we return false.
            Err(cause) => {
                handle_error(error_sink, cause, None, "wgpuDevicePoll");
                false
            }
        }
        },
    )
}

/// Creates a shader module from SPIR-V source, `wgpu-native`'s extension.
///
/// wgpu-core has a generic passthrough path
/// (`device_create_shader_module_passthrough`) that takes the source language
/// as data; this entry point is the SPIR-V one, and
/// `wgpuDeviceCreateShaderModuleTrusted` is the WGSL one.
#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateShaderModuleSpirV(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUShaderModuleDescriptorSpirV>,
) -> native::WGPUShaderModule {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateShaderModuleSpirV",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let source = Cow::Borrowed(make_slice(
            descriptor.source,
            descriptor.sourceSize as usize,
        ));

        let desc_label = string_view_into_label(descriptor.label);

        let desc = wgc::pipeline::ShaderModuleDescriptorPassthrough {
            label: desc_label.to_owned(),
            spirv: Some(source),
            ..Default::default()
        };

        let (shader_module_id, error) =
            context.device_create_shader_module_passthrough(device_id, &desc, None);
        let messages = match &error {
            Some(cause) => compilation_messages(cause),
            None => Vec::new(),
        };
        if let Some(cause) = error {
            handle_error(
                error_sink,
                cause,
                desc_label,
                "wgpuDeviceCreateShaderModuleSpirV",
            );
        }

        Arc::into_raw(Arc::new(WGPUShaderModuleImpl {
            context: context.clone(),
            id: Some(shader_module_id),
            messages,
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateShaderModulePassthrough(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUShaderModuleDescriptorPassthrough>,
) -> native::WGPUShaderModule {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateShaderModulePassthrough",
        std::ptr::null(),
        || {
        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor =
            descriptor.expect("invalid descriptor for wgpuDeviceCreateShaderModulePassthrough");
        let label = string_view_into_label(descriptor.label);

        let cow_u32 = |ptr: *const u32, words: usize| -> Option<Cow<[u32]>> {
            (words > 0).then(|| Cow::Borrowed(make_slice(ptr, words)))
        };
        let cow_u8 = |ptr: *const u8, size: usize| -> Option<Cow<[u8]>> {
            (size > 0).then(|| Cow::Borrowed(make_slice(ptr, size)))
        };
        let cow_str = |view: native::WGPUStringView| -> Option<Cow<str>> {
            unsafe { string_view_into_str(view) }.map(|s| s.to_string().into())
        };

        let desc = wgt::CreateShaderModuleDescriptorPassthrough {
            label,
            entry_points: make_slice(descriptor.entryPoints, descriptor.entryPointCount)
                .iter()
                .map(|entry| wgt::PassthroughShaderEntryPoint {
                    name: string_view_into_str(entry.name).unwrap_or("").to_string().into(),
                    workgroup_size: (
                        entry.workgroupSizeX,
                        entry.workgroupSizeY,
                        entry.workgroupSizeZ,
                    ),
                })
                .collect(),
            spirv: cow_u32(descriptor.spirv, descriptor.spirvWordCount),
            dxil: cow_u8(descriptor.dxil, descriptor.dxilSize),
            hlsl: cow_str(descriptor.hlsl),
            metallib: cow_u8(descriptor.metallib, descriptor.metallibSize),
            msl: cow_str(descriptor.msl),
            glsl: cow_str(descriptor.glsl),
            wgsl: cow_str(descriptor.wgsl),
        };

        // wgpu-core panics when every source is None: reject first.
        let has_source = [
            desc.spirv.is_some(),
            desc.dxil.is_some(),
            desc.hlsl.is_some(),
            desc.metallib.is_some(),
            desc.msl.is_some(),
            desc.glsl.is_some(),
            desc.wgsl.is_some(),
        ]
        .into_iter()
        .any(|has| has);
        if !has_source {
            log::error!(
                "wgpuDeviceCreateShaderModulePassthrough: no source set \
                 (one of spirv/dxil/hlsl/metallib/msl/glsl/wgsl is required)"
            );
            return std::ptr::null();
        }

        let (id, error) =
            context.device_create_shader_module_passthrough(device_id, &desc, None);
        if let Some(cause) = error {
            handle_error(
                error_sink,
                cause,
                desc.label,
                "wgpuDeviceCreateShaderModulePassthrough",
            );
            return std::ptr::null();
        }
        Arc::into_raw(Arc::new(WGPUShaderModuleImpl {
            context: context.clone(),
            id: Some(id),
            messages: Vec::new(),
        }))
        },
    )
}

/// Maps a WGPUComputeState (task/mesh stage of a mesh pipeline) to wgc's
/// ProgrammableStageDescriptor.
unsafe fn map_compute_state(
    state: &native::WGPUComputeState,
) -> wgc::pipeline::ProgrammableStageDescriptor {
    wgc::pipeline::ProgrammableStageDescriptor {
        module: state
            .module
            .as_ref()
            .expect("invalid shader module for stage")
            .id
            .expect("invalid shader module for stage"),
        entry_point: string_view_into_label(state.entryPoint),
        constants: make_slice(state.constants, state.constantCount)
            .iter()
            .map(|entry| {
                (
                    string_view_into_str(entry.key).unwrap_or("").to_string(),
                    entry.value,
                )
            })
            .collect(),
        zero_initialize_workgroup_memory: zero_init_from_chain(state.nextInChain),
    }
}

unsafe fn map_mesh_pipeline_descriptor(
    descriptor: &native::WGPUMeshPipelineDescriptor,
) -> wgc::pipeline::MeshPipelineDescriptor {
    wgc::pipeline::MeshPipelineDescriptor {
        label: string_view_into_label(descriptor.label),
        layout: descriptor.layout.as_ref().map(|v| v.id),
        task: descriptor.task.as_ref().map(|task| wgc::pipeline::TaskState {
            stage: map_compute_state(task),
        }),
        mesh: wgc::pipeline::MeshState {
            stage: map_compute_state(
                descriptor
                    .mesh
                    .as_ref()
                    .expect("invalid mesh stage for mesh pipeline descriptor"),
            ),
        },
        primitive: {
            follow_chain!(map_primitive_state((descriptor.primitive), WGPUSType_PrimitiveStateExtras => native::WGPUPrimitiveStateExtras))
        },
        depth_stencil: descriptor.depthStencil.as_ref().map(|desc| {
            let format = conv::map_texture_format(desc.format)
                .expect("invalid texture format for depth stencil state");

            // Validation per spec.
            if texture_format_has_depth(format) {
                if desc.depthWriteEnabled == native::WGPUOptionalBool_Undefined {
                    panic!("Depth write not specified for depth format")
                }
            } else if desc.depthWriteEnabled == native::WGPUOptionalBool_True {
                panic!("Depth write enabled for non-depth format")
            }

            wgt::DepthStencilState {
                format,
                depth_write_enabled: Some(desc.depthWriteEnabled == native::WGPUOptionalBool_True),
                // `WGPUCompareFunction_Undefined` is legal in webgpu.h: the
                // spec requires depthCompare only when depthWriteEnabled is
                // true or a stencil depthFailOp is not "keep". wgpu-core has
                // no undefined variant, so it maps to Always, which is the
                // identity of the comparison (every fragment passes).
                depth_compare: Some(
                    conv::map_compare_function(desc.depthCompare)
                        .expect("invalid depth compare function for depth stencil state")
                        .unwrap_or(wgt::CompareFunction::Always),
                ),
                stencil: wgt::StencilState {
                    front: conv::map_stencil_face_state(desc.stencilFront, "front"),
                    back: conv::map_stencil_face_state(desc.stencilBack, "back"),
                    read_mask: desc.stencilReadMask,
                    write_mask: desc.stencilWriteMask,
                },
                bias: wgt::DepthBiasState {
                    constant: desc.depthBias,
                    slope_scale: desc.depthBiasSlopeScale,
                    clamp: desc.depthBiasClamp,
                },
            }
        }),
        multisample: wgt::MultisampleState {
            count: descriptor.multisample.count,
            mask: descriptor.multisample.mask as u64,
            alpha_to_coverage_enabled: descriptor.multisample.alphaToCoverageEnabled != 0,
        },
        fragment: descriptor
            .fragment
            .as_ref()
            .map(|fragment| wgc::pipeline::FragmentState {
                stage: wgc::pipeline::ProgrammableStageDescriptor {
                    module: fragment
                        .module
                        .as_ref()
                        .expect("invalid fragment shader module for fragment state")
                        .id
                        .expect("invalid fragment shader module for fragment state"),
                    entry_point: string_view_into_label(fragment.entryPoint),
                    constants: make_slice(fragment.constants, fragment.constantCount)
                        .iter()
                        .map(|entry| {
                            (
                                string_view_into_str(entry.key).unwrap_or("").to_string(),
                                entry.value,
                            )
                        })
                        .collect(),
                    // No C knob for this yet: the WGSL spec default is false.
                    zero_initialize_workgroup_memory: zero_init_from_chain(fragment.nextInChain),
                },
                targets: Cow::Owned(
                    make_slice(fragment.targets, fragment.targetCount)
                        .iter()
                        .map(|color_target| {
                            conv::map_texture_format(color_target.format).map(|format| {
                                wgt::ColorTargetState {
                                    format,
                                    blend: color_target.blend.as_ref().map(|blend| {
                                        wgt::BlendState {
                                            color: conv::map_blend_component(blend.color),
                                            alpha: conv::map_blend_component(blend.alpha),
                                        }
                                    }),
                                    write_mask: from_u64_bits(color_target.writeMask).unwrap(),
                                }
                            })
                        })
                        .collect(),
                ),
            }),
        multiview: std::num::NonZeroU32::new(descriptor.multiview),
        cache: descriptor
            .cache
            .as_ref()
            .map(|cache| cache.id),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateMeshPipeline(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUMeshPipelineDescriptor>,
) -> native::WGPURenderPipeline {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateMeshPipeline",
        std::ptr::null(),
        || {

        let (device_id, context, error_sink) = {
            let device = device.as_ref().expect("invalid device");
            (device.id, &device.context, &device.error_sink)
        };
        let descriptor = descriptor.expect("invalid descriptor");

        let desc = map_mesh_pipeline_descriptor(descriptor);

        let (pipeline_id, error) = context.device_create_mesh_pipeline(device_id, &desc, None);
        if let Some(cause) = error {
            if let wgc::pipeline::CreateRenderPipelineError::Internal { stage, ref error } = cause {
                log::error!("Shader translation error for stage {:?}: {}", stage, error);
                log::error!("Please report it to https://github.com/gfx-rs/wgpu");
            }
            handle_error(error_sink, cause, desc.label, "wgpuDeviceCreateMeshPipeline");
            return std::ptr::null();
        }

        Arc::into_raw(Arc::new(WGPURenderPipelineImpl {
            context: context.clone(),
            id: pipeline_id,
            error_sink: error_sink.clone(),
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceGetInternalCounters(
    device: native::WGPUDevice,
    counters: *mut native::WGPUInternalCounters,
) {
    crate::ffi_utils::contain_entry("wgpuDeviceGetInternalCounters", (), || {
        let device = device.as_ref().expect("invalid device");
        let counters = counters.as_mut().expect("invalid return pointer");
        let internal = device.context.device_get_internal_counters(device.id);
        macro_rules! read {
            ($field:ident) => {
                internal.hal.$field.read() as i64
            };
        }
        *counters = native::WGPUInternalCounters {
            Buffers: read!(buffers),
            Textures: read!(textures),
            TextureViews: read!(texture_views),
            BindGroups: read!(bind_groups),
            BindGroupLayouts: read!(bind_group_layouts),
            RenderPipelines: read!(render_pipelines),
            ComputePipelines: read!(compute_pipelines),
            RayTracingPipelines: read!(ray_tracing_pipelines),
            PipelineLayouts: read!(pipeline_layouts),
            Samplers: read!(samplers),
            CommandEncoders: read!(command_encoders),
            ShaderModules: read!(shader_modules),
            QuerySets: read!(query_sets),
            Fences: read!(fences),
            BufferMemory: read!(buffer_memory),
            TextureMemory: read!(texture_memory),
            AccelerationStructureMemory: read!(acceleration_structure_memory),
            MemoryAllocations: read!(memory_allocations),
        };
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceGetAllocatorReport(
    device: native::WGPUDevice,
    report: *mut native::WGPUAllocatorReport,
) -> native::WGPUStatus {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceGetAllocatorReport",
        native::WGPUStatus_Error,
        || {
        let device = device.as_ref().expect("invalid device");
        let report = report.as_mut().expect("invalid return pointer");
        match device.context.device_generate_allocator_report(device.id) {
            Some(allocation) => {
                *report = native::WGPUAllocatorReport {
                    nextInChain: std::ptr::null(),
                    totalAllocatedBytes: allocation.total_allocated_bytes,
                    totalReservedBytes: allocation.total_reserved_bytes,
                    allocationCount: allocation.allocations.len(),
                    blockCount: allocation.blocks.len(),
                };
                native::WGPUStatus_Success
            }
            // Backends that allocate directly have no allocator report:
            // totals at zero is the honest answer.
            None => {
                *report = native::WGPUAllocatorReport {
                    nextInChain: std::ptr::null(),
                    totalAllocatedBytes: 0,
                    totalReservedBytes: 0,
                    allocationCount: 0,
                    blockCount: 0,
                };
                native::WGPUStatus_Success
            }
        }
        },
    )
}
