"""The verification run's scripts (ADR-0245), each on what it decides.

- `ci_plan.py` deals recipes into shards, the costliest first, the same way
  every time, leaves out the measurements of a machine, gives a recipe run
  against a database a shard and a PostgreSQL of its own, and reaches from a
  change to the mutation scripts it touches;
- `ci_recipes.py` takes what a recipe wrote to be the files whose bytes
  changed while it ran, and frees what its builds leave;
- `ci_summary.py` fails a run where a recipe failed, a mutant survived, or a
  shard reported nothing;
- `evidence_fetch.py` names the run after a file's `commit:` line, and
  refuses a file of another commit, and a run that failed anywhere but in
  the jobs it is told are known.
"""

import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


plan = load("ci_plan")
recipes = load("ci_recipes")
fetch = load("evidence_fetch")


class Plan(unittest.TestCase):
    def test_the_costliest_go_first_each_to_the_least_loaded_shard(self) -> None:
        costs = {"a": 10, "b": 6, "c": 5, "d": 1}
        self.assertEqual(plan.deal(list(costs), costs, 2), [["a", "d"], ["b", "c"]])
        self.assertEqual(
            plan.plan(list(costs), costs, 2),
            [
                {"index": 0, "recipes": ["a", "d"], "browsers": False, "build": False, "database": False},
                {"index": 1, "recipes": ["b", "c"], "browsers": False, "build": False, "database": False},
            ],
        )

    def test_a_kind_of_recipe_shares_its_shards_with_its_kind(self) -> None:
        # ADR-0249: only a shard with a recipe that drives a browser installs
        # one, and only one with a recipe that reads the build builds.
        costs = {"p1": 6, "p2": 6, "b1": 4, "x1": 8, "x2": 8, "x3": 8}
        need = {"p1": (True, True), "p2": (True, True), "b1": (False, True)}
        shards = plan.plan(list(costs), costs, 4, need)
        self.assertEqual(
            [(s["recipes"], s["browsers"], s["build"]) for s in shards],
            [
                (["p1", "p2"], True, True),
                (["b1"], False, True),
                (["x1", "x3"], False, False),
                (["x2"], False, False),
            ],
        )
        self.assertEqual([s["index"] for s in shards], [0, 1, 2, 3])

    def test_what_a_recipe_needs_is_read_from_its_lines(self) -> None:
        self.assertEqual(plan.needs("    pnpm exec playwright test e2e/feed.spec.mjs\n"), (True, True))
        self.assertEqual(plan.needs("    ./target/debug/pw check a.pw\n"), (False, True))
        self.assertEqual(plan.needs("    BUILD_ONLY=1 bash spikes/own-renderer/run.sh\n"), (False, True))
        self.assertEqual(plan.needs("    cargo test -p pw-core --test names\n"), (False, False))

    def test_a_plan_is_the_same_every_time(self) -> None:
        costs = {f"e14-r{i}": i % 4 + 1 for i in range(40)}
        names = list(costs)
        first = plan.plan(names, costs, 16)
        self.assertEqual(first, plan.plan(list(reversed(names)), costs, 16))
        self.assertLessEqual(len(first), 16)
        dealt = sorted(r for s in first for r in s["recipes"])
        self.assertEqual(dealt, sorted(names))

    def test_no_shard_is_planned_empty(self) -> None:
        self.assertEqual(len(plan.plan(["a", "b"], {}, 16)), 2)

    def test_a_measurement_of_the_machine_is_not_planned(self) -> None:
        self.assertIn("e10-close-bench", plan.LOCAL_ONLY)
        self.assertTrue(plan.EVIDENCE_RECIPE.match("e14-feed"))
        self.assertFalse(plan.EVIDENCE_RECIPE.match("ci"))

    def test_a_change_reaches_the_scripts_it_touches(self) -> None:
        # The script itself.
        self.assertIn(
            "statements_separated_mutations.py",
            plan.touched_scripts({"scripts/statements_separated_mutations.py": [(1, 1)]}, 30),
        )
        # A test file a script runs.
        self.assertIn(
            "each_heads_mutations.py",
            plan.touched_scripts({"compiler/pw-core/tests/each_heads.rs": [(1, 1)]}, 30),
        )
        # A line near a mutant's anchor, and not one far from every anchor.
        grammar = "compiler/pw-syntax/src/grammar.rs"
        source = (plan.ROOT / grammar).read_text()
        line = source[: source.find("let mut separated = true;")].count("\n") + 1
        self.assertIn(
            "statements_separated_mutations.py",
            plan.touched_scripts({grammar: [(line + 20, line + 20)]}, 30),
        )
        self.assertNotIn(
            "statements_separated_mutations.py",
            plan.touched_scripts({grammar: [(1, 1)]}, 30),
        )

    def test_a_changed_browser_spec_reaches_the_scripts_and_recipes_that_run_it(self) -> None:
        # ADR-0281: a spec a script runs is its test, as a Rust test file is;
        # a change to `accessibility.spec.mjs` planned nothing before.
        spec = "spikes/own-renderer/e2e/accessibility.spec.mjs"
        self.assertIn("accessibility_mutations.py", plan.touched_scripts({spec: [(1, 1)]}, 30))
        self.assertEqual(plan.changed_specs({spec: [(1, 1)]}), {"accessibility.spec.mjs"})
        # And each recipe that runs it: the one that names it in its own
        # lines, and the one that runs a script that runs it.
        planned = plan.recipes_for({spec: [(1, 1)]}, 30, plan.recipes())
        self.assertIn("e14-accessibility", planned)
        self.assertIn("e14-titles", planned)
        self.assertNotIn(
            "accessibility_mutations.py",
            plan.touched_scripts({"spikes/own-renderer/e2e/nothing.spec.mjs": [(1, 1)]}, 30),
        )

    def test_a_recipe_in_an_imported_file_is_read_and_planned(self) -> None:
        # ADR-0253: a track's recipes live in a file the justfile imports.
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            (root / "just").mkdir()
            (root / "justfile").write_text(
                "set positional-arguments\n\nimport 'just/t.just'\nimport? 'just/none.just'\n\n"
                "e14-root:\n    cargo test -p pw-core --test names\n"
            )
            (root / "just/t.just").write_text(
                "# A track's.\ne14-track:\n    python3 scripts/track_mutations.py\n"
            )
            self.assertEqual(plan.justfile_parts(root), ["justfile", "just/t.just"])
            body = plan.bodies(root)
            self.assertIn("scripts/track_mutations.py", body["e14-track"])
            self.assertIn("--test names", body["e14-root"])
            # A changed line of the imported file is its recipe's.
            text = (root / "just/t.just").read_text().splitlines()
            self.assertEqual(plan.recipes_at(text, [(3, 3)], ["e14-track"]), {"e14-track"})
            self.assertEqual(plan.recipes_at(text, [(1, 1)], ["e14-track"]), set())

    def test_a_recipe_run_against_a_database_is_a_shard_of_its_own(self) -> None:
        out = subprocess.run(
            [sys.executable, str(SCRIPTS / "ci_plan.py"), "--shards", "2", "e14-feed-postgres", "e14-feed"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.splitlines()
        # Its shard has a PostgreSQL beside it (ADR-0278), and sets up
        # nothing more than its recipe needs (ADR-0258): `e14-feed-postgres`
        # drives no browser and reads no page built.
        self.assertEqual(
            out,
            [
                'shards=[{"index": 0, "recipes": ["e14-feed"], "browsers": true, "build": true, '
                '"database": false}, {"index": 1, "recipes": ["e14-feed-postgres"], "browsers": false, '
                '"build": false, "database": true}]'
            ],
        )

    def test_a_shard_with_a_database_sets_up_what_its_recipe_needs(self) -> None:
        # `e14-identity` drives three browsers against its PostgreSQL tests'
        # database (ADR-0258): its shard installs them, as any would.
        out = subprocess.run(
            [sys.executable, str(SCRIPTS / "ci_plan.py"), "--shards", "2", "e14-identity"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.splitlines()
        self.assertEqual(
            out,
            ['shards=[{"index": 0, "recipes": ["e14-identity"], "browsers": true, "build": true, "database": true}]'],
        )

    def test_each_recipe_run_against_a_database_has_a_database_of_its_own(self) -> None:
        # ADR-0278: in series on one database, four recipes outlasted their
        # job's 120 minutes; each is a shard of its own, its PostgreSQL its
        # own, within the run's shards, the others dealt into those left.
        costs = {"a": 5, "b": 3, "d1": 9, "d2": 2, "d3": 1}
        need = {"d1": (True, True)}
        shards = plan.plan(list(costs), costs, 5, need, {"d1", "d2", "d3"})
        self.assertEqual(
            [(s["recipes"], s["browsers"], s["build"], s["database"]) for s in shards],
            [
                (["a"], False, False, False),
                (["b"], False, False, False),
                (["d1"], True, True, True),
                (["d2"], False, False, True),
                (["d3"], False, False, True),
            ],
        )
        self.assertEqual([s["index"] for s in shards], [0, 1, 2, 3, 4])

    def test_recipes_run_against_a_database_share_when_the_shards_run_out(self) -> None:
        # Three shards, and others to deal: one is left for them, and the
        # three on a database share two, dealt as any are.
        costs = {"a": 5, "b": 3, "d1": 9, "d2": 2, "d3": 1}
        shards = plan.plan(list(costs), costs, 3, {}, {"d1", "d2", "d3"})
        self.assertEqual(
            [(s["recipes"], s["database"]) for s in shards],
            [(["a", "b"], False), (["d1"], True), (["d2", "d3"], True)],
        )
        # With no others, every shard is theirs.
        only = plan.plan(["d1", "d2", "d3"], costs, 3, {}, {"d1", "d2", "d3"})
        self.assertEqual([s["recipes"] for s in only], [["d1"], ["d2"], ["d3"]])

    def test_a_recipes_cost_counts_the_mutants_it_plants(self) -> None:
        body = "    python3 scripts/statements_separated_mutations.py\n"
        self.assertGreater(plan.cost(body), 20)
        self.assertEqual(plan.cost("    cargo test\n"), 1)


class Recipes(unittest.TestCase):
    def test_what_a_recipe_wrote_is_what_changed(self) -> None:
        before = {"docs/evidence/a.txt": "1", "docs/evidence/b.txt": "2"}
        after = {"docs/evidence/a.txt": "1", "docs/evidence/b.txt": "3", "docs/evidence/c.txt": "4"}
        self.assertEqual(
            recipes.written(before, after),
            ["docs/evidence/b.txt", "docs/evidence/c.txt"],
        )


class Prune(unittest.TestCase):
    def test_what_builds_leave_is_freed_and_the_newest_binary_kept(self) -> None:
        import os
        import time

        with tempfile.TemporaryDirectory() as tmp:
            target = pathlib.Path(tmp)
            deps = target / "debug" / "deps"
            deps.mkdir(parents=True)
            (target / "debug" / "incremental" / "x").mkdir(parents=True)
            old, new = deps / "feed-0123456789abcdef", deps / "feed-fedcba9876543210"
            for i, binary in enumerate((old, new)):
                binary.write_text("binary")
                binary.chmod(0o755)
                os.utime(binary, (time.time() + i, time.time() + i))
            library = deps / "libpw_core-0123456789abcdef.rlib"
            library.write_text("library")
            (deps / "feed.a1.rcgu.o").write_text("object")
            recipes.prune(target)
            left = sorted(p.name for p in deps.iterdir())
            self.assertEqual(left, sorted([library.name, new.name]))
            self.assertFalse((target / "debug" / "incremental").exists())


class Fetch(unittest.TestCase):
    SHA = "a" * 40
    URL = "https://github.com/o/r/actions/runs/1"

    def test_the_run_is_named_after_the_commit(self) -> None:
        text = f"title\n\nproduced by: just e14-x\ncommit: {self.SHA}\nrust: 1\n"
        stamped = fetch.stamp(text, self.SHA, self.URL, "Ubuntu 24.04, x86_64")
        self.assertEqual(
            stamped,
            f"title\n\nproduced by: just e14-x\ncommit: {self.SHA}\n"
            f"ci: {self.URL} (Ubuntu 24.04, x86_64)\nrust: 1\n",
        )
        # Fetched again, the line is replaced, not repeated.
        again = fetch.stamp(stamped, self.SHA, self.URL + "2", "r")
        self.assertEqual(again.count("ci: "), 1)
        self.assertIn("runs/12 (r)", again)

    def test_a_file_of_another_commit_is_refused(self) -> None:
        with self.assertRaises(ValueError):
            fetch.stamp(f"commit: {'b' * 40}\n", self.SHA, self.URL, "r")

    def test_a_file_with_no_commit_is_copied_as_it_is(self) -> None:
        self.assertEqual(fetch.stamp("raw output\n", self.SHA, self.URL, "r"), "raw output\n")


    def test_main_reaches_the_refusal_by_its_own_name(self) -> None:
        # `main` keeps a list of refused files; a function of the same name
        # was shadowed there, and `main` failed before it fetched anything.
        import ast
        tree = ast.parse((SCRIPTS / "evidence_fetch.py").read_text())
        main = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "main")
        assigned = {t.id for n in ast.walk(main) if isinstance(n, ast.Assign) for t in n.targets if isinstance(t, ast.Name)}
        self.assertNotIn("refusal", assigned)

    def test_a_run_that_failed_is_fetched_only_where_its_failures_are_known(self) -> None:
        # ADR-0281: a merge names each failure open in NEXT, and its
        # evidence is fetched beside it.
        run = {"status": "completed", "conclusion": "failure"}
        jobs = [
            {"name": "recipes 0", "conclusion": "success"},
            {"name": "browser webkit", "conclusion": "failure"},
            {"name": "summary", "conclusion": "failure"},
        ]
        self.assertIsNone(fetch.refusal(run, jobs, ["browser webkit", "summary"]))
        self.assertIn("summary", fetch.refusal(run, jobs, ["browser webkit"]))
        self.assertIn("browser webkit", fetch.refusal(run, jobs, []))
        # A recipe shard is never known: its evidence is what is copied.
        shard = jobs + [{"name": "recipes 3", "conclusion": "failure"}]
        self.assertIn("recipes 3", fetch.refusal(run, shard, ["browser webkit", "summary", "recipes 3"]))
        # A run that has not finished, or one that failed nowhere, is not.
        self.assertIn("in_progress", fetch.refusal({"status": "in_progress", "conclusion": None}, jobs, []))
        self.assertIsNone(fetch.refusal({"status": "completed", "conclusion": "success"}, jobs, []))
        self.assertIn("no job", fetch.refusal(run, [{"name": "recipes 0", "conclusion": "success"}], ["x"]))

class Summary(unittest.TestCase):
    def run_summary(self, shards: list[list[dict]], files: dict[str, str], planned: int) -> int:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            for i, results in enumerate(shards):
                shard = root / f"evidence-{i}"
                (shard / "evidence").mkdir(parents=True)
                (shard / "results.json").write_text(json.dumps(results))
                for path, text in files.items():
                    target = shard / "evidence" / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text(text)
            return subprocess.run(
                [sys.executable, str(SCRIPTS / "ci_summary.py"), tmp, "--shards", str(planned)],
                capture_output=True,
                text=True,
            ).returncode

    def result(self, status: int = 0, wrote: list[str] | None = None) -> dict:
        return {"recipe": "e14-x", "status": status, "seconds": 1.0, "wrote": wrote or []}

    def test_a_run_whose_recipes_passed_passes(self) -> None:
        self.assertEqual(self.run_summary([[self.result(wrote=["a.txt"])]], {"a.txt": "3 of 3 mutants killed\n"}, 1), 0)

    def test_a_failed_recipe_fails_the_run(self) -> None:
        self.assertEqual(self.run_summary([[self.result(status=1)]], {}, 1), 1)

    def test_a_surviving_mutant_fails_the_run(self) -> None:
        self.assertEqual(self.run_summary([[self.result(wrote=["a.txt"])]], {"a.txt": "x: SURVIVED\n"}, 1), 1)

    def test_a_shard_that_reported_nothing_fails_the_run(self) -> None:
        self.assertEqual(self.run_summary([[self.result()]], {}, 2), 1)


if __name__ == "__main__":
    unittest.main()
