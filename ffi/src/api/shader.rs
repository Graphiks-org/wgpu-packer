use std::sync::Arc;

use crate::native;
use crate::new_userdata;
use crate::utils::str_into_string_view;

// ShaderModule methods

#[no_mangle]
pub unsafe extern "C" fn wgpuShaderModuleAddRef(shader_module: native::WGPUShaderModule) {
    assert!(!shader_module.is_null(), "invalid shader module");
    Arc::increment_strong_count(shader_module);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuShaderModuleRelease(shader_module: native::WGPUShaderModule) {
    assert!(!shader_module.is_null(), "invalid shader module");
    Arc::decrement_strong_count(shader_module);
}

/// Reports the diagnostics produced when this module was created
/// (webgpu.h `wgpuShaderModuleGetCompilationInfo`).
///
/// wgpu-core hands the creation error out once, at creation; the module keeps
/// it (see `WGPUShaderModuleImpl::messages`). A module that compiled cleanly
/// reports an empty message list with `Success`.
#[no_mangle]
pub unsafe extern "C" fn wgpuShaderModuleGetCompilationInfo(
    shader_module: native::WGPUShaderModule,
    callback_info: native::WGPUCompilationInfoCallbackInfo,
) -> native::WGPUFuture {
    let shader_module = shader_module.as_ref().expect("invalid shader module");
    crate::future::validate_callback_mode(callback_info.mode);
    let callback = callback_info.callback.expect("invalid callback");
    let userdata = new_userdata!(callback_info);

    // Synchronous resolution: the diagnostics are already known.
    let future_id = crate::future::register(
        &shader_module.context,
        callback_info.mode,
        crate::future::Source::Cpu,
    );

    // The payload must be Send, and raw pointers are not: carry plain values
    // and build the C structs inside the closure.
    let messages: Vec<(String, native::WGPUCompilationMessageType, u64, u64, u64, u64)> =
        shader_module
            .messages
            .iter()
            .map(|m| {
                (
                    m.message.clone(),
                    m.message_type,
                    m.line_num,
                    m.line_pos,
                    m.offset,
                    m.length,
                )
            })
            .collect();

    crate::future::complete(
        future_id,
        Box::new(move || {
            let c_messages: Vec<native::WGPUCompilationMessage> = messages
                .iter()
                .map(|(text, kind, line, pos, offset, length)| native::WGPUCompilationMessage {
                    nextInChain: std::ptr::null_mut(),
                    message: str_into_string_view(text),
                    type_: *kind,
                    lineNum: *line,
                    linePos: *pos,
                    offset: *offset,
                    length: *length,
                })
                .collect();
            let info = native::WGPUCompilationInfo {
                nextInChain: std::ptr::null_mut(),
                messageCount: c_messages.len(),
                messages: if c_messages.is_empty() {
                    std::ptr::null()
                } else {
                    c_messages.as_ptr()
                },
            };
            callback(
                native::WGPUCompilationInfoRequestStatus_Success,
                &info,
                userdata.get_1(),
                userdata.get_2(),
            );
        }),
    );

    native::WGPUFuture { id: future_id }
}
