# Spike: Android Native (`androidNativeArm64`, `androidNativeX64`)

## Question

Can the packer publish archives consumable by Kotlin/Native Android,
or does the libc++ runtime bundled with K/N make linking impossible?

## What the old library did

`ygdrasil-io/wgpu4k-native` (Kotlin 2.4.10, wgpu-native v29.0.0.0) declared
`androidNativeArm64()`/`androidNativeX64()`; its CI built a `NativeActivity`
APK (`verifyAndroidNativeApks`) containing `libwgpu4k_demo.so`
for both ABIs.

As the brief notes, that CI check only verifies the **presence** of the `.so`
files in the APK: it does not prove that the application **starts**. That is the gap this
spike closes, by going all the way to launching it on an emulator.

## Measurement

### Environment

| Item | Value |
| --- | --- |
| Host | macOS, Apple Silicon (arm64) |
| Emulator | AVD `Medium_Phone`, `arm64-v8a`, `sys.boot_completed=1` |
| Kotlin/Native | **2.4.10** (prebuilt present in `~/.konan`, contrary to what the brief assumed) |
| NDK that built the tested archive | **27.3.13750724** (r27d), clang 18.0.4 — this is the NDK present on the measurement machine, selected by `NDK_HOME` via `scripts/build-target.sh` |
| NDK pinned by CI | **27.0.11902866** (`.github/workflows/build.yml:86`) — **different** from the one used here; CI was not re-run for this spike |
| NDK bundled by K/N 2.4.10 | `target-toolchain-2-osx-android_ndk`, clang **8.0.7**, `AndroidVersion.txt` = `8.0.7`, binary built under `ndk-release-r20` → **NDK r20** era, not r19c |

The 27.3 / 27.0 difference does not change the nature of the measurement (the packer
does not produce C++ objects), but it is not nil: the verdict applies to an
archive built with 27.3.

### Reproduction on the emulator

Two infrastructure obstacles had to be cleared before obtaining a measurement;
neither is a libc++ problem:

1. The brief's `--depth 1` clone omits `--recurse-submodules`. `settings.gradle.kts`
   does `includeBuild("Kadre")`, and without the submodule the build fails in 9 s:

   ```
   * What went wrong:
   Project with path ':kadre' not found in build ':kadre-build'.
   ```

   Fixed by `git submodule update --init --depth 1` (`Kadre`, `kextract`).

2. The Android toolchain bundled by K/N is an **x86_64** binary and Rosetta 2
   was not installed. The K/N link then fails with:

   ```
   The /Users/chaos/.konan/dependencies/target-toolchain-2-osx-android_ndk/bin/aarch64-linux-android21-clang command returned non-zero exit code: 126.
   output:
   .../bin/aarch64-linux-android21-clang: line 3: .../bin/clang: Bad CPU type in executable
   ```

   Fixed by `softwareupdate --install-rosetta --agree-to-license`.

After these two fixes, `./gradlew :demo:android-native:assembleDebug`:

```
> Task :demo:android-native:assembleDebug

BUILD SUCCESSFUL in 53s
71 actionable tasks: 11 executed, 60 up-to-date
```

The APK does contain both ABIs:

```
 13098944  lib/arm64-v8a/libwgpu4k_demo.so
 14115296  lib/x86_64/libwgpu4k_demo.so
```

Install and launch (the real identifier is `io.ygdrasil.wgpu.app.native.debug`,
not `io.ygdrasil.wgpu.app.native` as in the brief):

```
$ adb install -r demo/android-native/build/outputs/apk/debug/android-native-debug.apk
Performing Streamed Install
Success
$ adb shell am start -n io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity
Starting: Intent { cmp=io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity }
```

logcat (excerpt):

```
D nativeloader: Load .../base.apk!/lib/arm64-v8a/libwgpu4k_demo.so using class loader ns clns-9 (caller=<unknown>): ok
I Konan_main: ANativeActivity_onCreate called
I NativeActivity: onNativeWindowCreated called
D vulkan  : searching for layers in .../lib/arm64-v8a
I ActivityTaskManager: Displayed io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity for user 0: +2s944ms
```

No `FATAL`, `UnsatisfiedLink`, `undefined symbol` or `dlopen failed` line.
The process stays alive (`adb shell pidof io.ygdrasil.wgpu.app.native.debug`
→ `3878`), and `adb exec-out screencap` shows the red triangle on a green background.
**The old library's application starts and renders.**

### Reproduction with the packer's archive

The APK above links `wgpu-native` v29, not our packer. To close the gap,
the `libWGPU.a` extracted by the old library was replaced with
`target/aarch64-linux-android/release/libwgpu_packer.a` (our archive, wgpu
v30.0.1.0), then the link was forced again.

**Warning**: the Gradle extraction task
`unzip-libwgpu_native.a-from-wgpu-android-aarch64-release.zip` can overwrite
`libWGPU.a` from the downloaded zip. You must therefore:

1. copy our archive **after** extraction;
2. re-run the build excluding that task (`-x unzip-...`);
3. check the md5 of `libWGPU.a` **after** the build.

Exact commands:

```
$ cp target/aarch64-linux-android/release/libwgpu_packer.a \
     /tmp/old-wgpu4k-native/wgpu4k-native/build/native/android-aarch64/lib/libWGPU.a
$ md5 .../libWGPU.a
MD5 (.../libWGPU.a) = 46b70a90f08d77a8e1bc78ee01c50b80   # ← our archive

$ cd /tmp/old-wgpu4k-native && ./gradlew :demo:android-native:assembleDebug --rerun-tasks \
     -x unzip-libwgpu_native.a-from-wgpu-android-aarch64-release.zip --no-daemon
BUILD SUCCESSFUL in 49s
70 actionable tasks: 70 executed

$ md5 .../libWGPU.a
MD5 (.../libWGPU.a) = 46b70a90f08d77a8e1bc78ee01c50b80   # ← still ours after the build
```

Proof that the final `.so` **contains our code** (and not merely that the link
succeeded):

```
$ llvm-nm -gU demo/common/build/bin/androidNativeArm64/debugShared/libwgpu4k_demo.so | grep -c wgpu_packer
37
$ strings -a .../libwgpu4k_demo.so | grep "panic contained"
$[wgpu_packer] panic contained in
```

(the string comes from `ffi/src/ffi_utils.rs:13`; the 37 symbols include
`_ZN11wgpu_packer...`, `...wgpu_packer..objects..WGPUDeviceImpl..drop...`, etc.)

Our archive exports 228 `wgpu*` symbols versus 226 for the old one, and it is
a **superset**: no symbol required by the v29 cinterop is missing.

Install + launch:

```
$ adb install -r demo/android-native/build/outputs/apk/debug/android-native-debug.apk
Success
$ adb shell am start -n io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity
Starting: Intent { cmp=io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity }
```

logcat (excerpt):

```
D nativeloader: Load .../base.apk!/lib/arm64-v8a/libwgpu4k_demo.so using class loader ns clns-9 (caller=<unknown>): ok
I Konan_main: ANativeActivity_onCreate called
I NativeActivity: onNativeWindowCreated called
I Konan_main: DEBUG : Instance::new: created Vulkan backend
I Konan_main: TRACE : Device::create_render_pipeline -> RenderPipelineId(0,1)
I Konan_main: TRACE : RenderPass::draw 3 1 0 0
I Konan_main: TRACE : Finished encoding render pass (success)
I Konan_main: TRACE : Queue::submit returned submit index 1
I ActivityTaskManager: Displayed io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity for user 0: +4s2ms
```

No `FATAL`, `tombstone`, `SIGSEGV`, `SIGABRT`, `UnsatisfiedLink`,
`undefined symbol` or `dlopen failed` line. Process alive
(`adb shell pidof io.ygdrasil.wgpu.app.native.debug` → `4810`). Screenshot:
`/tmp/packer-android-native-v2.png` — red triangle on a green background.

**Methodological note**: the `TRACE :` lines are emitted by the demo's
Kotlin (`demo/common/src/commonMain/kotlin/ext.kt`, `println("TRACE : ...")`) from
messages from `wgpu_core`, present in **both** archives (v29 as well as
v30). They therefore do **not** distinguish v29 from our v30. The proof that
the linked archive is ours is the pair **md5 of `libWGPU.a`** + **`wgpu_packer`
symbols and the `[wgpu_packer] panic contained` string in the final `.so`**.

### Unresolved libc++ symbols

The brief's `find` pattern (`-path "*android_arm64*"`) finds nothing: the
libc++ bundled by K/N lives under `~/.konan/dependencies/target-toolchain-2-osx-android_ndk/`
(the folder name is `android_ndk`, not `android_arm64`). Actual path:

```
/Users/chaos/.konan/dependencies/target-toolchain-2-osx-android_ndk/sysroot/usr/lib/aarch64-linux-android/libc++_static.a
```

Comparison of unresolved symbols (`llvm-nm -u`), filtered on
`std::__ndk1|__libcpp|__cxx_atomic`:

```
=== /tmp/libWGPU.a.orig   (old lib, wgpu-native v29, android-aarch64) ===
  -> 0 symbols
=== target/aarch64-linux-android/release/libwgpu_packer.a   (this packer, wgpu v30) ===
  -> 0 symbols
=== libc++ K/N defines? ===
0
```

**Scope of this filter**: it only covers symbols mangled under
`std::__ndk1` (the inline namespace of the NDK libc++), `__libcpp*` and
`__cxx_atomic*`. It does **not** cover `__cxa_*` or `_Unwind_*`, and "0
symbols" therefore does **not** mean "no reference to the C++ runtime".

Complementary measurements on the same archives:

| Filter | Old (v29) | Ours (v30) |
| --- | --- | --- |
| `std::__ndk1` / `__libcpp` / `__cxx_atomic` | 0 | 0 |
| `__cxa_*` | 0 | 0 |
| `_Unwind_*` | **124** | **127** |

The `_Unwind_*` symbols therefore do exist, in **both** archives. They are not
libc++ symbols: they are resolved at final link time by the unwinding runtime
provided by the K/N toolchain. Verification: the final `.so` does not leave them
undefined (`llvm-nm -D -u` → 0 `_Unwind_*`), and the application
starts. The libc++ of K/N 2.4.10 does not define `__libcpp_verbose_abort` or
`__libcpp_atomic_wait` (count = 0), but nothing in the archives requires them.

Same finding for `androidNativeX64` (0 symbols under the libc++ filter). The
final `.so` likewise carries no unresolved `std::__ndk1`/`__libcpp`/
`__cxx_atomic` reference (`llvm-nm -D -u` → 0).

## Verdict

VERDICT: supported

A Kotlin/Native 2.4.10 application linking this packer's archive builds,
installs and **runs** on an Android arm64 emulator: the triangle is
rendered via Vulkan, the process does not die, and the final `.so` does
indeed contain packer code (37 `wgpu_packer` symbols, the
`[wgpu_packer] panic contained` string) — the linked archive is therefore indeed ours, and
not wgpu-native v29's.

The claim from the dawn-packer spike ("the K/N link leaves unresolved libc++
symbols, the app crashes on load") does not reproduce here. The
dawn-packer spike had reported it for **its** C++ archive (Dawn references
`__libcpp_verbose_abort` and the like); this spike did not test Dawn and
therefore cannot confirm its cause. Our packer is Rust and emits no
libc++ reference.

## Consequence for the packer

Both targets move to `v1` with `linkages: ["static"]` in
`targets/matrix.json`:

- `androidNativeArm64` — `aarch64-linux-android`, `arm64-v8a`
- `androidNativeX64` — `x86_64-linux-android`, `x86_64`

`linkLibs: "-llog -landroid"`, `deployEnv: {"ANDROID_API": "24"}`,
`backends: ["vulkan", "gles", "null"]`, `runner: "ubuntu-24.04"`.

### Limits of what was proven

- **arm64 only, on an emulator.** `androidNativeX64` was validated only
  by the symbol comparison (0 unresolved libc++ reference) and by the
  fact that its `.so` is indeed packaged in the APK — not by a launch on
  an x86_64 emulator.
- **No test on a physical device.** The emulator uses the host's
  Vulkan; a real mobile GPU may behave differently (drivers, extensions),
  but that is not a libc++ matter.
- **The test binary is the old library's** (wgpu v29 cinterop) into
  which our archive was substituted. This is proof of linking and
  execution of our archive, not a test of a consumer integration
  written for wgpu v30.
- **`linkages: ["static"]` only.** The `shared` mode was not tested;
  the old library declared both, but nothing here proves the `.so`.
- **NDK 27.3.13750724 here, 27.0.11902866 in CI.** The measured archive was
  built with the local NDK 27.3; CI pins 27.0. CI was not
  re-run for this spike.
- **`androidNativeX64` required a local bindgen workaround.**
  `ffi/build.rs` has no Android branch; the x86_64 compilation was
  obtained by passing
  `BINDGEN_EXTRA_CLANG_ARGS_<triple>="--target=... --sysroot=<NDK sysroot>"`.
  If CI fails on this target, `ffi/build.rs` will need an
  Android branch like its iOS/macOS branches.
- The dawn-packer spike remains valid **for Dawn**: its C++ archive does
  reference symbols absent from K/N's libc++. This verdict does not contradict it; it
  shows that the problem is specific to Dawn's C++, not to the K/N Android
  linking mechanism.
