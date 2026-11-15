import os
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tests"))


def run_check(lib: str, headers: str) -> int:
    return subprocess.call(
        [sys.executable, os.path.join(ROOT, "tests", "checks.py"),
         "--lib", lib, "--headers", headers],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


class Checks(unittest.TestCase):
    def test_parse_extracts_declarations_only(self):
        import checks
        decls = checks.parse_declarations(textwrap.dedent("""
            WGPU_EXPORT void wgpuInstanceRelease(WGPUInstance instance);
            WGPU_EXPORT uint32_t wgpuDebugTriggerPanic(void);
            static inline void wgpuHelpersNotExported(void) {}
        """))
        self.assertIn("wgpuInstanceRelease", decls)
        self.assertIn("wgpuDebugTriggerPanic", decls)
        self.assertNotIn("wgpuHelpersNotExported", decls)

    def test_parse_ignores_comments_and_proc_typedefs(self):
        import checks
        decls = checks.parse_declarations(textwrap.dedent("""
            * > @copydoc wgpuAdapterInfoFreeMembers
            // wgpuSomeCommentedFn(void);
            typedef void (*WGPUProcAdapterInfoFreeMembers)(WGPUAdapterInfo adapterInfo);
        """))
        self.assertNotIn("wgpuAdapterInfoFreeMembers", decls)
        self.assertNotIn("wgpuSomeCommentedFn", decls)

    def test_missing_symbol_fails(self):
        # A header declares wgpuTotoTruc; a lib without the symbol must fail.
        with tempfile.TemporaryDirectory() as d:
            (open(os.path.join(d, "webgpu.h"), "w")
             .write("WGPU_EXPORT void wgpuTotoTruc(void);\n"))
            import checks
            decls = checks.parse_declarations("WGPU_EXPORT void wgpuTotoTruc(void);")
            self.assertEqual(decls, {"wgpuTotoTruc"})
            self.assertEqual(checks.exported_symbols_missing(decls, set(), set()), {"wgpuTotoTruc"})

    def test_allowlist_silences_missing(self):
        import checks
        decls = {"wgpuTotoTruc"}
        self.assertEqual(checks.exported_symbols_missing(decls, set(), {"wgpuTotoTruc"}), set())

    def test_ghost_symbol_detected(self):
        import checks
        ghosts = checks.ghost_symbols(
            {"wgpuReal"},
            {"wgpuReal", "wgpuFantome", "ZN4core3foo3barE", "RNvCs1_5wgpu_packer1x"},
            set(),
        )
        self.assertEqual(ghosts, {"wgpuFantome"})

    def test_dumpbin_parsing(self):
        import checks
        sample = """
          Ordinal    Hint  RVA      Name

                  1    0 001A2B3C wgpuCreateInstance
                  2    1 001A2C40 wgpuInstanceRelease
        """
        self.assertEqual(
            checks.parse_dumpbin(sample),
            {"wgpuCreateInstance", "wgpuInstanceRelease"},
        )


unittest.main()
