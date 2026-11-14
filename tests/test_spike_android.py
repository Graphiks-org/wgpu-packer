import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tests"))

import matrix  # noqa: E402

SPIKE = os.path.join(ROOT, "docs", "spikes", "android.md")


class AndroidSpike(unittest.TestCase):
    def test_spike_exists_and_has_verdict(self):
        self.assertTrue(os.path.isfile(SPIKE), "docs/spikes/android.md absent")
        with open(SPIKE, encoding="utf-8") as handle:
            text = handle.read()
        self.assertRegex(
            text,
            r"(?m)^## Verdict$",
            "le spike doit contenir une section '## Verdict'",
        )

    def test_matrix_matches_verdict(self):
        with open(SPIKE, encoding="utf-8") as handle:
            text = handle.read()
        verdict_line = next(
            (line for line in text.splitlines() if line.startswith("VERDICT:")),
            None,
        )
        self.assertIsNotNone(verdict_line, "le spike doit porter une ligne 'VERDICT: ...'")
        verdict = verdict_line.split(":", 1)[1].strip()
        self.assertIn(verdict, {"supported", "blocked"})
        entries = {t["kotlinTarget"]: t for t in matrix.load()["targets"]}
        for name in ("androidNativeArm64", "androidNativeX64"):
            if verdict == "supported":
                self.assertEqual(entries[name]["status"], "v1", name)
            else:
                self.assertNotEqual(entries[name]["status"], "v1", name)


if __name__ == "__main__":
    unittest.main()
