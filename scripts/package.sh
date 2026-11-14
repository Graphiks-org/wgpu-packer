#!/usr/bin/env bash
# Usage: package.sh <kotlinTarget> <static|shared>
# Produces dist/wgpu-<version>-<kotlinTarget>-<kind>.tar.gz + checksum (spec §6).
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=wgpu-pin.env
source wgpu-pin.env

NAME="$1"
KIND="$2"
TRIPLE="$(./scripts/build-target.sh --print-target "$NAME")" || {
  echo "package: unknown target: $NAME" >&2
  exit 2
}
VERSION="$WGPU_VERSION.$PACKER_VERSION"
STAGE="dist/stage/wgpu-$VERSION-$NAME-$KIND"
rm -rf "$STAGE"
mkdir -p "$STAGE/include" "$STAGE/lib"

cp ffi/ffi/webgpu.h ffi/ffi/wgpu.h ffi/ffi/wgpu_upstream.h "$STAGE/include/"
python3 - "$NAME" > "$STAGE/lib/link.txt" <<'EOF'
import sys
sys.path.insert(0, "tests")
import matrix
print(matrix.by_kotlin_target(sys.argv[1])["linkLibs"])
EOF

case "$KIND:$TRIPLE" in
  static:*windows-msvc) cp "target/$TRIPLE/release/wgpu_packer.lib" "$STAGE/lib/" ;;
  static:*) cp "target/$TRIPLE/release/libwgpu_packer.a" "$STAGE/lib/" ;;
  shared:*apple-darwin)
    cp "target/$TRIPLE/release/libwgpu_packer.dylib" "$STAGE/lib/"
    # Cargo no longer strips (rust#157750 leaves the LINKEDIT string pool
    # mis-aligned, which dyld rejects at dlopen); Apple's strip does it right.
    /usr/bin/strip -x "$STAGE/lib/libwgpu_packer.dylib" ;;
  shared:*linux*) cp "target/$TRIPLE/release/libwgpu_packer.so" "$STAGE/lib/" ;;
  shared:*windows-msvc)
    cp "target/$TRIPLE/release/wgpu_packer.dll" "$STAGE/lib/"
    [[ -f "target/$TRIPLE/release/wgpu_packer.dll.lib" ]] &&
      cp "target/$TRIPLE/release/wgpu_packer.dll.lib" "$STAGE/lib/" ;;
  shared:*windows-gnu)
    cp "target/$TRIPLE/release/wgpu_packer.dll" "$STAGE/lib/"
    [[ -f "target/$TRIPLE/release/libwgpu_packer.dll.a" ]] &&
      cp "target/$TRIPLE/release/libwgpu_packer.dll.a" "$STAGE/lib/" ;;
  *)
    echo "package: unknown combination: $KIND $TRIPLE" >&2
    exit 2
    ;;
esac

cp LICENSE "$STAGE/LICENSE"
for L in LICENSE-APACHE LICENSE-MIT; do
  [[ -f "packaging/$L" ]] && cp "packaging/$L" "$STAGE/"
done

mkdir -p dist
ARCHIVE="dist/wgpu-$VERSION-$NAME-$KIND.tar.gz"
# The archive name only carries the crates version: the manifest is the
# only place that also records the wgpu-native tag of the vendored headers.
# It is generated before the tar so it ends up inside the archive.
python3 packaging/make-manifest.py \
  --install-dir "$STAGE" \
  --kotlin-target "$NAME" \
  --entry-json "$(python3 -c "import sys,json; sys.path.insert(0,'tests'); import matrix; print(json.dumps(matrix.by_kotlin_target('$NAME')))")" \
  --wgpu-version "$WGPU_VERSION" \
  --native-tag "$WGPU_NATIVE_TAG" \
  --packer-version "$PACKER_VERSION" \
  --linkage "$KIND" \
  --out "$STAGE/manifest.json"
tar -czf "$ARCHIVE" -C "$STAGE" .
if command -v shasum >/dev/null; then HASHER="shasum -a 256"; else HASHER="sha256sum"; fi
( cd dist && $HASHER "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256" )
echo "PACKAGE_OK $ARCHIVE"
