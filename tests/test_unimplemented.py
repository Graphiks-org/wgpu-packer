"""Les stubs unimplemented!() exportés doivent être documentés dans le GUIDE
(revue finale, finding 4) — aucun écart silencieux (spec §4.5).

Le scan couvre tout ffi/src : les stubs ne vivent pas tous dans
unimplemented.rs (historiquement 5 vivaient ailleurs, passés inaperçus du
GUIDE). Un stub = fonction exportée dont le corps appelle `unimplemented!()`
jusqu'à la fonction exportée suivante du fichier.
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
    m = re.search(r"## [^\n]*[Nn]on implément[ée]*es?[^\n]*\n(.*?)(?=\n## |\Z)", guide, re.S)
    if not m:
        return set()
    return set(re.findall(r"\b(wgpu\w+)\b", m.group(1)))


class Unimplemented(unittest.TestCase):
    def test_every_stub_is_documented_in_guide(self):
        stubs = unimplemented_functions()
        self.assertTrue(stubs, "aucun stub trouvé ? grammaire du grep à revoir")
        missing = sorted(stubs - guide_documented())
        self.assertEqual(missing, [], f"stubs non documentés dans le GUIDE : {missing}")

    def test_guide_lists_no_unknown_stub(self):
        self.assertEqual(sorted(guide_documented() - unimplemented_functions()), [])


unittest.main()
