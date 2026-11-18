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
    crate::ffi_utils::contain_entry(
        "wgpuAdapterGetFeatures",
        (),
        || {

        let adapter = adapter.as_ref().expect("invalid adapter");
        let adapter_features = adapter.context.adapter_features(adapter.id);
        let features = features.as_mut().expect("invalid return pointer \"features\"");

        return_features(features, adapter_features);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterGetLimits(
    adapter: native::WGPUAdapter,
    mut limits: Option<&mut native::WGPULimits>,
) -> native::WGPUStatus {
    crate::ffi_utils::contain_entry(
        "wgpuAdapterGetLimits",
        native::WGPUStatus_Error,
        || {

        let adapter = adapter.as_ref().expect("invalid adapter");
        let wgt_limits = adapter.context.adapter_limits(adapter.id);
        let limits = limits.as_mut().expect("invalid return pointer \"limits\"");
        conv::write_limits_struct(wgt_limits, limits);
        // webgpu.h declares WGPUStatus (Success = 1, Error = 2): the limits and
        // any NativeLimits chain entry the caller set up are written above.
        native::WGPUStatus_Success
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterGetInfo(
    adapter: native::WGPUAdapter,
    info: Option<&mut native::WGPUAdapterInfo>,
) -> native::WGPUStatus {
    crate::ffi_utils::contain_entry(
        "wgpuAdapterGetInfo",
        native::WGPUStatus_Error,
        || {

        let adapter = adapter.as_ref().expect("invalid adapter");
        let info = info.expect("invalid return pointer \"info\"");
        let context = adapter.context.as_ref();
        let adapter_id = adapter.id;

        let result = context.adapter_get_info(adapter_id);
        conv::write_adapter_info(info, &result);

        native::WGPUStatus_Success
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterGetCooperativeMatrixProperties(
    adapter: native::WGPUAdapter,
    count: Option<&mut usize>,
    properties: *mut native::WGPUCooperativeMatrixProperties,
) -> native::WGPUStatus {
    crate::ffi_utils::contain_entry(
        "wgpuAdapterGetCooperativeMatrixProperties",
        native::WGPUStatus_Error,
        || {

        let adapter = adapter.as_ref().expect("invalid adapter");
        let count = count.expect("invalid return pointer \"count\"");

        fn map_scalar(value: wgt::CooperativeScalarType) -> native::WGPUNativeCooperativeScalarType {
            match value {
                wgt::CooperativeScalarType::F32 => native::WGPUNativeCooperativeScalarType_F32,
                wgt::CooperativeScalarType::F16 => native::WGPUNativeCooperativeScalarType_F16,
                wgt::CooperativeScalarType::I32 => native::WGPUNativeCooperativeScalarType_I32,
                wgt::CooperativeScalarType::U32 => native::WGPUNativeCooperativeScalarType_U32,
            }
        }

        let all: Vec<_> = adapter
            .context
            .adapter_cooperative_matrix_properties(adapter.id)
            .into_iter()
            .map(|props| native::WGPUCooperativeMatrixProperties {
                nextInChain: std::ptr::null(),
                mSize: props.m_size,
                nSize: props.n_size,
                kSize: props.k_size,
                abType: map_scalar(props.ab_type),
                crType: map_scalar(props.cr_type),
                saturatingAccumulation: props.saturating_accumulation as native::WGPUBool,
            })
            .collect();

        // Two-call protocol: a NULL buffer returns the total.
        if properties.is_null() {
            *count = all.len();
        } else {
            let fill = (*count).min(all.len());
            for (out, props) in std::iter::zip(
                std::slice::from_raw_parts_mut(properties, fill),
                all.into_iter().take(fill),
            ) {
                *out = props;
            }
            *count = fill;
        }
        native::WGPUStatus_Success
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterHasFeature(
    adapter: native::WGPUAdapter,
    feature: native::WGPUFeatureName,
) -> native::WGPUBool {
    crate::ffi_utils::contain_entry(
        "wgpuAdapterHasFeature",
        0,
        || {

        let adapter = adapter.as_ref().expect("invalid adapter");
        let adapter_features = adapter.context.adapter_features(adapter.id);

        let feature = match conv::map_feature(feature) {
            Some(feature) => feature,
            None => return false as native::WGPUBool,
        };

        adapter_features.contains(feature) as native::WGPUBool
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterInfoFreeMembers(adapter_info: native::WGPUAdapterInfo) {
    crate::ffi_utils::contain_entry(
        "wgpuAdapterInfoFreeMembers",
        (),
        || {

        utils::drop_string_view(adapter_info.vendor);
        utils::drop_string_view(adapter_info.architecture);
        utils::drop_string_view(adapter_info.device);
        utils::drop_string_view(adapter_info.description);
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterRequestDevice(
    adapter: native::WGPUAdapter,
    descriptor: Option<&native::WGPUDeviceDescriptor>,
    mut callback_info: native::WGPURequestDeviceCallbackInfo,
) -> native::WGPUFuture {
    crate::ffi_utils::contain_entry(
        "wgpuAdapterRequestDevice",
        native::WGPUFuture { id: 0 },
        || {

        let (adapter_id, context, adapter_arc) = {
            let adapter_ref = adapter.as_ref().expect("invalid adapter");
            // Strong reference kept by the device so wgpuDeviceGetAdapter can
            // hand it back; released with the device.
            Arc::increment_strong_count(adapter);
            let adapter_arc = Arc::from_raw(adapter as *const WGPUAdapterImpl);
            (adapter_ref.id, adapter_ref.context.clone(), adapter_arc)
        };
        future::normalize_callback_mode(&mut callback_info.mode);
        future::validate_callback_mode(callback_info.mode);
        let callback = callback_info.callback.expect("invalid callback");
        let userdata = new_userdata!(callback_info);

        let adapter_limits = context.adapter_limits(adapter_id);
        let base_limits = get_base_device_limits_from_adapter_limits(&adapter_limits);

        let (desc, device_lost_handler, device_lost_mode, error_callback) = match descriptor {
            Some(descriptor) => {
                let (desc, error_callback) = follow_chain!(
                    map_device_descriptor((descriptor, base_limits),
                    WGPUSType_DeviceExtras => native::WGPUDeviceExtras,
                    WGPUSType_ExperimentalFeaturesExtras => native::WGPUExperimentalFeaturesExtras)
                );
                // Mode 0 (INIT macro) normalizes to WaitAnyOnly + warning,
                // like every other CallbackInfo.
                let device_lost_mode =
                    future::effective_callback_mode(descriptor.deviceLostCallbackInfo.mode);
                let device_lost_handler = DeviceLostCallback {
                    callback: descriptor.deviceLostCallbackInfo.callback,
                    userdata: new_userdata!(descriptor.deviceLostCallbackInfo),
                };
                (desc, device_lost_handler, device_lost_mode, error_callback)
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
        },
    )
}

/// Returns the instance this adapter was enumerated from (Dawn's
/// `wgpuAdapterGetInstance`). The returned handle is a new strong reference
/// the caller releases with `wgpuInstanceRelease`.
#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterGetInstance(
    adapter: native::WGPUAdapter,
) -> native::WGPUInstance {
    crate::ffi_utils::contain_entry(
        "wgpuAdapterGetInstance",
        std::ptr::null(),
        || {

        let adapter = adapter.as_ref().expect("invalid adapter");
        Arc::into_raw(adapter.instance.clone()) as native::WGPUInstance
        },
    )
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
    crate::ffi_utils::contain_entry(
        "wgpuAdapterGetFormatCapabilities",
        native::WGPUStatus_Error,
        || {

        let adapter_ref = adapter.as_ref().expect("invalid adapter");
        match conv::map_texture_format(format) {
            Some(format) => {
                let features = adapter_ref
                    .context
                    .adapter_get_texture_format_features(adapter_ref.id, format);
                // Fill the caller's WGPUFormatCapabilities OUT-chain when
                // present: the Dawn struct itself carries no data field.
                if let Some(capabilities) = _capabilities {
                    // Fill every WGPUFormatCapabilities node of the out-chain
                    // in place; the chain nodes themselves stay untouched.
                    let mut chain = capabilities.nextInChain;
                    while let Some(link) = unsafe { chain.as_ref() } {
                        if link.sType == native::WGPUSType_FormatCapabilities {
                            let out = link as *const native::WGPUChainedStruct
                                as *mut native::WGPUFormatCapabilities;
                            unsafe {
                                (*out).allowedUsages = features.allowed_usages.bits() as u64;
                                (*out).flags =
                                    features.flags.bits() as native::WGPUNativeFormatFeatureFlags;
                            }
                        }
                        chain = link.next;
                    }
                }
                native::WGPUStatus_Success
            }
            None => native::WGPUStatus_Error,
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterAddRef(adapter: native::WGPUAdapter) {
    crate::ffi_utils::contain_entry(
        "wgpuAdapterAddRef",
        (),
        || {

        assert!(!adapter.is_null(), "invalid adapter");
        Arc::increment_strong_count(adapter);
        },
    )
}
#[no_mangle]
pub unsafe extern "C" fn wgpuAdapterRelease(adapter: native::WGPUAdapter) {
    crate::ffi_utils::contain_entry(
        "wgpuAdapterRelease",
        (),
        || {

        assert!(!adapter.is_null(), "invalid adapter");
        Arc::decrement_strong_count(adapter);
        },
    )
}
