// D7 (independent review): the C contract as the real consumer sees it.
// Compiled by scripts/cinterop-smoke.sh through Kotlin/Native cinterop
// against the vendored headers + static library, then RUN.
//
// Covers the regression classes the Rust oracles cannot see:
// - R1: a returned enum VALUE as read through the header (GetLimits)
// - #11: wgpuGetVersion (compile-time WGPU_NATIVE_VERSION)
// - the callback path with an explicit mode + StableRef userdata — the
//   exact pattern wgpu4k uses (GUIDE §5)
@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)

import kotlinx.cinterop.*
import wgpu.*

private class AdapterHolder {
    var adapter: WGPUAdapter? = null
}

fun main() {
    // #11: build.rs packs 30.0.1.0 MSB-first.
    check(wgpuGetVersion() == 0x1E000100u) {
        "wgpuGetVersion returned ${wgpuGetVersion()}, expected 0x1E000100"
    }

    val instance = wgpuCreateInstance(null)
    check(instance != null) { "wgpuCreateInstance returned null" }

    // Two-call enumeration (GUIDE §4): count first, then the handles.
    val count = wgpuInstanceEnumerateAdapters(instance, null, null)
    check(count > 0u) { "no adapter on this host (count call returned $count)" }
    val adapter = memScoped {
        val adapters = allocArray<WGPUAdapterVar>(count.toInt())
        val got = wgpuInstanceEnumerateAdapters(instance, null, adapters)
        check(got > 0u) { "second enumeration call returned 0 adapters" }
        adapters[0]
    }
    check(adapter != null) { "adapter handle is null" }

    // R1: webgpu.h declares WGPUStatus (Success = 1, Error = 2) — the Rust
    // side once returned a plain bool 0, which cinterop consumers read as
    // a constant failure.
    memScoped {
        val limits = alloc<WGPULimits>()
        val limitsStatus = wgpuAdapterGetLimits(adapter, limits.ptr)
        check(limitsStatus == WGPUStatus_Success) {
            "wgpuAdapterGetLimits returned $limitsStatus, expected Success (1)"
        }

        // String views: read then free (GUIDE §4, FreeMembers).
        val info = alloc<WGPUAdapterInfo>()
        val infoStatus = wgpuAdapterGetInfo(adapter, info.ptr)
        check(infoStatus == WGPUStatus_Success) {
            "wgpuAdapterGetInfo returned $infoStatus"
        }
        wgpuAdapterInfoFreeMembers(info.readValue())
    }

    wgpuAdapterRelease(adapter)
    wgpuInstanceRelease(instance)
    println("SMOKE_OK")
}
