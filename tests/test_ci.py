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

    def test_release_workflow_gated_on_tag_and_build(self):
        # Publication lives in its own workflow now, triggered by the `build`
        # run completing: artifacts are run-scoped, so a push-on-tag workflow
        # cannot see them. The tag is read from workflow_run.head_branch.
        with open(".github/workflows/release.yml") as f:
            rel = yaml.safe_load(f)
        # pyyaml parse la clé `on:` en booléen True (YAML 1.1)
        trigger = rel.get("on", rel.get(True))
        self.assertEqual(trigger["workflow_run"]["workflows"], ["build"])
        self.assertEqual(trigger["workflow_run"]["types"], ["completed"])
        guard = rel["jobs"]["release"]["if"]
        self.assertIn("conclusion == 'success'", guard)
        # Sans ce test, un workflow_dispatch lancé sur une branche v... publierait.
        self.assertIn("github.event.workflow_run.event == 'push'", guard)
        self.assertIn("startsWith(github.event.workflow_run.head_branch, 'v')", guard)
        # Le téléchargement doit cibler le run déclencheur, et le checkout sa révision.
        # On inspecte la structure parsée (str() d'un dict YAML utilise des quotes
        # simples, donc `run-id: ...` n'y apparaîtrait jamais littéralement).
        steps = rel["jobs"]["release"]["steps"]
        downloads = [s for s in steps if s.get("uses", "").startswith("actions/download-artifact")]
        self.assertEqual(len(downloads), 1)
        self.assertEqual(downloads[0]["with"]["run-id"], "${{ github.event.workflow_run.id }}")
        checkouts = [s for s in steps if s.get("uses", "").startswith("actions/checkout")]
        self.assertEqual(len(checkouts), 1)
        self.assertEqual(checkouts[0]["with"]["ref"], "${{ github.event.workflow_run.head_branch }}")
        # `build` must still run on tag pushes, or workflow_run never fires.
        build_trigger = self.wf.get("on", self.wf.get(True))
        self.assertIn("v*", build_trigger["push"]["tags"])

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
