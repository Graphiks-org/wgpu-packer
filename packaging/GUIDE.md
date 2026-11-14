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

The archive's `manifest.json` carries `wgpu.crates` and `wgpu.nativeTag`:
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
# → include/ (webgpu.h, wgpu_upstream.h, wgpu.h) · lib/ (libwgpu_packer.a, link.txt) · LICENSE* · manifest.json
```

## 2. cinterop

Copy `packaging/wgpu.def`, point `libraryPaths` at the archive's `lib/`.
The symbols land in the `wgpu` package (e.g. `wgpu.wgpuCreateInstance`).
The header to declare is `wgpu.h` — it includes the standard and the
vendored extensions; never include `wgpu_upstream.h` directly.

## 3. Linking

The exact flags per target are in the archive's `lib/link.txt` — copy them
into `linkerOpts` (Gradle) or the link config.
macOS: `-framework Metal -framework QuartzCore -framework CoreVideo
-framework IOKit -framework Foundation` · Linux: `-ldl -lpthread -lm` ·
Android: `-llog -landroid` · Windows: `user32 gdi32 dxgi d3d12`.

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
  covering the future). The `INIT` macro sets 0, which is **invalid**: the
  call panics.
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

## 6. Unimplemented functions (call = abort)

These symbols are exported but raise `unimplemented!()` (panic) when called
in v30.0.1.0 — do not call them from Kotlin. 6 stubs; the reasons are
detailed in `ffi/src/unimplemented.rs`:

wgpuInstanceGetWGSLLanguageFeatures, wgpuInstanceHasWGSLLanguageFeature,
wgpuSupportedWGSLLanguageFeaturesFreeMembers, wgpuShaderModuleGetCompilationInfo,
wgpuExternalTextureAddRef, wgpuExternalTextureRelease.

Note:

- The `*SetLabel` functions are accepted and ignored: wgpu-core 29 exposes
  no post-creation label change (see `ffi/src/api/labels.rs`). Error message
  labels remain those from creation.
- Async pipeline creation is synchronous internally (§5); validation errors
  only go through the callback.
- The texture binding view dimension is resolved on the packer side
  (explicit chained value or derivation rule) — wgpu-core 29 does not
  expose it (gfx-rs/wgpu#7428).

Any other function declared in webgpu.h/wgpu.h is implemented — including
procedure resolution by name, buffer mapping state, instance features,
device-side adapter info and instance futures/waits (§5).

## 7. Versioning

`<wgpu-major>.<minor>.<patch>.<packer>` — e.g. `30.0.1.0` = wgpu 30.0.1,
packer revision 0. Enums/flags are reserved for ray tracing, video and
advanced queries (non-breaking additions, spec §5).
