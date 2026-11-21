//! Ray tracing (phase 2 lot 3b): acceleration structures. wgpu 30 exposes
//! creation, building, marking built and compaction — the ray QUERY side
//! happens in shaders (the RayQuery feature); there is no dispatch_rays.
//! Everything here requires the experimental opt-in + the RT features.

use crate::{
    conv,
    error::{handle_error, ErrorSink},
    native,
    objects::WGPUBlasImpl,
    objects::WGPUTlasImpl,
    utils::{make_slice, string_view_into_label},
};
use std::sync::Arc;

// The C enum types are `u32` on some targets and `i32` on others (clang
// default per target): taking the native enum type keeps both compiling.
fn map_as_flags(flags: native::WGPUNativeAsFlags) -> wgt::AccelerationStructureFlags {
    wgt::AccelerationStructureFlags::from_bits_retain(flags as u8)
}

fn map_geometry_flags(
    flags: native::WGPUNativeAsGeometryFlags,
) -> wgt::AccelerationStructureGeometryFlags {
    wgt::AccelerationStructureGeometryFlags::from_bits_retain(flags as u8)
}

/// The C callback + userdata copies for wgpuBlasPrepareCompactAsync.
/// Raw pointers are not Send: the closure only forwards them back to C,
/// never dereferences them (same contract as every CallbackInfo).
struct CompactUserdata {
    callback: unsafe extern "C" fn(
        native::WGPUBool,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ),
    u1: *mut core::ffi::c_void,
    u2: *mut core::ffi::c_void,
}
unsafe impl Send for CompactUserdata {}

/// Maps the C size descriptors to wgt's enum, keeping the descriptors
/// alive for the duration of the call.
unsafe fn map_blas_sizes(
    sizes: &native::WGPUBlasGeometrySizeDescriptors,
) -> wgt::BlasGeometrySizeDescriptors {
    match sizes.type_ {
        0 => wgt::BlasGeometrySizeDescriptors::Triangles {
            descriptors: make_slice(sizes.triangleGeometries, sizes.triangleGeometryCount)
                .iter()
                .map(|size| wgt::BlasTriangleGeometrySizeDescriptor {
                    vertex_format: conv::map_vertex_format(size.vertexFormat)
                        .expect("invalid vertex format for BLAS geometry size"),
                    vertex_count: size.vertexCount,
                    index_format: match size.indexFormat {
                        native::WGPUIndexFormat_Undefined => None,
                        format => Some(
                            conv::map_index_format(format)
                                .expect("invalid index format for BLAS geometry size"),
                        ),
                    },
                    index_count: (size.indexCount != u32::MAX).then_some(size.indexCount),
                    flags: map_geometry_flags(size.flags),
                })
                .collect(),
        },
        1 => wgt::BlasGeometrySizeDescriptors::AABBs {
            descriptors: make_slice(sizes.aabbGeometries, sizes.aabbGeometryCount)
                .iter()
                .map(|size| wgt::BlasAABBGeometrySizeDescriptor {
                    primitive_count: size.primitiveCount,
                    flags: map_geometry_flags(size.flags),
                })
                .collect(),
        },
        _ => panic!("invalid BLAS geometry type (0 = triangles, 1 = AABBs)"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateBlas(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUBlasDescriptor>,
    sizes: Option<&native::WGPUBlasGeometrySizeDescriptors>,
) -> native::WGPUBlas {
    crate::ffi_utils::contain_entry("wgpuDeviceCreateBlas", std::ptr::null(), || {
        let device = device.as_ref().expect("invalid device");
        let descriptor = descriptor.expect("invalid descriptor");
        let sizes = sizes.expect("invalid geometry size descriptors");

        let desc = wgt::CreateBlasDescriptor {
            label: string_view_into_label(descriptor.label),
            flags: map_as_flags(descriptor.flags),
            update_mode: match descriptor.updateMode {
                native::WGPUNativeAsUpdateMode_PreferUpdate => {
                    wgt::AccelerationStructureUpdateMode::PreferUpdate
                }
                _ => wgt::AccelerationStructureUpdateMode::Build,
            },
        };
        let wgt_sizes = map_blas_sizes(sizes);

        let (blas_id, _, error) =
            device
                .context
                .device_create_blas(device.id, &desc, wgt_sizes, None);
        if let Some(cause) = error {
            handle_error(&device.error_sink, cause, desc.label, "wgpuDeviceCreateBlas");
            return std::ptr::null();
        }
        Arc::into_raw(Arc::new(WGPUBlasImpl {
            context: device.context.clone(),
            id: blas_id,
        }))
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateTlas(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUTlasDescriptor>,
) -> native::WGPUTlas {
    crate::ffi_utils::contain_entry("wgpuDeviceCreateTlas", std::ptr::null(), || {
        let device = device.as_ref().expect("invalid device");
        let descriptor = descriptor.expect("invalid descriptor");

        let desc = wgt::CreateTlasDescriptor {
            label: string_view_into_label(descriptor.label),
            flags: map_as_flags(descriptor.flags),
            update_mode: match descriptor.updateMode {
                native::WGPUNativeAsUpdateMode_PreferUpdate => {
                    wgt::AccelerationStructureUpdateMode::PreferUpdate
                }
                _ => wgt::AccelerationStructureUpdateMode::Build,
            },
            max_instances: descriptor.maxInstances,
        };

        let (tlas_id, error) = device.context.device_create_tlas(device.id, &desc, None);
        if let Some(cause) = error {
            handle_error(&device.error_sink, cause, desc.label, "wgpuDeviceCreateTlas");
            return std::ptr::null();
        }
        Arc::into_raw(Arc::new(WGPUTlasImpl {
            context: device.context.clone(),
            id: tlas_id,
        }))
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBlasAddRef(blas: native::WGPUBlas) {
    crate::ffi_utils::contain_entry("wgpuBlasAddRef", (), || {
        assert!(!blas.is_null(), "invalid blas");
        Arc::increment_strong_count(blas);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBlasRelease(blas: native::WGPUBlas) {
    crate::ffi_utils::contain_entry("wgpuBlasRelease", (), || {
        assert!(!blas.is_null(), "invalid blas");
        Arc::decrement_strong_count(blas);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTlasAddRef(tlas: native::WGPUTlas) {
    crate::ffi_utils::contain_entry("wgpuTlasAddRef", (), || {
        assert!(!tlas.is_null(), "invalid tlas");
        Arc::increment_strong_count(tlas);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuTlasRelease(tlas: native::WGPUTlas) {
    crate::ffi_utils::contain_entry("wgpuTlasRelease", (), || {
        assert!(!tlas.is_null(), "invalid tlas");
        Arc::decrement_strong_count(tlas);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBlasPrepareCompactAsync(
    blas: native::WGPUBlas,
    callback: Option<
        unsafe extern "C" fn(
            native::WGPUBool,
            *mut core::ffi::c_void,
            *mut core::ffi::c_void,
        ),
    >,
    userdata1: *mut core::ffi::c_void,
    userdata2: *mut core::ffi::c_void,
) -> u64 {
    crate::ffi_utils::contain_entry("wgpuBlasPrepareCompactAsync", 0, || {
        let blas = blas.as_ref().expect("invalid blas");

        // The wgc callback is a boxed FnOnce over the compact result; wrap
        // the C callback + userdata copies. Raw pointers are not Send: the
        // closure only forwards them back to C, never dereferences them.

        let closure: Option<wgc::resource::BlasCompactCallback> = callback.map(|callback| {
            let userdata = CompactUserdata {
                callback,
                u1: userdata1,
                u2: userdata2,
            };
            let boxed: wgc::resource::BlasCompactCallback =
                Box::new(move |result: wgc::resource::BlasPrepareCompactResult| {
                    // Moving the WHOLE binding (not its fields) makes the
                    // closure capture the struct itself, whose Send impl
                    // covers the raw pointers (RFC 2229 disjoint captures
                    // would otherwise bypass it).
                    let ud = userdata;
                    let ok = result.is_ok();
                    if !ok {
                        log::error!("wgpuBlasPrepareCompactAsync: {result:?}");
                    }
                    (ud.callback)(ok as native::WGPUBool, ud.u1, ud.u2);
                });
            boxed
        });

        match blas.context.blas_prepare_compact_async(blas.id, closure) {
            Ok(index) => index,
            Err(cause) => {
                log::error!("wgpuBlasPrepareCompactAsync: {cause}");
                0
            }
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuBlasReadyForCompaction(blas: native::WGPUBlas) -> native::WGPUBool {
    crate::ffi_utils::contain_entry("wgpuBlasReadyForCompaction", false as native::WGPUBool, || {
        let blas = blas.as_ref().expect("invalid blas");
        match blas.context.ready_for_compaction(blas.id) {
            Ok(ready) => ready as native::WGPUBool,
            Err(cause) => {
                log::error!("wgpuBlasReadyForCompaction: {cause}");
                false as native::WGPUBool
            }
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderBuildAccelerationStructures(
    command_encoder: native::WGPUCommandEncoder,
    blas_entries: *const native::WGPUBlasBuildEntry,
    blas_entry_count: usize,
    tlas_packages: *const native::WGPUTlasPackage,
    tlas_package_count: usize,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderBuildAccelerationStructures",
        (),
        || {
        let encoder = command_encoder.as_ref().expect("invalid command encoder");

        // wgc's build entries BORROW the size descriptors: materialize
        // them per entry (kept alive in these vectors), then zip with the
        // buffer bindings of the C descriptors.
        // Pass 1: materialize every entry's size descriptors (wgc's build
        // entries BORROW them). Pass 2 borrows the stable vectors.
        let entries = make_slice(blas_entries, blas_entry_count);
        let triangle_sizes: Vec<Option<Vec<wgt::BlasTriangleGeometrySizeDescriptor>>> = entries
            .iter()
            .map(|entry| {
                (entry.type_ == 0).then(|| {
                    make_slice(entry.triangleGeometries, entry.triangleGeometryCount)
                        .iter()
                        .map(|size| wgt::BlasTriangleGeometrySizeDescriptor {
                            vertex_format: conv::map_vertex_format(size.size.vertexFormat)
                                .expect("invalid vertex format for BLAS geometry"),
                            vertex_count: size.size.vertexCount,
                            index_format: match size.size.indexFormat {
                                native::WGPUIndexFormat_Undefined => None,
                                format => Some(
                                    conv::map_index_format(format)
                                        .expect("invalid index format for BLAS geometry"),
                                ),
                            },
                            index_count: (size.size.indexCount != u32::MAX)
                                .then_some(size.size.indexCount),
                            flags: map_geometry_flags(size.size.flags),
                        })
                        .collect()
                })
            })
            .collect();
        let aabb_sizes: Vec<Option<Vec<wgt::BlasAABBGeometrySizeDescriptor>>> = entries
            .iter()
            .map(|entry| {
                (entry.type_ == 1).then(|| {
                    make_slice(entry.aabbGeometries, entry.aabbGeometryCount)
                        .iter()
                        .map(|size| wgt::BlasAABBGeometrySizeDescriptor {
                            primitive_count: size.size.primitiveCount,
                            flags: map_geometry_flags(size.size.flags),
                        })
                        .collect()
                })
            })
            .collect();

        let mut blas: Vec<wgc::ray_tracing::BlasBuildEntry> = Vec::with_capacity(blas_entry_count);
        for (index, entry) in entries.iter().enumerate() {
            let blas_handle = entry.blas.as_ref().expect("invalid blas handle");
            match entry.type_ {
                0 => {
                    let sizes = triangle_sizes[index]
                        .as_ref()
                        .expect("triangle sizes materialized in pass 1");
                    let geometries: Vec<wgc::ray_tracing::BlasTriangleGeometry> = make_slice(
                        entry.triangleGeometries,
                        entry.triangleGeometryCount,
                    )
                    .iter()
                    .zip(sizes.iter())
                    .map(|(geometry, size)| wgc::ray_tracing::BlasTriangleGeometry {
                        size,
                        vertex_buffer: geometry
                            .vertexBuffer
                            .as_ref()
                            .expect("invalid vertex buffer for BLAS geometry")
                            .id,
                        index_buffer: geometry.indexBuffer.as_ref().map(|buffer| buffer.id),
                        transform_buffer: geometry.transformBuffer.as_ref().map(|b| b.id),
                        first_vertex: geometry.firstVertex,
                        vertex_stride: geometry.vertexStride,
                        first_index: (geometry.firstIndex != u32::MAX)
                            .then_some(geometry.firstIndex),
                        transform_buffer_offset: (geometry.transformBufferOffset != u64::MAX)
                            .then_some(geometry.transformBufferOffset),
                    })
                    .collect();
                    blas.push(wgc::ray_tracing::BlasBuildEntry {
                        blas_id: blas_handle.id,
                        geometries: wgc::ray_tracing::BlasGeometries::TriangleGeometries(
                            Box::new(geometries.into_iter()),
                        ),
                    });
                }
                1 => {
                    let sizes = aabb_sizes[index]
                        .as_ref()
                        .expect("AABB sizes materialized in pass 1");
                    let geometries: Vec<wgc::ray_tracing::BlasAabbGeometry> = make_slice(
                        entry.aabbGeometries,
                        entry.aabbGeometryCount,
                    )
                    .iter()
                    .zip(sizes.iter())
                    .map(|(geometry, size)| wgc::ray_tracing::BlasAabbGeometry {
                        size,
                        stride: geometry.stride,
                        aabb_buffer: geometry
                            .aabbBuffer
                            .as_ref()
                            .expect("invalid AABB buffer for BLAS geometry")
                            .id,
                        primitive_offset: geometry.primitiveOffset,
                    })
                    .collect();
                    blas.push(wgc::ray_tracing::BlasBuildEntry {
                        blas_id: blas_handle.id,
                        geometries: wgc::ray_tracing::BlasGeometries::AabbGeometries(
                            Box::new(geometries.into_iter()),
                        ),
                    });
                }
                _ => panic!("invalid BLAS geometry type (0 = triangles, 1 = AABBs)"),
            }
        }

        let tlas: Vec<wgc::ray_tracing::TlasPackage> = make_slice(tlas_packages, tlas_package_count)
            .iter()
            .map(|package| {
                let tlas_handle = package.tlas.as_ref().expect("invalid tlas handle");
                let instances: Vec<Option<wgc::ray_tracing::TlasInstance>> =
                    make_slice(package.instances, package.instanceCount)
                        .iter()
                        .map(|instance| {
                            instance.blas.as_ref().map(|blas_handle| {
                                wgc::ray_tracing::TlasInstance {
                                    blas_id: blas_handle.id,
                                    transform: &instance.transform,
                                    custom_data: instance.customData,
                                    mask: instance.mask,
                                }
                            })
                        })
                        .collect();
                wgc::ray_tracing::TlasPackage {
                    tlas_id: tlas_handle.id,
                    instances: Box::new(instances.into_iter()),
                    lowest_unmodified: package.lowestUnmodified,
                }
            })
            .collect();

        if let Err(cause) = encoder.context.command_encoder_build_acceleration_structures(
            encoder.id,
            blas.into_iter(),
            tlas.into_iter(),
        ) {
            handle_error(
                &encoder.error_sink,
                cause,
                None,
                "wgpuCommandEncoderBuildAccelerationStructures",
            );
        }
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuCommandEncoderMarkAccelerationStructuresBuilt(
    command_encoder: native::WGPUCommandEncoder,
    blas: *const native::WGPUBlas,
    blas_count: usize,
    tlas: *const native::WGPUTlas,
    tlas_count: usize,
) {
    crate::ffi_utils::contain_entry(
        "wgpuCommandEncoderMarkAccelerationStructuresBuilt",
        (),
        || {
        let encoder = command_encoder.as_ref().expect("invalid command encoder");

        let blas_ids: Vec<wgc::id::BlasId> = make_slice(blas, blas_count)
            .iter()
            .map(|blas| blas.as_ref().expect("invalid blas handle").id)
            .collect();
        let tlas_ids: Vec<wgc::id::TlasId> = make_slice(tlas, tlas_count)
            .iter()
            .map(|tlas| tlas.as_ref().expect("invalid tlas handle").id)
            .collect();

        if let Err(cause) = encoder.context.command_encoder_mark_acceleration_structures_built(
            encoder.id,
            &blas_ids,
            &tlas_ids,
        ) {
            handle_error(
                &encoder.error_sink,
                cause,
                None,
                "wgpuCommandEncoderMarkAccelerationStructuresBuilt",
            );
        }
        },
    )
}

// ---------------------------------------------------------------------------
// External textures (native video path).
// ---------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn wgpuDeviceCreateExternalTexture(
    device: native::WGPUDevice,
    descriptor: Option<&native::WGPUExternalTextureDescriptor>,
    planes: *const native::WGPUTextureView,
    plane_count: usize,
) -> native::WGPUExternalTexture {
    crate::ffi_utils::contain_entry(
        "wgpuDeviceCreateExternalTexture",
        std::ptr::null(),
        || {
        let device = device.as_ref().expect("invalid device");
        let descriptor = descriptor.expect("invalid descriptor");

        fn map_format(format: native::WGPUNativeExternalTextureFormat) -> wgt::ExternalTextureFormat {
            match format {
                native::WGPUNativeExternalTextureFormat_Rgba => wgt::ExternalTextureFormat::Rgba,
                native::WGPUNativeExternalTextureFormat_Nv12 => wgt::ExternalTextureFormat::Nv12,
                native::WGPUNativeExternalTextureFormat_Yu12 => wgt::ExternalTextureFormat::Yu12,
                _ => panic!("invalid external texture format"),
            }
        }

        let desc = wgt::ExternalTextureDescriptor {
            label: string_view_into_label(descriptor.label),
            width: descriptor.width,
            height: descriptor.height,
            format: map_format(descriptor.format),
            yuv_conversion_matrix: descriptor.yuvConversionMatrix,
            gamut_conversion_matrix: descriptor.gamutConversionMatrix,
            src_transfer_function: wgt::ExternalTextureTransferFunction {
                a: descriptor.srcTransferFunction.a,
                b: descriptor.srcTransferFunction.b,
                g: descriptor.srcTransferFunction.g,
                k: descriptor.srcTransferFunction.k,
            },
            dst_transfer_function: wgt::ExternalTextureTransferFunction {
                a: descriptor.dstTransferFunction.a,
                b: descriptor.dstTransferFunction.b,
                g: descriptor.dstTransferFunction.g,
                k: descriptor.dstTransferFunction.k,
            },
            sample_transform: descriptor.sampleTransform,
            load_transform: descriptor.loadTransform,
        };

        let plane_ids: Vec<wgc::id::TextureViewId> = make_slice(planes, plane_count)
            .iter()
            .map(|plane| {
                plane
                    .as_ref()
                    .expect("invalid texture view for external texture plane")
                    .id
            })
            .collect();

        let (external_texture_id, error) =
            device
                .context
                .device_create_external_texture(device.id, &desc, &plane_ids, None);
        if let Some(cause) = error {
            handle_error(
                &device.error_sink,
                cause,
                desc.label,
                "wgpuDeviceCreateExternalTexture",
            );
            return std::ptr::null();
        }
        Arc::into_raw(Arc::new(crate::objects::WGPUExternalTextureImpl {
            context: device.context.clone(),
            id: external_texture_id,
        }))
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn wgpuExternalTextureDestroy(
    external_texture: native::WGPUExternalTexture,
) {
    crate::ffi_utils::contain_entry("wgpuExternalTextureDestroy", (), || {
        let external_texture = external_texture
            .as_ref()
            .expect("invalid external texture");
        external_texture
            .context
            .external_texture_destroy(external_texture.id);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuExternalTextureAddRef(
    external_texture: native::WGPUExternalTexture,
) {
    crate::ffi_utils::contain_entry("wgpuExternalTextureAddRef", (), || {
        assert!(
            !external_texture.is_null(),
            "invalid external texture"
        );
        Arc::increment_strong_count(external_texture);
    })
}

#[no_mangle]
pub unsafe extern "C" fn wgpuExternalTextureRelease(
    external_texture: native::WGPUExternalTexture,
) {
    crate::ffi_utils::contain_entry("wgpuExternalTextureRelease", (), || {
        assert!(
            !external_texture.is_null(),
            "invalid external texture"
        );
        Arc::decrement_strong_count(external_texture);
    })
}
