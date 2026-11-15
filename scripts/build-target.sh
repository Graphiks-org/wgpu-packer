#!/usr/bin/env bash
# Usage:
#   build-target.sh <target-name>         -> cargo build --release + checks.py audit
#                                           (+ static link test on the host target)
#   build-target.sh --print-target <name> -> prints the Rust triple
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=wgpu-pin.env
source wgpu-pin.env

if [[ "${1:-}" == "--print-target" ]]; then
  # StopIteration (unknown target) is an expected failure: no traceback.
  python3 -c "import sys; sys.path.insert(0,'tests'); import matrix; print(matrix.by_kotlin_target('$2')['triple'])" 2>/dev/null
  exit $?
fi

NAME="$1"
TRIPLE="$(./scripts/build-target.sh --print-target "$NAME")" || {
  echo "build-target: unknown target: $NAME (see targets/matrix.json)" >&2
  exit 2
}

# Apple deployment environment (ANDROID_API is consumed further down).
eval "$(python3 - "$NAME" <<'EOF'
import shlex, sys
sys.path.insert(0, "tests")
import matrix
t = matrix.by_kotlin_target(sys.argv[1])
for k, v in t.get("deployEnv", {}).items():
    if k != "ANDROID_API":
        print(f"export {k}={shlex.quote(str(v))}")
EOF
)"

# Android linker via the NDK (generates .cargo/config.toml — independent of the host OS).
case "$TRIPLE" in
  *-linux-android)
    : "${NDK_HOME:?NDK_HOME required for $NAME}"
    PREBUILT="$(ls -d "$NDK_HOME"/toolchains/llvm/prebuilt/* | head -1)"
    API="$(python3 -c "import sys; sys.path.insert(0,'tests'); import matrix; print(matrix.by_kotlin_target('$NAME')['deployEnv']['ANDROID_API'])")"
    # bindgen needs the API level to build a versioned target triple; the
    # NDK's cdefs.h rejects an unversioned one.
    export WGPU_ANDROID_API="$API"
    mkdir -p .cargo
    cat > .cargo/config.toml <<EOF
[target.$TRIPLE]
linker = "$PREBUILT/bin/$TRIPLE${API}-clang"
ar = "$PREBUILT/bin/llvm-ar"
EOF
    ;;
esac

rustup target add "$TRIPLE" >/dev/null
cargo build --locked --release -p wgpu-packer-ffi --target "$TRIPLE"

# Symbol audit (spec §4.5) on this target's library. The audit reads the
# exports of the shared library when the target ships one (the checks.py
# dumpbin path needs the DLL), and the static archive otherwise.
case "$TRIPLE" in
  *-windows-msvc | *-windows-gnu) LIB="target/$TRIPLE/release/wgpu_packer.dll" ;;
  *)                              LIB="target/$TRIPLE/release/libwgpu_packer.a" ;;
esac
python3 tests/checks.py --lib "$LIB"

# Static link test on the host target only (Review Focus #5):
# link_main.c + libwgpu_packer.a + the matrix's exact linkLibs.
# (The Windows runner has no `cc`: the msvc path is covered by
# scripts/cross-link-test.sh in CI.)
HOST_TRIPLE="$(rustc -vV | sed -n 's/host: //p')"
if [[ "$TRIPLE" == "$HOST_TRIPLE" ]] && command -v cc >/dev/null; then
  LINK_LIBS="$(python3 -c "import sys; sys.path.insert(0,'tests'); import matrix; print(matrix.by_kotlin_target('$NAME')['linkLibs'])")"
  cc tests/link_main.c -Iffi/ffi "target/$TRIPLE/release/libwgpu_packer.a" $LINK_LIBS \
     -o /tmp/wgpu_packer_link_test
  /tmp/wgpu_packer_link_test
fi

echo "BUILD_OK $NAME ($TRIPLE)"
