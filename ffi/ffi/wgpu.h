/* wgpu.h — header d'extensions wgpu-packer.
 * Le standard vit dans webgpu.h (vendored), les additions wgpu-native dans
 * wgpu_upstream.h (vendored). Nos extensions propres sont ajoutées en fin de
 * fichier (Task 2). */
#include "webgpu.h"
#include "wgpu_upstream.h"

/* ==== Extensions wgpu-packer ==== */
#ifdef __cplusplus
extern "C" {
#endif

/** Debug uniquement : panique contenue par le wrapper FFI. 1 = contenue, 0 = pas de panique. */
WGPU_EXPORT uint32_t wgpuDebugTriggerPanic(void);

#ifdef __cplusplus
}
#endif
