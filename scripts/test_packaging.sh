#!/usr/bin/env bash
# Structural test of the consumer packaging (plan Task 8).
set -euo pipefail
cd "$(dirname "$0")/.."
fail() { echo "FAIL: $1" >&2; exit 1; }

[[ -f packaging/wgpu.def ]] || fail "wgpu.def missing"
grep -q '^headers = wgpu.h' packaging/wgpu.def || fail "def: headers"
grep -q 'libwgpu_packer' packaging/wgpu.def || fail "def: library"

[[ -f packaging/GUIDE.md ]] || fail "GUIDE.md missing"
for motif in "StableRef" "thread" "Release" "unmap" "link.txt" "uncapturedErrorCallbackInfo" "deviceLostCallbackInfo"; do
  grep -qi "$motif" packaging/GUIDE.md || fail "GUIDE: section '$motif' missing"
done

echo "PACKAGING_OK"
