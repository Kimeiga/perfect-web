"""`mutant_programs.py`: a mutant of a program's source leaves a program
the checker takes, or one its script names the checker's rule for. Read on
a tree of its own, with a `pw` that refuses a source holding `BROKEN` with
PW0604 and one holding `SHARED` with PW5001, and its controls.
"""

import importlib.util
import os
import pathlib
import stat
import sys
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load():
    spec = importlib.util.spec_from_file_location("mutant_programs", SCRIPTS / "mutant_programs.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PW = """#!/usr/bin/env python3
import pathlib, sys
errors = 0
for f in sys.argv[2:]:
    text = pathlib.Path(f).read_text()
    if "BROKEN" in text:
        print("error: [PW0604] `place` declares 3 arguments and this call passes 1"); errors += 1
    if "SHARED" in text:
        print("error: [PW5001] `Unread` declares a shared cache"); errors += 1
print(f"pw check: {len(sys.argv) - 2} file(s), {errors} error(s), 0 warning(s)")
"""

SCRIPT = '''import pathlib
ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = ROOT / "examples/store/app.pw"
CHECKED = {CHECKED}
MUTANTS = [
    ("{what}", "server", APP, "    place(a, b, c)\\n", "    {replacement}\\n"),
]
'''


class MutantPrograms(unittest.TestCase):
    def run_on(self, replacement: str, checked: str = "{}", what: str = "a mutant") -> tuple[int, str]:
        root = pathlib.Path(tempfile.mkdtemp(prefix="pw-mutant-programs-test-")).resolve()
        (root / "scripts").mkdir()
        (root / "examples/store").mkdir(parents=True)
        (root / "packages/pw-std").mkdir(parents=True)
        (root / "packages/pw-platform-web").mkdir(parents=True)
        (root / "examples/store/app.pw").write_text("fn main() {\n    place(a, b, c)\n}\n")
        (root / "scripts/x_mutations.py").write_text(
            SCRIPT.replace("{CHECKED}", checked).replace("{what}", what).replace("{replacement}", replacement)
        )
        pw = root / "pw"
        pw.write_text(PW)
        pw.chmod(pw.stat().st_mode | stat.S_IEXEC)
        module = load()
        module.ROOT = root
        module.PROGRAMS = {"store": ["examples/store/*.pw"]}
        module.WORKERS = 1
        out = pathlib.Path(tempfile.mkdtemp()) / "out.txt"
        saved, sys.argv = sys.argv, ["mutant_programs.py", str(pw)]
        try:
            with open(out, "w") as f:
                stdout, sys.stdout = sys.stdout, f
                try:
                    code = module.main()
                finally:
                    sys.stdout = stdout
        finally:
            sys.argv = saved
        return code, out.read_text()

    def test_a_mutant_the_checker_refuses_for_another_reason_fails(self):
        code, out = self.run_on("place(a) // BROKEN")
        self.assertEqual(code, 1, out)
        self.assertIn("leaves a program the checker refuses: [PW0604]", out)

    def test_a_mutant_its_script_names_the_rule_for_passes(self):
        code, out = self.run_on("place(a, b, c) // SHARED", checked='{"a mutant": {"PW5001"}}')
        self.assertEqual(code, 0, out)
        self.assertIn("1 refused by the rule their script names, 0 failing", out)

    def test_a_named_rule_the_mutant_does_not_meet_fails(self):
        code, out = self.run_on("place(a, b, c) // BROKEN", checked='{"a mutant": {"PW5001"}}')
        self.assertEqual(code, 1, out)
        code, out = self.run_on("place(b, a, c)", checked='{"a mutant": {"PW5001"}}')
        self.assertEqual(code, 1, out)
        self.assertIn("names ['PW5001'], and the program it leaves checks", out)

    def test_the_control_a_mutant_that_leaves_a_program_passes(self):
        code, out = self.run_on("place(b, a, c)")
        self.assertEqual(code, 0, out)
        self.assertIn("1 mutants of a program's source, 0 refused", out)


if __name__ == "__main__":
    unittest.main()
