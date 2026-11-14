import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(__file__))
import matrix

REQUIRED = {"name", "rust_target", "platform", "os_runner", "backends", "link_libs", "deploy_env"}


class Matrix(unittest.TestCase):
    def test_ten_targets(self):
        self.assertEqual(len(matrix.load()["targets"]), 10)

    def test_schema(self):
        for t in matrix.load()["targets"]:
            self.assertTrue(REQUIRED <= set(t), f"champs manquants dans {t['name']}")

    def test_unique_names_and_targets(self):
        m = matrix.load()["targets"]
        names = [t["name"] for t in m]
        triples = [t["rust_target"] for t in m]
        self.assertEqual(len(names), len(set(names)), "noms de cibles dupliqués")
        self.assertEqual(len(triples), len(set(triples)), "triples rust dupliqués")

    def test_runner_coverage(self):
        runners = {t["os_runner"] for t in matrix.load()["targets"]}
        self.assertTrue(runners <= {"macos-14", "ubuntu-24.04", "windows-2022"})

    def test_apple_have_deploy_env(self):
        for t in matrix.load()["targets"]:
            if t["platform"] in ("macos", "ios", "ios-sim"):
                self.assertTrue(t["deploy_env"], f"{t['name']} doit épingler un déploiement")

    def test_static_and_shared_flags(self):
        for t in matrix.load()["targets"]:
            self.assertIsInstance(t["static_ok"], bool)
            self.assertIsInstance(t["shared_ok"], bool)


unittest.main()
