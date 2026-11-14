import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(__file__))
import matrix

REQUIRED = {"kotlinTarget", "triple", "os", "arch", "runner", "toolchain", "backends", "status"}
WGPU_SPECIFIC = {"linkLibs", "deployEnv"}
DROPPED = {"tvosArm64", "tvosSimulatorArm64"}


class Matrix(unittest.TestCase):
    def test_schema_version(self):
        self.assertEqual(matrix.load()["schemaVersion"], 1)

    def test_ten_v1_targets(self):
        v1 = [t for t in matrix.load()["targets"] if t["status"] == "v1"]
        self.assertEqual(len(v1), 10)

    def test_android_targets_are_static_only(self):
        # docs/spikes/android.md only proves the static mode (see "Limitations").
        for t in matrix.load()["targets"]:
            if t["os"] == "android":
                self.assertEqual(t["linkages"], ["static"], t["kotlinTarget"])

    def test_schema(self):
        for t in matrix.load()["targets"]:
            self.assertTrue(REQUIRED <= set(t), f"missing fields in {t['kotlinTarget']}")

    def test_unique_kotlin_targets_and_triples(self):
        m = matrix.load()["targets"]
        names = [t["kotlinTarget"] for t in m]
        triples = [t["triple"] for t in m]
        self.assertEqual(len(names), len(set(names)), "duplicate kotlinTargets")
        self.assertEqual(len(triples), len(set(triples)), "duplicate triples")

    def test_runner_coverage(self):
        runners = {t["runner"] for t in matrix.load()["targets"]}
        self.assertTrue(runners <= {"macos-15", "ubuntu-24.04", "windows-2022"})

    def test_apple_have_deploy_env(self):
        for t in matrix.load()["targets"]:
            if t["os"] in ("macos", "ios", "ios-sim"):
                self.assertTrue(t.get("deployEnv"), f"{t['kotlinTarget']} must pin a deployment")

    def test_wgpu_specific_fields_preserved(self):
        for t in matrix.load()["targets"]:
            self.assertTrue(WGPU_SPECIFIC <= set(t), f"{t['kotlinTarget']} loses {WGPU_SPECIFIC - set(t)}")

    def test_linkages_are_valid(self):
        for t in matrix.load()["targets"]:
            self.assertTrue(t["linkages"], f"{t['kotlinTarget']} without linkage")
            self.assertTrue(set(t["linkages"]) <= {"static", "shared"})

    def test_dropped_targets_are_documented(self):
        dropped = {t["kotlinTarget"] for t in matrix.load()["targets"] if t["status"] == "dropped"}
        self.assertEqual(dropped, DROPPED)

    def test_null_backend_is_mandatory(self):
        for t in matrix.load()["targets"]:
            self.assertIn("null", t["backends"], t["kotlinTarget"])


unittest.main()
