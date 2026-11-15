#[allow(clippy::missing_safety_doc)]

use crate::conv::map_device_descriptor;
use parking_lot::Mutex;
use std::sync::{atomic, Arc};
use crate::utils::{
    get_base_device_limits_from_adapter_limits, str_into_string_view,
};

use crate::callbacks::*;
use crate::conv;
use crate::error::*;
use crate::follow_chain;
use crate::future::{self, Source};
use crate::native;
use crate::new_userdata;
use crate::objects::*;
use crate::utils;
use super::return_features;

// Adapter methods

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterGetFeatures(
    adapter: native::WGPUAdapter,
    features: *mut native::WGPUSupportedFeatures,
) {
    let adapter = adapter.as_ref().expect("invalid adapter");
    let adapter_features = adapter.context.adapter_features(adapter.id);
    let features = features.as_mut().expect("invalid return pointer \"features\"");

    return_features(features, adapter_features);
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterGetLimits(
    adapter: native::WGPUAdapter,
    limits: Option<&mut native::WGPULimits>,
) -> native::WGPUBool {
    let (adapter_id, context, adapter_arc) = {
        let adapter_ref = adapter.as_ref().expect("invalid adapter");
        // Strong reference kept by the device so wgpuDeviceGetAdapter can
        // hand it back; released with the device.
        Arc::increment_strong_count(adapter);
        let adapter_arc = Arc::from_raw(adapter as *const WGPUAdapterImpl);
        (adapter_ref.id, adapter_ref.context.clone(), adapter_arc)
    };
    let limits = limits.expect("invalid return pointer \"limits\"");

    let wgt_limits = context.adapter_limits(adapter_id);
    conv::write_limits_struct(wgt_limits, limits);

    // The return value means "the chained WGPUChainedStructOut entries were
    // filled". This packer fills none: write_limits_struct only writes the
    // NativeLimits link when the caller already chained it. Report that.
    false as native::WGPUBool
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterGetInfo(
    adapter: native::WGPUAdapter,
    info: Option<&mut native::WGPUAdapterInfo>,
) -> native::WGPUStatus {
    let adapter = adapter.as_ref().expect("invalid adapter");
    let info = info.expect("invalid return pointer \"info\"");
    let context = adapter.context.as_ref();
    let adapter_id = adapter.id;

    let result = context.adapter_get_info(adapter_id);
    conv::write_adapter_info(info, &result);

    native::WGPUStatus_Success
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterHasFeature(
    adapter: native::WGPUAdapter,
    feature: native::WGPUFeatureName,
) -> native::WGPUBool {
    let (adapter_id, context, adapter_arc) = {
        let adapter_ref = adapter.as_ref().expect("invalid adapter");
        // Strong reference kept by the device so wgpuDeviceGetAdapter can
        // hand it back; released with the device.
        Arc::increment_strong_count(adapter);
        let adapter_arc = Arc::from_raw(adapter as *const WGPUAdapterImpl);
        (adapter_ref.id, adapter_ref.context.clone(), adapter_arc)
    };
    let adapter_features = context.adapter_features(adapter_id);

    let feature = match conv::map_feature(feature) {
        Some(feature) => feature,
        None => return false as native::WGPUBool,
    };

    adapter_features.contains(feature) as native::WGPUBool
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterInfoFreeMembers(adapter_info: native::WGPUAdapterInfo) {
    utils::drop_string_view(adapter_info.vendor);
    utils::drop_string_view(adapter_info.architecture);
    utils::drop_string_view(adapter_info.device);
    utils::drop_string_view(adapter_info.description);
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterRequestDevice(
    adapter: native::WGPUAdapter,
    descriptor: Option<&native::WGPUDeviceDescriptor>,
    callback_info: native::WGPURequestDeviceCallbackInfo,
) -> native::WGPUFuture {
    let (adapter_id, context, adapter_arc) = {
        let adapter_ref = adapter.as_ref().expect("invalid adapter");
        // Strong reference kept by the device so wgpuDeviceGetAdapter can
        // hand it back; released with the device.
        Arc::increment_strong_count(adapter);
        let adapter_arc = Arc::from_raw(adapter as *const WGPUAdapterImpl);
        (adapter_ref.id, adapter_ref.context.clone(), adapter_arc)
    };
    future::validate_callback_mode(callback_info.mode);
    let callback = callback_info.callback.expect("invalid callback");
    let userdata = new_userdata!(callback_info);

    let adapter_limits = context.adapter_limits(adapter_id);
    let base_limits = get_base_device_limits_from_adapter_limits(&adapter_limits);

    let (desc, device_lost_handler, device_lost_mode, error_callback) = match descriptor {
        Some(descriptor) => {
            let (desc, error_callback) = follow_chain!(
                map_device_descriptor((descriptor, base_limits),
                WGPUSType_DeviceExtras => native::WGPUDeviceExtras)
            );
            future::validate_callback_mode(descriptor.deviceLostCallbackInfo.mode);
            let device_lost_handler = DeviceLostCallback {
                callback: descriptor.deviceLostCallbackInfo.callback,
                userdata: new_userdata!(descriptor.deviceLostCallbackInfo),
            };
            (
                desc,
                device_lost_handler,
                descriptor.deviceLostCallbackInfo.mode,
                error_callback,
            )
        }
        None => (
            wgt::DeviceDescriptor {
                required_limits: base_limits,
                ..Default::default()
            },
            DEFAULT_DEVICE_LOST_HANDLER,
            native::WGPUCallbackMode_AllowSpontaneous,
            None,
        ),
    };

    // Synchronous resolution on the wgc side: the future completes before the
    // return, the callback is delivered according to the mode (AllowSpontaneous → before return,
    // historical convention of this packer; GUIDE §5).
    let future_id = future::register(&context, callback_info.mode, Source::Cpu);

    let result = context.adapter_request_device(adapter_id, &desc, None, None);
    match result {
        Ok((device_id, queue_id)) => {
            let device_impl = Arc::new_cyclic(|device| {
                let mut error_sink =
                    ErrorSinkRaw::new(device_lost_handler, device_lost_mode, device.clone());
                if let Some(error_callback) = error_callback {
                    error_sink.uncaptured_handler = error_callback;
                }
                let error_sink = Arc::new(Mutex::new(error_sink));
                WGPUDeviceImpl {
                    context: context.clone(),
                    id: device_id,
                    adapter: adapter_arc.clone(),
                    queue: Arc::new(QueueId {
                        context: context.clone(),
                        id: queue_id,
                    }),
                    error_sink: error_sink.clone(),
                    lost_future: Mutex::new(None),
                    device_lost: atomic::AtomicBool::new(false),
                }
            });
            let mut device = future::OwnedHandle::new(Arc::into_raw(device_impl) as *mut WGPUDeviceImpl);

            future::complete(
                future_id,
                Box::new(move || {
                    callback(
                        native::WGPURequestDeviceStatus_Success,
                        device.take(),
                        EMPTY_STRING,
                        userdata.get_1(),
                        userdata.get_2(),
                    );
                }),
            );
        }
        Err(err) => {
            let message = format_error(&err);
            future::complete(
                future_id,
                Box::new(move || {
                    callback(
                        native::WGPURequestDeviceStatus_Error,
                        std::ptr::null_mut(),
                        str_into_string_view(&message),
                        userdata.get_1(),
                        userdata.get_2(),
                    );
                }),
            );
        }
    };

    native::WGPUFuture { id: future_id }
}

/// Returns the instance this adapter was enumerated from (Dawn's
/// `wgpuAdapterGetInstance`). The returned handle is a new strong reference
/// the caller releases with `wgpuInstanceRelease`.
#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterGetInstance(
    adapter: native::WGPUAdapter,
) -> native::WGPUInstance {
    let adapter = adapter.as_ref().expect("invalid adapter");
    Arc::into_raw(adapter.instance.clone()) as native::WGPUInstance
}

/// Texture-format capabilities of this adapter (Dawn's
/// `wgpuAdapterGetFormatCapabilities`). The chain is unused by this packer,
/// so only the status is meaningful; the struct is left untouched.
#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterGetFormatCapabilities(
    adapter: native::WGPUAdapter,
    format: native::WGPUTextureFormat,
    _capabilities: Option<&mut native::WGPUDawnFormatCapabilities>,
) -> native::WGPUStatus {
    let adapter_ref = adapter.as_ref().expect("invalid adapter");
    match conv::map_texture_format(format) {
        Some(format) => {
            let _features = adapter_ref
                .context
                .adapter_get_texture_format_features(adapter_ref.id, format);
            native::WGPUStatus_Success
        }
        None => native::WGPUStatus_Error,
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterAddRef(adapter: native::WGPUAdapter) {
    assert!(!adapter.is_null(), "invalid adapter");
    Arc::increment_strong_count(adapter);
}
#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterRelease(adapter: native::WGPUAdapter) {
    assert!(!adapter.is_null(), "invalid adapter");
    Arc::decrement_strong_count(adapter);
}
