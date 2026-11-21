import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tests"))


def pin():
    values = {}
    with open(os.path.join(ROOT, "wgpu-pin.env"), encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            values[key] = value
    return values


class Version(unittest.TestCase):
    def test_wgpu_version_is_triplet(self):
        self.assertRegex(pin()["WGPU_VERSION"], r"^\d+\.\d+\.\d+$")

    def test_packer_version_is_non_negative_integer(self):
        self.assertRegex(pin()["PACKER_VERSION"], r"^\d+$")

    def test_headers_revision_is_declared(self):
        with open(os.path.join(ROOT, "ffi", "ffi", "webgpu.h"), encoding="utf-8") as handle:
            text = handle.read()
        self.assertRegex(text, r'(?m)^#define WEBGPU_HEADERS_REVISION "[^"]+"$')

    def test_archive_name_matches_pin(self):
        p = pin()
        expected = f"wgpu-{p['WGPU_VERSION']}.{p['PACKER_VERSION']}"
        with open(os.path.join(ROOT, "scripts", "package.sh"), encoding="utf-8") as handle:
            text = handle.read()
        self.assertIn('VERSION="$WGPU_VERSION.$PACKER_VERSION"', text)
        version_line = next(
            line for line in text.splitlines() if line.startswith("VERSION=")
        )
        composed = version_line.split("=", 1)[1].strip().strip('"')
        self.assertEqual(
            composed
            .replace("$WGPU_VERSION", p["WGPU_VERSION"])
            .replace("$PACKER_VERSION", p["PACKER_VERSION"]),
            expected.removeprefix("wgpu-"),
        )
        self.assertIn('ARCHIVE="dist/wgpu-$VERSION-$NAME-$KIND.tar.gz"', text)

    def test_release_does_not_clobber(self):
        with open(os.path.join(ROOT, ".github", "workflows", "release.yml"), encoding="utf-8") as handle:
            text = handle.read()
        self.assertNotIn("--clobber", text)

    def test_release_triggered_on_version_tags(self):
        with open(os.path.join(ROOT, ".github", "workflows", "release.yml"), encoding="utf-8") as handle:
            text = handle.read()
        self.assertIn("startsWith(github.event.workflow_run.head_branch, 'v')", text)


if __name__ == "__main__":
    unittest.main()
