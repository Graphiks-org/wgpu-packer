#!/usr/bin/env python3
"""Audit header <-> symboles exportés (spec §4.5).

Chaque fonction déclarée dans webgpu.h/wgpu.h doit être exportée par la
bibliothèque (ou allowlistée dans tests/unimplemented.txt), et chaque symbole
wgpu* exporté doit être déclaré. Exit 1 sur tout écart.
"""
import argparse
import pathlib
import re
import subprocess
import sys

DECL = re.compile(r"^\s*(?:WGPU_EXPORT\s+)?[A-Za-z_][\w \t\*&]*?\b(wgpu[A-Z]\w*)\s*\(", re.M)

# Toujours considérée exportée : sonde debug-only, absente des builds release.
DEBUG_ONLY = {"wgpuDebugTriggerPanic"}


def parse_declarations(text: str) -> set[str]:
    out = set()
    for m in DECL.finditer(text):
        line = text[m.start():text.find("\n", m.start())]
        stripped = line.lstrip()
        if "static inline" in line or stripped.startswith("//") or stripped.startswith("*"):
            continue
        out.add(m.group(1))
    return out


def exported_symbols_missing(declared: set[str], exported: set[str], allow: set[str]) -> set[str]:
    """Déclarées mais ni exportées ni allowlistées."""
    return (declared - allow) - exported


def ghost_symbols(declared: set[str], exported: set[str], allow: set[str]) -> set[str]:
    """Symboles wgpu* exportés mais déclarés nulle part (hors allowlist).

    Seuls les symboles non manglés préfixés wgpu comptent : les symboles
    internes Rust (RNv…, ZN…) font partie de la std liée dans la statique.
    """
    api = {s for s in exported if s.startswith("wgpu")}
    return (api - declared) - allow


def read_allowlist(path: str) -> set[str]:
    p = pathlib.Path(path)
    if not p.exists():
        return set()
    return {l.split("#")[0].strip() for l in p.read_text().splitlines()
            if l.strip() and not l.lstrip().startswith("#")}


def parse_dumpbin(raw: str) -> set[str]:
    """Parse la sortie `dumpbin /exports` (dll MSVC)."""
    syms = set()
    for line in raw.splitlines():
        parts = line.split()
        # "  1    0 001A2B3C wgpuCreateInstance" (ordinal hint RVA name)
        if len(parts) == 4 and parts[0].isdigit() and parts[3].startswith("wgpu"):
            syms.add(parts[3])
    return syms


def nm_exported(lib: pathlib.Path) -> set[str]:
    if lib.suffix == ".dll":
        raw = subprocess.run(
            ["dumpbin", "/exports", str(lib)], capture_output=True, text=True, check=True
        ).stdout
        return parse_dumpbin(raw)
    raw = subprocess.run(["nm", "-gU", str(lib)], capture_output=True, text=True, check=True).stdout
    syms = set()
    for line in raw.splitlines():
        parts = line.split()
        if len(parts) >= 3 and parts[1] in ("T", "W"):
            syms.add(parts[2].lstrip("_"))  # macOS : préfixe _
    return syms


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--lib", required=True)
    ap.add_argument("--headers", default="ffi/ffi")
    ap.add_argument("--allowlist", default="tests/unimplemented.txt")
    args = ap.parse_args()

    declared = set()
    for name in ("webgpu.h", "wgpu_upstream.h", "wgpu.h"):
        p = pathlib.Path(args.headers) / name
        if p.exists():
            declared |= parse_declarations(p.read_text())

    allow = read_allowlist(args.allowlist)
    exported = nm_exported(pathlib.Path(args.lib)) | DEBUG_ONLY

    missing = sorted(exported_symbols_missing(declared, exported, allow))
    ghosts = sorted(ghost_symbols(declared, exported, allow))
    for n in missing:
        print(f"MANQUANT  {n} : déclaré mais non exporté (ni allowlisté)")
    for n in ghosts:
        print(f"FANTOME   {n} : exporté mais non déclaré dans les headers")
    print(f"audit: {len(declared)} déclarées, {len(exported)} exportées, "
          f"{len(missing)} manquantes, {len(ghosts)} fantômes")
    return 1 if missing or ghosts else 0


if __name__ == "__main__":
    sys.exit(main())
