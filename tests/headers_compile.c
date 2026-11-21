/* Minimal TU (spec §4.5): the vendored headers compile on their own, without patches. */
#include "webgpu.h"
#include "wgpu.h"

/* No `main` here: this TU is linked together with link_main.c, which owns
 * the entry point. It only proves the declarations are usable. */
void wgpu_headers_compile_probe(void) {
    (void)wgpuCreateInstance;      /* the declarations are linkable */
    (void)wgpuDebugTriggerPanic;

    /* Regression (independent review): the standard's INIT macros must be
     * DEFINED, not merely referenced by other macros — WGPU_EXTENT_3D_INIT
     * and WGPU_ORIGIN_3D_INIT were dropped in the header rebuild. */
    WGPUExtent3D extent = WGPU_EXTENT_3D_INIT;
    WGPUOrigin3D origin = WGPU_ORIGIN_3D_INIT;
    WGPUTexelCopyTextureInfo copy_info = WGPU_TEXEL_COPY_TEXTURE_INFO_INIT;
    WGPUBufferDescriptor buffer_desc = WGPU_BUFFER_DESCRIPTOR_INIT;
    (void)extent; (void)origin; (void)copy_info; (void)buffer_desc;
}
