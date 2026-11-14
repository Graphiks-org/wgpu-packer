/* Minimal TU (spec §4.5): the vendored headers compile on their own, without patches. */
#include "webgpu.h"
#include "wgpu.h"

int main(void) {
    (void)wgpuCreateInstance;      /* the declarations are linkable */
    (void)wgpuDebugTriggerPanic;
    return 0;
}
