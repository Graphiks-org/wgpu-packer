#!/usr/bin/env bash
# Usage :
#   build-target.sh <nom-cible>           -> cargo build --release + audit checks.py
#                                           (+ test de lien statique sur cible hôte)
#   build-target.sh --print-target <nom>  -> affiche le triple rust
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=wgpu-pin.env
source wgpu-pin.env

if [[ "${1:-}" == "--print-target" ]]; then
  # StopIteration (cible inconnue) est un échec attendu : pas de traceback.
  python3 -c "import sys; sys.path.insert(0,'tests'); import matrix; print(matrix.by_kotlin_target('$2')['triple'])" 2>/dev/null
  exit $?
fi

NAME="$1"
TRIPLE="$(./scripts/build-target.sh --print-target "$NAME")" || {
  echo "build-target: cible inconnue : $NAME (cf. targets/matrix.json)" >&2
  exit 2
}

# Environnement de déploiement Apple (ANDROID_API est consommé plus bas).
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

# Linker Android via NDK (génère .cargo/config.toml — indépendant de l'OS hôte).
case "$TRIPLE" in
  *-linux-android)
    : "${NDK_HOME:?NDK_HOME requis pour $NAME}"
    PREBUILT="$(ls -d "$NDK_HOME"/toolchains/llvm/prebuilt/* | head -1)"
    API="$(python3 -c "import sys; sys.path.insert(0,'tests'); import matrix; print(matrix.by_kotlin_target('$NAME')['deployEnv']['ANDROID_API'])")"
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

# Audit de symboles (spec §4.5) sur la bibliothèque de cette cible.
case "$(uname -s)" in
  Darwin | Linux) LIB="target/$TRIPLE/release/libwgpu_packer.a" ;;
  MINGW*)         LIB="target/$TRIPLE/release/wgpu_packer.dll" ;;
esac
python3 tests/checks.py --lib "$LIB"

# Test de lien statique sur cible hôte uniquement (Review Focus #5) :
# link_main.c + libwgpu_packer.a + linkLibs exacts de la matrice.
# (Le runner Windows n'a pas de `cc` : la voie msvc est couverte par
# scripts/cross-link-test.sh dans la CI.)
HOST_TRIPLE="$(rustc -vV | sed -n 's/host: //p')"
if [[ "$TRIPLE" == "$HOST_TRIPLE" ]] && command -v cc >/dev/null; then
  LINK_LIBS="$(python3 -c "import sys; sys.path.insert(0,'tests'); import matrix; print(matrix.by_kotlin_target('$NAME')['linkLibs'])")"
  cc tests/link_main.c -Iffi/ffi "target/$TRIPLE/release/libwgpu_packer.a" $LINK_LIBS \
     -o /tmp/wgpu_packer_link_test
  /tmp/wgpu_packer_link_test
fi

echo "BUILD_OK $NAME ($TRIPLE)"
