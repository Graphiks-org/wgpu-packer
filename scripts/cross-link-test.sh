#!/usr/bin/env bash
# Test de liaison CROISÉ pour les cibles non-hôte (revue finale, finding 5) :
# compile tests/headers_compile.c (TU §4.5) et link tests/link_main.c contre la
# statique avec les link_libs EXACTS de la matrice. Compilation + lien, sans
# exécution. À appeler depuis la CI après build-target.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
source wgpu-pin.env

NAME="$1"
TRIPLE="$(./scripts/build-target.sh --print-target "$NAME")"
LIB="target/$TRIPLE/release/libwgpu_packer.a"
[[ -f "$LIB" ]] || { echo "cross-link-test: $LIB absent — build d'abord" >&2; exit 2; }

LINK_LIBS="$(python3 -c "import json;m=json.load(open('targets/matrix.json'));t=next(t for t in m['targets'] if t['name']=='''$NAME''');print(t['link_libs'])")"

# Résolution du compilateur croisé par famille de cible.
case "$TRIPLE" in
  *-apple-darwin)        CC="cc -arch $(echo "$TRIPLE" | cut -d- -f1)"; SYSROOT=(-isysroot "$(xcrun --sdk macosx --show-sdk-path)") ;;
  aarch64-apple-ios)     CC="cc"; SYSROOT=(-isysroot "$(xcrun --sdk iphoneos --show-sdk-path)" -target arm64-apple-ios) ;;
  aarch64-apple-ios-sim) CC="cc"; SYSROOT=(-isysroot "$(xcrun --sdk iphonesimulator --show-sdk-path)" -target arm64-apple-ios-simulator) ;;
  aarch64-unknown-linux-gnu) CC="aarch64-linux-gnu-gcc"; SYSROOT=() ;;
  x86_64-pc-windows-gnu) CC="x86_64-w64-mingw32-gcc"; SYSROOT=() ;;
  *-linux-android)
    : "${NDK_HOME:?NDK_HOME requis}"
    PREBUILT="$(ls -d "$NDK_HOME"/toolchains/llvm/prebuilt/* | head -1)"
    API="$(python3 -c "import json;m=json.load(open('targets/matrix.json'));t=next(t for t in m['targets'] if t['name']=='''$NAME''');print(t['deploy_env']['ANDROID_API'])")"
    CC="$PREBUILT/bin/$TRIPLE${API}-clang"; SYSROOT=() ;;
  x86_64-pc-windows-msvc)
    # cl.exe via l'environnement msvc-dev-cmd (CI) : TU + lien sans exécution.
    CC="cl"; SYSROOT=() ;;
  *) echo "cross-link-test: triple non géré : $TRIPLE" >&2; exit 2 ;;
esac

if [[ "$CC" == cl ]]; then
  # shellcheck disable=SC2086
  cl //nologo tests/headers_compile.c //Fo:/tmp/wgpu_tu.obj //c //Iffi/ffi
  # shellcheck disable=SC2086
  cl //nologo tests/link_main.c /tmp/wgpu_tu.obj "$LIB" $LINK_LIBS //Fe:/tmp/wgpu_cross_link.exe //link //SUBSYSTEM:CONSOLE
else
  # shellcheck disable=SC2086
  $CC "${SYSROOT[@]}" -c tests/headers_compile.c -Iffi/ffi -o /tmp/wgpu_tu.o
  # shellcheck disable=SC2086
  $CC "${SYSROOT[@]}" tests/link_main.c /tmp/wgpu_tu.o "$LIB" $LINK_LIBS -o /tmp/wgpu_cross_link_test
fi
echo "CROSS_LINK_OK $NAME ($TRIPLE)"
