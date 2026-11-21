#!/usr/bin/env python3
"""Build a manifest.json for an installed wgpu-packer tree (stdlib only)."""
import argparse
import datetime
import hashlib
import json
import pathlib
import sys


def portable_path(path):
    """Return a path with forward slashes, whatever the host or path flavour.

    A manifest is a shipped, portable contract: str(PureWindowsPath(...)) yields
    backslashes, which a consumer on any platform must never see and which would
    not match the POSIX member names in the archive.
    """
    return str(path).replace("\\", "/")


def sha256_of(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--install-dir", required=True)
    parser.add_argument("--kotlin-target", required=True)
    parser.add_argument("--entry-json", required=True, help="one matrix entry as JSON")
    parser.add_argument("--wgpu-version", required=True, help="X.Y.Z of wgpu-core/hal/naga")
    parser.add_argument("--headers-revision", required=True, help="revision of the vendored webgpu.h")
    parser.add_argument("--linkage", required=True, choices=["static", "shared"])
    parser.add_argument("--packer-version", default="0")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    install = pathlib.Path(args.install_dir).resolve()
    entry = json.loads(args.entry_json)

    # Enumerate the staged tree instead of allowlisting glob patterns. A glob
    # allowlist silently drops any directory nobody thought of, which is how
    # bin/webgpu_dawn.dll went missing from Windows archives while the manifest
    # still claimed to be exhaustive. Files the packaging step adds itself are
    # the only exclusions.
    excluded_top_level = {".wgpu-packer-install", "manifest.json"}
    payload = {}
    for path in sorted(install.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(install)
        if relative.parts[0] in excluded_top_level:
            continue
        payload[portable_path(relative)] = path
    artifacts = []
    for relative, path in sorted(payload.items()):
        artifacts.append({"path": relative, "sha256": sha256_of(path), "size": path.stat().st_size})
    if not artifacts:
        print("ERROR: no headers or libraries found in install dir", file=sys.stderr)
        return 1

    # wgpu is built by Cargo, not CMake: no compiler/cmake provenance to record.
    compiler = {}
    cmake = {"buildType": "Release"}

    manifest = {
        "schemaVersion": 1,
        "packerVersion": args.packer_version,
        # The archive name carries only the crates version; headersRevision
        # is the only record of which webgpu.h the archive was built against.
        "wgpu": {"crates": args.wgpu_version, "headersRevision": args.headers_revision},
        "target": {
            "kotlinTarget": args.kotlin_target,
            "triple": entry["triple"],
            "os": entry["os"],
            "arch": entry["arch"],
        },
        "linkage": args.linkage,
        "backends": entry["backends"],
        "compiler": compiler,
        "cmake": cmake,
        "artifacts": artifacts,
        "createdAt": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }
    pathlib.Path(args.out).write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"manifest: {args.out} ({len(artifacts)} artifacts)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
