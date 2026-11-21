//! Memory management oracle: refcounting, leaks (counting allocator),
//! pairing of mapped ranges, userdata freed exactly once.
//! The intent of each test is the specification.

#[global_allocator]
static ALLOC: common::CountingAllocator = common::CountingAllocator;

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use wgpu_packer::native::*;

unsafe fn zeroed<T>() -> T {
    std::mem::zeroed()
}

unsafe fn enumerate(instance: WGPUInstance) -> Vec<WGPUAdapter> {
    let count = wgpu_packer::wgpuInstanceEnumerateAdapters(instance, None, std::ptr::null_mut());
    if count == 0 {
        return Vec::new();
    }
    let mut adapters: Vec<WGPUAdapter> = vec![std::ptr::null(); count];
    wgpu_packer::wgpuInstanceEnumerateAdapters(instance, None, adapters.as_mut_ptr());
    adapters
}

unsafe fn release_adapters(adapters: &[WGPUAdapter]) {
    for a in adapters {
        wgpu_packer::wgpuAdapterRelease(*a);
    }
}

/// Requests a device on the first adapter (synchronous, uncaptured error
/// handler installed — the default panics). Returns null without a GPU.
unsafe fn request_first_device(adapter: WGPUAdapter) -> WGPUDevice {
    static SLOT: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "C" fn on_device(
        _status: WGPURequestDeviceStatus,
        device: WGPUDevice,
        _message: WGPUStringView,
        _u1: *mut core::ffi::c_void,
        _u2: *mut core::ffi::c_void,
    ) {
        SLOT.store(device as usize, Ordering::SeqCst);
    }
    unsafe extern "C" fn on_error(
        _device: *const WGPUDevice,
        _typ: WGPUErrorType,
        _message: WGPUStringView,
        _u1: *mut core::ffi::c_void,
        _u2: *mut core::ffi::c_void,
    ) {
    }
    SLOT.store(0, Ordering::SeqCst);
    let mut desc = zeroed::<WGPUDeviceDescriptor>();
    desc.uncapturedErrorCallbackInfo.callback = Some(on_error);
    desc.deviceLostCallbackInfo.mode = WGPUCallbackMode_AllowSpontaneous;
    let mut cb = zeroed::<WGPURequestDeviceCallbackInfo>();
    cb.mode = WGPUCallbackMode_AllowSpontaneous;
    cb.callback = Some(on_device);
    wgpu_packer::wgpuAdapterRequestDevice(adapter, Some(&desc), cb);
    SLOT.load(Ordering::SeqCst) as WGPUDevice
}

unsafe fn make_buffer(device: WGPUDevice, size: u64) -> WGPUBuffer {
    let mut bdesc = zeroed::<WGPUBufferDescriptor>();
    bdesc.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
    bdesc.size = size;
    wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc))
}

unsafe extern "C" fn noop_map_callback(
    _status: WGPUMapAsyncStatus,
    _message: WGPUStringView,
    _u1: *mut core::ffi::c_void,
    _u2: *mut core::ffi::c_void,
) {
}

#[test]
fn refcount_release_destroys_at_zero() {
    let _serial = common::TEST_SERIAL_LOCKED();
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };

        // AddRef + Release: refcount 1 -> 2 -> 1, the object stays alive and
        // usable (GetInfo succeeds).
        wgpu_packer::wgpuAdapterAddRef(adapter);
        wgpu_packer::wgpuAdapterRelease(adapter);
        let mut info = zeroed::<WGPUAdapterInfo>();
        let status = wgpu_packer::wgpuAdapterGetInfo(adapter, Some(&mut info));
        assert_eq!(status, WGPUStatus_Success, "object alive after a balanced AddRef/Release");
        wgpu_packer::wgpuAdapterInfoFreeMembers(info);

        release_adapters(&adapters); // last Release of each one: destruction
        common::release_instance(instance);
    }
}

#[test]
fn create_destroy_cycles_return_to_allocation_baseline() {
    let _serial = common::TEST_SERIAL_LOCKED();
    unsafe {
        // Warm-up: installs any lazy caches.
        for _ in 0..3 {
            let i = common::make_instance();
            common::release_instance(i);
        }
        // wgpu's internal threads allocate/stop asynchronously: the baseline
        // is a STABLE value (consecutive reads equal), not a snapshot —
        // otherwise a transient allocation from the previous test skews the
        // comparison.
        fn settled_live() -> u64 {
            let mut last = u64::MAX;
            let mut stable = 0;
            for _ in 0..150 {
                std::thread::sleep(std::time::Duration::from_millis(20));
                let cur = common::live().0;
                if cur == last {
                    stable += 1;
                    if stable >= 5 {
                        return cur;
                    }
                } else {
                    stable = 0;
                }
                last = cur;
            }
            panic!("the allocation counter does not settle (last: {last})");
        }

        for _ in 0..3 {
            let i = common::make_instance();
            common::release_instance(i);
        }
        let base = settled_live();
        for _ in 0..10 {
            let i = common::make_instance();
            common::release_instance(i);
        }
        let after = settled_live();
        assert_eq!(
            after, base,
            "create/destroy cycles must return to the settled baseline"
        );
    }
}

#[test]
fn mapped_range_pairing() {
    let _serial = common::TEST_SERIAL_LOCKED();
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        if device.is_null() {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        }

        let buffer = make_buffer(device, 32);
        let mut cb = zeroed::<WGPUBufferMapCallbackInfo>();
        cb.mode = WGPUCallbackMode_AllowSpontaneous;
        cb.callback = Some(noop_map_callback);
        wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, 32, cb);
        wgpu_packer::wgpuDevicePoll(device, true, None);

        let r1 = wgpu_packer::wgpuBufferGetMappedRange(buffer, 0, 32);
        assert!(!r1.is_null(), "mapped range available between map and unmap");
        *r1 = 0xCD;
        wgpu_packer::wgpuBufferUnmap(buffer);
        let r2 = wgpu_packer::wgpuBufferGetMappedRange(buffer, 0, 32);
        assert!(r2.is_null(), "get_mapped_range after unmap must return NULL");

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn const_mapped_range_after_unmap_returns_null() {
    let _serial = common::TEST_SERIAL_LOCKED();
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        if device.is_null() {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        }

        // Symmetric read path: COPY_DST|MAP_READ → map(Read).
        let mut bdesc = zeroed::<WGPUBufferDescriptor>();
        bdesc.usage = WGPUBufferUsage_MapRead | WGPUBufferUsage_CopyDst;
        bdesc.size = 16;
        let buffer = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc));
        let mut cb = zeroed::<WGPUBufferMapCallbackInfo>();
        cb.mode = WGPUCallbackMode_AllowSpontaneous;
        cb.callback = Some(noop_map_callback);
        wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Read, 0, 16, cb);
        wgpu_packer::wgpuDevicePoll(device, true, None);

        let r1 = wgpu_packer::wgpuBufferGetConstMappedRange(buffer, 0, 16);
        assert!(!r1.is_null(), "const mapped range available between map and unmap");
        wgpu_packer::wgpuBufferUnmap(buffer);
        // After unmap, NULL — never a panic across the ABI.
        let r2 = wgpu_packer::wgpuBufferGetConstMappedRange(buffer, 0, 16);
        assert!(r2.is_null(), "get_const_mapped_range after unmap must return NULL");

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn map_async_userdata_released_exactly_once() {
    let _serial = common::TEST_SERIAL_LOCKED();
    unsafe {
        let instance = common::make_instance();
        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        if device.is_null() {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        }

        static CALLS: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn cb(
            _status: WGPUMapAsyncStatus,
            _message: WGPUStringView,
            userdata1: *mut core::ffi::c_void,
            _u2: *mut core::ffi::c_void,
        ) {
            CALLS.fetch_add(1, Ordering::SeqCst);
            drop(Box::from_raw(userdata1 as *mut usize)); // freed HERE, exactly once
        }

        let buffer = make_buffer(device, 8);
        let userdata = Box::into_raw(Box::new(42usize)) as *mut core::ffi::c_void;
        let mut info = zeroed::<WGPUBufferMapCallbackInfo>();
        info.mode = WGPUCallbackMode_AllowSpontaneous;
        info.callback = Some(cb);
        info.userdata1 = userdata;

        wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, 8, info);
        // CONTRACT: poll BEFORE any Release of the buffer — the poll delivers
        // the callback (and frees the userdata).
        wgpu_packer::wgpuDevicePoll(device, true, None);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1, "userdata delivered/freed exactly once");

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}
