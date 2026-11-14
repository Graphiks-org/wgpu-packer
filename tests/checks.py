#!/usr/bin/env python3
"""Audit header <-> exported symbols (spec §4.5).

Every function declared in webgpu.h/wgpu.h must be exported by the
library (or allowlisted in tests/unimplemented.txt), and every exported
wgpu* symbol must be declared. Exit 1 on any discrepancy.
"""
import argparse
import pathlib
import re
import subprocess
import sys

DECL = re.compile(r"^\s*(?:WGPU_EXPORT\s+)?[A-Za-z_][\w \t\*&]*?\b(wgpu[A-Z]\w*)\s*\(", re.M)

# Always considered exported: debug-only probe, absent from release builds.
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
    """Declared but neither exported nor allowlisted."""
    return (declared - allow) - exported


def ghost_symbols(declared: set[str], exported: set[str], allow: set[str]) -> set[str]:
    """wgpu* symbols exported but declared nowhere (outside the allowlist).

    Only non-mangled symbols prefixed with wgpu count: internal Rust
    symbols (RNv…, ZN…) are part of the std linked into the static library.
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
    """Parse the output of `dumpbin /exports` (MSVC dll)."""
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
    # `nm` exits non-zero on some toolchains when an archive member has no
    # symbols (e.g. the runner's nm on empty Rust CGUs), and warns about the
    # `__.SYMDEF` table member. Neither affects the exported symbols we parse
    # from stdout, so the exit status is not checked; a missing library still
    # surfaces as an empty stdout and a zero-symbol audit failure.
    raw = subprocess.run(["nm", "-gU", str(lib)], capture_output=True, text=True).stdout
    syms = set()
    for line in raw.splitlines():
        parts = line.split()
        if len(parts) >= 3 and parts[1] in ("T", "W"):
            syms.add(parts[2].lstrip("_"))  # macOS: _ prefix
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
        print(f"MISSING   {n}: declared but not exported (nor allowlisted)")
    for n in ghosts:
        print(f"GHOST     {n}: exported but not declared in the headers")
    print(f"audit: {len(declared)} declared, {len(exported)} exported, "
          f"{len(missing)} missing, {len(ghosts)} ghosts")
    return 1 if missing or ghosts else 0


if __name__ == "__main__":
    sys.exit(main())
