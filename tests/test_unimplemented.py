"""Exported unimplemented!() stubs must be documented in the GUIDE
("Unimplemented functions" section) — no silent discrepancy.

The scan covers all of ffi/src: not every stub lives in
unimplemented.rs (historically 5 lived elsewhere, unnoticed by the
GUIDE). A stub = an exported function whose body calls `unimplemented!()`
up to the next exported function in the file.
"""
import os
import re
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

FN_DECL = re.compile(r'pub (?:unsafe )?extern "C" fn (wgpu\w+)\s*\(')


def unimplemented_functions() -> set[str]:
    src_dir = os.path.join(ROOT, "ffi", "src")
    stubs: set[str] = set()
    for root, _dirs, files in os.walk(src_dir):
        for name in sorted(files):
            if not name.endswith(".rs"):
                continue
            src = open(os.path.join(root, name)).read()
            decls = list(FN_DECL.finditer(src))
            for i, m in enumerate(decls):
                body_end = decls[i + 1].start() if i + 1 < len(decls) else len(src)
                if "unimplemented!" in src[m.start() : body_end]:
                    stubs.add(m.group(1))
    return stubs


def guide_documented() -> set[str]:
    guide = open(os.path.join(ROOT, "packaging/GUIDE.md")).read()
    m = re.search(r"## [^\n]*[Uu]nimplemented functions[^\n]*\n(.*?)(?=\n## |\Z)", guide, re.S)  # matches the GUIDE heading
    if not m:
        return set()
    return set(re.findall(r"\b(wgpu\w+)\b", m.group(1)))


class Unimplemented(unittest.TestCase):
    def test_every_stub_is_documented_in_guide(self):
        stubs = unimplemented_functions()
        self.assertTrue(stubs, "no stub found? grep grammar needs review")
        missing = sorted(stubs - guide_documented())
        self.assertEqual(missing, [], f"stubs not documented in the GUIDE: {missing}")

    def test_guide_lists_no_unknown_stub(self):
        self.assertEqual(sorted(guide_documented() - unimplemented_functions()), [])


unittest.main()
