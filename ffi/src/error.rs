use parking_lot::Mutex;
use std::{
    error,
    fmt::Display,
    sync::{
        atomic,
        Arc, Weak,
    },
};
use crate::utils::{str_into_string_view, Userdata};
use wgc::Label;

use crate::callbacks::*;
use crate::native;
use crate::objects::*;

#[derive(Debug)]
pub enum Error {
    DeviceLost {
        source: Box<dyn error::Error + Send + 'static>,
    },
    OutOfMemory {
        source: Box<dyn error::Error + Send + 'static>,
    },
    Validation {
        source: Box<dyn error::Error + Send + 'static>,
        description: String,
    },
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::DeviceLost { source } => Some(source.as_ref()),
            Error::OutOfMemory { source } => Some(source.as_ref()),
            Error::Validation { source, .. } => Some(source.as_ref()),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DeviceLost { .. } => f.write_str("Device lost"),
            Error::OutOfMemory { .. } => f.write_str("Out of Memory"),
            Error::Validation { description, .. } => f.write_str(description),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub enum ErrorFilter {
    /// Catch only out-of-memory errors.
    OutOfMemory,
    /// Catch only validation errors.
    Validation,
}

pub(crate) type ErrorSink = Arc<Mutex<ErrorSinkRaw>>;

pub(crate) struct ErrorScope {
    pub(crate) error: Option<crate::Error>,
    pub(crate) filter: crate::ErrorFilter,
}

pub(crate) struct ErrorSinkRaw {
    pub(crate) scopes: Vec<ErrorScope>,
    pub(crate) uncaptured_handler: UncapturedErrorCallback,
    pub(crate) device_lost_handler: DeviceLostCallback,
    /// Mode of the deviceLostCallbackInfo: governs the delivery of the callback and
    /// the completion of the lost future (wgpuDeviceGetLostFuture).
    pub(crate) device_lost_mode: native::WGPUCallbackMode,
    pub(crate) device: Weak<WGPUDeviceImpl>,
}

impl ErrorSinkRaw {
    pub(crate) fn new(
        device_lost_handler: DeviceLostCallback,
        device_lost_mode: native::WGPUCallbackMode,
        device: Weak<WGPUDeviceImpl>,
    ) -> ErrorSinkRaw {
        ErrorSinkRaw {
            scopes: Vec::new(),
            uncaptured_handler: DEFAULT_UNCAPTURED_ERROR_HANDLER,
            device_lost_handler,
            device_lost_mode,
            device,
        }
    }

    fn handle_error(&mut self, err: crate::Error) {
        let (typ, filter) = match err {
            crate::Error::DeviceLost { .. } => {
                // Loss is delivered only ONCE: the first report creates
                // the lost future if wgpuDeviceGetLostFuture has not been called
                // yet, then completes it — delivery (immediate or deferred)
                // follows the mode of the deviceLostCallbackInfo. Subsequent
                // reports are ignored (any post-loss operation fails
                // with DeviceError::Lost).
                let message = err.to_string();
                if let Some(device) = self.device.upgrade() {
                    if !device.device_lost.swap(true, atomic::Ordering::SeqCst) {
                        // Lock order: sink (held by the caller) then
                        // lost_future — never the reverse.
                        let future_id = {
                            let mut lost = device.lost_future.lock();
                            match *lost {
                                Some(id) => id,
                                None => {
                                    let id = crate::future::register(
                                        &device.context,
                                        self.device_lost_mode,
                                        crate::future::Source::Cpu,
                                    );
                                    *lost = Some(id);
                                    id
                                }
                            }
                        };
                        let payload = lost_callback_payload(
                            &self.device_lost_handler,
                            device,
                            message,
                        );
                        crate::future::complete(future_id, payload);
                    }
                }
                // Device already destroyed (upgrade failed): no object left to
                // notify — the callback is never fired on a dangling
                // handle.
                return;
            }
            crate::Error::OutOfMemory { .. } => (
                native::WGPUErrorType_OutOfMemory,
                crate::ErrorFilter::OutOfMemory,
            ),
            crate::Error::Validation { .. } => (
                native::WGPUErrorType_Validation,
                crate::ErrorFilter::Validation,
            ),
        };

        match self
            .scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.filter == filter)
        {
            Some(scope) => {
                if scope.error.is_none() {
                    scope.error = Some(err);
                }
            }
            None => {
                if let Some(callback) = self.uncaptured_handler.callback {
                    let userdata = &self.uncaptured_handler.userdata;
                    let msg = err.to_string();
                    let device = self.device.as_ptr();
                    unsafe {
                        callback(
                            &device,
                            typ,
                            str_into_string_view(&msg),
                            userdata.get_1(),
                            userdata.get_2(),
                        )
                    };
                }
            }
        }
    }
}

pub(crate) fn format_error(err: &(impl error::Error + 'static)) -> String {
    let mut output = String::new();
    let mut level = 1;

    fn print_tree(output: &mut String, level: &mut usize, e: &(dyn error::Error + 'static)) {
        let mut print = |e: &(dyn error::Error + 'static)| {
            use std::fmt::Write;
            writeln!(output, "{}{}", " ".repeat(*level * 2), e).unwrap();

            if let Some(e) = e.source() {
                *level += 1;
                print_tree(output, level, e);
                *level -= 1;
            }
        };
        if let Some(multi) = e.downcast_ref::<wgc::error::MultiError>() {
            for e in multi.errors() {
                print(e);
            }
        } else {
            print(e);
        }
    }

    print_tree(&mut output, &mut level, err);

    format!("Validation Error\n\nCaused by:\n{}", output)
}

pub(crate) fn handle_error_fatal(
    cause: impl error::Error + Send + Sync + 'static,
    operation: &'static str,
) -> ! {
    panic!("Error in {operation}: {f}", f = format_error(&cause));
}

pub(crate) fn handle_error(
    sink_mutex: &Mutex<ErrorSinkRaw>,
    source: impl error::Error + Send + Sync + 'static,
    label: Label<'_>,
    fn_ident: &'static str,
) {
    let error = wgc::error::ContextError {
        fn_ident,
        source: Box::new(source),
        label: label.unwrap_or_default().to_string(),
    };
    let mut sink = sink_mutex.lock();
    let mut source_opt: Option<&(dyn error::Error + 'static)> = Some(&error);
    while let Some(source) = source_opt {
        match source.downcast_ref::<wgc::device::DeviceError>() {
            Some(wgc::device::DeviceError::Lost) => {
                return sink.handle_error(crate::Error::DeviceLost {
                    source: Box::new(error),
                });
            }
            Some(wgc::device::DeviceError::OutOfMemory) => {
                return sink.handle_error(crate::Error::OutOfMemory {
                    source: Box::new(error),
                });
            }
            _ => (),
        }
        source_opt = source.source();
    }

    // Otherwise, it is a validation error
    sink.handle_error(crate::Error::Validation {
        description: format_error(&error),
        source: Box::new(error),
    });
}

/// Async pipeline creation errors: returns the callback status
/// (webgpu.h WGPUCreatePipelineAsyncStatus) and the message. Device errors
/// (lost/OOM) stay routed to the device handlers (GUIDE §5) with an
/// InternalError status; validation errors are delivered ONLY TO THE
/// callback (webgpu.h CallbackStatuses) with a ValidationError status.
pub(crate) fn route_async_pipeline_error(
    sink_mutex: &Mutex<ErrorSinkRaw>,
    source: impl error::Error + Send + Sync + 'static,
    label: Label<'_>,
    fn_ident: &'static str,
) -> (native::WGPUCreatePipelineAsyncStatus, String) {
    let error = wgc::error::ContextError {
        fn_ident,
        source: Box::new(source),
        label: label.unwrap_or_default().to_string(),
    };
    let mut sink = sink_mutex.lock();
    let mut source_opt: Option<&(dyn error::Error + 'static)> = Some(&error);
    while let Some(source) = source_opt {
        match source.downcast_ref::<wgc::device::DeviceError>() {
            Some(wgc::device::DeviceError::Lost) => {
                let err = crate::Error::DeviceLost {
                    source: Box::new(error),
                };
                let message = err.to_string();
                sink.handle_error(err);
                return (native::WGPUCreatePipelineAsyncStatus_InternalError, message);
            }
            Some(wgc::device::DeviceError::OutOfMemory) => {
                let err = crate::Error::OutOfMemory {
                    source: Box::new(error),
                };
                let message = err.to_string();
                sink.handle_error(err);
                return (native::WGPUCreatePipelineAsyncStatus_InternalError, message);
            }
            _ => (),
        }
        source_opt = source.source();
    }

    // Validation error: delivered to the callback only.
    let message = format_error(&error);
    (
        native::WGPUCreatePipelineAsyncStatus_ValidationError,
        message,
    )
}

/// "device lost" payload: fires the deviceLostCallbackInfo (reason
/// `Destroyed` — wgc 29 does not expose the real cause of the loss). The handle
/// is owned by the payload (`OwnedHandle`): the reference lives until
/// delivery (or cancellation at instance destruction), never a
/// dangling pointer on deferred delivery.
pub(crate) fn lost_callback_payload(
    handler: &DeviceLostCallback,
    device: Arc<WGPUDeviceImpl>,
    message: String,
) -> crate::future::Payload {
    let callback = handler.callback;
    let userdata = Userdata::new(handler.userdata.get_1(), handler.userdata.get_2());
    let mut device = crate::future::OwnedHandle::new(Arc::into_raw(device) as *mut WGPUDeviceImpl);
    Box::new(move || {
        if let Some(callback) = callback {
            let device = device.take() as *const WGPUDeviceImpl;
            unsafe {
                callback(
                    &device,
                    native::WGPUDeviceLostReason_Destroyed,
                    str_into_string_view(&message),
                    userdata.get_1(),
                    userdata.get_2(),
                );
            }
        }
    })
}
