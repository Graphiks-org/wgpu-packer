/* Minimal TU (spec §4.5): the vendored headers compile on their own, without patches. */
#include "webgpu.h"
#include "wgpu.h"

/* No `main` here: this TU is linked together with link_main.c, which owns
 * the entry point. It only proves the declarations are usable. */
void wgpu_headers_compile_probe(void) {
    (void)wgpuCreateInstance;      /* the declarations are linkable */
    (void)wgpuDebugTriggerPanic;
}
