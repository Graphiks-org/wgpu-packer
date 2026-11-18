use parking_lot::Mutex;
use std::{
    sync::{atomic, Arc},
    thread,
};
use wgc::{
    command::{ComputePass, RenderPass},
    id,
};

use crate::error::*;
use crate::native;

pub type Context = wgc::global::Global;

pub struct WGPUAdapterImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::AdapterId,
    /// The instance this adapter was enumerated from (wgpuAdapterGetInstance).
    /// Held as a strong reference so the instance outlives the adapter.
    pub(crate) instance: Arc<WGPUInstanceImpl>,
}
impl Drop for WGPUAdapterImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.adapter_drop(self.id);
        }
    }
}

pub struct WGPUBindGroupImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::BindGroupId,
}
impl Drop for WGPUBindGroupImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.bind_group_drop(self.id);
        }
    }
}

pub struct WGPUBindGroupLayoutImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::BindGroupLayoutId,
}
impl Drop for WGPUBindGroupLayoutImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.bind_group_layout_drop(self.id);
        }
    }
}

pub(crate) struct BufferData {
    pub(crate) usage: native::WGPUBufferUsage,
    pub(crate) size: u64,
    /// Queue of the creating device: timeline of the async map future
    /// ("Mixed Sources" rule of timed waits).
    pub(crate) queue: id::QueueId,
    /// Mapping state tracked packer-side for wgpuBufferGetMapState: wgpu-core
    /// keeps its own in a pub(crate) field not accessible here. Shared by
    /// Arc with the map callback (cf. wgpuBufferMapAsync), updated in
    /// MapAsync / Unmap / Destroy.
    pub(crate) map_state: Arc<Mutex<native::WGPUBufferMapState>>,
}
pub struct WGPUBufferImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::BufferId,
    pub(crate) error_sink: ErrorSink,
    pub(crate) data: BufferData,
}
impl Drop for WGPUBufferImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.buffer_drop(self.id);
        }
    }
}

pub struct WGPUCommandBufferImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::CommandBufferId,
    pub(crate) open: atomic::AtomicBool,
}
impl Drop for WGPUCommandBufferImpl {
    fn drop(&mut self) {
        if self.open.load(atomic::Ordering::SeqCst) && !thread::panicking() {
            let context = &self.context;
            context.command_buffer_drop(self.id);
        }
    }
}

pub struct WGPUCommandEncoderImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::CommandEncoderId,
    pub(crate) error_sink: ErrorSink,
    pub(crate) open: atomic::AtomicBool,
}
impl Drop for WGPUCommandEncoderImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.command_encoder_drop(self.id);
        }
    }
}

pub struct WGPUComputePassEncoderImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) encoder: *mut ComputePass,
    pub(crate) error_sink: ErrorSink,
}
impl Drop for WGPUComputePassEncoderImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            drop(unsafe { Box::from_raw(self.encoder) });
        }
    }
}
// ComputePassEncoder is thread-unsafe
unsafe impl Send for WGPUComputePassEncoderImpl {}
unsafe impl Sync for WGPUComputePassEncoderImpl {}

pub struct WGPUComputePipelineImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::ComputePipelineId,
    pub(crate) error_sink: ErrorSink,
}
impl Drop for WGPUComputePipelineImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.compute_pipeline_drop(self.id);
        }
    }
}

pub(crate) struct QueueId {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::QueueId,
}
impl Drop for QueueId {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.queue_drop(self.id);
        }
    }
}

pub struct WGPUDeviceImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::DeviceId,
    /// The adapter this device was requested from (wgpuDeviceGetAdapter).
    /// Held as a strong reference so the adapter outlives the device.
    pub(crate) adapter: Arc<WGPUAdapterImpl>,
    pub(crate) queue: Arc<QueueId>,
    pub(crate) error_sink: ErrorSink,
    /// Loss future (wgpuDeviceGetLostFuture): created lazily on the
    /// first call, stable id afterwards, completed when the loss is reported
    /// via the error sink.
    pub(crate) lost_future: Mutex<Option<u64>>,
    /// Loss already reported: a late GetLostFuture completes immediately.
    pub(crate) device_lost: atomic::AtomicBool,
}
impl Drop for WGPUDeviceImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;

            // pre v27 this would wait for 60 seconds instead of waiting indefinitely
            match context.device_poll(self.id, wgt::PollType::wait_indefinitely()) {
                Ok(_) => (),
                // No more abort at destruction: the final drain error
                // (device loss) is delivered to the sink. The sink's weak ref can no
                // longer upgrade (strong count already 0 during drop):
                // no callback fired on a dying handle.
                Err(err) => handle_error(&self.error_sink, err, None, "WGPUDeviceImpl::drop"),
            }

            context.device_drop(self.id);
        }
    }
}

pub struct WGPUInstanceImpl {
    pub(crate) context: Arc<Context>,
    /// TimedWaitAny feature requested at creation: allows
    /// `wgpuInstanceWaitAny` with `timeoutNS > 0` on this instance.
    pub(crate) timed_wait_any: bool,
}
impl Drop for WGPUInstanceImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            // Silent cancellation of the instance's undelivered callbacks.
            crate::future::purge(&self.context);
        }
    }
}

pub struct WGPUPipelineLayoutImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::PipelineLayoutId,
}
impl Drop for WGPUPipelineLayoutImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.pipeline_layout_drop(self.id);
        }
    }
}

pub(crate) struct QuerySetData {
    pub(crate) query_type: native::WGPUQueryType,
    pub(crate) query_count: u32,
}
pub struct WGPUQuerySetImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::QuerySetId,
    pub(crate) data: QuerySetData,
}
impl Drop for WGPUQuerySetImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.query_set_drop(self.id);
        }
    }
}

pub struct WGPUQueueImpl {
    pub(crate) queue: Arc<QueueId>,
    pub(crate) error_sink: ErrorSink,
}

pub struct WGPURenderBundleImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::RenderBundleId,
}
impl Drop for WGPURenderBundleImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.render_bundle_drop(self.id);
        }
    }
}

pub struct WGPURenderBundleEncoderImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) encoder: *mut Option<*mut wgc::command::RenderBundleEncoder>,
    /// Device sink captured at creation: content validation of the
    /// bundle is a record-time error delivered at finish (same routing
    /// as the pass encoders) — never fatal.
    pub(crate) error_sink: ErrorSink,
}
impl Drop for WGPURenderBundleEncoderImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let encoder = unsafe { Box::from_raw(self.encoder) };
            if let Some(encoder) = *encoder {
                drop(unsafe { Box::from_raw(encoder) });
            }
        }
    }
}
// RenderBundleEncoder is thread-unsafe
unsafe impl Send for WGPURenderBundleEncoderImpl {}
unsafe impl Sync for WGPURenderBundleEncoderImpl {}

pub struct WGPURenderPassEncoderImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) encoder: *mut RenderPass,
    pub(crate) error_sink: ErrorSink,
    /// `maxDrawCount` (`WGPURenderPassMaxDrawCount` chain): draw call cap
    /// of the pass, counted packer-side — wgpu-core 30 no longer has this field
    /// (the WebGPU spec removed it), the C contract carries it. Default
    /// `WGPU_RENDER_PASS_MAX_DRAW_COUNT_INIT` (50 000 000) when the chain
    /// is absent; 0 (zeroed descriptor) counts as this default.
    pub(crate) max_draw_count: u64,
    /// Number of draw calls already encoded in this pass.
    pub(crate) draw_count: atomic::AtomicU64,
}
impl Drop for WGPURenderPassEncoderImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            drop(unsafe { Box::from_raw(self.encoder) });
        }
    }
}
// RenderPassEncodee is thread-unsafe
unsafe impl Send for WGPURenderPassEncoderImpl {}
unsafe impl Sync for WGPURenderPassEncoderImpl {}

pub struct WGPURenderPipelineImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::RenderPipelineId,
    pub(crate) error_sink: ErrorSink,
}
impl Drop for WGPURenderPipelineImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.render_pipeline_drop(self.id);
        }
    }
}

pub struct WGPUSamplerImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::SamplerId,
}
impl Drop for WGPUSamplerImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.sampler_drop(self.id);
        }
    }
}

pub struct WGPUShaderModuleImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: Option<id::ShaderModuleId>,
    /// Diagnostics produced when the module was created, kept so
    /// `wgpuShaderModuleGetCompilationInfo` can report them. Empty for a
    /// module that compiled cleanly.
    pub(crate) messages: Vec<CompilationMessage>,
}

/// One diagnostic of a shader module, in the shape webgpu.h expects.
pub struct CompilationMessage {
    pub message: String,
    pub message_type: native::WGPUCompilationMessageType,
    pub line_num: u64,
    pub line_pos: u64,
    pub offset: u64,
    pub length: u64,
}
impl Drop for WGPUShaderModuleImpl {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            if !thread::panicking() {
                let context = &self.context;
                context.shader_module_drop(id);
            }
        }
    }
}

pub(crate) struct SurfaceData {
    pub(crate) error_sink: ErrorSink,
    pub(crate) texture_data: TextureData,
}

pub struct WGPUSurfaceImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::SurfaceId,
    pub(crate) data: Mutex<Option<SurfaceData>>,
    // Shared bool between Texture & Surface to track surface_present calls
    pub(crate) has_surface_presented: Arc<atomic::AtomicBool>,
}
impl Drop for WGPUSurfaceImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            self.context.surface_drop(self.id);
        }
    }
}

#[derive(Copy, Clone)]
pub(crate) struct TextureData {
    pub(crate) usage: native::WGPUTextureUsage,
    pub(crate) dimension: native::WGPUTextureDimension,
    pub(crate) size: native::WGPUExtent3D,
    pub(crate) format: native::WGPUTextureFormat,
    pub(crate) mip_level_count: u32,
    pub(crate) sample_count: u32,
    /// Resolved dimension of the binding views (chained webgpu.h
    /// WGPUTextureBindingViewDimension): wgc 30 does not have this concept
    /// (gfx-rs/wgpu#7428), the resolution is done packer-side at creation.
    pub(crate) texture_binding_view_dimension: native::WGPUTextureViewDimension,
}

pub struct WGPUBlasImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::BlasId,
}

impl Drop for WGPUBlasImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            self.context.blas_drop(self.id);
        }
    }
}

pub struct WGPUTlasImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::TlasId,
}

impl Drop for WGPUTlasImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            self.context.tlas_drop(self.id);
        }
    }
}

pub struct WGPUPipelineCacheImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::PipelineCacheId,
}

pub struct WGPUTextureImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::TextureId,
    pub(crate) error_sink: ErrorSink,
    pub(crate) data: TextureData,
    pub(crate) surface_id: Option<id::SurfaceId>,
    // Shared bool between Texture & Surface to track surface_present calls
    pub(crate) has_surface_presented: Arc<atomic::AtomicBool>,
}
impl Drop for WGPUTextureImpl {
    fn drop(&mut self) {
        if thread::panicking() {
            return;
        }
        if let Some(surface_id) = self.surface_id {
            if !self.has_surface_presented.load(atomic::Ordering::SeqCst) {
                match self.context.surface_texture_discard(surface_id) {
                    Ok(_) => (),
                    // Inside Drop: log is the only report — the release
                    // itself must complete, never abort.
                    Err(cause) => log::error!("wgpuTextureRelease: {cause}"),
                }
            }
        }
        self.context.texture_drop(self.id);
    }
}

pub struct WGPUTextureViewImpl {
    pub(crate) context: Arc<Context>,
    pub(crate) id: id::TextureViewId,
}
impl Drop for WGPUTextureViewImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;
            context.texture_view_drop(self.id);
        }
    }
}
