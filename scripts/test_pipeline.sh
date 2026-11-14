#!/usr/bin/env bash
# Test structurel du pipeline (Task 7 du plan).
set -euo pipefail
cd "$(dirname "$0")/.."
fail() { echo "FAIL: $1" >&2; exit 1; }

for f in scripts/sync.sh scripts/build-target.sh scripts/package.sh; do
  [[ -x "$f" ]] || fail "$f absent ou non exécutable"
done

# sync.sh --check doit réussir sur un état propre (pins cohérents)
./scripts/sync.sh --check >/dev/null 2>&1 || fail "sync.sh --check doit réussir sur un état propre"

# build-target.sh rejette une cible inconnue
if ./scripts/build-target.sh cible-inexistante >/dev/null 2>&1; then
  fail "cible inconnue acceptée"
fi

# Les 10 cibles v1 résolvent leur triple.
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

# package.sh rejette un kind inconnu
if ./scripts/package.sh macosArm64 bidule >/dev/null 2>&1; then
  fail "kind inconnu accepté"
fi

# Pins wgpu exacts (contrainte globale du plan : =30.0.1, jamais caret)
grep -q 'version = "=30.0.1"' Cargo.toml || fail "pins wgpu exacts (=30.0.1) attendus dans Cargo.toml"

echo "PIPELINE_STRUCTURE_OK"
