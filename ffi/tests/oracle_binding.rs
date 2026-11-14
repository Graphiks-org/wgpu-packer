//! Oracle de binding : cycle de vie complet via l'ABI C, conversions et
//! livraison des callbacks. L'intention de chaque test est la spécification ;
//! les signatures suivent les headers vendored (modèle CallbackInfo).

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
    assert_eq!(written, count, "le second appel d'énumération remplit tout");
    adapters
}

unsafe fn release_adapters(adapters: &[WGPUAdapter]) {
    for a in adapters {
        wgpu_packer::wgpuAdapterRelease(*a);
    }
}

/// StringView C sur des octets statiques (longueur explicite, sans NUL).
unsafe fn sv(bytes: &[u8]) -> WGPUStringView {
    WGPUStringView {
        length: bytes.len(),
        data: bytes.as_ptr() as *const i8,
    }
}

/// Demande un device sur le premier adapter (synchrone, handler d'erreurs non
/// capturées installé — le défaut panique). Retourne null sans GPU.
/// Le slot de résultat passe par userdata1 : les tests tournent en parallèle,
/// un static serait partagé entre eux.
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
    // Mode explicite : webgpu.h n'a pas de mode par défaut (0 = invalide).
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
        assert!(!instance.is_null(), "instance non NULL");

        let adapters = enumerate(instance);
        if let Some(adapter) = adapters.first() {
            let mut info = zeroed::<WGPUAdapterInfo>();
            let status = wgpu_packer::wgpuAdapterGetInfo(*adapter, Some(&mut info));
            assert_eq!(status, WGPUStatus_Success, "GetInfo doit réussir");
            let vendor = string_view_bytes(&info.vendor);
            assert!(
                std::str::from_utf8(&vendor).is_ok(),
                "le nom vendor doit être de l'UTF-8 valide (got {vendor:?})"
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
            // machine sans GPU : la chaîne GPU n'est pas testable ici
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };

        // requestDevice est synchrone dans cette implémentation : le callback
        // est invoqué avant le retour de l'appel. On installe un handler
        // d'erreurs non capturées (le défaut panique — cf. lib.rs:422).
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
            "requestDevice doit réussir"
        );
        let device = DEVICE.load(Ordering::SeqCst) as WGPUDevice;
        assert!(!device.is_null());

        // Buffer MAP_WRITE|COPY_SRC (règle WebGPU : MAP s'combine au COPY opposé),
        // mapping async livré au poll.
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
        assert_ne!(map_future.id, 0, "MapAsync retourne une future réelle");
        wgpu_packer::wgpuDevicePoll(device, true, None);

        assert_eq!(MAP_CALLS.load(Ordering::SeqCst), 1, "map_async livré exactement une fois");
        assert_eq!(MAP_STATUS.load(Ordering::SeqCst), WGPUMapAsyncStatus_Success as usize);
        assert_eq!(UNCAPPED_ERRORS.load(Ordering::SeqCst), 0, "aucune erreur non capturée");

        let range = wgpu_packer::wgpuBufferGetMappedRange(buffer, 0, 16);
        assert!(!range.is_null(), "plage mappée accessible après map");
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
        // Toute fonction déclarée dans les headers est résoluble — y compris
        // les stubs documentés (résoudre n'est pas appeler, cf. GUIDE §6).
        for name in [
            "wgpuCreateInstance",
            "wgpuBufferMapAsync",
            "wgpuGetProcAddress",
            "wgpuInstanceWaitAny",
        ] {
            assert!(
                wgpu_packer::wgpuGetProcAddress(sv(name.as_bytes())).is_some(),
                "{name} doit être résoluble"
            );
        }
        assert!(
            wgpu_packer::wgpuGetProcAddress(sv(b"wgpuDoesNotExist")).is_none(),
            "nom inconnu => NULL"
        );
        let empty = WGPUStringView {
            length: 0,
            data: std::ptr::null(),
        };
        assert!(
            wgpu_packer::wgpuGetProcAddress(empty).is_none(),
            "nom vide => NULL"
        );

        // Features d'instance : liste non vide, cohérente avec HasInstanceFeature.
        let mut features = zeroed::<WGPUSupportedInstanceFeatures>();
        wgpu_packer::wgpuGetInstanceFeatures(Some(&mut features));
        assert!(features.featureCount > 0, "au moins une feature d'instance");
        let list =
            std::slice::from_raw_parts(features.features, features.featureCount);
        assert!(
            list.contains(&WGPUInstanceFeatureName_MultipleDevicesPerAdapter),
            "wgc supporte plusieurs devices par adapter"
        );
        assert_eq!(
            wgpu_packer::wgpuHasInstanceFeature(WGPUInstanceFeatureName_MultipleDevicesPerAdapter),
            1,
        );
        // TimedWaitAny est désormais supporté (attente temporisée activée à la
        // création via requiredFeatures ; cf. l'oracle dédié plus bas).
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
        assert!(!device.is_null(), "device requis pour l'oracle adapter info");

        // Le device expose les mêmes informations que son adapter.
        let mut dev_info = zeroed::<WGPUAdapterInfo>();
        let status = wgpu_packer::wgpuDeviceGetAdapterInfo(device, Some(&mut dev_info));
        assert_eq!(status, WGPUStatus_Success, "DeviceGetAdapterInfo doit réussir");
        let mut ad_info = zeroed::<WGPUAdapterInfo>();
        let status = wgpu_packer::wgpuAdapterGetInfo(adapter, Some(&mut ad_info));
        assert_eq!(status, WGPUStatus_Success);

        assert_eq!(
            string_view_bytes(&dev_info.device),
            string_view_bytes(&ad_info.device),
            "même nom d'adaptateur via device et via adapter"
        );
        assert_eq!(dev_info.backendType, ad_info.backendType);
        assert_eq!(dev_info.vendorID, ad_info.vendorID);
        assert!(
            std::str::from_utf8(&string_view_bytes(&dev_info.description)).is_ok(),
            "description UTF-8 valide"
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
        assert!(!device.is_null(), "device requis pour l'oracle map state");

        let mut bdesc = zeroed::<WGPUBufferDescriptor>();
        bdesc.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
        bdesc.size = 16;
        let buffer = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc));
        assert!(!buffer.is_null());
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Unmapped,
            "buffer neuf non mappé"
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

        // Pending dès l'appel (état posé synchrône), Mapped après livraison.
        wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, 16, cb);
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Pending,
            "map async en attente"
        );
        wgpu_packer::wgpuDevicePoll(device, true, None);
        assert_eq!(MAP_STATUS.load(Ordering::SeqCst), WGPUMapAsyncStatus_Success as usize);
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Mapped,
            "map livré => mappé"
        );

        // Mode WaitAnyOnly : complétion au poll du device MAIS callback retenu
        // — livré uniquement par wgpuInstanceWaitAny sur cette future.
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
            "WaitAnyOnly : pas de livraison au poll du device"
        );
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Mapped,
            "l'opération est complétée (état), seul le callback est retenu"
        );
        let mut wait_info = WGPUFutureWaitInfo {
            future: waitonly_future,
            completed: 0,
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut wait_info, 0),
            WGPUWaitStatus_Success,
            "WaitAny livre la future complétée"
        );
        assert_eq!(wait_info.completed, 1, "flag completed posé");
        assert_eq!(WAITONLY_CALLS.load(Ordering::SeqCst), 1, "callback livré par WaitAny");
        assert_eq!(
            WAITONLY_STATUS.load(Ordering::SeqCst),
            WGPUMapAsyncStatus_Success as usize
        );
        // Future consommée : une seconde attente dessus est invalide.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut wait_info, 0),
            WGPUWaitStatus_Error,
            "future déjà livrée => Error"
        );
        // Le buffer reste mappé : la section Write/ReadMappedRange ci-dessous
        // s'exerce sur ce mapping.

        // Écriture via WriteMappedRange, relecture via ReadMappedRange.
        let payload: [u8; 16] = core::array::from_fn(|i| (i as u8) ^ 0xA5);
        let status = wgpu_packer::wgpuBufferWriteMappedRange(
            buffer,
            0,
            payload.as_ptr() as *const core::ffi::c_void,
            payload.len(),
        );
        assert_eq!(status, WGPUStatus_Success, "écriture dans la plage mappée");
        let mut out = [0u8; 16];
        let status = wgpu_packer::wgpuBufferReadMappedRange(
            buffer,
            0,
            out.as_mut_ptr() as *mut core::ffi::c_void,
            out.len(),
        );
        assert_eq!(status, WGPUStatus_Success, "lecture de la plage mappée");
        assert_eq!(out, payload, "aller-retour Read/WriteMappedRange");

        // Refus : WGPU_WHOLE_MAP_SIZE non accepté, data NULL, hors bornes.
        let mut scratch = [0u8; 8];
        assert_eq!(
            wgpu_packer::wgpuBufferReadMappedRange(
                buffer,
                0,
                scratch.as_mut_ptr() as *mut core::ffi::c_void,
                usize::MAX,
            ),
            WGPUStatus_Error,
            "WGPU_WHOLE_MAP_SIZE refusé"
        );
        assert_eq!(
            wgpu_packer::wgpuBufferReadMappedRange(buffer, 0, std::ptr::null_mut(), 8),
            WGPUStatus_Error,
            "destination NULL refusée"
        );
        assert_eq!(
            wgpu_packer::wgpuBufferReadMappedRange(
                buffer,
                8,
                scratch.as_mut_ptr() as *mut core::ffi::c_void,
                16,
            ),
            WGPUStatus_Error,
            "plage hors bornes refusée"
        );

        wgpu_packer::wgpuBufferUnmap(buffer);
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer),
            WGPUBufferMapState_Unmapped,
            "unmap => non mappé"
        );
        assert_eq!(
            wgpu_packer::wgpuBufferReadMappedRange(
                buffer,
                0,
                scratch.as_mut_ptr() as *mut core::ffi::c_void,
                8,
            ),
            WGPUStatus_Error,
            "lecture hors mapping refusée (abort contenu, pas de panique)"
        );

        // mappedAtCreation : mappé dès la création.
        let mut bdesc2 = zeroed::<WGPUBufferDescriptor>();
        bdesc2.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
        bdesc2.size = 8;
        bdesc2.mappedAtCreation = 1;
        let buffer2 = wgpu_packer::wgpuDeviceCreateBuffer(device, Some(&bdesc2));
        assert!(!buffer2.is_null());
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer2),
            WGPUBufferMapState_Mapped,
            "mappedAtCreation => mappé"
        );
        wgpu_packer::wgpuBufferDestroy(buffer2);
        assert_eq!(
            wgpu_packer::wgpuBufferGetMapState(buffer2),
            WGPUBufferMapState_Unmapped,
            "destroy => non mappé"
        );
        wgpu_packer::wgpuBufferRelease(buffer2);

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// Shader module WGSL minimal (calcul trivial), pour les oracles de pipelines.
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
        assert!(!device.is_null(), "device requis pour l'oracle pipeline async");
        let shader = make_compute_shader(device);
        assert!(!shader.is_null());

        // Chemin heureux : callback livré avant le retour, pipeline valide.
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
        assert_ne!(future.id, 0, "future réelle (complétée avant le retour)");
        assert_eq!(
            STATUS.load(Ordering::SeqCst),
            WGPUCreatePipelineAsyncStatus_Success as usize,
            "création async réussie (AllowSpontaneous => callback avant retour)"
        );
        let pipeline = PIPELINE.load(Ordering::SeqCst) as WGPUComputePipeline;
        assert!(!pipeline.is_null(), "pipeline livré au callback");
        wgpu_packer::wgpuComputePipelineRelease(pipeline);

        // Chemin d'erreur : entry point inconnu -> ValidationError au callback
        // uniquement, pipeline NULL, message non vide, pas d'abort.
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
            assert!(pipeline.is_null(), "aucun pipeline sur erreur");
        }
        let mut bad = zeroed::<WGPUComputePipelineDescriptor>();
        bad.compute.module = shader;
        bad.compute.entryPoint = sv(b"entree_inexistante");
        let mut cb_err = zeroed::<WGPUCreateComputePipelineAsyncCallbackInfo>();
        cb_err.mode = WGPUCallbackMode_AllowSpontaneous;
        cb_err.callback = Some(on_pipeline_error);
        let err_future =
            wgpu_packer::wgpuDeviceCreateComputePipelineAsync(device, Some(&bad), cb_err);
        assert_ne!(err_future.id, 0, "même en erreur, la future se complète");
        assert_eq!(
            ERR_STATUS.load(Ordering::SeqCst),
            WGPUCreatePipelineAsyncStatus_ValidationError as usize,
            "erreur de validation livrée au callback"
        );
        assert!(ERR_MSG_LEN.load(Ordering::SeqCst) > 0, "message explicite fourni");

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

        // (0 future, timeout 0) : équivalent ProcessEvents -> Success.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 0, std::ptr::null_mut(), 0),
            WGPUWaitStatus_Success,
        );
        // Timeout sans aucune future : invalide (spec Wait-Any).
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 0, std::ptr::null_mut(), 1),
            WGPUWaitStatus_Error,
            "timeout sans future => Error"
        );
        // Pointeur nul avec futureCount > 0 : invalide.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, std::ptr::null_mut(), 0),
            WGPUWaitStatus_Error,
            "futures NULL avec futureCount > 0 => Error"
        );

        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null());

        // Future de perte : réelle, stable, jamais complétée sur un device
        // sain (détection de perte passive — cf. GUIDE).
        let lost = wgpu_packer::wgpuDeviceGetLostFuture(device);
        assert_ne!(lost.id, 0, "la future de perte n'est pas la sentinelle NULL");
        assert_eq!(
            wgpu_packer::wgpuDeviceGetLostFuture(device).id,
            lost.id,
            "id stable entre appels"
        );

        // Attente non temporisée (timeout 0) : un seul poll sans blocage, la
        // future de perte ne se complète pas -> TimedOut, flag non posé.
        let mut info = WGPUFutureWaitInfo {
            future: lost,
            completed: 0,
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut info, 0),
            WGPUWaitStatus_TimedOut,
            "timeout 0 = poll unique, sans blocage"
        );
        assert_eq!(info.completed, 0);

        // Attente temporisée sans la feature TimedWaitAny (instance par
        // défaut) : Error.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut info, 1),
            WGPUWaitStatus_Error,
            "timeoutNS > 0 exige la feature TimedWaitAny à la création"
        );

        // Futures invalides : id nul, doublon.
        let mut null_info = WGPUFutureWaitInfo {
            future: WGPUFuture { id: 0 },
            completed: 0,
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut null_info, 0),
            WGPUWaitStatus_Error,
            "id 0 (sentinelle) => Error"
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
            "doublon dans la liste => Error"
        );

        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

#[test]
fn timed_wait_any_feature_and_delivery_modes() {
    unsafe {
        // Instance avec la feature TimedWaitAny demandée à la création.
        let mut idesc = zeroed::<WGPUInstanceDescriptor>();
        let features = [WGPUInstanceFeatureName_TimedWaitAny];
        idesc.requiredFeatures = features.as_ptr();
        idesc.requiredFeatureCount = 1;
        let instance = wgpu_packer::wgpuCreateInstance(Some(&idesc));
        assert!(!instance.is_null(), "création avec TimedWaitAny acceptée");

        // Limits : timedWaitAnyMaxCount non nul (aucune limite réelle).
        let mut limits = zeroed::<WGPUInstanceLimits>();
        assert_eq!(
            wgpu_packer::wgpuGetInstanceLimits(Some(&mut limits)),
            WGPUStatus_Success
        );
        assert_eq!(
            limits.timedWaitAnyMaxCount,
            usize::MAX,
            "pas de limite réelle sur le nombre de futures"
        );

        let adapters = enumerate(instance);
        let Some(&adapter) = adapters.first() else {
            release_adapters(&adapters);
            common::release_instance(instance);
            return;
        };
        let device = request_first_device(adapter);
        assert!(!device.is_null(), "device requis pour l'oracle timed wait");

        // Attente temporisée qui réussit : map en WaitAnyOnly, complétée par
        // le poll interne de WaitAny.
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
        // timeout 100 ms : largement au-delà du poll de complétion.
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut info, 100_000_000),
            WGPUWaitStatus_Success,
            "attente temporisée réussie (complétion par le poll interne)"
        );
        assert_eq!(info.completed, 1);
        assert_eq!(TIMED_CALLS.load(Ordering::SeqCst), 1, "callback livré");
        assert_eq!(
            TIMED_STATUS.load(Ordering::SeqCst),
            WGPUMapAsyncStatus_Success as usize
        );

        // Attente temporisée qui expire : la future de perte d'un device sain
        // ne se complète jamais -> TimedOut (déterministe).
        let lost = wgpu_packer::wgpuDeviceGetLostFuture(device);
        let mut lost_info = WGPUFutureWaitInfo {
            future: lost,
            completed: 0,
        };
        assert_eq!(
            wgpu_packer::wgpuInstanceWaitAny(instance, 1, &mut lost_info, 5_000_000),
            WGPUWaitStatus_TimedOut,
            "expiration après le timeout (5 ms)"
        );
        assert_eq!(lost_info.completed, 0, "aucune complétion signalée");

        // Mode AllowProcessEvents : complétion au poll du device, callback
        // livré par wgpuInstanceProcessEvents (pas avant).
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
            "AllowProcessEvents : pas de livraison au poll du device"
        );
        wgpu_packer::wgpuInstanceProcessEvents(instance);
        assert_eq!(
            PE_CALLS.load(Ordering::SeqCst),
            1,
            "livré par ProcessEvents"
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
        // Hint accepté et ignoré (wgc 29 sans setter) : l'ABI ne doit pas abort.
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

        // 2D d'une couche, sans chaîne : dérivation -> 2D.
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

        // 2D de 6 couches : dérivation -> 2DArray.
        desc.size.depthOrArrayLayers = 6;
        let layered = wgpu_packer::wgpuDeviceCreateTexture(device, Some(&desc));
        assert!(!layered.is_null());
        assert_eq!(
            wgpu_packer::wgpuTextureGetTextureBindingViewDimension(layered),
            WGPUTextureViewDimension_2DArray,
        );
        wgpu_packer::wgpuTextureRelease(layered);

        // Chaîne explicite WGPUSType_TextureBindingViewDimension : respectée.
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
            "valeur chaînée explicite respectée"
        );
        wgpu_packer::wgpuTextureRelease(one_d);

        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// Audit binding n°4 + n°2 : défauts du header dans les binding layouts
/// (viewDimension Undefined « defaults to 2D ») et usage StorageBinding
/// relayé — wgpu 30 l'exige sur la vue au moment du bind group.
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
        assert!(!device.is_null(), "device requis pour l'oracle binding layouts");

        // Slot 0 : texture avec les défauts des macros INIT du header
        // (viewDimension laissé à Undefined). Slot 1 : storage texture,
        // viewDimension Undefined également (défaut documenté 2D).
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
        assert!(!layout.is_null(), "défauts header (viewDimension 2D) acceptés");

        // Texture échantillonnée + texture storage : le bit
        // WGPUTextureUsage_StorageBinding doit survivre au mapping (audit
        // n°2) sinon wgc 30 rejette le bind group.
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
            "bind group valide : défauts 2D + usage StorageBinding relayés"
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

/// Audit binding n°5 + n°6 : format de vertex Unorm8x4BGRA accepté, et
/// « trou » dans les buffers de vertex (stepMode Undefined + 0 attributs)
/// valide selon webgpu.h.
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
        assert!(!device.is_null(), "device requis pour l'oracle vertex formats");

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

        // Buffer 0 réel (Unorm8x4BGRA), slot 1 = trou valide.
        let mut attributes = zeroed::<[WGPUVertexAttribute; 1]>();
        attributes[0].format = WGPUVertexFormat_Unorm8x4BGRA;
        attributes[0].offset = 0;
        attributes[0].shaderLocation = 0;
        let mut buffers = zeroed::<[WGPUVertexBufferLayout; 2]>();
        buffers[0].arrayStride = 16;
        buffers[0].stepMode = WGPUVertexStepMode_Vertex;
        buffers[0].attributes = attributes.as_ptr();
        buffers[0].attributeCount = attributes.len();
        // Slot 1 : stepMode Undefined + aucun attribut = « trou » (webgpu.h :
        // équivalent de null côté JS) — plus de panic.

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
            "pipeline créé : format BGRA mappé + trou vertex accepté"
        );

        wgpu_packer::wgpuRenderPipelineRelease(pipeline);
        wgpu_packer::wgpuShaderModuleRelease(shader);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}

/// Audit binding n°9 : pop sur pile vide = WGPUPopErrorScopeStatus_Error
/// livré au callback (webgpu.h), pas une panic.
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
                "message vide sur pile vide"
            );
        }
        let mut cb = zeroed::<WGPUPopErrorScopeCallbackInfo>();
        cb.mode = WGPUCallbackMode_AllowSpontaneous;
        cb.callback = Some(on_pop);

        // Aucun PushErrorScope : le pop doit livrer Error, sans abort.
        let future = wgpu_packer::wgpuDevicePopErrorScope(device, cb);
        assert_ne!(future.id, 0, "même en erreur, la future est réelle");
        assert_eq!(
            POP_STATUS.load(Ordering::SeqCst),
            WGPUPopErrorScopeStatus_Error as usize,
            "pile vide => statut Error (webgpu.h)"
        );
        assert_eq!(
            POP_TYPE.load(Ordering::SeqCst),
            WGPUErrorType_NoError as usize
        );

        // Un second pop reste une erreur propre (toujours pas de panic).
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

/// Audit binding n°11 : WGPU_WHOLE_MAP_SIZE dans MapAsync = mapper jusqu'à
/// la fin du buffer (webgpu.h), pas une erreur de validation.
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

        // SIZE_MAX = WGPU_WHOLE_MAP_SIZE : le mapping couvre [0, taille).
        let future =
            wgpu_packer::wgpuBufferMapAsync(buffer, WGPUMapMode_Write, 0, usize::MAX, cb);
        assert_ne!(future.id, 0);
        wgpu_packer::wgpuDevicePoll(device, true, None);
        assert_eq!(
            MAP_STATUS.load(Ordering::SeqCst),
            WGPUMapAsyncStatus_Success as usize,
            "WGPU_WHOLE_MAP_SIZE => mapping jusqu'à la fin"
        );
        let range = wgpu_packer::wgpuBufferGetMappedRange(buffer, 0, 16);
        assert!(!range.is_null(), "plage complète accessible");
        *range = 0xCD;
        wgpu_packer::wgpuBufferUnmap(buffer);

        wgpu_packer::wgpuBufferRelease(buffer);
        wgpu_packer::wgpuDeviceRelease(device);
        release_adapters(&adapters);
        common::release_instance(instance);
    }
}
