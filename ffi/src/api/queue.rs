use smallvec::SmallVec;
use std::{
    ffi::c_void,
    sync::{atomic, Arc},
};
use crate::utils::make_slice;

use crate::callbacks::*;
use crate::conv;
use crate::new_userdata;
use crate::error::*;
use crate::native;

// Queue methods

#[no_mangle]
pub unsafe extern "C" fn wgpuQueueOnSubmittedWorkDone(
    queue: native::WGPUQueue,
    callback_info: native::WGPUQueueWorkDoneCallbackInfo,
) -> native::WGPUFuture {
    let (queue_id, context) = {
        let queue = queue.as_ref().expect("invalid queue");
        (queue.queue.id, &queue.queue.context)
    };
    crate::future::validate_callback_mode(callback_info.mode);
    let callback = callback_info.callback.expect("invalid callback");
    let userdata = new_userdata!(callback_info);

    // Packer-side future (timeline: this queue). wgc fires the closure at the
    // next poll covering the submission; delivery follows the mode.
    let future_id = crate::future::register(
        context,
        callback_info.mode,
        crate::future::Source::Queue(queue_id),
    );

    let closure: wgc::device::queue::SubmittedWorkDoneClosure = Box::new(move || {
        crate::future::complete(
            future_id,
            Box::new(move || {
                callback(
                    native::WGPUQueueWorkDoneStatus_Success,
                    EMPTY_STRING,
                    userdata.get_1(),
                    userdata.get_2(),
                );
            }),
        );
    });

    context.queue_on_submitted_work_done(queue_id, closure);

    native::WGPUFuture { id: future_id }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQueueSubmit(
    queue: native::WGPUQueue,
    command_count: usize,
    commands: *const native::WGPUCommandBuffer,
) {
    let (queue_id, context, error_sink) = {
        let queue = queue.as_ref().expect("invalid queue");
        (queue.queue.id, &queue.queue.context, &queue.error_sink)
    };

    let command_buffers = make_slice(commands, command_count)
        .iter()
        .map(|command_buffer| {
            let command_buffer = command_buffer.as_ref().expect("invalid command buffer");
            command_buffer.open.store(true, atomic::Ordering::SeqCst);
            command_buffer.id
        })
        .collect::<SmallVec<[_; 4]>>();

    if let Err(cause) = context.queue_submit(queue_id, &command_buffers) {
        // WebGPU semantics: a validation error at submit is delivered to the
        // sink (current error scope or uncaptured error handler) — the call
        // returns, the process never stops.
        handle_error(error_sink, cause.1, None, "wgpuQueueSubmit");
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQueueGetTimestampPeriod(queue: native::WGPUQueue) -> f32 {
    let (queue_id, context) = {
        let queue = queue.as_ref().expect("invalid queue");
        (queue.queue.id, &queue.queue.context)
    };

    context.queue_get_timestamp_period(queue_id)
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQueueGetNativeMetalCommandQueue(
    queue: native::WGPUQueue,
) -> *mut c_void {
    #[cfg(all(any(target_os = "ios", target_os = "macos"), feature = "metal"))]
    {
        let queue = queue.as_ref().expect("invalid queue");
        // wgpu-hal v29 no longer exposes a public accessor for the raw
        // MTLCommandQueue. Return null until upstream adds one back.
        log::warn!(
            "wgpuQueueGetNativeMetalCommandQueue: not available in current wgpu-hal version"
        );
        let _ = queue;
        std::ptr::null_mut()
    }
    #[cfg(not(all(any(target_os = "ios", target_os = "macos"), feature = "metal")))]
    {
        let _ = queue;
        std::ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQueueWriteBuffer(
    queue: native::WGPUQueue,
    buffer: native::WGPUBuffer,
    buffer_offset: u64,
    data: *const c_void,
    data_size: usize,
) {
    let (queue_id, context, error_sink) = {
        let queue = queue.as_ref().expect("invalid queue");
        (queue.queue.id, &queue.queue.context, &queue.error_sink)
    };
    let buffer_id = buffer.as_ref().expect("invalid buffer").id;

    if let Err(cause) = context.queue_write_buffer(
        queue_id,
        buffer_id,
        buffer_offset,
        make_slice(data.cast(), data_size),
    ) {
        handle_error(error_sink, cause, None, "wgpuQueueWriteBuffer");
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQueueWriteTexture(
    queue: native::WGPUQueue,
    destination: Option<&native::WGPUTexelCopyTextureInfo>,
    data: *const c_void,
    data_size: usize,
    data_layout: Option<&native::WGPUTexelCopyBufferLayout>,
    write_size: Option<&native::WGPUExtent3D>,
) {
    let (queue_id, context, error_sink) = {
        let queue = queue.as_ref().expect("invalid queue");
        (queue.queue.id, &queue.queue.context, &queue.error_sink)
    };

    if let Err(cause) = context.queue_write_texture(
        queue_id,
        &conv::map_image_copy_texture(destination.expect("invalid destination")),
        make_slice(data.cast(), data_size),
        &conv::map_texture_data_layout(data_layout.expect("invalid data layout")),
        &conv::map_extent3d(write_size.expect("invalid write size")),
    ) {
        handle_error(error_sink, cause, None, "wgpuQueueWriteTexture");
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQueueAddRef(queue: native::WGPUQueue) {
    assert!(!queue.is_null(), "invalid queue");
    Arc::increment_strong_count(queue);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuQueueRelease(queue: native::WGPUQueue) {
    assert!(!queue.is_null(), "invalid queue");
    Arc::decrement_strong_count(queue);
}

#[no_mangle]
pub unsafe extern "C" fn wgpuQueueSubmitForIndex(
    queue: native::WGPUQueue,
    command_count: usize,
    commands: *const native::WGPUCommandBuffer,
) -> native::WGPUSubmissionIndex {
    let (queue_id, context, error_sink) = {
        let queue = queue.as_ref().expect("invalid queue");
        (queue.queue.id, &queue.queue.context, &queue.error_sink)
    };

    let command_buffers = make_slice(commands, command_count)
        .iter()
        .map(|command_buffer| {
            let command_buffer = command_buffer.as_ref().expect("invalid command buffer");
            command_buffer.open.store(true, atomic::Ordering::SeqCst);
            command_buffer.id
        })
        .collect::<SmallVec<[_; 4]>>();

    match context.queue_submit(queue_id, &command_buffers) {
        Ok(submission_index) => submission_index,
        // WebGPU semantics: error delivered to the sink (scope or uncaptured
        // handler), the call returns. The index reserved before validation
        // is still returned — that is the role of the queue_submit error tuple.
        Err(cause) => {
            handle_error(error_sink, cause.1, None, "wgpuQueueSubmitForIndex");
            cause.0
        }
    }
}
