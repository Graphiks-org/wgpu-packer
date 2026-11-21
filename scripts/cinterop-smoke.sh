#!/usr/bin/env bash
# Consume the headers the way wgpu4k does —
# Kotlin/Native cinterop against packaging/wgpu.def + the static library —
# then compile AND RUN a real Kotlin program calling the C ABI.
#
# Host tooling: kotlinc-native + cinterop on PATH (brew install
# kotlin-native, or any Kotlin/Native prebuilt's bin/ on PATH).
set -euo pipefail
cd "$(dirname "$0")/.."

command -v cinterop >/dev/null || { echo "cinterop not found on PATH" >&2; exit 2; }
command -v kotlinc-native >/dev/null || { echo "kotlinc-native not found on PATH" >&2; exit 2; }

LIB="target/release/libwgpu_packer.a"
[[ -f "$LIB" ]] || cargo build --release -p wgpu-packer-ffi

# Platform link flags: the same ones targets/matrix.json carries and the
# archive's lib/link.txt repeats (packaging/wgpu.def comments).
case "$(uname -s)" in
  Darwin) FRAMEWORKS="-framework Metal -framework QuartzCore -framework CoreGraphics -framework CoreVideo -framework IOKit -framework Foundation" ;;
  Linux)  FRAMEWORKS="-ldl -lpthread -lm" ;;
  *) echo "unsupported host: $(uname -s)" >&2; exit 2 ;;
esac

OUT="build/cinterop-smoke"
rm -rf "$OUT"; mkdir -p "$OUT"
FFI_DIR="$(pwd)/ffi/ffi"
LIBDIR="$(pwd)/target/release"

# The consumer's def (GUIDE §2: "Copy packaging/wgpu.def, point
# libraryPaths at the lib/"), with local paths — the smoke tests the same
# artifact the archive ships.
sed -e "s|^libraryPaths = .*|libraryPaths = $LIBDIR|" \
    -e "s|^linkerOpts = .*|linkerOpts = $FRAMEWORKS|" \
    packaging/wgpu.def > "$OUT/wgpu.def"
# Resolve `headers = wgpu.h` against the vendored headers (the archive has
# them in include/, the build tree in ffi/ffi/).
printf 'compilerOpts = -I%s\n' "$FFI_DIR" >> "$OUT/wgpu.def"

echo "== cinterop =="
(cd "$OUT" && cinterop -def wgpu.def -o wgpu)

echo "== kotlinc-native =="
(cd "$OUT" && kotlinc-native ../..//tests/cinterop_smoke.kt -l wgpu.klib -o smoke)

echo "== run =="
"$OUT/smoke.kexe"
