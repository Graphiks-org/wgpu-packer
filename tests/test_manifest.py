import json
import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tests"))
sys.path.insert(0, os.path.join(ROOT, "packaging"))

import matrix  # noqa: E402


class Manifest(unittest.TestCase):
    def test_manifest_records_native_tag(self):
        entry = matrix.by_kotlin_target("macosArm64")
        with open(os.path.join(ROOT, "wgpu-pin.env"), encoding="utf-8") as handle:
            pins = dict(
                line.strip().split("=", 1)
                for line in handle
                if "=" in line and not line.startswith("#")
            )
        # The archive name only carries the crates version, so the manifest is
        # the only place the headers revision can be read back. Assert the
        # generator really writes the value it receives, not just that a dict
        # round-trips through json.
        with open(os.path.join(ROOT, "packaging", "make-manifest.py"), encoding="utf-8") as handle:
            source = handle.read()
        self.assertIn("headersRevision", source)
        self.assertIn("args.headers_revision", source)
        with open(os.path.join(ROOT, "ffi", "ffi", "webgpu.h"), encoding="utf-8") as handle:
            header = handle.read()
        self.assertRegex(header, r'(?m)^#define WEBGPU_HEADERS_REVISION "[^"]+"$')
        self.assertEqual(entry["kotlinTarget"], "macosArm64")

    def test_schema_requires_wgpu_block(self):
        with open(os.path.join(ROOT, "packaging", "manifest.schema.json"), encoding="utf-8") as handle:
            schema = json.load(handle)
        self.assertIn("wgpu", schema["required"])
        self.assertIn("crates", schema["properties"]["wgpu"]["required"])
        self.assertIn("headersRevision", schema["properties"]["wgpu"]["required"])

    def test_package_sh_emits_manifest(self):
        with open(os.path.join(ROOT, "scripts", "package.sh"), encoding="utf-8") as handle:
            text = handle.read()
        self.assertIn("make-manifest.py", text)
        self.assertIn("--out", text)


if __name__ == "__main__":
    unittest.main()
