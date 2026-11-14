use crate::utils::string_view_into_str;

use crate::native;
use crate::utils;

// La sentinelle « future nulle » (id 0) est celle de WGPU_FUTURE_INIT côté
// headers ; côté Rust, les futures réelles ne valent jamais 0 (future.rs).
pub(crate) const EMPTY_STRING: native::WGPUStringView = native::WGPUStringView {
    length: 0,
    data: std::ptr::null(),
};

pub(crate) struct DeviceCallback<T> {
    pub(crate) callback: T,
    pub(crate) userdata: utils::Userdata,
}
unsafe impl<T> Send for DeviceCallback<T> {}

pub(crate) type UncapturedErrorCallback = DeviceCallback<native::WGPUUncapturedErrorCallback>;
pub(crate) type DeviceLostCallback = DeviceCallback<native::WGPUDeviceLostCallback>;

pub(crate) unsafe extern "C" fn default_uncaptured_error_handler(
    _device: *const native::WGPUDevice,
    _typ: native::WGPUErrorType,
    message: native::WGPUStringView,
    _userdata1: *mut ::std::os::raw::c_void,
    _userdata2: *mut ::std::os::raw::c_void,
) {
    let message = string_view_into_str(message).unwrap_or("");
    // Spec §4.1 : log sans paniquer — installer uncapturedErrorCallbackInfo pour
    // être notifié (GUIDE §5).
    log::error!("wgpu uncaptured error (handler par defaut, aucun callback consommateur) : {message}");
}
pub(crate) const DEFAULT_UNCAPTURED_ERROR_HANDLER: UncapturedErrorCallback = UncapturedErrorCallback {
    callback: Some(default_uncaptured_error_handler),
    userdata: utils::Userdata::NULL,
};

pub(crate) unsafe extern "C" fn default_device_lost_handler(
    _device: *const native::WGPUDevice,
    _reason: native::WGPUDeviceLostReason,
    message: native::WGPUStringView,
    _userdata1: *mut ::std::os::raw::c_void,
    _userdata2: *mut ::std::os::raw::c_void,
) {
    let message = string_view_into_str(message).unwrap_or("");
    // Spec §4.1 : la perte de device n'est pas une erreur de programmation
    // (reset GPU, thermique Android, TDR Windows) — log sans paniquer, sinon
    // le processus abort pendant le poll du consommateur.
    log::error!("wgpu device lost (handler par defaut, aucun callback consommateur) : {message}");
}
pub(crate) const DEFAULT_DEVICE_LOST_HANDLER: DeviceLostCallback = DeviceLostCallback {
    callback: Some(default_device_lost_handler),
    userdata: utils::Userdata::NULL,
};
