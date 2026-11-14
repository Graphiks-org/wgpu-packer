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
    /// Queue du device créateur : timeline de la future de map async
    /// (règle « Mixed Sources » des attentes temporisées).
    pub(crate) queue: id::QueueId,
    /// État de mapping suivi côté packer pour wgpuBufferGetMapState : wgpu-core
    /// garde le sien dans un champ pub(crate) non accessible ici. Partagé par
    /// Arc avec le callback de map (cf. wgpuBufferMapAsync), mis à jour dans
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
    pub(crate) queue: Arc<QueueId>,
    pub(crate) error_sink: ErrorSink,
    /// Future de perte (wgpuDeviceGetLostFuture) : créée paresseusement au
    /// premier appel, id stable ensuite, complétée quand la perte remonte
    /// via le sink d'erreurs.
    pub(crate) lost_future: Mutex<Option<u64>>,
    /// Perte déjà remontée : un GetLostFuture tardif complète immédiatement.
    pub(crate) device_lost: atomic::AtomicBool,
}
impl Drop for WGPUDeviceImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            let context = &self.context;

            // pre v27 this would wait for 60 seconds instead of waiting indefinitely
            match context.device_poll(self.id, wgt::PollType::wait_indefinitely()) {
                Ok(_) => (),
                // Plus d'abort à la destruction : l'erreur du drain final
                // (perte du device) est livrée au sink. Le weak du sink ne
                // peut plus upgrader (strong count déjà à 0 pendant drop) :
                // pas de callback tiré sur un handle mourant.
                Err(err) => handle_error(&self.error_sink, err, None, "WGPUDeviceImpl::drop"),
            }

            context.device_drop(self.id);
        }
    }
}

pub struct WGPUInstanceImpl {
    pub(crate) context: Arc<Context>,
    /// Feature TimedWaitAny demandée à la création : autorise
    /// `wgpuInstanceWaitAny` avec `timeoutNS > 0` sur cette instance.
    pub(crate) timed_wait_any: bool,
}
impl Drop for WGPUInstanceImpl {
    fn drop(&mut self) {
        if !thread::panicking() {
            // Annulation silencieuse des callbacks non livrés de l'instance.
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
    /// Sink du device capturé à la création : la validation de contenu du
    /// bundle est une erreur de record-time livrée au finish (même routage
    /// que les encodeurs de passe) — jamais fatale.
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
    /// `maxDrawCount` (chaîne `WGPURenderPassMaxDrawCount`): plafond de draw
    /// calls de la passe, comptés côté packer — wgpu-core 30 n'a plus ce champ
    /// (la spec WebGPU l'a retiré), le contrat C le porte. Défaut
    /// `WGPU_RENDER_PASS_MAX_DRAW_COUNT_INIT` (50 000 000) quand la chaîne
    /// est absente ; 0 (descripteur zéro) compte comme ce défaut.
    pub(crate) max_draw_count: u64,
    /// Nombre de draw calls déjà encodés dans cette passe.
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
    /// Dimension résolue des vues de binding (webgpu.h
    /// WGPUTextureBindingViewDimension chaîné) : wgc 29 n'a pas ce concept
    /// (gfx-rs/wgpu#7428), la résolution est faite côté packer à la création.
    pub(crate) texture_binding_view_dimension: native::WGPUTextureViewDimension,
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
                    Err(cause) => handle_error_fatal(cause, "wgpuTextureRelease"),
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
