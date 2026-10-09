"""The verification run's scripts (ADR-0245), each on what it decides.

- `ci_plan.py` deals recipes into shards, the longest first, each where it
  ends soonest, its setup counted, by the seconds each last took, the same
  way every time; leaves out the measurements of a machine; gives the
  recipes run against a database shards and a PostgreSQL of their own, as
  many as end the run soonest; and reaches from a change to the mutation
  scripts it touches;
- `ci_recipes.py` takes what a recipe wrote to be the files whose bytes
  changed while it ran, and frees what its builds leave;
- `ci_summary.py` fails a run where a recipe failed, a mutant survived, or a
  shard reported nothing;
- `evidence_fetch.py` names the run after a file's `commit:` line, and
  refuses a file of another commit, and a run that failed anywhere but in
  the jobs it is told are known; and keeps the seconds of each recipe that
  ran to its end.
"""

import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import time
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
    def test_the_costliest_go_first_each_to_where_it_ends_soonest(self) -> None:
        costs = {"a": 10, "b": 6, "c": 5, "d": 1}
        self.assertEqual([r for r, _, _ in plan.deal(list(costs), costs, 2)], [["a", "d"], ["b", "c"]])
        self.assertEqual(
            plan.plan(list(costs), costs, 2),
            [
                {"index": 0, "recipes": ["a", "d"], "browsers": False, "build": False, "database": False},
                {"index": 1, "recipes": ["b", "c"], "browsers": False, "build": False, "database": False},
            ],
        )

    def test_a_recipe_needing_less_may_run_where_more_is_set_up(self) -> None:
        # ADR-0249: a shard installs browsers, or builds, only for a recipe
        # dealt it that needs them. ADR-0290: a recipe that needs neither
        # runs in such a shard where it ends soonest there.
        costs = {"x": 4000, "p": 1000, "y": 500}
        need = {"p": (True, True)}
        self.assertEqual(
            [(s["recipes"], s["browsers"], s["build"]) for s in plan.plan(list(costs), costs, 2, need)],
            [(["x"], False, False), (["p", "y"], True, True)],
        )

    def test_what_a_recipe_adds_to_a_shards_setup_counts(self) -> None:
        # `q` would end sooner after `x`, but not with the browsers that
        # shard would install for it.
        costs = {"x": 1200, "b": 1000, "q": 100}
        need = {"b": (True, True), "q": (True, True)}
        dealt = plan.deal(list(costs), costs, 2, need)
        self.assertEqual([(r, k) for r, k, _ in dealt], [(["x"], (False, False)), (["b", "q"], (True, True))])
        setup = plan.SETUP
        self.assertEqual([end for _, _, end in dealt], [setup[(False, False)] + 1200, setup[(True, True)] + 1100])
        # Setting up a build, or browsers and a build, takes longer than a
        # plain shard's setup.
        self.assertLess(setup[(False, False)], setup[(False, True)])
        self.assertLessEqual(setup[(False, True)], setup[(True, True)])

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

    def test_recipes_run_against_a_database_take_the_shards_that_end_the_run_soonest(self) -> None:
        # ADR-0278: each shard of theirs has a PostgreSQL of its own, and
        # no other recipe runs there. ADR-0290: they take as many shards as
        # end the run soonest. Long, each its own.
        db = {"d1", "d2", "d3"}
        costs = {"a": 3000, "b": 3000, "d1": 3000, "d2": 3000, "d3": 3000}
        shards = plan.plan(list(costs), costs, 5, {"d1": (True, True)}, db)
        self.assertEqual(
            [(s["recipes"], s["browsers"], s["database"]) for s in shards],
            [
                (["a"], False, False),
                (["b"], False, False),
                (["d1"], True, True),
                (["d2"], False, True),
                (["d3"], False, True),
            ],
        )
        self.assertEqual([s["index"] for s in shards], [0, 1, 2, 3, 4])

    def test_short_recipes_run_against_a_database_share_one(self) -> None:
        db = {"d1", "d2", "d3"}
        costs = {"a": 3000, "b": 3000, "d1": 300, "d2": 200, "d3": 100}
        shards = plan.plan(list(costs), costs, 3, {}, db)
        self.assertEqual(
            [(s["recipes"], s["database"]) for s in shards],
            [(["a"], False), (["b"], False), (["d1", "d2", "d3"], True)],
        )
        # The fewest where more would end the run no sooner.
        costs = {"d1": 3000, "d2": 100, "d3": 100}
        self.assertEqual([s["recipes"] for s in plan.plan(list(costs), costs, 3, {}, db)], [["d1"], ["d2", "d3"]])
        # With no others, every shard may be theirs.
        costs = {"d1": 3000, "d2": 3000, "d3": 3000}
        self.assertEqual([s["recipes"] for s in plan.plan(list(costs), costs, 3, {}, db)], [["d1"], ["d2"], ["d3"]])
        # One shard asked for, and others to run: one for each.
        self.assertEqual(len(plan.plan(["a", "d1"], costs, 1, {}, db)), 2)

    def test_a_recipes_cost_counts_the_mutants_it_plants(self) -> None:
        body = "    python3 scripts/statements_separated_mutations.py\n"
        self.assertGreater(plan.cost(body), 20)
        self.assertEqual(plan.cost("    cargo test\n"), 1)

    def test_a_recipe_costs_the_seconds_it_last_took(self) -> None:
        # ADR-0290: mutants foretold a recipe's time poorly (0.43, over the
        # nightly of 2026-10-08), and two shards outlasted their job.
        body = {n: "    cargo test\n" for n in ("a", "b", "c", "new")}
        body["m"] = "    python3 scripts/statements_separated_mutations.py\n"
        seconds = {"a": 10.4, "b": 30, "c": 20}
        costs = plan.estimated(["a", "new", "m"], body, seconds)
        self.assertEqual(costs["a"], 10)
        # One not yet measured: itself and its mutants, each at the median
        # of what a measured recipe took one.
        self.assertEqual(costs["new"], 20)
        self.assertEqual(costs["m"], 20 * plan.cost(body["m"]))
        # Where none is measured, at `RATE`.
        self.assertEqual(plan.estimated(["new"], body, {}), {"new": plan.RATE})

    def test_a_run_is_planned_by_the_seconds_kept(self) -> None:
        # Three recipes that set up nothing, in two shards: whichever took
        # longest last is alone.
        import contextlib
        import io

        names = ["e10-affine-bindings", "e10-attribute-case", "e10-built-pages"]
        real, argv = plan.SECONDS, sys.argv
        try:
            with tempfile.TemporaryDirectory() as d:
                plan.SECONDS = pathlib.Path(d) / "seconds.json"
                sys.argv = ["ci_plan.py", "--shards", "2", *names]
                for longest in names:
                    plan.SECONDS.write_text(json.dumps({n: 5000 if n == longest else 1000 for n in names}))
                    out = io.StringIO()
                    with contextlib.redirect_stdout(out):
                        self.assertEqual(plan.main(), 0)
                    shards = json.loads(out.getvalue().strip()[len("shards="):])
                    self.assertEqual(shards[0]["recipes"], [longest])
        finally:
            plan.SECONDS, sys.argv = real, argv

    def test_the_seconds_are_read_where_the_fetch_keeps_them(self) -> None:
        self.assertEqual(plan.SECONDS, fetch.SECONDS)
        with tempfile.TemporaryDirectory() as d:
            path = pathlib.Path(d) / "seconds.json"
            self.assertEqual(plan.measured(path), {})
            path.write_text('{"e14-a": 61}\n')
            self.assertEqual(plan.measured(path), {"e14-a": 61.0})
            path.write_text("not json")
            self.assertEqual(plan.measured(path), {})


class Recipes(unittest.TestCase):
    def test_what_a_recipe_wrote_is_what_changed(self) -> None:
        before = {"docs/evidence/a.txt": "1", "docs/evidence/b.txt": "2"}
        after = {"docs/evidence/a.txt": "1", "docs/evidence/b.txt": "3", "docs/evidence/c.txt": "4"}
        self.assertEqual(
            recipes.written(before, after),
            ["docs/evidence/b.txt", "docs/evidence/c.txt"],
        )


    def test_a_failed_recipe_says_its_last_lines_in_the_jobs_log(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = pathlib.Path(tmp)
            (out / "logs").mkdir()
            (out / "logs" / "e14-x.log").write_text("".join(f"line {i}\n" for i in range(100)))
            (out / "evidence" / "docs").mkdir(parents=True)
            (out / "evidence" / "docs" / "x.txt").write_text("built\nError: no such module\n")
            said = recipes.failed_tail("e14-x", out, ["docs/x.txt"], lines=5)
        self.assertIn("| line 99", said)
        self.assertNotIn("| line 94", said)
        self.assertIn("| Error: no such module", said)

    def test_a_heartbeat_says_what_a_recipe_is_doing_until_it_ends(self) -> None:
        # A runner that dies mid-recipe keeps what reached the job's log.
        import contextlib, io, threading
        stop = threading.Event()
        said = io.StringIO()
        with contextlib.redirect_stdout(said):
            beat = threading.Thread(target=recipes.heartbeat, args=("e14-x", stop, 0.05))
            beat.start()
            time.sleep(0.3)
            stop.set()
            beat.join()
        lines = [l for l in said.getvalue().splitlines() if l.startswith("  [e14-x, ")]
        self.assertTrue(lines, said.getvalue())
        self.assertIn("evidence", lines[-1])

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

    def test_a_recipe_that_ran_to_its_end_has_its_seconds_kept(self) -> None:
        # ADR-0290: the plan deals the next runs by them.
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            for i, results in enumerate(
                [
                    [{"recipe": "e14-a", "status": 0, "seconds": 61.6}],
                    # A recipe that failed took what it took to fail: not kept.
                    [{"recipe": "e14-b", "status": 2, "seconds": 5.0}, {"recipe": "e14-d", "status": 0, "seconds": 7}],
                ]
            ):
                (root / f"evidence-{i}").mkdir()
                (root / f"evidence-{i}" / "results.json").write_text(json.dumps(results))
            # A shard that stopped before its first recipe ended wrote none.
            (root / "evidence-2").mkdir()
            timed = fetch.timed(fetch.results_of(fetch.shard_dirs(root)))
            self.assertEqual(timed, {"e14-a": 62, "e14-d": 7})
            path = root / "seconds.json"
            path.write_text('{"e14-a": 10, "e14-c": 30}\n')
            self.assertEqual(fetch.remember(timed, path), 2)
            # Each kept over its last; every other recipe's as it was.
            self.assertEqual(json.loads(path.read_text()), {"e14-a": 62, "e14-c": 30, "e14-d": 7})
            # A run of one shard, extracted into the download's own directory.
            lone = root / "lone"
            (lone / "evidence").mkdir(parents=True)
            (lone / "results.json").write_text(json.dumps([{"recipe": "e14-x", "status": 0, "seconds": 1}]))
            self.assertEqual(fetch.timed(fetch.results_of(fetch.shard_dirs(lone))), {"e14-x": 1})

    def test_the_seconds_kept_are_each_a_recipes(self) -> None:
        kept = json.loads(fetch.SECONDS.read_text())
        self.assertGreater(len(kept), 100)
        self.assertTrue(all(plan.EVIDENCE_RECIPE.match(r) and isinstance(s, int) and s >= 0 for r, s in kept.items()))
        self.assertEqual(list(kept), sorted(kept))

class Summary(unittest.TestCase):
    def run_summary(self, shards: list[list[dict]], files: dict[str, str], planned: int) -> int:
        return self.summary_of(shards, files, planned).returncode

    def summary_of(self, shards: list[list[dict]], files: dict[str, str], planned: int):
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
            )

    def result(self, status: int = 0, wrote: list[str] | None = None) -> dict:
        return {"recipe": "e14-x", "status": status, "seconds": 1.0, "wrote": wrote or []}

    def test_a_run_whose_recipes_passed_passes(self) -> None:
        self.assertEqual(self.run_summary([[self.result(wrote=["a.txt"])]], {"a.txt": "3 of 3 mutants killed\n"}, 1), 0)

    def test_a_kill_perhaps_the_memory_bounds_alone_is_listed_apart(self) -> None:
        # A kind of its own (mutation_bound.py): not a failure, and not
        # counted with a test's kills.
        stopped = (
            "  memory bound: stopped `graphs`, which held more than 4 GiB\n"
            "the renderer takes a node twice: KILLED (1 of 25 tests fail)\n"
            "19 of 19 mutants killed\n"
            "memory bound: a process was stopped in 1 of 19 mutants' runs, "
            "each perhaps killed by the bound alone: the renderer takes a node twice\n"
        )
        done = self.summary_of([[self.result(wrote=["a.txt"])]], {"a.txt": stopped}, 1)
        self.assertEqual(done.returncode, 0)
        self.assertIn(
            "Killed, perhaps by the memory bound alone, not by a test:\n\n"
            "- e14-x: a process was stopped in 1 of 19 mutants' runs, each perhaps killed "
            "by the bound alone: the renderer takes a node twice\n",
            done.stdout,
        )
        quiet = self.summary_of(
            [[self.result(wrote=["a.txt"])]], {"a.txt": "3 of 3 mutants killed\nmemory bound: no process was stopped\n"}, 1
        )
        self.assertNotIn("memory bound", quiet.stdout)

    def test_a_failed_recipe_fails_the_run(self) -> None:
        self.assertEqual(self.run_summary([[self.result(status=1)]], {}, 1), 1)

    def test_a_surviving_mutant_fails_the_run(self) -> None:
        self.assertEqual(self.run_summary([[self.result(wrote=["a.txt"])]], {"a.txt": "x: SURVIVED\n"}, 1), 1)

    def test_a_shard_that_reported_nothing_fails_the_run(self) -> None:
        self.assertEqual(self.run_summary([[self.result()]], {}, 2), 1)

    def test_a_run_of_one_shard_is_read_where_it_was_extracted(self) -> None:
        # `download-artifact` puts a lone match in the path itself: W6's 1b
        # ran one shard, passed it, and the run reported none.
        def one(results: list[dict], files: dict[str, str]) -> int:
            with tempfile.TemporaryDirectory() as tmp:
                root = pathlib.Path(tmp)
                (root / "evidence").mkdir()
                (root / "results.json").write_text(json.dumps(results))
                for path, text in files.items():
                    (root / "evidence" / path).write_text(text)
                return subprocess.run(
                    [sys.executable, str(SCRIPTS / "ci_summary.py"), tmp, "--shards", "1"],
                    capture_output=True,
                    text=True,
                ).returncode

        self.assertEqual(one([self.result(wrote=["a.txt"])], {"a.txt": "3 of 3 mutants killed\n"}), 0)
        self.assertEqual(one([self.result(wrote=["a.txt"])], {"a.txt": "x: SURVIVED\n"}), 1)


if __name__ == "__main__":
    unittest.main()
