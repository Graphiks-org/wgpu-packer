use std::{
    ffi::c_void,
    sync::Arc,
};
use crate::utils::str_into_string_view;
use wgc::resource;

use crate::conv;
use crate::new_userdata;
use crate::ffi_utils::catch_unwind_contained;
use crate::error::*;
use crate::native;

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferGetMapState(
    buffer: native::WGPUBuffer,
) -> native::WGPUBufferMapState {
    let buffer = buffer.as_ref().expect("invalid buffer");
    *buffer.data.map_state.lock()
}

/// Validates and returns the pointer of the mapped range for Read/WriteMappedRange
/// (None = no copy performed). WGPU_WHOLE_MAP_SIZE is not accepted here
/// (webgpu.h: explicit size required).
unsafe fn mapped_range_ptr(
    buffer: native::WGPUBuffer,
    offset: usize,
    size: usize,
    fn_ident: &'static str,
) -> Option<*mut u8> {
    if size == conv::WGPU_WHOLE_MAP_SIZE {
        return None;
    }
    catch_unwind_contained(fn_ident, || {
        let (buffer_id, context) = {
            let buffer = buffer.as_ref().expect("invalid buffer");
            (buffer.id, &buffer.context)
        };
        context
            .buffer_get_mapped_range(buffer_id, offset as wgt::BufferAddress, Some(size as u64))
            .map(|(ptr, _)| ptr.as_ptr())
            .map_err(|err| log::error!("{fn_ident}: {err}"))
            .ok()
    })
    .flatten()
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferReadMappedRange(
    buffer: native::WGPUBuffer,
    offset: usize,
    data: *mut c_void,
    size: usize,
) -> native::WGPUStatus {
    if data.is_null() {
        return native::WGPUStatus_Error;
    }
    match mapped_range_ptr(buffer, offset, size, "wgpuBufferReadMappedRange") {
        Some(ptr) => {
            std::ptr::copy_nonoverlapping(ptr as *const u8, data as *mut u8, size);
            native::WGPUStatus_Success
        }
        None => native::WGPUStatus_Error,
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferWriteMappedRange(
    buffer: native::WGPUBuffer,
    offset: usize,
    data: *const c_void,
    size: usize,
) -> native::WGPUStatus {
    if data.is_null() {
        return native::WGPUStatus_Error;
    }
    match mapped_range_ptr(buffer, offset, size, "wgpuBufferWriteMappedRange") {
        Some(ptr) => {
            std::ptr::copy_nonoverlapping(data as *const u8, ptr, size);
            native::WGPUStatus_Success
        }
        None => native::WGPUStatus_Error,
    }
}

// Buffer methods

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferDestroy(buffer: native::WGPUBuffer) {
    let (buffer_id, context, map_state) = {
        let buffer = buffer.as_ref().expect("invalid buffer");
        (buffer.id, &buffer.context, buffer.data.map_state.clone())
    };
    // Per spec, no error to report. Even calling destroy multiple times is valid.
    context.buffer_destroy(buffer_id);
    // Destroy invalidates the mapping (aborts a pending map via callback).
    *map_state.lock() = native::WGPUBufferMapState_Unmapped;
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferGetConstMappedRange(
    buffer: native::WGPUBuffer,
    offset: usize,
    size: usize,
) -> *const u8 {
    // Spec §4.1: no panic crosses the C ABI — an unmapped buffer
    // returns NULL (C contract) instead of the inherited handle_error_fatal.
    let mapped = catch_unwind_contained("wgpuBufferGetConstMappedRange", || {
        let (buffer_id, context) = {
            let buffer = buffer.as_ref().expect("invalid buffer");
            (buffer.id, &buffer.context)
        };
        context
            .buffer_get_mapped_range(
                buffer_id,
                offset as wgt::BufferAddress,
                match size {
                    conv::WGPU_WHOLE_MAP_SIZE => None,
                    _ => Some(size as u64),
                },
            )
            .map(|(ptr, _)| ptr)
            .map_err(|err| log::error!("wgpuBufferGetConstMappedRange: {err}"))
            .ok()
    })
    .flatten();

    match mapped {
        Some(ptr) => ptr.as_ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferGetMappedRange(
    buffer: native::WGPUBuffer,
    offset: usize,
    size: usize,
) -> *mut u8 {
    // Spec §4.1: no panic crosses the C ABI — an unmapped buffer
    // returns NULL (C contract) instead of the inherited handle_error_fatal.
    let mapped = catch_unwind_contained("wgpuBufferGetMappedRange", || {
        let (buffer_id, context) = {
            let buffer = buffer.as_ref().expect("invalid buffer");
            (buffer.id, &buffer.context)
        };
        context
            .buffer_get_mapped_range(
                buffer_id,
                offset as wgt::BufferAddress,
                match size {
                    conv::WGPU_WHOLE_MAP_SIZE => None,
                    _ => Some(size as u64),
                },
            )
            .map(|(ptr, _)| ptr)
            .map_err(|err| log::error!("wgpuBufferGetMappedRange: {err}"))
            .ok()
    })
    .flatten();

    match mapped {
        Some(ptr) => ptr.as_ptr(),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferGetSize(buffer: native::WGPUBuffer) -> u64 {
    let buffer = buffer.as_ref().expect("invalid buffer");
    buffer.data.size
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferGetUsage(buffer: native::WGPUBuffer) -> native::WGPUBufferUsage {
    let buffer = buffer.as_ref().expect("invalid buffer");
    buffer.data.usage
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferMapAsync(
    buffer: native::WGPUBuffer,
    mode: native::WGPUMapMode,
    offset: usize,
    size: usize,
    callback_info: native::WGPUBufferMapCallbackInfo,
) -> native::WGPUFuture {
    let (buffer_id, context, error_sink, map_state, queue_id) = {
        let buffer = buffer.as_ref().expect("invalid buffer");
        (
            buffer.id,
            &buffer.context,
            &buffer.error_sink,
            buffer.data.map_state.clone(),
            buffer.data.queue,
        )
    };
    crate::future::validate_callback_mode(callback_info.mode);
    let callback = callback_info.callback.expect("invalid callback");
    let userdata = new_userdata!(callback_info);
    let map_state_in_callback = map_state.clone();

    // Packer-side future (timeline: queue of the creating device). The
    // internal wgc closure updates the map state then completes the future;
    // delivery follows the mode of the callbackInfo.
    let future_id = crate::future::register(
        context,
        callback_info.mode,
        crate::future::Source::Queue(queue_id),
    );

    // Copies owned by the wgc closure (Copy types): the payload built
    // at completion time derives its own copies from them — everything is
    // 'static, delivery can be deferred by the mode.
    let callback_for_op = callback;
    let userdata_for_op = userdata;

    let operation = wgc::resource::BufferMapOperation {
        host: match mode as native::WGPUMapMode {
            native::WGPUMapMode_Write => wgc::device::HostMap::Write,
            native::WGPUMapMode_Read => wgc::device::HostMap::Read,
            _ => panic!("invalid map mode"),
        },
        callback: Some(Box::new(move |result: resource::BufferAccessResult| {
            // State tracked packer-side (wgpuBufferGetMapState): kept in sync with
            // the result delivered to the consumer.
            *map_state_in_callback.lock() = if result.is_ok() {
                native::WGPUBufferMapState_Mapped
            } else {
                native::WGPUBufferMapState_Unmapped
            };

            let (status, message) = match result {
                Ok(()) => (native::WGPUMapAsyncStatus_Success, String::default()),
                Err(cause) => {
                    let code = match cause {
                        resource::BufferAccessError::MapAborted => {
                            native::WGPUMapAsyncStatus_Aborted
                        }
                        _ => native::WGPUMapAsyncStatus_Error,
                    };

                    (code, format_error(&cause))
                }
            };

            crate::future::complete(future_id, Box::new(move || {
                callback_for_op(
                    status,
                    str_into_string_view(&message),
                    userdata_for_op.get_1(),
                    userdata_for_op.get_2(),
                );
            }));
        })),
    };

    if let Err(cause) = context.buffer_map_async(
        buffer_id,
        offset as wgt::BufferAddress,
        // webgpu.h: WGPU_WHOLE_MAP_SIZE (SIZE_MAX) = up to the end of the
        // buffer (wgc: None). GetMappedRange already applies this rule.
        match size {
            conv::WGPU_WHOLE_MAP_SIZE => None,
            _ => Some(size as wgt::BufferAddress),
        },
        operation,
    ) {
        // Synchronous rejection (validation): error routed as before AND the
        // future is completed anyway — awaiting it must not
        // block forever.
        let message = format_error(&cause);
        handle_error(error_sink, cause, None, "wgpuBufferMapAsync");
        *map_state.lock() = native::WGPUBufferMapState_Unmapped;
        let status = native::WGPUMapAsyncStatus_Error;
        crate::future::complete(
            future_id,
            Box::new(move || {
                callback(
                    status,
                    str_into_string_view(&message),
                    userdata.get_1(),
                    userdata.get_2(),
                );
            }),
        );
    } else {
        *map_state.lock() = native::WGPUBufferMapState_Pending;
    };

    native::WGPUFuture { id: future_id }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferUnmap(buffer: native::WGPUBuffer) {
    let (buffer_id, context, error_sink, map_state) = {
        let buffer = buffer.as_ref().expect("invalid buffer");
        (
            buffer.id,
            &buffer.context,
            &buffer.error_sink,
            buffer.data.map_state.clone(),
        )
    };

    if let Err(cause) = context.buffer_unmap(buffer_id) {
        handle_error(error_sink, cause, None, "wgpuBufferUnmap");
    }
    *map_state.lock() = native::WGPUBufferMapState_Unmapped;
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBufferAddRef(buffer: native::WGPUBuffer) {
    assert!(!buffer.is_null(), "invalid buffer");
    Arc::increment_strong_count(buffer);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuBufferRelease(buffer: native::WGPUBuffer) {
    assert!(!buffer.is_null(), "invalid buffer");
    Arc::decrement_strong_count(buffer);
}
