/* Static link + smoke test (Review Focus #5): linked by
 * build-target.sh on the host target with the matrix's exact link_libs. */
#include "webgpu.h"
#include "wgpu.h"
#include <stdio.h>
#include <string.h>

static WGPUStringView wgpu_string(const char *s) {
    WGPUStringView sv;
    sv.data = s;
    sv.length = strlen(s);
    return sv;
}

int main(void) {
    WGPUInstance instance = wgpuCreateInstance(NULL);
    if (instance == NULL) {
        fprintf(stderr, "LINK_TEST: instance NULL\n");
        return 1;
    }
    size_t count = wgpuInstanceEnumerateAdapters(instance, NULL, NULL);

    // GetProcAddress probe: resolution by name (case sensitivity of the
    // symbol, useful for dlopen consumers).
    WGPUProc create_proc = wgpuGetProcAddress(wgpu_string("wgpuCreateInstance"));
    if (create_proc == NULL) {
        fprintf(stderr, "LINK_TEST: wgpuGetProcAddress does not resolve wgpuCreateInstance\n");
        return 1;
    }

    printf("LINK_TEST_OK adapters=%zu\n", count);
    wgpuInstanceRelease(instance);
    return 0;
}
