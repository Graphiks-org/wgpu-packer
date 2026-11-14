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
LIB="target/$TRIPLE/release/libwgpu_packer.a"
[[ -f "$LIB" ]] || { echo "cross-link-test: $LIB missing — build first" >&2; exit 2; }

LINK_LIBS="$(python3 -c "import sys; sys.path.insert(0,'tests'); import matrix; print(matrix.by_kotlin_target('$NAME')['linkLibs'])")"

# Cross compiler resolution by target family.
case "$TRIPLE" in
  *-apple-darwin)        CC="cc -arch $(echo "$TRIPLE" | cut -d- -f1)"; SYSROOT=(-isysroot "$(xcrun --sdk macosx --show-sdk-path)") ;;
  aarch64-apple-ios)     CC="cc"; SYSROOT=(-isysroot "$(xcrun --sdk iphoneos --show-sdk-path)" -target arm64-apple-ios) ;;
  aarch64-apple-ios-sim) CC="cc"; SYSROOT=(-isysroot "$(xcrun --sdk iphonesimulator --show-sdk-path)" -target arm64-apple-ios-simulator) ;;
  aarch64-unknown-linux-gnu) CC="aarch64-linux-gnu-gcc"; SYSROOT=() ;;
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
  cl //nologo tests/headers_compile.c //Fo:/tmp/wgpu_tu.obj //c //Iffi/ffi
  # shellcheck disable=SC2086
  cl //nologo tests/link_main.c /tmp/wgpu_tu.obj "$LIB" $LINK_LIBS //Iffi/ffi //Fe:/tmp/wgpu_cross_link.exe //link //SUBSYSTEM:CONSOLE
else
  # shellcheck disable=SC2086
  $CC "${SYSROOT[@]}" -c tests/headers_compile.c -Iffi/ffi -o /tmp/wgpu_tu.o
  # shellcheck disable=SC2086
  $CC "${SYSROOT[@]}" -Iffi/ffi tests/link_main.c /tmp/wgpu_tu.o "$LIB" $LINK_LIBS -o /tmp/wgpu_cross_link_test
fi
echo "CROSS_LINK_OK $NAME ($TRIPLE)"
