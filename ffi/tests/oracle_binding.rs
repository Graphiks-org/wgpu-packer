//! Binding oracle: full lifecycle through the C ABI, conversions and
//! callback delivery. The intent of each test is the specification;
//! signatures follow the vendored headers (CallbackInfo model).

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use wgpu_packer::native::*;

unsafe fn zeroed<T>() -> T {
    std::mem::zeroed()
}

fn string_view_bytes(sv: &WGPUStringView) -> Vec<u8> {
    unsafe {
        if sv.data.is_null() {
            return Vec::new();
        }
        let len = if sv.length == usize::MAX {
            libc_strlen(sv.data)
        } else {
            sv.length
        };
        std::slice::from_raw_parts(sv.data as *const u8, len).to_vec()
    }
}

unsafe fn libc_strlen(p: *const i8) -> usize {
    let p = p as *const u8;
    let mut n = 0;
    while *p.add(n) != 0 {
        n += 1;
    }
    n
}

unsafe fn enumerate(instance: WGPUInstance) -> Vec<WGPUAdapter> {
    let count = wgpu_packer::wgpuInstanceEnumerateAdapters(instance, None, std::ptr::null_mut());
    if count == 0 {
        return Vec::new();
    }
    let mut adapters: Vec<WGPUAdapter> = vec![std::ptr::null(); count];
    let written = wgpu_packer::wgpuInstanceEnumerateAdapters(
        instance,
        None,
        adapters.as_mut_ptr(),
    );
    assert_eq!(written, count, "second enumeration call fills everything");
    adapters
}

unsafe fn release_adapters(adapters: &[WGPUAdapter]) {
    for a in adapters {
        wgpu_packer::wgpuAdapterRelease(*a);
    }
}

/// C StringView over static bytes (explicit length, no NUL).
unsafe fn sv(bytes: &[u8]) -> WGPUStringView {
    WGPUStringView {
        length: bytes.len(),
        data: bytes.as_ptr() as *const i8,
    }
}

/// Requests a device on the first adapter (synchronous, uncaptured error
/// handler installed — the default panics). Returns null without a GPU.
/// The result slot goes through userdata1: the tests run in parallel,
/// a static would be shared between them.
unsafe fn request_first_device(adapter: WGPUAdapter) -> WGPUDevice {
    unsafe extern "C" fn on_device(
        _status: WGPURequestDeviceStatus,
        device: WGPUDevice,
        _message: WGPUStringView,
        userdata1: *mut core::ffi::c_void,
        _u2: *mut core::ffi::c_void,
    ) {
        *(userdata1 as *mut usize) = device as usize;
    }
    unsafe extern "C" fn on_uncaptured_error(
        _device: *const WGPUDevice,
        _typ: WGPUErrorType,
        _message: WGPUStringView,
        _u1: *mut core::ffi::c_void,
        _u2: *mut core::ffi::c_void,
    ) {
    }
    let mut slot: usize = 0;
    let mut desc = zeroed::<WGPUDeviceDescriptor>();
    desc.uncapturedErrorCallbackInfo.callback = Some(on_uncaptured_error);
    // Explicit mode: 0 would normalize to WaitAnyOnly (warning); the tests
    // choose explicit modes for delivery determinism.
    desc.deviceLostCallbackInfo.mode = WGPUCallbackMode_AllowSpontaneous;
    let mut cb_info = zeroed::<WGPURequestDeviceCallbackInfo>();
    cb_info.mode = WGPUCallbackMode_AllowSpontaneous;
    cb_info.callback = Some(on_device);
    cb_info.userdata1 = &mut slot as *mut usize as *mut core::ffi::c_void;
    wgpu_packer::wgpuAdapterRequestDevice(adapter, Some(&desc), cb_info);
    slot as WGPUDevice
}

#[test]
fn instance_lifecycle_enumeration_and_string_conversion() {
    unsafe {
        let instance = common::make_instance();
        assert!(!instance.is_null(), "instance is not NULL");

        let adapters = enumerate(instance);
        if let Some(adapter) = adapters.first() {
            let mut info = zeroed::<WGPUAdapterInfo>();
            let status = wgpu_packer::wgpuAdapterGetInfo(*adapter, Some(&mut info));
            assert_eq!(status, WGPUStatus_Success, "GetInfo must succeed");
            let vendor = string_view_bytes(&info.vendor);
            assert!(
                std::str::from_utf8(&vendor).is_ok(),
                "vendor name must be valid UTF-8 (got {vendor:?})"
            );
            wgpu_packer::wgpuAdapterInfoFreeMembers(info);
        }
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn device_request_buffer_map_and_lifecycle() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            // machine without a GPU: the GPU chain is not testable here
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };

        // requestDevice is synchronous in this implementation: the callback
        // is invoked before the call returns. We install an uncaptured error
        // handler (the default panics — cf. lib.rs:422).
        static DEVICE: AtomicUsize = AtomicUsize::new(0);
        static STATUS: AtomicUsize = AtomicUsize::new(usize::MAX);
        static UNCAPPED_ERRORS: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn on_device(
            status: WGPURequestDeviceStatus,
            device: WGPUDevice,
            _message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            STATUS.store(status as usize, Ordering::SeqCst);
            DEVICE.store(device as usize, Ordering::SeqCst);
        }
        unsafe extern "C" fn on_uncaptured_error(
            _device: *const WGPUDevice,
            _typ: WGPUErrorType,
            _message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            UNCAPPED_ERRORS.fetch_add(1, Ordering::SeqCst);
        }
        let mut desc = zeroed::<WGPUDeviceDescriptor>();
        desc.uncapturedErrorCallbackInfo.callback = Some(on_uncaptured_error);
        desc.deviceLostCallbackInfo.mode = WGPUCallbackMode_AllowSpontaneous;
        let mut cb_info = zeroed::<WGPURequestDeviceCallbackInfo>();
        cb_info.mode = WGPUCallbackMode_AllowSpontaneous;
        cb_info.callback = Some(on_device);
        wgpu_packer::wgpuAdapterRequestDevice(adapter, Some(&desc), cb_info);
        assert_eq!(
            STATUS.load(Ordering::SeqCst),
            WGPURequestDeviceStatus_Success as usize,
            "requestDevice must succeed"
        );
        let device = DEVICE.load(Ordering::SeqCst) as WGPUDevice;
        assert!(!device.is_null());

        // Buffer MAP_WRITE|COPY_SRC (WebGPU rule: MAP combines with the
        // opposite COPY), async mapping delivered at poll.
        let mut bdesc = zeroed::<WGPUBufferDescriptor>();
        bdesc.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
        bdesc.size = 16;
        let buffer = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc));
        assert!(!buffer.is_null());

        static MAP_CALLS: AtomicUsize = AtomicUsize::new(0);
        static MAP_STATUS: AtomicUsize = AtomicUsize::new(usize::MAX);
        unsafe extern "C" fn on_mapped(
            status: WGPUMapAsyncStatus,
            _message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            MAP_CALLS.fetch_add(1, Ordering::SeqCst);
            MAP_STATUS.store(status as usize, Ordering::SeqCst);
        }
        let mut map_cb = zeroed::<WGPUBufferMapCallbackInfo>();
        map_cb.mode = WGPUCallbackMode_AllowSpontaneous;
        map_cb.callback = Some(on_mapped);
        let map_future =
            wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, 16, map_cb);
        assert_ne!(map_future.id, 0, "MapAsync returns a real future");
        wgpu_packer::wgpuDevicePoll(device, true, None);

        assert_eq!(MAP_CALLS.load(Ordering::SeqCst), 1, "map_async delivered exactly once");
        assert_eq!(MAP_STATUS.load(Ordering::SeqCst), WGPUMapAsyncStatus_Success as usize);
        assert_eq!(UNCAPPED_ERRORS.load(Ordering::SeqCst), 0, "no uncaptured error");

        let range = wgpu_packer::wgpuBufferGetMappedRange(buffer, 0, 16);
        assert!(!range.is_null(), "mapped range accessible after map");
        *range = 0xAB;
        wgpu_packer::wgpuBufferUnmap(buffer);

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn get_proc_address_and_instance_features() {
    unsafe {
        // Every function declared in the headers is resolvable — including
        // the documented stubs (resolving is not calling, cf. GUIDE §6).
        for name in [
            "wgpuCreateInstance",
            "wgpuBufferMapAsync",
            "wgpuGetProcAddress",
            "wgpuInstanceWaitAny",
        ] {
            assert!(
                wgpu_packer::wgpuGetProcAddress(sv(name.as_bytes())).is_some(),
                "{name} must be resolvable"
            );
        }
        assert!(
            wgpu_packer::wgpuGetProcAddress(sv(b"wgpuDoesNotExist")).is_none(),
            "unknown name => NULL"
        );
        let empty = WGPUStringView {
            length: 0,
            data: std::ptr::null(),
        };
        assert!(
            wgpu_packer::wgpuGetProcAddress(empty).is_none(),
            "empty name => NULL"
        );

        // Instance features: non-empty list, consistent with HasInstanceFeature.
        let mut features = zeroed::<WGPUSupportedInstanceFeatures>();
        wgpu_packer::wgpuGetInstanceFeatures(Some(&mut features));
        assert!(features.featureCount > 0, "at least one instance feature");
        let list =
            std::slice::from_raw_parts(features.features, features.featureCount);
        assert!(
            list.contains(&WGPUInstanceFeatureName_MultipleDevicesPerAdapter),
            "wgc supports multiple devices per adapter"
        );
        assert_eq!(
            wgpu_packer::wgpuHasInstanceFeature(WGPUInstanceFeatureName_MultipleDevicesPerAdapter),
            1,
        );
        // TimedWaitAny is now supported (timed wait enabled at
        // creation via requiredFeatures; cf. the dedicated oracle below).
        assert_eq!(
            wgpu_packer::wgpuHasInstanceFeature(WGPUInstanceFeatureName_TimedWaitAny),
            1,
        );
        wgpu_packer::wgpuSupportedInstanceFeaturesFreeMembers(features);
    }
}

#[test]
fn device_get_adapter_info_matches_adapter() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null(), "device required for the adapter info oracle");

        // The device exposes the same information as its adapter.
        let mut dev_info = zeroed::<WGPUAdapterInfo>();
        let status = wgpu_packer::wgpuDeviceGetAdapterInfo(device, Some(&mut dev_info));
        assert_eq!(status, WGPUStatus_Success, "DeviceGetAdapterInfo must succeed");
        let mut ad_info = zeroed::<WGPUAdapterInfo>();
        let status = wgpu_packer::wgpuAdapterGetInfo(adapter, Some(&mut ad_info));
        assert_eq!(status, WGPUStatus_Success);

        assert_eq!(
            string_view_bytes(&dev_info.device),
            string_view_bytes(&ad_info.device),
            "same adapter name via device and via adapter"
        );
        assert_eq!(dev_info.backendType, ad_info.backendType);
        assert_eq!(dev_info.vendorID, ad_info.vendorID);
        assert!(
            std::str::from_utf8(&string_view_bytes(&dev_info.description)).is_ok(),
            "valid UTF-8 description"
        );

        wgpu_packer::wgpuAdapterInfoFreeMembers(dev_info);
        wgpu_packer::wgpuAdapterInfoFreeMembers(ad_info);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn buffer_map_state_and_read_write_mapped_range() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null(), "device required for the map state oracle");

        let mut bdesc = zeroed::<WGPUBufferDescriptor>();
        bdesc.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
        bdesc.size = 16;
        let buffer = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc));
        assert!(!buffer.is_null());
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Unmapped,
            "new buffer is unmapped"
        );

        static MAP_STATUS: AtomicUsize = AtomicUsize::new(usize::MAX);
        unsafe extern "C" fn on_mapped(
            status: WGPUMapAsyncStatus,
            _message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            MAP_STATUS.store(status as usize, Ordering::SeqCst);
        }
        let mut cb = zeroed::<WGPUBufferMapCallbackInfo>();
        cb.mode = WGPUCallbackMode_AllowSpontaneous;
        cb.callback = Some(on_mapped);

        // Pending as soon as the call is made (state set synchronously), Mapped after delivery.
        wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, 16, cb);
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Pending,
            "map async is pending"
        );
        wgpu_packer::wgpuDevicePoll(device, true, None);
        assert_eq!(MAP_STATUS.load(Ordering::SeqCst), WGPUMapAsyncStatus_Success as usize);
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Mapped,
            "map delivered => mapped"
        );

        // WaitAnyOnly mode: completion at device poll BUT callback held back
        // — delivered only by wgpuInstanceWaitAny on this future.
        static WAITONLY_CALLS: AtomicUsize = AtomicUsize::new(0);
        static WAITONLY_STATUS: AtomicUsize = AtomicUsize::new(usize::MAX);
        unsafe extern "C" fn on_waitonly(
            status: WGPUMapAsyncStatus,
            _message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            WAITONLY_CALLS.fetch_add(1, Ordering::SeqCst);
            WAITONLY_STATUS.store(status as usize, Ordering::SeqCst);
        }
        wgpu_packer::wgpuBufferUnmap(buffer);
        let mut cb_waitonly = zeroed::<WGPUBufferMapCallbackInfo>();
        cb_waitonly.mode = WGPUCallbackMode_WaitAnyOnly;
        cb_waitonly.callback = Some(on_waitonly);
        let waitonly_future =
            wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, 16, cb_waitonly);
        assert_ne!(waitonly_future.id, 0);
        wgpu_packer::wgpuDevicePoll(device, true, None);
        assert_eq!(
            WAITONLY_CALLS.load(Ordering::SeqCst),
            0,
            "WaitAnyOnly: no delivery at the device poll"
        );
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Mapped,
            "operation completed (state), only the callback is held back"
        );
        let mut wait_info = WGPUFutureWaitInfo {
            future: waitonly_future,
            completed: 0,
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut wait_info, 0),
            WGPUWaitStatus_Success,
            "WaitAny delivers the completed future"
        );
        assert_eq!(wait_info.completed, 1, "completed flag set");
        assert_eq!(WAITONLY_CALLS.load(Ordering::SeqCst), 1, "callback delivered by WaitAny");
        assert_eq!(
            WAITONLY_STATUS.load(Ordering::SeqCst),
            WGPUMapAsyncStatus_Success as usize
        );
        // Future consumed: a second wait on it is invalid.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut wait_info, 0),
            WGPUWaitStatus_Error,
            "future already delivered => Error"
        );
        // The buffer stays mapped: the Write/ReadMappedRange section below
        // exercises this mapping.

        // Write via WriteMappedRange, read back via ReadMappedRange.
        let payload: [u8; 16] = core::array::from_fn(|i| (i as u8) ^ 0xA5);
        let status = wgpu_packer::wgpuBufferWriteMappedRange(
            buffer,
            0,
            payload.as_ptr() as *const core::ffi::c_void,
            payload.len(),
        );
        assert_eq!(status, WGPUStatus_Success, "write into the mapped range");
        let mut out = [0u8; 16];
        let status = wgpu_packer::wgpuBufferReadMappedRange(
            buffer,
            0,
            out.as_mut_ptr() as *mut core::ffi::c_void,
            out.len(),
        );
        assert_eq!(status, WGPUStatus_Success, "read from the mapped range");
        assert_eq!(out, payload, "Read/WriteMappedRange round trip");

        // Rejections: WGPU_WHOLE_MAP_SIZE not accepted, NULL data, out of bounds.
        let mut scratch = [0u8; 8];
        assert_eq!(
            wgpu_packer::wgpuBufferReadMappedRange(
                buffer,
                0,
                scratch.as_mut_ptr() as *mut core::ffi::c_void,
                usize::MAX,
            ),
            WGPUStatus_Error,
            "WGPU_WHOLE_MAP_SIZE rejected"
        );
        assert_eq!(
            wgpu_packer::wgpuBufferReadMappedRange(buffer, 0, std::ptr::null_mut(), 8),
            WGPUStatus_Error,
            "NULL destination rejected"
        );
        assert_eq!(
            wgpu_packer::wgpuBufferReadMappedRange(
                buffer,
                8,
                scratch.as_mut_ptr() as *mut core::ffi::c_void,
                16,
            ),
            WGPUStatus_Error,
            "out-of-bounds range rejected"
        );

        wgpu_packer::wgpuBufferUnmap(buffer);
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Unmapped,
            "unmap => unmapped"
        );
        assert_eq!(
            wgpu_packer::wgpuBufferReadMappedRange(
                buffer,
                0,
                scratch.as_mut_ptr() as *mut core::ffi::c_void,
                8,
            ),
            WGPUStatus_Error,
            "read outside the mapping rejected (contained abort, no panic)"
        );

        // mappedAtCreation: mapped as soon as it is created.
        let mut bdesc2 = zeroed::<WGPUBufferDescriptor>();
        bdesc2.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
        bdesc2.size = 8;
        bdesc2.mappedAtCreation = 1;
        let buffer2 = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc2));
        assert!(!buffer2.is_null());
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer2),
            WGPUBufferMapState_Mapped,
            "mappedAtCreation => mapped"
        );
        wgpu_packer::wgpuBufferDestroy(buffer2);
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer2),
            WGPUBufferMapState_Unmapped,
            "destroy => unmapped"
        );
        wgpu_packer::wgpuBufferRelease(buffer2);

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// Minimal WGSL shader module (trivial compute), for the pipeline oracles.
unsafe fn make_compute_shader(device: WGPUDevice) -> WGPUShaderModule {
    const WGSL: &[u8] =
        b"@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) _gid: vec3<u32>) {}";
    let mut source = zeroed::<WGPUShaderSourceWGSL>();
    source.chain.sType = WGPUSType_ShaderSourceWGSL;
    source.code = sv(WGSL);
    let mut desc = zeroed::<WGPUShaderModuleDescriptor>();
    desc.nextInChain = &mut source.chain;
    wgpu_packer::wgpuDeviceCreateShaderModule(device, Some(&desc))
}

#[test]
fn async_pipeline_creation_and_error_routing() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null(), "device required for the async pipeline oracle");
        let shader = make_compute_shader(device);
        assert!(!shader.is_null());

        // Happy path: callback delivered before the return, valid pipeline.
        static STATUS: AtomicUsize = AtomicUsize::new(0);
        static PIPELINE: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn on_pipeline(
            status: WGPUCreatePipelineAsyncStatus,
            pipeline: WGPUComputePipeline,
            _message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            STATUS.store(status as usize, Ordering::SeqCst);
            PIPELINE.store(pipeline as usize, Ordering::SeqCst);
        }
        let mut cdesc = zeroed::<WGPUComputePipelineDescriptor>();
        cdesc.compute.module = shader;
        cdesc.compute.entryPoint = sv(b"main");
        let mut cb = zeroed::<WGPUCreateComputePipelineAsyncCallbackInfo>();
        cb.mode = WGPUCallbackMode_AllowSpontaneous;
        cb.callback = Some(on_pipeline);
        let future = wgpu_packer::wgpuDeviceCreateComputePipelineAsync(device, Some(&cdesc), cb);
        assert_ne!(future.id, 0, "real future (completed before the return)");
        assert_eq!(
            STATUS.load(Ordering::SeqCst),
            WGPUCreatePipelineAsyncStatus_Success as usize,
            "async creation succeeded (AllowSpontaneous => callback before return)"
        );
        let pipeline = PIPELINE.load(Ordering::SeqCst) as WGPUComputePipeline;
        assert!(!pipeline.is_null(), "pipeline delivered to the callback");
        wgpu_packer::wgpuComputePipelineRelease(pipeline);

        // Error path: unknown entry point -> ValidationError at the callback
        // only, NULL pipeline, non-empty message, no abort.
        static ERR_STATUS: AtomicUsize = AtomicUsize::new(0);
        static ERR_MSG_LEN: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn on_pipeline_error(
            status: WGPUCreatePipelineAsyncStatus,
            pipeline: WGPUComputePipeline,
            message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            ERR_STATUS.store(status as usize, Ordering::SeqCst);
            ERR_MSG_LEN.store(message.length, Ordering::SeqCst);
            assert!(pipeline.is_null(), "no pipeline on error");
        }
        let mut bad = zeroed::<WGPUComputePipelineDescriptor>();
        bad.compute.module = shader;
        bad.compute.entryPoint = sv(b"entree_inexistante");
        let mut cb_err = zeroed::<WGPUCreateComputePipelineAsyncCallbackInfo>();
        cb_err.mode = WGPUCallbackMode_AllowSpontaneous;
        cb_err.callback = Some(on_pipeline_error);
        let err_future =
            wgpu_packer::wgpuDeviceCreateComputePipelineAsync(device, Some(&bad), cb_err);
        assert_ne!(err_future.id, 0, "even on error, the future completes");
        assert_eq!(
            ERR_STATUS.load(Ordering::SeqCst),
            WGPUCreatePipelineAsyncStatus_ValidationError as usize,
            "validation error delivered to the callback"
        );
        assert!(ERR_MSG_LEN.load(Ordering::SeqCst) > 0, "explicit message provided");

        wgpu_packer::wgpuShaderModuleRelease(shader);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn wait_any_validation_and_device_lost_future() {
    unsafe {
        let instance = common::make_instance();

        // (0 futures, timeout 0): equivalent to ProcessEvents -> Success.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 0, std::ptr::null_mut(), 0),
            WGPUWaitStatus_Success,
        );
        // Timeout without any future: invalid (Wait-Any spec).
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 0, std::ptr::null_mut(), 1),
            WGPUWaitStatus_Error,
            "timeout without a future => Error"
        );
        // Null pointer with futureCount > 0: invalid.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, std::ptr::null_mut(), 0),
            WGPUWaitStatus_Error,
            "NULL futures with futureCount > 0 => Error"
        );

        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null());

        // Lost future: real, stable, never completed on a healthy
        // device (passive loss detection — cf. GUIDE).
        let lost = wgpu_packer::wgpuDeviceGetLostFuture(device);
        assert_ne!(lost.id, 0, "the lost future is not the NULL sentinel");
        assert_eq!(
            wgpu_packer::wgpuDeviceGetLostFuture(device).id,
            lost.id,
            "stable id across calls"
        );

        // Untimed wait (timeout 0): a single non-blocking poll, the
        // lost future does not complete -> TimedOut, flag not set.
        let mut info = WGPUFutureWaitInfo {
            future: lost,
            completed: 0,
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut info, 0),
            WGPUWaitStatus_TimedOut,
            "timeout 0 = single poll, no blocking"
        );
        assert_eq!(info.completed, 0);

        // Timed wait without the TimedWaitAny feature (default
        // instance): Error.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut info, 1),
            WGPUWaitStatus_Error,
            "timeoutNS > 0 requires the TimedWaitAny feature at creation"
        );

        // Invalid futures: zero id, duplicate.
        let mut null_info = WGPUFutureWaitInfo {
            future: WGPUFuture { id: 0 },
            completed: 0,
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut null_info, 0),
            WGPUWaitStatus_Error,
            "id 0 (sentinel) => Error"
        );
        let mut dup = [
            WGPUFutureWaitInfo {
                future: lost,
                completed: 0,
            },
            WGPUFutureWaitInfo {
                future: lost,
                completed: 0,
            },
        ];
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 2, dup.as_mut_ptr(), 0),
            WGPUWaitStatus_Error,
            "duplicate in the list => Error"
        );

        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn timed_wait_any_feature_and_delivery_modes() {
    unsafe {
        // Instance with the TimedWaitAny feature requested at creation.
        let mut idesc = zeroed::<WGPUInstanceDescriptor>();
        let features = [WGPUInstanceFeatureName_TimedWaitAny];
        idesc.requiredFeatures = features.as_ptr();
        idesc.requiredFeatureCount = 1;
        let instance = wgpu_packer::wgpuCreateInstance(Some(&idesc));
        assert!(!instance.is_null(), "creation with TimedWaitAny accepted");

        // Limits: timedWaitAnyMaxCount non-zero (no real limit).
        let mut limits = zeroed::<WGPUInstanceLimits>();
        assert_eq!(
            wgpu_packer::wgpuGetInstanceLimits(Some(&mut limits)),
            WGPUStatus_Success
        );
        assert_eq!(
            limits.timedWaitAnyMaxCount,
            usize::MAX,
            "no real limit on the number of futures"
        );

        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null(), "device required for the timed wait oracle");

        // Timed wait that succeeds: map in WaitAnyOnly, completed by
        // WaitAny's internal poll.
        static TIMED_CALLS: AtomicUsize = AtomicUsize::new(0);
        static TIMED_STATUS: AtomicUsize = AtomicUsize::new(usize::MAX);
        unsafe extern "C" fn on_timed(
            status: WGPUMapAsyncStatus,
            _message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            TIMED_CALLS.fetch_add(1, Ordering::SeqCst);
            TIMED_STATUS.store(status as usize, Ordering::SeqCst);
        }
        let mut bdesc = zeroed::<WGPUBufferDescriptor>();
        bdesc.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
        bdesc.size = 16;
        let buffer = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc));
        assert!(!buffer.is_null());
        let mut cb = zeroed::<WGPUBufferMapCallbackInfo>();
        cb.mode = WGPUCallbackMode_WaitAnyOnly;
        cb.callback = Some(on_timed);
        let map_future = wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, 16, cb);
        assert_ne!(map_future.id, 0);
        let mut info = WGPUFutureWaitInfo {
            future: map_future,
            completed: 0,
        };
        // timeout 100 ms: far beyond the completion poll.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut info, 100_000_000),
            WGPUWaitStatus_Success,
            "timed wait succeeded (completion by the internal poll)"
        );
        assert_eq!(info.completed, 1);
        assert_eq!(TIMED_CALLS.load(Ordering::SeqCst), 1, "callback delivered");
        assert_eq!(
            TIMED_STATUS.load(Ordering::SeqCst),
            WGPUMapAsyncStatus_Success as usize
        );

        // Timed wait that expires: the lost future of a healthy device
        // never completes -> TimedOut (deterministic).
        let lost = wgpu_packer::wgpuDeviceGetLostFuture(device);
        let mut lost_info = WGPUFutureWaitInfo {
            future: lost,
            completed: 0,
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut lost_info, 5_000_000),
            WGPUWaitStatus_TimedOut,
            "expired after the timeout (5 ms)"
        );
        assert_eq!(lost_info.completed, 0, "no completion reported");

        // AllowProcessEvents mode: completion at device poll, callback
        // delivered by wgpuInstanceProcessEvents (not before).
        static PE_CALLS: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn on_process_events(
            _status: WGPUMapAsyncStatus,
            _message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            PE_CALLS.fetch_add(1, Ordering::SeqCst);
        }
        wgpu_packer::wgpuBufferUnmap(buffer);
        let mut cb_pe = zeroed::<WGPUBufferMapCallbackInfo>();
        cb_pe.mode = WGPUCallbackMode_AllowProcessEvents;
        cb_pe.callback = Some(on_process_events);
        wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, 16, cb_pe);
        wgpu_packer::wgpuDevicePoll(device, true, None);
        assert_eq!(
            PE_CALLS.load(Ordering::SeqCst),
            0,
            "AllowProcessEvents: no delivery at the device poll"
        );
        wgpu_packer::wgpuInstanceProcessEvents(instance);
        assert_eq!(
            PE_CALLS.load(Ordering::SeqCst),
            1,
            "delivered by ProcessEvents"
        );

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn set_label_is_accepted_and_ignored() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null());

        let mut bdesc = zeroed::<WGPUBufferDescriptor>();
        bdesc.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
        bdesc.size = 4;
        let buffer = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc));
        assert!(!buffer.is_null());
        // Hint accepted and ignored (wgc 29 without a setter): the ABI must not abort.
        wgpu_packer::wgpuBufferSetLabel(buffer, sv(b"oracle-buffer"));
        wgpu_packer::wgpuDeviceSetLabel(device, sv(b"oracle-device"));
        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn texture_binding_view_dimension_explicit_and_derived() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null());

        // 2D with one layer, no chain: derivation -> 2D.
        let mut desc = zeroed::<WGPUTextureDescriptor>();
        desc.usage = WGPUTextureUsage_TextureBinding;
        desc.dimension = WGPUTextureDimension_2D;
        desc.size = WGPUExtent3D {
            width: 4,
            height: 4,
            depthOrArrayLayers: 1,
        };
        desc.format = WGPUTextureFormat_RGBA8Unorm;
        let texture = wgpu_packer::wgpuDeviceCreateTexture(device, Some(&desc));
        assert!(!texture.is_null());
        assert_eq!(
            wgpu_packer::wgpuTextureGetTextureBindingViewDimension(texture),
            WGPUTextureViewDimension_2D,
        );
        wgpu_packer::wgpuTextureRelease(texture);

        // 2D with 6 layers: derivation -> 2DArray.
        desc.size.depthOrArrayLayers = 6;
        let layered = wgpu_packer::wgpuDeviceCreateTexture(device, Some(&desc));
        assert!(!layered.is_null());
        assert_eq!(
            wgpu_packer::wgpuTextureGetTextureBindingViewDimension(layered),
            WGPUTextureViewDimension_2DArray,
        );
        wgpu_packer::wgpuTextureRelease(layered);

        // Explicit WGPUSType_TextureBindingViewDimension chain: honored.
        let mut explicit = zeroed::<WGPUTextureBindingViewDimension>();
        explicit.chain.sType = WGPUSType_TextureBindingViewDimension;
        explicit.textureBindingViewDimension = WGPUTextureViewDimension_1D;
        desc.size = WGPUExtent3D {
            width: 64,
            height: 1,
            depthOrArrayLayers: 1,
        };
        desc.dimension = WGPUTextureDimension_1D;
        desc.nextInChain = &mut explicit.chain;
        let one_d = wgpu_packer::wgpuDeviceCreateTexture(device, Some(&desc));
        assert!(!one_d.is_null());
        assert_eq!(
            wgpu_packer::wgpuTextureGetTextureBindingViewDimension(one_d),
            WGPUTextureViewDimension_1D,
            "explicit chained value honored"
        );
        wgpu_packer::wgpuTextureRelease(one_d);

        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// Binding audit #4 + #2: header defaults in binding layouts
/// (viewDimension Undefined "defaults to 2D") and StorageBinding usage
/// relayed — wgpu 30 requires it on the view at bind group time.
#[test]
fn bind_group_layout_defaults_and_storage_binding_usage() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null(), "device required for the binding layouts oracle");

        // Slot 0: texture with the header INIT macros' defaults
        // (viewDimension left Undefined). Slot 1: storage texture,
        // viewDimension Undefined as well (documented default 2D).
        let mut entries = zeroed::<[WGPUBindGroupLayoutEntry; 2]>();
        entries[0].binding = 0;
        entries[0].visibility = WGPUShaderStage_Fragment;
        entries[0].texture.sampleType = WGPUTextureSampleType_Float;
        entries[1].binding = 1;
        entries[1].visibility = WGPUShaderStage_Fragment;
        entries[1].storageTexture.access = WGPUStorageTextureAccess_WriteOnly;
        entries[1].storageTexture.format = WGPUTextureFormat_RGBA8Unorm;
        let mut ldesc = zeroed::<WGPUBindGroupLayoutDescriptor>();
        ldesc.entries = entries.as_ptr();
        ldesc.entryCount = entries.len();
        let layout = wgpu_packer::wgpuDeviceCreateBindGroupLayout(device, Some(&ldesc));
        assert!(!layout.is_null(), "header defaults (viewDimension 2D) accepted");

        // Sampled texture + storage texture: the
        // WGPUTextureUsage_StorageBinding bit must survive the mapping (audit
        // #2) otherwise wgc 30 rejects the bind group.
        let mut tdesc = zeroed::<WGPUTextureDescriptor>();
        tdesc.dimension = WGPUTextureDimension_2D;
        tdesc.size = WGPUExtent3D {
            width: 4,
            height: 4,
            depthOrArrayLayers: 1,
        };
        tdesc.format = WGPUTextureFormat_RGBA8Unorm;

        tdesc.usage = WGPUTextureUsage_TextureBinding;
        let sampled = wgpu_packer::wgpuDeviceCreateTexture(device, Some(&tdesc));
        assert!(!sampled.is_null());
        tdesc.usage = WGPUTextureUsage_StorageBinding;
        let storage = wgpu_packer::wgpuDeviceCreateTexture(device, Some(&tdesc));
        assert!(!storage.is_null());

        let sampled_view = wgpu_packer::wgpuTextureCreateView(sampled, None);
        let storage_view = wgpu_packer::wgpuTextureCreateView(storage, None);
        assert!(!sampled_view.is_null() && !storage_view.is_null());

        let mut bentries = zeroed::<[WGPUBindGroupEntry; 2]>();
        bentries[0].binding = 0;
        bentries[0].textureView = sampled_view;
        bentries[1].binding = 1;
        bentries[1].textureView = storage_view;
        let mut bdesc = zeroed::<WGPUBindGroupDescriptor>();
        bdesc.layout = layout;
        bdesc.entries = bentries.as_ptr();
        bdesc.entryCount = bentries.len();
        let bind_group = wgpu_packer::wgpuDeviceCreateBindGroup(device, Some(&bdesc));
        assert!(
            !bind_group.is_null(),
            "valid bind group: 2D defaults + StorageBinding usage relayed"
        );

        wgpu_packer::wgpuBindGroupRelease(bind_group);
        wgpu_packer::wgpuTextureViewRelease(storage_view);
        wgpu_packer::wgpuTextureViewRelease(sampled_view);
        wgpu_packer::wgpuTextureRelease(storage);
        wgpu_packer::wgpuTextureRelease(sampled);
        wgpu_packer::wgpuBindGroupLayoutRelease(layout);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// Binding audit #5 + #6: Unorm8x4BGRA vertex format accepted, and
/// "hole" in vertex buffers (stepMode Undefined + 0 attributes)
/// valid according to webgpu.h.
#[test]
fn render_pipeline_vertex_bgra_format_and_hole() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null(), "device required for the vertex formats oracle");

        const WGSL: &[u8] = b"struct VOut { @builtin(position) pos: vec4<f32>, }
@vertex fn vs(@location(0) p: vec4<f32>) -> VOut { var o: VOut; o.pos = p; return o; }
@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(0.0, 1.0, 0.0, 1.0); }";
        let mut source = zeroed::<WGPUShaderSourceWGSL>();
        source.chain.sType = WGPUSType_ShaderSourceWGSL;
        source.code = sv(WGSL);
        let mut sdesc = zeroed::<WGPUShaderModuleDescriptor>();
        sdesc.nextInChain = &mut source.chain;
        let shader = wgpu_packer::wgpuDeviceCreateShaderModule(device, Some(&sdesc));
        assert!(!shader.is_null());

        // Real buffer 0 (Unorm8x4BGRA), slot 1 = valid hole.
        let mut attributes = zeroed::<[WGPUVertexAttribute; 1]>();
        attributes[0].format = WGPUVertexFormat_Unorm8x4BGRA;
        attributes[0].offset = 0;
        attributes[0].shaderLocation = 0;
        let mut buffers = zeroed::<[WGPUVertexBufferLayout; 2]>();
        buffers[0].arrayStride = 16;
        buffers[0].stepMode = WGPUVertexStepMode_Vertex;
        buffers[0].attributes = attributes.as_ptr();
        buffers[0].attributeCount = attributes.len();
        // Slot 1: stepMode Undefined + no attributes = "hole" (webgpu.h:
        // equivalent of null on the JS side) — no more panic.

        let mut target = zeroed::<WGPUColorTargetState>();
        target.format = WGPUTextureFormat_RGBA8Unorm;
        let mut fragment = zeroed::<WGPUFragmentState>();
        fragment.module = shader;
        fragment.entryPoint = sv(b"fs");
        fragment.targets = &target;
        fragment.targetCount = 1;

        let mut pdesc = zeroed::<WGPURenderPipelineDescriptor>();
        pdesc.vertex.module = shader;
        pdesc.vertex.entryPoint = sv(b"vs");
        pdesc.vertex.buffers = buffers.as_ptr();
        pdesc.vertex.bufferCount = buffers.len();
        pdesc.fragment = &mut fragment;
        let pipeline = wgpu_packer::wgpuDeviceCreateRenderPipeline(device, Some(&pdesc));
        assert!(
            !pipeline.is_null(),
            "pipeline created: BGRA format mapped + vertex hole accepted"
        );

        wgpu_packer::wgpuRenderPipelineRelease(pipeline);
        wgpu_packer::wgpuShaderModuleRelease(shader);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// Binding audit #9: pop on an empty stack = WGPUPopErrorScopeStatus_Error
/// delivered to the callback (webgpu.h), not a panic.
#[test]
fn pop_error_scope_empty_stack_returns_error_status() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null());

        static POP_STATUS: AtomicUsize = AtomicUsize::new(0);
        static POP_TYPE: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn on_pop(
            status: WGPUPopErrorScopeStatus,
            typ: WGPUErrorType,
            message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            POP_STATUS.store(status as usize, Ordering::SeqCst);
            POP_TYPE.store(typ as usize, Ordering::SeqCst);
            assert!(
                message.data.is_null() || message.length == 0,
                "empty message on empty stack"
            );
        }
        let mut cb = zeroed::<WGPUPopErrorScopeCallbackInfo>();
        cb.mode = WGPUCallbackMode_AllowSpontaneous;
        cb.callback = Some(on_pop);

        // No PushErrorScope: the pop must deliver Error, without abort.
        let future = wgpu_packer::wgpuDevicePopErrorScope(device, cb);
        assert_ne!(future.id, 0, "even on error, the future is real");
        assert_eq!(
            POP_STATUS.load(Ordering::SeqCst),
            WGPUPopErrorScopeStatus_Error as usize,
            "empty stack => Error status (webgpu.h)"
        );
        assert_eq!(
            POP_TYPE.load(Ordering::SeqCst),
            WGPUErrorType_NoError as usize
        );

        // A second pop is still a clean error (still no panic).
        wgpu_packer::wgpuDevicePopErrorScope(device, cb);
        assert_eq!(
            POP_STATUS.load(Ordering::SeqCst),
            WGPUPopErrorScopeStatus_Error as usize
        );

        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// Binding audit #11: WGPU_WHOLE_MAP_SIZE in MapAsync = map through
/// the end of the buffer (webgpu.h), not a validation error.
#[test]
fn map_async_whole_map_size() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null());

        let mut bdesc = zeroed::<WGPUBufferDescriptor>();
        bdesc.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
        bdesc.size = 16;
        let buffer = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc));
        assert!(!buffer.is_null());

        static MAP_STATUS: AtomicUsize = AtomicUsize::new(usize::MAX);
        unsafe extern "C" fn on_mapped(
            status: WGPUMapAsyncStatus,
            _message: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            MAP_STATUS.store(status as usize, Ordering::SeqCst);
        }
        let mut cb = zeroed::<WGPUBufferMapCallbackInfo>();
        cb.mode = WGPUCallbackMode_AllowSpontaneous;
        cb.callback = Some(on_mapped);

        // SIZE_MAX = WGPU_WHOLE_MAP_SIZE: the mapping covers [0, size).
        let future =
            wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, usize::MAX, cb);
        assert_ne!(future.id, 0);
        wgpu_packer::wgpuDevicePoll(device, true, None);
        assert_eq!(
            MAP_STATUS.load(Ordering::SeqCst),
            WGPUMapAsyncStatus_Success as usize,
            "WGPU_WHOLE_MAP_SIZE => mapping through the end"
        );
        let range = wgpu_packer::wgpuBufferGetMappedRange(buffer, 0, 16);
        assert!(!range.is_null(), "full range accessible");
        *range = 0xCD;
        wgpu_packer::wgpuBufferUnmap(buffer);

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

// ---------------------------------------------------------------------------
// Regression tests — independent review, 2026-11-18 (D1 + D3)
// ---------------------------------------------------------------------------

/// webgpu.h declares `WGPUStatus wgpu{Adapter,Device}GetLimits(...)` with
/// Success = 1 / Error = 2. The implementation used to return a WGPUBool:
/// 0 matched neither enumerator, so every consumer read the call as a
/// failure.
#[test]
fn get_limits_returns_wgpu_status_success() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        if let Some(&adapter) = adapters.first() {
            let mut limits = zeroed::<WGPULimits>();
            assert_eq!(
                wgpu_packer::wgpuAdapterGetLimits(adapter, Some(&mut limits)),
                WGPUStatus_Success,
                "adapter GetLimits must report WGPUStatus_Success"
            );
            let device = request_first_device(adapter);
            if !device.is_null() {
                let mut limits = zeroed::<WGPULimits>();
                assert_eq!(
                    wgpu_packer::wgpuDeviceGetLimits(device, Some(&mut limits)),
                    WGPUStatus_Success,
                    "device GetLimits must report WGPUStatus_Success"
                );
                wgpu_packer::wgpuDeviceRelease(device);
            }
        }
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// WGPU_NATIVE_VERSION comes from wgpu-pin.env: 30.0.1.0 packed MSB-first
/// is 0x1E000100. Before the fix the env var was never set: always 0.
#[test]
fn get_version_reports_the_pinned_version() {
    assert_eq!(wgpu_packer::wgpuGetVersion(), 0x1E000100);
}

/// A `WGPUFutureWaitInfo` array is reusable across WaitAny calls: an entry
/// that did not complete must be reset to 0 — a stale 1 from a previous
/// call would report a completion that did not happen.
#[test]
fn wait_any_resets_stale_completed_flags() {
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null());
        let lost = wgpu_packer::wgpuDeviceGetLostFuture(device);

        let mut info = WGPUFutureWaitInfo {
            future: lost,
            completed: 1, // stale flag from a previous call
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut info, 0),
            WGPUWaitStatus_TimedOut
        );
        assert_eq!(info.completed, 0, "a non-completed entry must be reset to 0");

        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// mapAsync validation errors go to the C callback ONLY (webgpu.h): one
/// delivery, nothing on the device error sink / uncaptured handler.
#[test]
fn map_async_validation_error_single_channel() {
    unsafe {
        static UNCAPPED: AtomicUsize = AtomicUsize::new(0);
        static DELIVERED: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn on_uncaptured(
            _d: *const WGPUDevice,
            _t: WGPUErrorType,
            _m: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            UNCAPPED.fetch_add(1, Ordering::SeqCst);
        }
        unsafe extern "C" fn on_map(
            _s: WGPUMapAsyncStatus,
            _m: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            DELIVERED.fetch_add(1, Ordering::SeqCst);
        }

        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = {
            let mut slot: usize = 0;
            unsafe extern "C" fn on_device(
                _status: WGPURequestDeviceStatus,
                device: WGPUDevice,
                _m: WGPUStringView,
                u1: *mut core::ffi::c_void,
                _u2: *mut core::ffi::c_void,
            ) {
                *(u1 as *mut usize) = device as usize;
            }
            let mut ddesc = zeroed::<WGPUDeviceDescriptor>();
            ddesc.uncapturedErrorCallbackInfo.callback = Some(on_uncaptured);
            ddesc.deviceLostCallbackInfo.mode = WGPUCallbackMode_AllowSpontaneous;
            let mut cb = zeroed::<WGPURequestDeviceCallbackInfo>();
            cb.mode = WGPUCallbackMode_AllowSpontaneous;
            cb.callback = Some(on_device);
            cb.userdata1 = &mut slot as *mut usize as *mut core::ffi::c_void;
            wgpu_packer::wgpuAdapterRequestDevice(adapter, Some(&ddesc), cb);
            slot as WGPUDevice
        };
        assert!(!device.is_null());

        // No MAP usage: synchronous validation error at map time.
        let mut bdesc = zeroed::<WGPUBufferDescriptor>();
        bdesc.size = 16;
        bdesc.usage = WGPUBufferUsage_CopySrc;
        let buffer = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc));
        assert!(!buffer.is_null());

        let mut map_cb = zeroed::<WGPUBufferMapCallbackInfo>();
        map_cb.mode = WGPUCallbackMode_AllowSpontaneous; // synchronous delivery
        map_cb.callback = Some(on_map);
        let future = wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, 16, map_cb);
        assert_ne!(future.id, 0, "a real future is returned");
        assert_eq!(
            DELIVERED.load(Ordering::SeqCst),
            1,
            "exactly one callback delivery"
        );
        assert_eq!(
            UNCAPPED.load(Ordering::SeqCst),
            0,
            "no second report on the error sink"
        );

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// D2-A: every entry point contains its panics at the ABI boundary — a
/// null handle (the zero-initialized C idiom) returns the type's error
/// value instead of killing the process.
#[test]
fn null_handles_return_error_values_not_aborts() {
    unsafe {
        let mut info = zeroed::<WGPUAdapterInfo>();
        assert_eq!(
            wgpu_packer::wgpuAdapterGetInfo(std::ptr::null(), Some(&mut info)),
            WGPUStatus_Error
        );
        assert!(wgpu_packer::wgpuDeviceCreateBuffer(std::ptr::null(), None).is_null());
        let mut limits = zeroed::<WGPULimits>();
        assert_eq!(
            wgpu_packer::wgpuDeviceGetLimits(std::ptr::null(), Some(&mut limits)),
            WGPUStatus_Error
        );
        // Unimplemented stubs no longer abort either: contained + error value.
        assert_eq!(
            wgpu_packer::wgpuInstanceHasWGSLLanguageFeature(std::ptr::null(), 0),
            0
        );
    }
}

/// The INIT macros set `.mode` = 0. The packer accepts it as WaitAnyOnly
/// (with a warning) instead of aborting: the future is real and its
/// callback is delivered through a wait.
#[test]
fn callback_mode_zero_from_init_macros_is_accepted() {
    unsafe {
        let instance = common::make_instance();
        let mut slot: usize = 0;
        unsafe extern "C" fn on_adapter(
            _status: WGPURequestAdapterStatus,
            adapter: WGPUAdapter,
            _m: WGPUStringView,
            u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            *(u1 as *mut usize) = adapter as usize;
        }
        let mut cb = zeroed::<WGPURequestAdapterCallbackInfo>(); // mode = 0
        cb.callback = Some(on_adapter);
        cb.userdata1 = &mut slot as *mut usize as *mut core::ffi::c_void;
        let future = wgpu_packer::wgpuInstanceRequestAdapter(instance, None, cb);
        assert_ne!(future.id, 0, "mode 0 must not abort: a real future is returned");

        // WaitAnyOnly: delivery through a wait; timeout 0 polls once.
        let mut info = WGPUFutureWaitInfo {
            future,
            completed: 0,
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut info, 0),
            WGPUWaitStatus_Success,
            "the pending WaitAnyOnly delivery completes in a wait"
        );
        assert_eq!(info.completed, 1);
        assert_ne!(slot, 0, "the adapter handle was delivered");
        wgpu_packer::wgpuAdapterRelease(slot as WGPUAdapter);
        common::release_instance(instance);
    }
}

/// Final review: smoke the phase 2-3 entry points end to end on a real
/// adapter — coherent answers, no abort, whatever the support level.
#[test]
fn parity_surface_smoke() {
    unsafe {
        static UNCAPPED: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn on_uncaptured(
            _d: *const WGPUDevice,
            _t: WGPUErrorType,
            _m: WGPUStringView,
            _u1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            UNCAPPED.fetch_add(1, Ordering::SeqCst);
        }
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = {
            let mut slot: usize = 0;
            unsafe extern "C" fn on_device(
                _s: WGPURequestDeviceStatus,
                device: WGPUDevice,
                _m: WGPUStringView,
                u1: *mut core::ffi::c_void,
                _u2: *mut core::ffi::c_void,
            ) {
                *(u1 as *mut usize) = device as usize;
            }
            let mut ddesc = zeroed::<WGPUDeviceDescriptor>();
            ddesc.uncapturedErrorCallbackInfo.callback = Some(on_uncaptured);
            ddesc.deviceLostCallbackInfo.mode = WGPUCallbackMode_AllowSpontaneous;
            let mut cb = zeroed::<WGPURequestDeviceCallbackInfo>();
            cb.mode = WGPUCallbackMode_AllowSpontaneous;
            cb.callback = Some(on_device);
            cb.userdata1 = &mut slot as *mut usize as *mut core::ffi::c_void;
            wgpu_packer::wgpuAdapterRequestDevice(adapter, Some(&ddesc), cb);
            slot as WGPUDevice
        };
        assert!(!device.is_null());

        // Diagnostics queries: coherent answers on any adapter.
        let mut counters = zeroed::<WGPUInternalCounters>();
        wgpu_packer::wgpuDeviceGetInternalCounters(device, &mut counters);
        assert!(counters.Buffers >= 0);

        let mut report = zeroed::<WGPUAllocatorReport>();
        assert_eq!(
            wgpu_packer::wgpuDeviceGetAllocatorReport(device, &mut report),
            WGPUStatus_Success
        );

        let mut downlevel = zeroed::<WGPUDownlevelCapabilities>();
        wgpu_packer::wgpuAdapterGetDownlevelCapabilities(adapter, &mut downlevel);
        let _ = wgpu_packer::wgpuAdapterGetPresentationTimestamp(adapter);

        let mut count: usize = 0;
        assert_eq!(
            wgpu_packer::wgpuAdapterGetCooperativeMatrixProperties(
                adapter,
                Some(&mut count),
                std::ptr::null_mut()
            ),
            WGPUStatus_Success
        );

        // Pipeline cache: feature-dependent — either a handle or a routed
        // error, never an abort.
        let cache_desc = zeroed::<WGPUPipelineCacheDescriptor>();
        let cache = wgpu_packer::wgpuDeviceCreatePipelineCache(device, Some(&cache_desc));
        if !cache.is_null() {
            let size = wgpu_packer::wgpuPipelineCacheGetData(cache, std::ptr::null_mut(), 0);
            assert!(size == 0); // a fresh cache holds no data
            wgpu_packer::wgpuPipelineCacheRelease(cache);
        }

        // Mesh pipeline without the experimental opt-in: routed validation
        // error (sink), NULL handle, no abort.
        let before = UNCAPPED.load(Ordering::SeqCst);
        let mesh = zeroed::<WGPUMeshPipelineDescriptor>();
        assert!(wgpu_packer::wgpuDeviceCreateMeshPipeline(device, Some(&mesh)).is_null());
        assert!(UNCAPPED.load(Ordering::SeqCst) >= before);

        // Passthrough without any source: packer-side rejection, NULL.
        let passthrough = zeroed::<WGPUShaderModuleDescriptorPassthrough>();
        assert!(
            wgpu_packer::wgpuDeviceCreateShaderModulePassthrough(device, Some(&passthrough))
                .is_null()
        );

        // Transition resources with nothing to do: success-shaped no-op.
        let encoder_desc = zeroed::<WGPUCommandEncoderDescriptor>();
        let encoder = wgpu_packer::wgpuDeviceCreateCommandEncoder(device, Some(&encoder_desc));
        wgpu_packer::wgpuCommandEncoderTransitionResources(
            encoder,
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
        );
        let cb_desc = zeroed::<WGPUCommandBufferDescriptor>();
        let finished = wgpu_packer::wgpuCommandEncoderFinish(encoder, Some(&cb_desc));
        wgpu_packer::wgpuCommandBufferRelease(finished);
        wgpu_packer::wgpuCommandEncoderRelease(encoder);

        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}
