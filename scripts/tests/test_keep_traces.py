"""`scripts/keep_traces.py` and the places that call it: a failing browser run's
output is kept where the next run does not clear it, bounded in size, and the
upload steps of `verify.yml` name it (track ci-diagnostics, 2026-10-19).

Train-10's verify lost the trace of `e10-browser`'s second Chromium run: the
suite ran a third time, and a mutation control after it, and each cleared
`test-results/`. `scripts/run_suite.sh` (`test_run_suite.py`) and a red
mutation baseline (`test_mutation_baseline.py`) keep what they would lose;
this file holds what they keep it with, and what they name it by.
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

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


keep_traces = load("keep_traces")


def failed_run(out: pathlib.Path, **tests: int) -> pathlib.Path:
    """A failed run's `test-results/`: a directory per failed test with a
    trace of the given size, and the run's `.last-run.json`."""
    out.mkdir(parents=True)
    (out / ".last-run.json").write_text('{"status": "failed"}\n')
    for name, nbytes in tests.items():
        (out / name).mkdir()
        (out / name / "trace.zip").write_bytes(b"t" * nbytes)
    return out


class Keeping(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="pw-keep-traces-"))
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.root = self.tmp / "kept-traces"

    def test_each_entry_of_a_failed_run_is_kept(self) -> None:
        source = failed_run(self.tmp / "test-results", feed_webkit=3000, store_chromium=2000)
        dest = keep_traces.destination(self.root, "e10-browser", "T1", "run-2")
        kept = keep_traces.keep(source, dest, self.root, budget=10**6)
        self.assertEqual(dest, self.root / "e10-browser" / "T1" / "run-2")
        self.assertEqual(sorted(n for n, _ in kept.kept), [".last-run.json", "feed_webkit", "store_chromium"])
        self.assertEqual(kept.skipped, [])
        self.assertEqual((dest / "feed_webkit" / "trace.zip").read_bytes(), b"t" * 3000)
        self.assertFalse((dest / keep_traces.NOT_KEPT).exists())
        # It is a copy: the next run clears the source, and not what was kept.
        shutil.rmtree(source)
        self.assertTrue((dest / "store_chromium" / "trace.zip").exists())

    def test_what_does_not_fit_the_budget_is_named_and_not_copied(self) -> None:
        source = failed_run(self.tmp / "test-results", a_first=3000, b_second=3000, c_third=500)
        dest = keep_traces.destination(self.root, "e14-x", "T1", "run-1")
        # Room for the json and the first trace and the small one, in name
        # order: the second trace takes the root past its budget.
        kept = keep_traces.keep(source, dest, self.root, budget=4000)
        self.assertEqual([n for n, _ in kept.kept], [".last-run.json", "a_first", "c_third"])
        self.assertEqual([n for n, _ in kept.skipped], ["b_second"])
        self.assertTrue((dest / "a_first" / "trace.zip").exists())
        self.assertFalse((dest / "b_second").exists())
        listing = (dest / keep_traces.NOT_KEPT).read_text()
        self.assertIn("b_second\t3000 bytes", listing)
        self.assertIn("4000 bytes", listing)
        self.assertIn("1 (2.9 KiB) were over the 3.9 KiB", kept.says())

    def test_the_budget_is_the_roots_whatever_was_kept_there_before(self) -> None:
        # Two runs, one root: the second finds what the first took.
        first = failed_run(self.tmp / "one", only=3000)
        second = failed_run(self.tmp / "two", only=3000)
        a = keep_traces.keep(first, keep_traces.destination(self.root, "e14-x", "T1", "run-1"), self.root, budget=4000)
        b = keep_traces.keep(second, keep_traces.destination(self.root, "e14-x", "T1", "run-2"), self.root, budget=4000)
        self.assertEqual([n for n, _ in a.kept], [".last-run.json", "only"])
        self.assertEqual([n for n, _ in b.skipped], ["only"])
        # And the root never holds more than the budget, but for the note.
        held = keep_traces.tree_bytes(self.root)
        note = keep_traces.tree_bytes(self.root / "e14-x" / "T1" / "run-2" / keep_traces.NOT_KEPT)
        self.assertLessEqual(held - note, 4000)

    def test_a_missing_or_empty_output_creates_nothing(self) -> None:
        for source in (self.tmp / "absent", self.tmp / "empty"):
            if source.name == "empty":
                source.mkdir()
            dest = keep_traces.destination(self.root, "e14-x", "T1", "run-1")
            kept = keep_traces.keep(source, dest, self.root, budget=10**6)
            self.assertEqual((kept.kept, kept.skipped), ([], []))
            self.assertEqual(kept.says(), "there was no output to keep")
        self.assertFalse(self.root.exists())

    def test_a_name_never_leaves_the_root(self) -> None:
        for label in ("../../etc", "..", ".", "", None, "a/b", "e14 x;rm"):
            dest = keep_traces.destination(self.root, label, "../T", "../run")
            self.assertEqual(dest.parent.parent.parent, self.root, (label, dest))
        self.assertEqual(keep_traces.label_of(None), "local")
        self.assertEqual(keep_traces.label_of("e14-evidence-integrity"), "e14-evidence-integrity")
        self.assertEqual(keep_traces.label_of("../x"), "_x")

    def test_the_budget_is_the_environments_where_it_names_one(self) -> None:
        saved = os.environ.get("PW_KEPT_TRACES_KIB")
        self.addCleanup(lambda: os.environ.pop("PW_KEPT_TRACES_KIB", None) if saved is None else os.environ.update(PW_KEPT_TRACES_KIB=saved))
        os.environ.pop("PW_KEPT_TRACES_KIB", None)
        self.assertEqual(keep_traces.budget_kib(), keep_traces.BUDGET_KIB)
        self.assertEqual(keep_traces.BUDGET_KIB, 32 * 1024)
        os.environ["PW_KEPT_TRACES_KIB"] = "7"
        self.assertEqual(keep_traces.budget_kib(), 7)
        os.environ["PW_KEPT_TRACES_KIB"] = "lots"
        self.assertEqual(keep_traces.budget_kib(), keep_traces.BUDGET_KIB)

    def test_the_command_line_keeps_a_run_and_says_so(self) -> None:
        source = failed_run(self.tmp / "test-results", feed_webkit=3000)
        run = subprocess.run(
            [sys.executable, str(SCRIPTS / "keep_traces.py"), str(source), "--root", str(self.root), "--tag", "T9", "--run", "run-1"],
            capture_output=True,
            text=True,
            env={**os.environ, "PW_RECIPE": "e10-browser"},
        )
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIn(f"in {self.root}/e10-browser/T9/run-1", run.stdout)
        self.assertTrue((self.root / "e10-browser" / "T9" / "run-1" / "feed_webkit" / "trace.zip").exists())
        # A root it cannot write is said and exits 1, which the caller ignores.
        (self.tmp / "file").write_text("not a directory")
        run = subprocess.run(
            [sys.executable, str(SCRIPTS / "keep_traces.py"), str(source), "--root", str(self.tmp / "file"), "--tag", "T9", "--run", "run-1"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(run.returncode, 1)
        self.assertIn("could not keep the run's output", run.stdout)


class Recipes(unittest.TestCase):
    """`ci_recipes.py` runs each recipe with its name in `PW_RECIPE`, which is
    what `kept-traces/` is labelled by."""

    def test_a_recipe_runs_with_its_name_in_pw_recipe(self) -> None:
        tmp = pathlib.Path(tempfile.mkdtemp(prefix="pw-ci-recipes-"))
        self.addCleanup(shutil.rmtree, tmp, True)
        (tmp / "scripts").mkdir()
        shutil.copy(SCRIPTS / "ci_recipes.py", tmp / "scripts" / "ci_recipes.py")
        (tmp / "bin").mkdir()
        just = tmp / "bin" / "just"
        just.write_text('#!/usr/bin/env bash\necho "just $1 with PW_RECIPE=${PW_RECIPE:-unset}"\n')
        just.chmod(0o755)
        run = subprocess.run(
            [sys.executable, str(tmp / "scripts" / "ci_recipes.py"), "--out", str(tmp / "out"), "e14-first", "e10-second"],
            capture_output=True,
            text=True,
            env={**os.environ, "PATH": f"{tmp / 'bin'}:{os.environ['PATH']}", "PW_RECIPE": "inherited-from-outside"},
            cwd=tmp,
        )
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertEqual((tmp / "out/logs/e14-first.log").read_text(), "just e14-first with PW_RECIPE=e14-first\n")
        self.assertEqual((tmp / "out/logs/e10-second.log").read_text(), "just e10-second with PW_RECIPE=e10-second\n")


class Uploads(unittest.TestCase):
    """`verify.yml` uploads every `kept-traces/` its recipes and browser jobs
    leave, beside the last run's `test-results/`."""

    UPLOAD = re.compile(r"^(?P<indent> *)- uses: actions/upload-artifact@")

    @classmethod
    def uploads(cls, text: str) -> dict[str, list[str]]:
        """Each upload step's artifact name, with the paths it uploads."""
        lines = text.splitlines()
        found: dict[str, list[str]] = {}
        for i, line in enumerate(lines):
            m = cls.UPLOAD.match(line)
            if not m:
                continue
            width = len(m.group("indent"))
            step = []
            for later in lines[i + 1 :]:
                if later.strip() and not later.lstrip().startswith("#") and len(later) - len(later.lstrip()) <= width:
                    break
                step.append(later)
            name = next((s.split("name:", 1)[1].strip() for s in step if s.lstrip().startswith("name:")), "?")
            paths: list[str] = []
            for j, s in enumerate(step):
                if not s.lstrip().startswith("path:"):
                    continue
                inline = s.split("path:", 1)[1].strip()
                if inline and inline != "|":
                    paths.append(inline)
                    continue
                indent = len(s) - len(s.lstrip())
                for rest in step[j + 1 :]:
                    if rest.strip() and len(rest) - len(rest.lstrip()) <= indent:
                        break
                    if rest.strip() and not rest.lstrip().startswith("#"):
                        paths.append(rest.strip())
            found[name] = paths
        return found

    def test_the_traces_and_browser_uploads_name_every_kept_traces(self) -> None:
        found = self.uploads((ROOT / ".github/workflows/verify.yml").read_text())
        traces = found["traces-${{ matrix.shard.index }}"]
        browser = found["browser-${{ matrix.engine }}"]
        # What a recipe shard's runs leave: the own renderer's suite, and the
        # benchmark harness's (`run_suite.sh -C benchmarks/harness`).
        for path in ("spikes/own-renderer/kept-traces/", "benchmarks/harness/kept-traces/", "spikes/own-renderer/test-results/"):
            self.assertIn(path, traces)
        # The browser jobs run the own renderer's suite.
        for path in ("spikes/own-renderer/kept-traces/", "spikes/own-renderer/test-results/"):
            self.assertIn(path, browser)

    def test_the_reader_of_the_workflow_reads_a_dropped_path(self) -> None:
        # The control: the same reading of a workflow that drops a path says so.
        text = (ROOT / ".github/workflows/verify.yml").read_text()
        dropped = text.replace("            spikes/own-renderer/kept-traces/\n            benchmarks/harness/kept-traces/\n", "", 1)
        self.assertNotEqual(dropped, text)
        found = self.uploads(dropped)
        self.assertNotIn("spikes/own-renderer/kept-traces/", found["traces-${{ matrix.shard.index }}"])
        self.assertIn("spikes/own-renderer/kept-traces/", found["browser-${{ matrix.engine }}"])

    def test_the_directory_is_ignored_by_git(self) -> None:
        for gitignore in (ROOT / ".gitignore", ROOT / "spikes/own-renderer/.gitignore"):
            self.assertIn("kept-traces/", gitignore.read_text().splitlines(), gitignore)


if __name__ == "__main__":
    unittest.main()
