/* TU minimale (spec §4.5) : les headers vendored compilent seuls, sans patch. */
#include "webgpu.h"
#include "wgpu.h"

int main(void) {
    (void)wgpuCreateInstance;      /* les déclarations sont liables */
    (void)wgpuDebugTriggerPanic;
    return 0;
}
