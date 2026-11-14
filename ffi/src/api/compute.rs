use std::sync::Arc;
use crate::utils::{
    make_slice, string_view_into_str,
};

use crate::error::*;
use crate::native;
use crate::objects::*;

// ComputePassEncoder methods

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderDispatchWorkgroups(
    pass: native::WGPUComputePassEncoder,
    workgroup_count_x: u32,
    workgroup_count_y: u32,
    workgroup_count_z: u32,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass.context.compute_pass_dispatch_workgroups(
        encoder,
        workgroup_count_x,
        workgroup_count_y,
        workgroup_count_z,
    ) {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderDispatchWorkgroups",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderDispatchWorkgroupsIndirect(
    pass: native::WGPUComputePassEncoder,
    indirect_buffer: native::WGPUBuffer,
    indirect_offset: u64,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let indirect_buffer_id = indirect_buffer
        .as_ref()
        .expect("invalid indirect buffer")
        .id;

    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass.context.compute_pass_dispatch_workgroups_indirect(
        encoder,
        indirect_buffer_id,
        indirect_offset,
    ) {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderDispatchWorkgroupsIndirect",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderEnd(pass: native::WGPUComputePassEncoder) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass.context.compute_pass_end(encoder) {
        Ok(()) => (),
        Err(cause) => handle_error(&pass.error_sink, cause, None, "wgpuComputePassEncoderEnd"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderInsertDebugMarker(
    pass: native::WGPUComputePassEncoder,
    marker_label: native::WGPUStringView,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass.context.compute_pass_insert_debug_marker(
        encoder,
        string_view_into_str(marker_label).unwrap_or(""),
        0,
    ) {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderInsertDebugMarker",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderPopDebugGroup(pass: native::WGPUComputePassEncoder) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass.context.compute_pass_pop_debug_group(encoder) {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderPopDebugGroup",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderPushDebugGroup(
    pass: native::WGPUComputePassEncoder,
    group_label: native::WGPUStringView,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass.context.compute_pass_push_debug_group(
        encoder,
        string_view_into_str(group_label).unwrap_or(""),
        0,
    ) {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderPushDebugGroup",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderSetBindGroup(
    pass: native::WGPUComputePassEncoder,
    group_index: u32,
    bind_group: native::WGPUBindGroup,
    dynamic_offset_count: usize,
    dynamic_offsets: *const u32,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let bind_group_id = bind_group.as_ref().map(|bg| bg.id);
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass.context.compute_pass_set_bind_group(
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
            "wgpuComputePassEncoderSetBindGroup",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderSetPipeline(
    pass: native::WGPUComputePassEncoder,
    compute_pipeline: native::WGPUComputePipeline,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let compute_pipeline_id = compute_pipeline
        .as_ref()
        .expect("invalid compute pipeline")
        .id;
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass
        .context
        .compute_pass_set_pipeline(encoder, compute_pipeline_id)
    {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderSetPipeline",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderAddRef(
    compute_pass_encoder: native::WGPUComputePassEncoder,
) {
    assert!(
        !compute_pass_encoder.is_null(),
        "invalid command pass encoder"
    );
    Arc::increment_strong_count(compute_pass_encoder);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderRelease(
    compute_pass_encoder: native::WGPUComputePassEncoder,
) {
    assert!(
        !compute_pass_encoder.is_null(),
        "invalid command pass encoder"
    );
    Arc::decrement_strong_count(compute_pass_encoder);
}

// ComputePipeline methods

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePipelineGetBindGroupLayout(
    pipeline: native::WGPUComputePipeline,
    group_index: u32,
) -> native::WGPUBindGroupLayout {
    let (pipeline_id, context, error_sink) = {
        let pipeline = pipeline.as_ref().expect("invalid pipeline");
        (pipeline.id, &pipeline.context, &pipeline.error_sink)
    };

    let (bind_group_layout_id, error) =
        context.compute_pipeline_get_bind_group_layout(pipeline_id, group_index, None);
    if let Some(cause) = error {
        handle_error(
            error_sink,
            cause,
            None,
            "wgpuComputePipelineGetBindGroupLayout",
        );
    }

    Arc::into_raw(Arc::new(WGPUBindGroupLayoutImpl {
        context: context.clone(),
        id: bind_group_layout_id,
    }))
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePipelineAddRef(compute_pipeline: native::WGPUComputePipeline) {
    assert!(!compute_pipeline.is_null(), "invalid command pipeline");
    Arc::increment_strong_count(compute_pipeline);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuComputePipelineRelease(compute_pipeline: native::WGPUComputePipeline) {
    assert!(!compute_pipeline.is_null(), "invalid command pipeline");
    Arc::decrement_strong_count(compute_pipeline);
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderSetImmediates(
    pass: native::WGPUComputePassEncoder,
    offset: u32,
    data: *const u8,
    size: u32,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass
        .context
        .compute_pass_set_immediates(encoder, offset, make_slice(data, size as usize))
    {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderSetImmediates",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderBeginPipelineStatisticsQuery(
    pass: native::WGPUComputePassEncoder,
    query_set: native::WGPUQuerySet,
    query_index: u32,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let query_set_id = query_set.as_ref().expect("invalid query set").id;
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass.context.compute_pass_begin_pipeline_statistics_query(
        encoder,
        query_set_id,
        query_index,
    ) {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderBeginPipelineStatisticsQuery",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderEndPipelineStatisticsQuery(
    pass: native::WGPUComputePassEncoder,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass
        .context
        .compute_pass_end_pipeline_statistics_query(encoder)
    {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderEndPipelineStatisticsQuery",
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuComputePassEncoderWriteTimestamp(
    pass: native::WGPUComputePassEncoder,
    query_set: native::WGPUQuerySet,
    query_index: u32,
) {
    let pass = pass.as_ref().expect("invalid compute pass");
    let query_set_id = query_set.as_ref().expect("invalid query set").id;
    let encoder = pass.encoder.as_mut().expect("invalid compute pass encoder");

    match pass
        .context
        .compute_pass_write_timestamp(encoder, query_set_id, query_index)
    {
        Ok(()) => (),
        Err(cause) => handle_error(
            &pass.error_sink,
            cause,
            None,
            "wgpuComputePassEncoderWriteTimestamp",
        ),
    }
}
