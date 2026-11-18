# Consuming wgpu-packer from Kotlin/Native

## 0. Where to find the archives

Each tag `v<WGPU_VERSION>.<PACKER_VERSION>` publishes a GitHub release at
`https://github.com/Graphiks-org/wgpu-packer/releases`. Each release contains:

- `wgpu-<version>-<kotlinTarget>-<static|shared>.tar.gz` — the archive;
- `SHA256SUMS` — the checksums of all the archives in the release;
- `index.json` — the list of the `manifest.json` of all the archives.

Verify the checksum before unpacking:

```bash
# GNU coreutils (Linux)
sha256sum -c SHA256SUMS --ignore-missing
# macOS
shasum -a 256 -c SHA256SUMS
```

The archive's `manifest.json` carries `wgpu.crates` and `wgpu.headersRevision`:
the file name only contains the crates version, not the headers version.

A published release is immutable. For a fix, a new release `v30.0.1.1` is
published; the old one remains downloadable.

The publishing workflow never overwrites an existing file: it therefore
cannot repair a release whose upload was interrupted. If a release is
incomplete (missing archive, missing `SHA256SUMS` or `index.json`), delete
the release and push the tag again, or publish a new `PACKER_VERSION`.

## 1. Unpacking the archive

```bash
tar xzf wgpu-<version>-<target>-static.tar.gz
# → include/ (webgpu.h, wgpu.h) · lib/ (libwgpu_packer.a, link.txt) · LICENSE* · manifest.json
```

## 2. cinterop

Copy `packaging/wgpu.def`, point `libraryPaths` at the archive's `lib/`.
The symbols land in the `wgpu` package (e.g. `wgpu.wgpuCreateInstance`).
The header to declare is `wgpu.h` — it includes the standard and the
`wgpu.h` is the single header to include: it pulls in `webgpu.h` and
declares everything the packer adds.

## 3. Linking

The exact flags per target are in the archive's `lib/link.txt` — copy them
into `linkerOpts` (Gradle) or the link config.
macOS: `-framework Metal -framework QuartzCore -framework CoreVideo
-framework IOKit -framework Foundation` · Linux: `-ldl -lpthread -lm` ·
Android: `-llog -landroid` · Windows MSVC: `user32 gdi32 dxgi d3d12 dxguid` · Windows mingw (shared): `ws2_32 userenv ntdll ole32 oleaut32 uuid bcrypt dbghelp` — copy the exact flags from the archive's `lib/link.txt`.

## 4. Memory contract (mandatory)

- Every object obtained (Instance, Adapter, Device, Buffer…) must be freed
  by its `wgpuXxxRelease` function — strict 1:1 pairing; `wgpuXxxAddRef`
  if you copy the handle.
- `wgpuInstanceEnumerateAdapters`: first call with `adapters == NULL` →
  count; second call with an array of that size → fill.
  **Every returned adapter must be Release'd.**
- `wgpuBufferGetMappedRange` is only valid between the delivery of the
  `map_async` and `wgpuBufferUnmap`; after `unmap` the function returns NULL
  (never a crash).
- `MAP_WRITE` combines with `COPY_SRC`, `MAP_READ` with `COPY_DST` (WebGPU
  rule "MAP + opposite COPY") — any other combination is a validation
  error.

## 5. Threading and callback contract

- Callbacks go through `WGPU*CallbackInfo` structs
  (`callback` + `userdata1`/`userdata2`). Put the function pointer in
  `.callback`, the routing pointer (consumed by the packer's trampolines,
  see `routingUserdataField: userdata2`) in `.userdata2`, and the Kotlin
  `StableRef` — the application payload — in `.userdata1`.
- **The `.mode` field is mandatory**: `AllowSpontaneous` (delivery as soon
  as the operation completes — before the return for a synchronously
  resolved operation), `AllowProcessEvents` (delivered on the instance's
  next event processing) or `WaitAnyOnly` (delivered only by a wait
  covering the future). The `INIT` macro sets 0: accepted as `WaitAnyOnly`
  with a warning — set an explicit mode when you want another behaviour.
- `wgpuAdapterRequestDevice` and async pipeline creations are
  **synchronous** internally: with `AllowSpontaneous`, the callback fires
  before the call returns.
- `wgpuBufferMapAsync` is resolved at **poll** (device poll, or the
  internal poll of an event processing / a wait): poll BEFORE any
  `Release` of the source object; free the `StableRef` (`dispose()`) in the
  callback itself, exactly once.
- Every async operation returns a **real future** (id ≠ 0).
  `wgpuInstanceWaitAny`: `timeoutNS > 0` requires the `TimedWaitAny`
  feature requested at instance creation (reported limitation: none) and
  futures from a single source (at most one queue — "Mixed Sources");
  `timeoutNS == 0` = single non-blocking poll; (0 futures, timeout 0) =
  equivalent of ProcessEvents. Unknown, already delivered, null or
  duplicate futures → `WGPUWaitStatus_Error`. Callbacks not delivered at
  instance destruction are silently canceled.
- **No entry point aborts on a caught panic** (D2 of the independent
  review): every `extern "C"` boundary contains its panics — a null
  handle, an invalid enum value or an internal invariant failure logs an
  error and returns the type's error value (`NULL` handle, error status,
  0, `false`, the null future). Use-after-release of a handle is memory
  UB and stays outside this guarantee: respect the 1:1 `Release` contract
  (§4). Dangling-pointer reads are not catchable panics.
- Uncaptured GPU errors are **never abort**: with no handler installed,
  the packer's default handler **logs and ignores** (callbacks.rs: "log
  without panicking"), and validation errors from `wgpuQueueSubmit` as well
  as from a render bundle's `finish` are delivered to the device sink
  (current error scope or uncaptured error handler).
  Set `uncapturedErrorCallbackInfo.callback` in the `WGPUDeviceDescriptor`
  from the very first device to **consume** errors instead of leaving them
  to the default log.
- **Device loss** is not a programming error (GPU reset, Android thermal,
  Windows TDR): also set `deviceLostCallbackInfo.callback` on every device
  to be notified and be able to rebuild your resources. Loss is detected
  when it surfaces (poll or error), with no dedicated thread; the loss
  future (`wgpuDeviceGetLostFuture`, stable id per device) follows this
  callback's mode.

## 6. Unimplemented functions (call = contained error)

These symbols are exported but raise `unimplemented!()` when called in
v30.0.1.0 — do not call them from Kotlin. The panic is contained at the
ABI boundary (§5): the call logs an error and returns the type's error
value (`NULL`, `WGPUStatus_Error`, 0), never an abort. 5 stubs; the
reasons are detailed in `ffi/src/unimplemented.rs`:

wgpuInstanceGetWGSLLanguageFeatures, wgpuInstanceHasWGSLLanguageFeature,
wgpuSupportedWGSLLanguageFeaturesFreeMembers,
wgpuExternalTextureAddRef, wgpuExternalTextureRelease.

### Native chains added by the parity work (phase 1)

All of them chain through `nextInChain` and live only in `wgpu.h`:

- `WGPURenderPipelineDescriptorExtras` / `WGPURenderPassDescriptorExtras`
  (`multiviewMask`, 0 = all views) and
  `WGPURenderBundleEncoderDescriptorExtras` (`multiview`): multiview /
  selective-multiview rendering.
- `WGPUSurfaceColorManagementExtras` (`WGPUNativeSurfaceColorSpace`): the
  FULL color space set — including PQ (`Bt2100Pq`) and HLG (`Bt2100Hlg`)
  HDR presentation — taking precedence over the standard
  `WGPUSurfaceColorManagement` chain, which only covers SRGB/DisplayP3.
- `WGPUShaderRuntimeChecksDescriptor` chained on the standard
  shader-module creation path, plus the
  `WGPUShaderRuntimeChecks_IntDivChecks` native bit (0x20): runtime
  checks without the `Trusted` entry point.
- `WGPUExperimentalFeaturesExtras`: the UNSAFE opt-in to wgpu's
  `EXPERIMENTAL_*` features (mesh shaders, ray tracing, cooperative
  matrix). Enabling it acknowledges those APIs may contain
  UB-producing bugs — the same contract as wgpu-types'
  `ExperimentalFeatures::enabled()`.
- `WGPUInstanceFlag_StrictWebGPUCompliance` (1 << 7), and
  `WGPUInstanceFlag_WithEnv` now applies to the WHOLE instance
  descriptor (backends, power preference, DX12 options) through
  wgpu-types' `with_env`.
- `WGPUNativePresentMode_AutoVsync` / `_AutoNoVsync`: browser-style
  automatic present modes, accepted in `WGPUSurfaceConfiguration`.
- `WGPUDeviceExtras.tracePath` is now live (api trace capture): it costs
  ~2.6 MB of trace serialization in the static library.

Documented limitations (no hook in wgpu-core/wgpu-types 30.0.1):

- `WGPUTextureComponentSwizzleDescriptor` (standard chain) stays
  unread: wgpu-types has no component-swizzle field on texture views.
- `WGPURequestAdapterOptions::featureLevel` is read and checked
  (`Force32` logs a warning) but wgpu-types has no feature-level field:
  Core and Compatibility are served by the same adapter query.
- `WGPUQueueDescriptor.defaultQueue.label` is ignored: wgpu-core offers
  no hook to label the implicit queue at device creation.
- Compilation info positions non-parsing (type-check / validation)
  shader errors at line 0: naga reports those without source spans.

Note:

- The `*SetLabel` functions are accepted and ignored: wgpu-core 30 exposes
  no post-creation label change (see `ffi/src/api/labels.rs`). Error message
  labels remain those from creation.
- Async pipeline creation is synchronous internally (§5); validation errors
  only go through the callback.
- The texture binding view dimension is resolved on the packer side
  (explicit chained value or derivation rule) — wgpu-core 30 does not
  expose it (gfx-rs/wgpu#7428).

Any other function declared in webgpu.h/wgpu.h is implemented — including
procedure resolution by name, buffer mapping state, instance features,
device-side adapter info and instance futures/waits (§5).

## 7. Versioning

`<wgpu-major>.<minor>.<patch>.<packer>` — e.g. `30.0.1.0` = wgpu 30.0.1,
packer revision 0. Enums/flags are reserved for ray tracing, video and
advanced queries (non-breaking additions, spec §5).
