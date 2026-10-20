"""`scripts/mutant_incremental.py`: a mutation script's builds compile
incrementally where the run asks, and the incremental caches are bounded
(track ci-diagnostics, 2026-10-20).

Each mutant undoes one line, so the build after it rebuilds a crate whose
other lines stand; incrementally it redoes what the line reaches: 21 to 23 s
against 33 to 37 for a one-line `pw-core` mutant on the PC. `verify.yml` had
`CARGO_INCREMENTAL=0` for every job, and 104 recipes put the same before their
mutation script; `PW_MUTANT_INCREMENTAL=1`, set on the `Recipes` step, turns it
on for a mutation script and what it starts, and nothing else.
"""

from __future__ import annotations

import importlib.util
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


incremental = load("mutant_incremental")
recipes = load("ci_recipes")


def fill(directory: pathlib.Path, nbytes: int) -> None:
    """A directory holding `nbytes` in two files, as a cache does."""
    directory.mkdir(parents=True)
    (directory / "a").write_bytes(b"i" * (nbytes // 2))
    (directory / "b").write_bytes(b"i" * (nbytes - nbytes // 2))


class Asked(unittest.TestCase):
    def test_the_run_asks_with_one(self):
        self.assertTrue(incremental.asked({"PW_MUTANT_INCREMENTAL": "1"}))
        for said in (None, "", "0", "true", "yes", "2"):
            env = {} if said is None else {"PW_MUTANT_INCREMENTAL": said}
            self.assertFalse(incremental.asked(env), said)

    def test_asked_it_turns_incremental_on_over_a_recipes_own_zero(self):
        # A recipe's line, `CARGO_INCREMENTAL=0 python3 scripts/..._mutations.py`.
        with mock.patch.dict(os.environ, {"CARGO_INCREMENTAL": "0", "PW_MUTANT_INCREMENTAL": "1"}):
            incremental.start()
            self.assertEqual(os.environ["CARGO_INCREMENTAL"], "1")

    def test_not_asked_it_leaves_the_environment_as_the_recipe_has_it(self):
        with mock.patch.dict(os.environ, {"CARGO_INCREMENTAL": "0"}):
            os.environ.pop("PW_MUTANT_INCREMENTAL", None)
            incremental.start()
            self.assertEqual(os.environ["CARGO_INCREMENTAL"], "0")
        with mock.patch.dict(os.environ, {"PW_MUTANT_INCREMENTAL": "0"}):
            os.environ.pop("CARGO_INCREMENTAL", None)
            incremental.start()
            self.assertNotIn("CARGO_INCREMENTAL", os.environ)

    def test_a_mutation_script_and_what_it_starts_build_incrementally_when_asked(self):
        # The path a recipe takes: a script that imports `mutation_baseline` first
        # thing in `main()`, started with its recipe's line's zero.
        tmp = pathlib.Path(tempfile.mkdtemp(prefix="pw-incremental-"))
        self.addCleanup(shutil.rmtree, tmp, True)
        (tmp / "toy_mutations.py").write_text(
            "import os, subprocess, sys\n"
            f"sys.path.insert(0, {str(SCRIPTS)!r})\n"
            "def main():\n"
            "    import mutation_baseline  # keeps what each command says\n"
            "    child = subprocess.run(['sh', '-c', 'echo $CARGO_INCREMENTAL'], capture_output=True, text=True)\n"
            "    print('script', os.environ.get('CARGO_INCREMENTAL'), 'child', child.stdout.strip())\n"
            "main()\n"
        )

        def run(asked: bool) -> str:
            env = {k: v for k, v in os.environ.items() if k not in ("PW_MUTANT_INCREMENTAL", "CARGO_INCREMENTAL")}
            env["CARGO_INCREMENTAL"] = "0"
            if asked:
                env["PW_MUTANT_INCREMENTAL"] = "1"
            out = subprocess.run([sys.executable, str(tmp / "toy_mutations.py")], cwd=tmp, env=env, capture_output=True, text=True, timeout=60)
            self.assertEqual(out.returncode, 0, out.stdout + out.stderr)
            return out.stdout.splitlines()[0]

        self.assertEqual(run(True), "script 1 child 1")
        # The control: not asked, the recipe's zero stands for both.
        self.assertEqual(run(False), "script 0 child 0")


class Bounded(unittest.TestCase):
    def setUp(self):
        self.target = pathlib.Path(tempfile.mkdtemp(prefix="pw-incremental-target-"))
        self.addCleanup(shutil.rmtree, self.target, True)
        self.on = mock.patch.dict(os.environ, {"CARGO_INCREMENTAL": "1"})
        self.on.start()
        self.addCleanup(self.on.stop)

    def caches(self):
        fill(self.target / "debug" / "incremental" / "pw_core-abc", 3000)
        fill(self.target / "wasm32-unknown-unknown" / "debug" / "incremental" / "pw_render-def", 3000)

    def test_both_a_profiles_and_a_triples_caches_are_found(self):
        self.caches()
        (self.target / "debug" / "deps").mkdir()
        self.assertEqual(
            [str(p.relative_to(self.target)) for p in incremental.caches(self.target)],
            ["debug/incremental", "wasm32-unknown-unknown/debug/incremental"],
        )
        self.assertEqual(incremental.tree_bytes(self.target / "debug" / "incremental"), 3000)

    def test_past_the_limit_every_cache_is_removed_and_the_rest_stays(self):
        self.caches()
        (self.target / "debug" / "deps").mkdir()
        (self.target / "debug" / "deps" / "pw_core-abc.rlib").write_bytes(b"r" * 100)
        removed = incremental.bounded(["cargo", "test"], target=self.target, limit=5000)
        self.assertEqual(removed, 6000)
        self.assertEqual(incremental.caches(self.target), [])
        self.assertTrue((self.target / "debug" / "deps" / "pw_core-abc.rlib").exists())

    def test_within_the_limit_nothing_is_removed(self):
        self.caches()
        self.assertEqual(incremental.bounded(["cargo", "test"], target=self.target, limit=6000), 0)
        self.assertEqual(len(incremental.caches(self.target)), 2)

    def test_a_command_that_is_no_cargos_removes_nothing(self):
        self.caches()
        for command in (["python3", "x.py"], "bash run.sh", ["pnpm", "exec", "playwright", "test"], []):
            self.assertEqual(incremental.bounded(command, target=self.target, limit=1), 0, command)
        self.assertEqual(len(incremental.caches(self.target)), 2)
        # And cargo's, as a line or by its path, is.
        self.assertEqual(incremental.bounded("cargo build --quiet", target=self.target, limit=1), 6000)
        self.caches()
        self.assertEqual(incremental.bounded(["/home/u/.cargo/bin/cargo", "test"], target=self.target, limit=1), 6000)

    def test_with_incremental_off_nothing_is_removed(self):
        self.caches()
        with mock.patch.dict(os.environ, {"CARGO_INCREMENTAL": "0"}):
            self.assertEqual(incremental.bounded(["cargo", "test"], target=self.target, limit=1), 0)
        self.assertEqual(len(incremental.caches(self.target)), 2)

    def test_the_limit_is_the_environments_where_it_names_one(self):
        with mock.patch.dict(os.environ, {"PW_INCREMENTAL_MAX_MIB": "7"}):
            self.assertEqual(incremental.limit_bytes(), 7 * 1024 * 1024)
        with mock.patch.dict(os.environ, {"PW_INCREMENTAL_MAX_MIB": "lots"}):
            self.assertEqual(incremental.limit_bytes(), incremental.MAX_MIB * 1024 * 1024)
        self.assertEqual(incremental.MAX_MIB, 6 * 1024)

    def test_a_mutation_scripts_cargo_command_removes_the_caches_past_the_limit(self):
        # Through the hook `mutation_baseline` puts on `Popen.communicate`: a
        # command ends, and the caches the script's builds left are looked at.
        baseline = load("mutation_baseline")
        self.assertTrue(getattr(subprocess.Popen.communicate, "keeps", False))
        self.caches()
        env = {"CARGO_TARGET_DIR": str(self.target), "PW_INCREMENTAL_MAX_MIB": "0"}
        with mock.patch.dict(os.environ, env):
            baseline.heard.clear()
            # A command that is cargo's by its first word, and ends at once.
            fake = pathlib.Path(tempfile.mkdtemp(prefix="pw-fake-cargo-"))
            self.addCleanup(shutil.rmtree, fake, True)
            cargo = fake / "cargo"
            cargo.write_text("#!/bin/sh\nexit 0\n")
            cargo.chmod(0o755)
            subprocess.run([str(cargo), "build"], capture_output=True)
            self.assertEqual(incremental.caches(self.target), [])


class Pruned(unittest.TestCase):
    def test_every_cache_is_removed_between_recipes_a_triples_among_them(self):
        target = pathlib.Path(tempfile.mkdtemp(prefix="pw-prune-"))
        self.addCleanup(shutil.rmtree, target, True)
        fill(target / "debug" / "incremental" / "pw_core-abc", 100)
        fill(target / "wasm32-unknown-unknown" / "debug" / "incremental" / "pw_render-def", 100)
        (target / "debug" / "deps").mkdir()
        keep = target / "debug" / "deps" / "libpw_core-0123456789abcdef.rlib"
        keep.write_text("library")
        recipes.prune(target)
        self.assertEqual(incremental.caches(target), [])
        self.assertTrue(keep.exists())


class Workflow(unittest.TestCase):
    """`verify.yml` asks for it on the step that runs the recipes, and nowhere
    else; the workflow's own zero stands for every other build. (Read as
    text, as the other tests read it: a runner's Python need not have PyYAML.)"""

    @classmethod
    def lines(cls):
        return (ROOT / ".github/workflows/verify.yml").read_text().splitlines()

    @classmethod
    def step(cls, name):
        """The lines of the step `- name: <name>` (to the next step)."""
        lines = cls.lines()
        start = next(i for i, l in enumerate(lines) if l == f"      - name: {name}")
        end = next(
            (
                i
                for i in range(start + 1, len(lines))
                if lines[i].startswith("      - ") or (lines[i].strip() and not lines[i].startswith("        "))
            ),
            len(lines),
        )
        return lines[start:end]

    def test_the_recipes_step_asks_and_no_other_does(self):
        asking = [i for i, l in enumerate(self.lines()) if re.match(r"^\s*PW_MUTANT_INCREMENTAL:", l)]
        self.assertEqual(len(asking), 1, asking)
        self.assertIn('          PW_MUTANT_INCREMENTAL: "1"', self.step("Recipes"))
        # Under that step's `env:`, not the workflow's or a job's, where every step would have it.
        lines = self.step("Recipes")
        env = next(i for i, l in enumerate(lines) if l == "        env:")
        run = next(i for i, l in enumerate(lines) if l == "        run: |")
        self.assertLess(env, lines.index('          PW_MUTANT_INCREMENTAL: "1"'))
        self.assertLess(lines.index('          PW_MUTANT_INCREMENTAL: "1"'), run)

    def test_the_workflows_own_zero_stands_for_every_other_build(self):
        lines = self.lines()
        top = lines.index("env:")
        block = lines[top + 1 : next((i for i in range(top + 1, len(lines)) if lines[i] and not lines[i].startswith(" ")), len(lines))]
        self.assertIn('  CARGO_INCREMENTAL: "0"', block)
        self.assertEqual([l for l in lines if re.match(r"^\s*CARGO_INCREMENTAL:", l)], ['  CARGO_INCREMENTAL: "0"'])

    def test_no_recipe_line_is_edited_for_it(self):
        # The recipes' `CARGO_INCREMENTAL=0` prefixes stand: the variable above
        # overrides them from inside the mutation script, and no recipe is
        # planned for a changed line.
        prefixed = 0
        for path in [ROOT / "justfile", *sorted((ROOT / "just").glob("*.just"))]:
            prefixed += len(re.findall(r"CARGO_INCREMENTAL=0 python3 scripts/\w+_mutations\.py", path.read_text()))
        self.assertGreaterEqual(prefixed, 100)


if __name__ == "__main__":
    unittest.main()
