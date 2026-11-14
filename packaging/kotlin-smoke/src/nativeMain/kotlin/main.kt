@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)

import kotlinx.cinterop.*
import wgpu.*

fun main() = memScoped {
    val instance = wgpuCreateInstance(null)
        ?: error("wgpuCreateInstance a retourné NULL")
    try {
        // Deux appels : NULL -> nombre, puis tableau -> remplissage (GUIDE §4).
        val n = wgpuInstanceEnumerateAdapters(instance, null, null).toInt()
        check(n >= 1) { "aucun adapter" }
        val adapters = allocArray<CPointerVarOf<WGPUAdapter>>(n)
        wgpuInstanceEnumerateAdapters(instance, null, adapters)
        repeat(n) { i -> wgpuAdapterRelease(adapters[i]) }
        println("KOTLIN_SMOKE_OK adapters=$n")
    } finally {
        wgpuInstanceRelease(instance)
    }
}
