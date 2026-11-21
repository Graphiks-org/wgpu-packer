# wgpu-packer

Prebuilt WebGPU libraries published as GitHub Releases for non-web
Kotlin/Native targets.

This repository only produces libraries (`webgpu.h` headers plus
`libwgpu_packer`). Any Kotlin/Native, JNI or Swift binding lives in the
consuming project.

## Versioning and republication

The archive name is `wgpu-<WGPU_VERSION>.<PACKER_VERSION>-<kotlinTarget>-<linkage>.tar.gz`.

| What changed | Counter | Example |
|---|---|---|
| wgpu crates (`wgpu-core`, `wgpu-hal`, `naga`) | `WGPU_VERSION` + reset `PACKER_VERSION=0` | `30.0.1.0` → `30.0.2.0` |
| `webgpu.h` revision (ABI) | edit `WEBGPU_HEADERS_REVISION` + reset `PACKER_VERSION=0` | `30.0.1.0` → `30.0.1.0` (new `headersRevision`) |
| Bug in this packer's Rust (`ffi/`, `scripts/`, `targets/`) | `PACKER_VERSION` +1 | `30.0.1.0` → `30.0.1.1` |

To republish after a fix: increment `PACKER_VERSION` in `wgpu-pin.env`, push
a tag `v<WGPU_VERSION>.<PACKER_VERSION>`. **Never reuse an existing tag**: a
published release is immutable, and consumers pin its SHA256.

Each packaged archive carries a `manifest.json` recording both `wgpu.crates`
and `wgpu.headersRevision`, since the file name only carries the crates version.

## Target status

`targets/matrix.json` is the source of truth. Targets with `status != "dropped"`
are built by CI and published.

| Target | Backends | Local validation |
|---|---|---|
| `macosArm64` | metal, null | archive + install (static + shared); link-and-call test (`link_main.c`) |
| `macosX64` | metal, null | archive + install (cross) |
| `iosArm64` | metal, null | archive + install; device: structural check |
| `iosSimulatorArm64` | metal, null | archive + install (cross) |
| `linuxX64` | vulkan, gl, null | built in CI (`ubuntu-24.04`) |
| `linuxArm64` | vulkan, gl, null | built in CI (`ubuntu-24.04`) |
| `mingwX64` | d3d12, vulkan, gl, null | built in CI (`ubuntu-24.04`) |
| `windowsMsvcX64` | d3d12, vulkan, gl, null | built in CI (`windows-2022`) |
| `androidNativeArm64` | vulkan, gles, null | archive + install; K/N consumption validated during the v1 spike (`arm64-v8a`, static only) |
| `androidNativeX64` | vulkan, gles, null | built in CI; NDK pinned (static only) |

Dropped targets:

| Target | Reason |
|---|---|
| `tvosArm64` | no Rust target; wgpu-hal has no tvOS code |
| `tvosSimulatorArm64` | no Rust target; wgpu-hal has no tvOS code |

Note: the Android archives are consumed by Kotlin/Native fine — a K/N 2.4.10
app linking `libwgpu_packer.a` builds, installs and renders on an emulator;
the linked archive was confirmed to be the packer's (md5 + `wgpu_packer`
symbols in the final `.so`). Only the static linkage is supported and
validated; the shared `.so` is not tested.

## Local build

```bash
bash scripts/build-target.sh macosArm64
bash scripts/package.sh macosArm64 static  # -> dist/wgpu-30.0.2.0-macosArm64-static.tar.gz
```

Android targets require `NDK_HOME` pointing at NDK 28 or newer: NDK 28 is
the first release whose linker aligns shared-library segments on 16 KiB,
which Google Play requires. NDK 27 and older produce 4 KiB segments.

## Tests

```bash
python3 tests/test_matrix.py
python3 tests/test_version.py
python3 tests/test_checks.py
python3 tests/test_unimplemented.py
python3 tests/test_ci.py
python3 tests/test_manifest.py
python3 tests/test_android_targets.py
bash scripts/test_pipeline.sh
bash scripts/test_packaging.sh
bash scripts/cinterop-smoke.sh   # Kotlin/Native consumer path (brew install kotlin-native)
```

## Consuming from Kotlin

See `packaging/GUIDE.md`: archive layout, cinterop `.def` file, Gradle
snippet, memory and threading contract, link flags per target.
