/* wgpu.h - wgpu-packer extension header.
 * The standard lives in webgpu.h (vendored), the wgpu-native additions in
 * wgpu_upstream.h (vendored). Our own extensions are appended at the end of
 * the file. */
#include "webgpu.h"
#include "wgpu_upstream.h"

/* ==== wgpu-packer extensions ==== */
#ifdef __cplusplus
extern "C" {
#endif

/** Debug only: panic contained by the FFI wrapper. 1 = contained, 0 = no panic. */
WGPU_EXPORT uint32_t wgpuDebugTriggerPanic(void);

#ifdef __cplusplus
}
#endif
