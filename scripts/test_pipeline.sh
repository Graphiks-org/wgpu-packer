#!/usr/bin/env bash
# Structural test of the build and packaging pipeline.
set -euo pipefail
cd "$(dirname "$0")/.."
fail() { echo "FAIL: $1" >&2; exit 1; }

for f in scripts/build-target.sh scripts/package.sh; do
  [[ -x "$f" ]] || fail "$f missing or not executable"
done

# build-target.sh rejects an unknown target
if ./scripts/build-target.sh cible-inexistante >/dev/null 2>&1; then
  fail "unknown target accepted"
fi

# The 10 v1 targets resolve their triple.
python3 - <<'EOF'
import subprocess, sys
sys.path.insert(0, "tests")
import matrix
v1 = [t for t in matrix.load()["targets"] if t["status"] == "v1"]
assert len(v1) == 10
for t in v1:
    r = subprocess.run(["./scripts/build-target.sh", "--print-target", t["kotlinTarget"]],
                       capture_output=True, text=True)
    assert r.returncode == 0, t["kotlinTarget"]
    assert r.stdout.strip() == t["triple"], f"{t['kotlinTarget']}: {r.stdout.strip()}"
EOF

# package.sh rejects an unknown kind
if ./scripts/package.sh macosArm64 bidule >/dev/null 2>&1; then
  fail "unknown kind accepted"
fi

# Exact wgpu pins (never caret): Cargo.toml must match WGPU_VERSION in wgpu-pin.env
expected_pin="$(sed -n 's/^WGPU_VERSION=//p' wgpu-pin.env)"
grep -q "version = \"=$expected_pin\"" Cargo.toml || fail "exact wgpu pins (=$expected_pin) expected in Cargo.toml"

echo "PIPELINE_STRUCTURE_OK"
