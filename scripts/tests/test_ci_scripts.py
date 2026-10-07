"""The verification run's scripts (ADR-0245), each on what it decides.

- `ci_plan.py` deals recipes into shards, the costliest first, the same way
  every time, leaves out the measurements of a machine, sends a recipe run
  against a database to the database job, and reaches from a change to the
  mutation scripts it touches;
- `ci_recipes.py` takes what a recipe wrote to be the files whose bytes
  changed while it ran;
- `ci_summary.py` fails a run where a recipe failed, a mutant survived, or a
  shard reported nothing;
- `evidence_fetch.py` names the run after a file's `commit:` line, and
  refuses a file of another commit.
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
        shards = plan.plan(list(costs), costs, 2)
        self.assertEqual(
            shards,
            [{"index": 0, "recipes": ["a", "d"]}, {"index": 1, "recipes": ["b", "c"]}],
        )

    def test_a_plan_is_the_same_every_time(self) -> None:
        costs = {f"e14-r{i}": i % 4 + 1 for i in range(40)}
        names = list(costs)
        first = plan.plan(names, costs, 16)
        self.assertEqual(first, plan.plan(list(reversed(names)), costs, 16))
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

    def test_a_recipe_run_against_a_database_goes_to_the_database_job(self) -> None:
        out = subprocess.run(
            [sys.executable, str(SCRIPTS / "ci_plan.py"), "--shards", "2", "e14-feed-postgres", "e14-feed"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.splitlines()
        self.assertEqual(out[0], 'shards=[{"index": 0, "recipes": ["e14-feed"]}]')
        self.assertEqual(out[1], 'database=["e14-feed-postgres"]')

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
