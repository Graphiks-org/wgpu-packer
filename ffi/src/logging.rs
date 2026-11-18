use crate::{map_enum, native, utils};
use log::{Level, LevelFilter, Metadata, Record};
use parking_lot::RwLock;

#[no_mangle]
pub extern "C" fn wgpuGetVersion() -> std::os::raw::c_uint {
    crate::ffi_utils::contain_entry(
        "wgpuGetVersion",
        0,
        || {

        // Take the string of WGPU_NATIVE_VERSION, strip any leading v's, split on dots,
        // and map the first 4 parts to the bytes of an uint32, consuming MSB first.
        // e.g. "v4.1"      -> 0x04010000
        //      "5.4.3.2.1" -> 0x05040302
        let static_str = match option_env!("WGPU_NATIVE_VERSION") {
            Some(s) => s.trim().trim_start_matches('v'),
            None => "",
        };
        let mut version: u32 = 0;
        for (index, part) in (0..).zip(static_str.split('.')) {
            let versionpart = part.parse::<u32>().unwrap_or_default();
            let shift: i32 = 8 * (3 - index);
            if shift < 0 {
                break;
            }
            version += versionpart << shift;
        }
        version
        },
    )
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let logger = LOGGER_INFO.read();

        if let Some(callback) = logger.callback {
            let msg = record.args().to_string();
            let level = match record.level() {
                Level::Error => native::WGPULogLevel_Error,
                Level::Warn => native::WGPULogLevel_Warn,
                Level::Info => native::WGPULogLevel_Info,
                Level::Debug => native::WGPULogLevel_Debug,
                Level::Trace => native::WGPULogLevel_Trace,
            };

            unsafe {
                callback(level, utils::str_into_string_view(&msg), logger.userdata);
            }

            // We do not use std::mem::forget(msg_c), so Rust will reclaim the memory
            // once msg_c gets cleared. The callback should thus make a copy.
        }
    }

    fn flush(&self) {}
}

struct LoggerInfo {
    initialized: bool,
    callback: native::WGPULogCallback,
    userdata: *mut std::os::raw::c_void,
}
unsafe impl Send for LoggerInfo {}
unsafe impl Sync for LoggerInfo {}

static LOGGER_INFO: RwLock<LoggerInfo> = RwLock::new(LoggerInfo {
    initialized: false,
    callback: None,
    userdata: std::ptr::null_mut(),
});

#[no_mangle]
pub extern "C" fn wgpuSetLogCallback(
    callback: native::WGPULogCallback,
    userdata: *mut std::os::raw::c_void,
) {
    crate::ffi_utils::contain_entry(
        "wgpuSetLogCallback",
        (),
        || {

        let mut logger = LOGGER_INFO.write();
        logger.callback = callback;
        logger.userdata = userdata;
        if !logger.initialized {
            logger.initialized = true;
            // A host process may already have installed a `log` logger (a Rust
            // host, another library): the packer's callback then never fires,
            // but the call must not abort.
            let _ = log::set_logger(&Logger);
            if log::max_level() == LevelFilter::Off {
                log::set_max_level(LevelFilter::Warn);
            }
        }
        },
    )
}

#[no_mangle]
pub extern "C" fn wgpuSetLogLevel(level: native::WGPULogLevel) {
    crate::ffi_utils::contain_entry(
        "wgpuSetLogLevel",
        (),
        || {

        log::set_max_level(map_log_level(level));
        },
    )
}

map_enum!(
    map_log_level,
    WGPULogLevel,
    LevelFilter,
    "Unknown log level",
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace
);
