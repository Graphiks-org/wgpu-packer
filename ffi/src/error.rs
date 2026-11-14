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
    /// Mode du deviceLostCallbackInfo : gouverne la livraison du callback et
    /// la complétion de la lost future (wgpuDeviceGetLostFuture).
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
                // La perte n'est livrée qu'UNE fois : la première remontée crée
                // la lost future si wgpuDeviceGetLostFuture n'a pas encore été
                // appelé, puis la complète — la livraison (immédiate ou différée)
                // suit le mode du deviceLostCallbackInfo. Les remontées
                // ultérieures sont ignorées (toute opération post-perte échoue
                // avec DeviceError::Lost).
                let message = err.to_string();
                if let Some(device) = self.device.upgrade() {
                    if !device.device_lost.swap(true, atomic::Ordering::SeqCst) {
                        // Ordre des verrous : sink (tenu par l'appelant) puis
                        // lost_future — jamais l'inverse.
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
                // Device déjà détruit (upgrade échoué) : plus d'objet à
                // notifier — on ne tire jamais le callback sur un handle
                // dangling.
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

/// Erreurs des créations de pipelines async : retourne le statut du callback
/// (webgpu.h WGPUCreatePipelineAsyncStatus) et le message. Les erreurs device
/// (lost/OOM) restent routées vers les handlers du device (GUIDE §5) avec un
/// statut InternalError ; les erreurs de validation ne sont livrées QU'AU
/// callback (webgpu.h CallbackStatuses) avec un statut ValidationError.
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

    // Erreur de validation : livrée au callback uniquement.
    let message = format_error(&error);
    (
        native::WGPUCreatePipelineAsyncStatus_ValidationError,
        message,
    )
}

/// Payload « device lost » : tire le deviceLostCallbackInfo (raison
/// `Destroyed` — wgc 29 n'expose pas la cause réelle de la perte). Le handle
/// est possédé par le payload (`OwnedHandle`) : la référence vit jusqu'à la
/// livraison (ou l'annulation à la destruction de l'instance), jamais de
/// pointeur dangling en livraison différée.
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
