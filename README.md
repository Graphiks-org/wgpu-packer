# wgpu-packer

Prebuilt [wgpu-native](https://github.com/gfx-rs/wgpu-native) libraries
published as GitHub Releases for non-web Kotlin/Native targets.

This repository only produces libraries (`webgpu.h` headers plus
`libwgpu_packer`). Any Kotlin/Native, JNI or Swift binding lives in the
consuming project.

## Versioning and republication

The archive name is `wgpu-<WGPU_VERSION>.<PACKER_VERSION>-<kotlinTarget>-<linkage>.tar.gz`,
following wgpu-native's own `X.Y.Z.N` scheme.

| What changed | Counter | Example |
|---|---|---|
| wgpu crates (`wgpu-core`, `wgpu-hal`, `naga`) | `WGPU_VERSION` + reset `PACKER_VERSION=0` | `30.0.1.0` → `30.0.2.0` |
| wgpu-native headers (ABI) | `WGPU_NATIVE_TAG` + reset `PACKER_VERSION=0` | `30.0.1.0` → `30.0.1.0` (new `nativeTag`) |
| Bug in this packer's Rust (`ffi/`, `scripts/`, `targets/`) | `PACKER_VERSION` +1 | `30.0.1.0` → `30.0.1.1` |

To republish after a fix: increment `PACKER_VERSION` in `wgpu-pin.env`, push
a tag `v<WGPU_VERSION>.<PACKER_VERSION>`. **Never reuse an existing tag**: a
published release is immutable, and consumers pin its SHA256.

The `manifest.json` inside each archive records both `wgpu.crates` and
`wgpu.nativeTag`, since the file name only carries the crates version.

## Target status

`targets/matrix.json` is the source of truth. Targets with `status != "dropped"`
are built by CI and published.

| Target | Backends | Local validation |
|---|---|---|
| `macosArm64` | metal, null | archive + install (static + shared); smoke test run |
| `macosX64` | metal, null | archive + install (cross) |
| `iosArm64` | metal, null | archive + install; device: structural check |
| `iosSimulatorArm64` | metal, null | archive + install (cross) |
| `linuxX64` | vulkan, gl, null | built in CI (`ubuntu-24.04`) |
| `linuxArm64` | vulkan, gl, null | built in CI (`ubuntu-24.04`) |
| `mingwX64` | d3d12, vulkan, gl, null | built in CI (`ubuntu-24.04`) |
| `windowsMsvcX64` | d3d12, vulkan, gl, null | built in CI (`windows-2022`) |
| `androidNativeArm64` | vulkan, gles, null | built in CI; NDK pinned |
| `androidNativeX64` | vulkan, gles, null | built in CI; NDK pinned |

Dropped targets:

| Target | Reason |
|---|---|
| `tvosArm64` | no Rust target; wgpu-hal has no tvOS code |
| `tvosSimulatorArm64` | no Rust target; wgpu-hal has no tvOS code |

Note: the `androidNativeArm64` **archive** builds and publishes fine; it is
the Kotlin/Native *consumer* that is blocked (libc++ incompatibility). That is
a downstream concern, not a packer one.

## Local build

```bash
bash scripts/sync.sh                       # wgpu-native clone + headers + .pin-state
bash scripts/build-target.sh macosArm64
bash scripts/package.sh macosArm64 static  # -> dist/wgpu-30.0.1.0-macosArm64-static.tar.gz
```

Android targets require `NDK_HOME` pointing at NDK 29.0.14206865.

## Tests

```bash
python3 tests/test_matrix.py
python3 tests/test_version.py
bash scripts/test_pipeline.sh
bash scripts/test_packaging.sh
```

## Consuming from Kotlin

See `packaging/GUIDE.md`: archive layout, cinterop `.def` file, Gradle
snippet, memory and threading contract, link flags per target.
