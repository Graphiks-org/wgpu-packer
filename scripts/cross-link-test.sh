#!/usr/bin/env bash
# CROSS-link test for non-host targets (final review, finding 5):
# compiles tests/headers_compile.c (TU §4.5) and links tests/link_main.c against
# the static library with the matrix's EXACT linkLibs. Compile + link, no
# execution. To be called from CI after build-target.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
source wgpu-pin.env

NAME="$1"
TRIPLE="$(./scripts/build-target.sh --print-target "$NAME")"
# MSVC names its static library `wgpu_packer.lib`, not `libwgpu_packer.a`.
case "$TRIPLE" in
  *-windows-msvc) LIB="target/$TRIPLE/release/wgpu_packer.lib" ;;
  *)              LIB="target/$TRIPLE/release/libwgpu_packer.a" ;;
esac
[[ -f "$LIB" ]] || { echo "cross-link-test: $LIB missing — build first" >&2; exit 2; }

LINK_LIBS="$(python3 -c "import sys; sys.path.insert(0,'tests'); import matrix; print(matrix.by_kotlin_target('$NAME')['linkLibs'])")"

# Cross compiler resolution by target family.
case "$TRIPLE" in
  *-apple-darwin)
    # Rust triples say `aarch64`/`x86_64`; clang's -arch wants `arm64`/`x86_64`.
    case "$TRIPLE" in
      aarch64-*) ARCH=arm64 ;;
      *)         ARCH=$(echo "$TRIPLE" | cut -d- -f1) ;;
    esac
    CC="cc -arch $ARCH"; SYSROOT=(-isysroot "$(xcrun --sdk macosx --show-sdk-path)") ;;
  aarch64-apple-ios)     CC="cc"; SYSROOT=(-isysroot "$(xcrun --sdk iphoneos --show-sdk-path)" -target arm64-apple-ios) ;;
  aarch64-apple-ios-sim) CC="cc"; SYSROOT=(-isysroot "$(xcrun --sdk iphonesimulator --show-sdk-path)" -target arm64-apple-ios-simulator) ;;
  aarch64-unknown-linux-gnu) CC="aarch64-linux-gnu-gcc"; SYSROOT=() ;;
  x86_64-unknown-linux-gnu)  CC="cc"; SYSROOT=() ;;
  x86_64-pc-windows-gnu) CC="x86_64-w64-mingw32-gcc"; SYSROOT=() ;;
  *-linux-android)
    : "${NDK_HOME:?NDK_HOME required}"
    PREBUILT="$(ls -d "$NDK_HOME"/toolchains/llvm/prebuilt/* | head -1)"
    API="$(python3 -c "import sys; sys.path.insert(0,'tests'); import matrix; print(matrix.by_kotlin_target('$NAME')['deployEnv']['ANDROID_API'])")"
    CC="$PREBUILT/bin/$TRIPLE${API}-clang"; SYSROOT=() ;;
  x86_64-pc-windows-msvc)
    # cl.exe via the msvc-dev-cmd environment (CI): TU + link without execution.
    CC="cl"; SYSROOT=() ;;
  *) echo "cross-link-test: unsupported triple: $TRIPLE" >&2; exit 2 ;;
esac

if [[ "$CC" == cl ]]; then
  # shellcheck disable=SC2086
  # `-I` (not `//I`) for the include path: Git Bash rewrites `//X` to `/X`,
  # which cl then rejects. `//nologo` and `//link` are safe because cl accepts
  # `/nologo` and `/link` too.
  # A relative output directory: `/tmp` means `D:\tmp` to cl on Windows.
  mkdir -p build/cross-link
  cl -nologo tests/headers_compile.c -Fobuild/cross-link/wgpu_tu.obj -c -Iffi/ffi
  # shellcheck disable=SC2086
  # The import libraries (user32.lib, ...) are linker inputs: they must come
  # after `-link`, which is why the whole command ends with $LINK_LIBS.
  # Passing `LIB` from a POSIX shell to cl does not work: the value is a
  # Windows path list and Git Bash mangles it. `msvc-dev-cmd` also exports
  # `WindowsSdkDir` and `WindowsSDKLibVersion`, which locate the import
  # libraries unambiguously.
  SDK_LIBS=()
  if [[ -n "${WindowsSdkDir:-}" && -n "${WindowsSDKLibVersion:-}" ]] && command -v cygpath >/dev/null; then
    sdk_root="$(cygpath -u "${WindowsSdkDir}Lib\\${WindowsSDKLibVersion}")"
    for dir in "$sdk_root/um/x64" "$sdk_root/ucrt/x64"; do
      for lib in user32 gdi32 dxgi d3d12 dxguid kernel32; do
        [[ -f "$dir/$lib.lib" ]] && SDK_LIBS+=("$(cygpath -w "$dir/$lib.lib")")
      done
    done
  fi
  echo "cross-link: SDK_LIBS=${SDK_LIBS[*]:-<none>}" >&2
  echo "cross-link: LINK_LIBS=$LINK_LIBS" >&2
  cl -nologo tests/link_main.c build/cross-link/wgpu_tu.obj "$LIB" -Iffi/ffi \
     -Fobuild/cross-link/wgpu_cross_link.exe -link -SUBSYSTEM:CONSOLE \
     "${SDK_LIBS[@]}" $LINK_LIBS
else
  # shellcheck disable=SC2086
  $CC "${SYSROOT[@]}" -c tests/headers_compile.c -Iffi/ffi -o /tmp/wgpu_tu.o
  # shellcheck disable=SC2086
  $CC "${SYSROOT[@]}" -Iffi/ffi tests/link_main.c /tmp/wgpu_tu.o "$LIB" $LINK_LIBS -o /tmp/wgpu_cross_link_test
fi
echo "CROSS_LINK_OK $NAME ($TRIPLE)"
