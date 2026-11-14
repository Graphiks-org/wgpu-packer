#!/usr/bin/env bash
# Rafraîchit les sources vendored selon wgpu-pin.env (spec §3).
# Idempotent. --check : ne modifie rien, exit 1 si drift des pins.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=wgpu-pin.env
source wgpu-pin.env

CHECK_ONLY=0
[[ "${1:-}" == "--check" ]] && CHECK_ONLY=1

if [[ ! -d third-party/wgpu-native ]]; then
  if [[ $CHECK_ONLY -eq 1 ]]; then
    echo "sync: third-party absent — exécuter ./scripts/sync.sh sans --check" >&2
    exit 1
  fi
  git clone https://github.com/gfx-rs/wgpu-native third-party/wgpu-native
fi
git -C third-party/wgpu-native fetch --tags --quiet
git -C third-party/wgpu-native checkout --quiet "$WGPU_NATIVE_TAG"
git -C third-party/wgpu-native submodule update --init --depth 1 --quiet || true

NEW_STATE="$(git -C third-party/wgpu-native rev-parse HEAD)
WEBGPU_HEADERS_SHA=$(git -C third-party/wgpu-native rev-parse HEAD:ffi/webgpu-headers)"

if [[ $CHECK_ONLY -eq 1 ]]; then
  if [[ "$(cat .pin-state 2>/dev/null)" == "$NEW_STATE" ]]; then
    exit 0
  fi
  echo "sync: drift — .pin-state ne correspond pas à $WGPU_NATIVE_TAG" >&2
  exit 1
fi

printf '%s\n' "$NEW_STATE" > .pin-state
mkdir -p packaging
cp third-party/wgpu-native/ffi/webgpu-headers/webgpu.h ffi/ffi/webgpu.h
cp third-party/wgpu-native/ffi/wgpu.h ffi/ffi/wgpu_upstream.h
cp third-party/wgpu-native/LICENSE.APACHE packaging/LICENSE-APACHE
cp third-party/wgpu-native/LICENSE.MIT packaging/LICENSE-MIT
echo "sync: pins à jour ($NEW_STATE)"
