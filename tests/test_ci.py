import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(__file__))
import matrix

try:
    import yaml
except ImportError:
    yaml = None


@unittest.skipIf(yaml is None, "pyyaml absent : pip install pyyaml")
class CI(unittest.TestCase):
    def setUp(self):
        with open(".github/workflows/build.yml") as f:
            self.wf = yaml.safe_load(f)

    def test_matrix_covers_all_ten_targets(self):
        build = self.wf["jobs"]["build"]
        entries = {inc["target"] for inc in build["strategy"]["matrix"]["include"]}
        expected = {t["kotlinTarget"] for t in matrix.load()["targets"] if t["status"] == "v1"}
        self.assertEqual(entries, expected)

    def test_oracles_on_three_hosts_both_profiles(self):
        for host in ("oracle_macos", "oracle_linux", "oracle_windows"):
            self.assertIn(host, self.wf["jobs"])
            runs = str(self.wf["jobs"][host]["steps"])
            self.assertIn("cargo test ", runs)
            self.assertIn("--release", runs)

    def test_release_job_gated_on_tag_and_build(self):
        rel = self.wf["jobs"]["release"]
        self.assertIn("build", rel["needs"])
        self.assertIn("kotlin_smoke", rel["needs"])
        # pyyaml parse la clé `on:` en booléen True (YAML 1.1)
        trigger = self.wf.get("on", self.wf.get(True))
        self.assertIn("v*", trigger["push"]["tags"])

    def test_kotlin_smoke_consumes_build_artifact(self):
        ks = self.wf["jobs"]["kotlin_smoke"]
        needs = ks["needs"]
        needs = needs if isinstance(needs, list) else [needs]  # YAML : singleton = chaîne
        self.assertEqual(needs, ["build"])

    def test_cross_link_test_in_build_job(self):
        runs = str(self.wf["jobs"]["build"]["steps"])
        self.assertIn("cross-link-test", runs)
        script = open("scripts/cross-link-test.sh").read()
        self.assertIn("headers_compile.c", script)  # TU §4.5 embarquée
        self.assertIn("link_main.c", script)


unittest.main()
