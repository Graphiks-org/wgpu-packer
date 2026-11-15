import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tests"))

import matrix  # noqa: E402


class AndroidTargets(unittest.TestCase):
    """The Android targets are validated: static linkage only, on arm64."""

    NAMES = ("androidNativeArm64", "androidNativeX64")

    def test_targets_are_v1(self):
        entries = {t["kotlinTarget"]: t for t in matrix.load()["targets"]}
        for name in self.NAMES:
            self.assertEqual(entries[name]["status"], "v1", name)

    def test_static_linkage_only(self):
        entries = {t["kotlinTarget"]: t for t in matrix.load()["targets"]}
        for name in self.NAMES:
            self.assertEqual(entries[name]["linkages"], ["static"], name)

    def test_link_flags_cover_the_rust_runtime(self):
        entries = {t["kotlinTarget"]: t for t in matrix.load()["targets"]}
        for name in self.NAMES:
            libs = entries[name]["linkLibs"].split()
            for required in ("-llog", "-landroid", "-lm"):
                self.assertIn(required, libs, f"{name}: {required}")


if __name__ == "__main__":
    unittest.main()
