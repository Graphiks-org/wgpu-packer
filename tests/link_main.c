/* Test de lien statique + fumée (Review Focus #5) : linké par
 * build-target.sh sur la cible hôte avec les link_libs exacts de la matrice. */
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

    // Sonde GetProcAddress : résolution par nom (sensibilité à la casse du
    // symbole, utile aux consommateurs dlopen).
    WGPUProc create_proc = wgpuGetProcAddress(wgpu_string("wgpuCreateInstance"));
    if (create_proc == NULL) {
        fprintf(stderr, "LINK_TEST: wgpuGetProcAddress ne résout pas wgpuCreateInstance\n");
        return 1;
    }

    printf("LINK_TEST_OK adapters=%zu\n", count);
    wgpuInstanceRelease(instance);
    return 0;
}
