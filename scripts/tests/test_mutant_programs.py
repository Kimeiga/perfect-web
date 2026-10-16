"""`mutant_programs.py`: a mutant of a program's source leaves a program
the checker takes, or one its script names the checker's rule for; and
every program a mutation script mutates is found from the mutants' paths
(track evidence-integrity-2: until 2026-10-15 a list of four, and a mutant
elsewhere, `examples/demo`'s among them, was skipped unread). Read on trees
of their own, with a `pw` that refuses a source holding `BROKEN` with
PW0604 and one holding `SHARED` with PW5001, and a program where a source
`uses helper` and none declares `fn helper` with PW0100; and its controls.
"""

import importlib.util
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
texts = {f: pathlib.Path(f).read_text() for f in sys.argv[2:]}
for f, text in texts.items():
    if "BROKEN" in text:
        print("error: [PW0604] `place` declares 3 arguments and this call passes 1"); errors += 1
    if "SHARED" in text:
        print("error: [PW5001] `Unread` declares a shared cache"); errors += 1
    if "uses helper" in text and not any("fn helper" in t for t in texts.values()):
        print("error: [PW0100] unknown name `helper`"); errors += 1
print(f"pw check: {len(texts)} file(s), {errors} error(s), 0 warning(s)")
"""

STORE = "module store.app\nimport domain.{ Id }\nfn main() {\n    place(a, b, c)\n}\n"


def tree(files: dict[str, str], script: str) -> pathlib.Path:
    """A repository of `files` (path: text) and one mutation script."""
    root = pathlib.Path(tempfile.mkdtemp(prefix="pw-mutant-programs-test-")).resolve()
    (root / "scripts").mkdir()
    (root / "packages/pw-std").mkdir(parents=True)
    (root / "packages/pw-std/List.pw").write_text("module List\n")
    for rel, text in files.items():
        (root / rel).parent.mkdir(parents=True, exist_ok=True)
        (root / rel).write_text(text)
    (root / "scripts/x_mutations.py").write_text(
        "import pathlib\nROOT = pathlib.Path(__file__).resolve().parent.parent\n" + script
    )
    pw = root / "pw"
    pw.write_text(PW)
    pw.chmod(pw.stat().st_mode | stat.S_IEXEC)
    return root


def run(root: pathlib.Path) -> tuple[int, str]:
    module = load()
    module.ROOT = root
    module.WORKERS = 1
    out = pathlib.Path(tempfile.mkdtemp()) / "out.txt"
    saved, sys.argv = sys.argv, ["mutant_programs.py", str(root / "pw")]
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


def one(path: str, anchor: str, replacement: str, checked: str = "{}", what: str = "a mutant") -> str:
    return (
        f"CHECKED = {checked}\n"
        f"MUTANTS = [({what!r}, 'server', ROOT / {path!r}, {anchor!r}, {replacement!r})]\n"
    )


class MutantPrograms(unittest.TestCase):
    def run_on(self, replacement: str, checked: str = "{}", what: str = "a mutant") -> tuple[int, str]:
        root = tree(
            {"examples/domain.pw": "module domain\n", "examples/store/app.pw": STORE},
            one("examples/store/app.pw", "    place(a, b, c)\n", f"    {replacement}\n", checked, what),
        )
        return run(root)

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
        self.assertIn("1 mutants of a program's source, 1 judged, in 1 programs (examples/store)", out)

    def test_a_root_no_list_names_is_found_from_its_path(self):
        # `examples/demo`, whose program is its sources and what they import.
        root = tree(
            {
                "examples/domain.pw": "module domain\n",
                "examples/demo/panel.pw": "module demo.panel\nimport domain.{ Id }\nimport demo.drawer\nshow()\n",
                "examples/demo/drawer.pw": "module demo.drawer\n",
            },
            one("examples/demo/panel.pw", "show()\n", "show() // BROKEN\n"),
        )
        code, out = run(root)
        self.assertEqual(code, 1, out)
        self.assertIn("in 1 programs (examples/demo)", out)
        self.assertIn("leaves a program the checker refuses: [PW0604]", out)
        program, problems = load().Programs(root).program("examples/demo")
        self.assertEqual(
            program,
            ["examples/demo/drawer.pw", "examples/demo/panel.pw", "examples/domain.pw", "packages/pw-std/List.pw"],
        )
        self.assertEqual(problems, [])

    def test_a_librarys_mutant_is_checked_in_each_program_that_imports_it(self):
        # Alone, the library checks without its helper; the store, which
        # uses it, does not.
        files = {
            "examples/domain.pw": "module domain\n",
            "examples/lib/Tools.pw": "module Tools\nfn helper() {}\n",
            "examples/store/app.pw": "module store.app\nimport Tools\nuses helper\nplace(a, b, c)\n",
        }
        script = (
            "MUTANTS = [\n"
            "    ('the helper is gone', 'x', ROOT / 'examples/lib/Tools.pw', 'fn helper() {}\\n', '\\n'),\n"
            "    ('a call reordered', 'x', ROOT / 'examples/store/app.pw', 'place(a, b, c)\\n', 'place(b, a, c)\\n'),\n"
            "]\n"
        )
        code, out = run(tree(files, script))
        self.assertEqual(code, 1, out)
        self.assertIn("the helper is gone: leaves a program the checker refuses: [PW0100]", out)
        self.assertIn("2 mutants of a program's source, 2 judged, in 2 programs (examples/lib, examples/store); 3 checks", out)

    def test_a_fixture_whose_directory_declares_a_module_twice_is_its_own_program(self):
        # Fixtures reuse module names (E2B): each is its own program, its
        # imports from beside its directory, never from another fixture.
        files = {
            "examples/rejected/R-1.pw": "module store.page\nimport store.queries\nx\n",
            "examples/rejected/R-2.pw": "module store.page\n",
            "examples/rejected/R-3.pw": "module store.queries\n// BROKEN\n",
            "examples/accepted/A-1.pw": "module store.queries\n",
        }
        root = tree(files, one("examples/rejected/R-1.pw", "x\n", "y\n"))
        code, out = run(root)
        self.assertEqual(code, 0, out)
        self.assertIn("in 1 programs (examples/rejected/R-1.pw)", out)
        program, problems = load().Programs(root).program("examples/rejected/R-1.pw")
        self.assertEqual(program, ["examples/accepted/A-1.pw", "examples/rejected/R-1.pw", "packages/pw-std/List.pw"])
        self.assertEqual(problems, [])

    def test_an_import_it_cannot_resolve_fails(self):
        for files, why in (
            ({"examples/store/app.pw": "module store.app\nimport Missing\nx\n"}, "imports `Missing`: no source declares it"),
            (
                {
                    "examples/store/app.pw": "module store.app\nimport Twice\nx\n",
                    "examples/a/T.pw": "module Twice\n",
                    "examples/b/T.pw": "module Twice\n",
                },
                "imports `Twice`: 2 sources as near declare it",
            ),
        ):
            with self.subTest(why=why):
                code, out = run(tree(files, one("examples/store/app.pw", "x\n", "y\n")))
                self.assertEqual(code, 1, out)
                self.assertIn(why, out)

    def test_a_gate_that_judged_nothing_or_a_source_outside_every_program_fails(self):
        code, out = run(tree({"examples/store/app.pw": STORE}, "MUTANTS = []\n"))
        self.assertEqual(code, 1, out)
        self.assertIn("no mutant of a `.pw` source found: the gate read nothing", out)
        code, out = run(tree({}, one("examples/gone/app.pw", "x\n", "y\n")))
        self.assertEqual(code, 1, out)
        self.assertIn("its source examples/gone/app.pw is in no program found", out)


if __name__ == "__main__":
    unittest.main()
