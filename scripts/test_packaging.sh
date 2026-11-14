#!/usr/bin/env bash
# Test structurel du packaging consommateur (Task 8 du plan).
set -euo pipefail
cd "$(dirname "$0")/.."
fail() { echo "FAIL: $1" >&2; exit 1; }

[[ -f packaging/wgpu.def ]] || fail "wgpu.def absent"
grep -q '^headers = wgpu.h' packaging/wgpu.def || fail "def: headers"
grep -q 'libwgpu_packer' packaging/wgpu.def || fail "def: bibliothèque"

[[ -f packaging/GUIDE.md ]] || fail "GUIDE.md absent"
for motif in "StableRef" "thread" "Release" "unmap" "link.txt" "uncapturedErrorCallbackInfo" "deviceLostCallbackInfo"; do
  grep -qi "$motif" packaging/GUIDE.md || fail "GUIDE: section '$motif' manquante"
done

echo "PACKAGING_OK"
