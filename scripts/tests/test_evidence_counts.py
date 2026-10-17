"""`evidence_counts.py`: an evidence file's mutants are its script's (track
evidence-integrity, finding 2). Read on trees of its own, each with its
control, and held of the repository's own evidence: a disagreement fails
unless a train's push records it again.
"""

import importlib.util
import pathlib
import shutil
import subprocess
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


counts = load("evidence_counts")
plan = load("ci_plan")

# A script of three mutants, described as the scripts describe theirs: a
# description, and a suite beside it.
X = """\
import pathlib
ROOT = pathlib.Path(__file__).resolve().parent.parent
MUTANTS = [
    ("a sink is read", "core", ROOT / "src.rs", "a", "b"),
    ("a callee is found by its spelling", "core", ROOT / "src.rs", "c", "d"),
    ("PW1 speaks where PW2 does", "server", ROOT / "src.rs", "e", "f"),
]
"""
# And one of two, whose verdicts a script prints after its stack.
Y = """\
import pathlib
ROOT = pathlib.Path(__file__).resolve().parent.parent
MUTANTS = [
    ("next-react", "the count is distinct items", ROOT / "store.ts", "g", "h"),
    ("sveltekit", "the count is distinct items", ROOT / "store.ts", "i", "j"),
]
"""
# A script of three mutants run in two parts, each a recipe of its own (the
# integrator's ruling 1, train-10: `e14-refusal` no longer fit a CI job).
Z = """\
import pathlib
ROOT = pathlib.Path(__file__).resolve().parent.parent
MUTANTS = [
    ("a refusal is not read", "core", ROOT / "src.rs", "a", "b"),
    ("a refusal is not told", "core", ROOT / "src.rs", "c", "d"),
    ("an announcer says nothing", "browser", ROOT / "src.rs", "e", "f"),
]
PARTS = [(0, 2), (2, 3)]
"""
PARTED = (
    "e14-z:\n"
    "    @{ python3 scripts/z_mutations.py --part 1/2; } > docs/evidence/E14/z.txt\n"
    "e14-z-2:\n"
    "    @{ python3 scripts/z_mutations.py --part 2/2; } > docs/evidence/E14/z-2.txt\n"
)
Z1 = (
    "part 1 of 2: mutants 1 to 2 of 3\n"
    "baseline (core): 4 passed, 0 failed\n"
    "a refusal is not read [core]: KILLED (1 of 4 tests fail)\n"
    "a refusal is not told [core]: KILLED (2 of 4 tests fail)\n"
    "2 of 2 mutants killed\n"
)
Z2 = (
    "part 2 of 2: mutants 3 to 3 of 3\n"
    "baseline (browser): 6 passed, 0 failed\n"
    "an announcer says nothing [browser]: KILLED (1 of 6 tests fail)\n"
    "1 of 1 mutants killed\n"
)
RECIPE = (
    "e14-x:\n"
    '    @{ echo "commit: $(git rev-parse HEAD)"; python3 scripts/x_mutations.py; '
    "} > docs/evidence/E14/x.txt\n"
)
BOTH = (
    "e14-both:\n"
    "    @{ python3 scripts/y_mutations.py; python3 scripts/x_mutations.py; "
    "} > docs/evidence/E14/both.txt\n"
)
AGREES = (
    "baseline: 4 passed, 0 failed\n"
    "a sink is read [core]: KILLED (1 of 4 tests fail)\n"
    "a callee is found by its spelling [core]: KILLED (3 of 4 tests fail)\n"
    "PW1 speaks where PW2 does [server]: KILLED (1 of 4 tests fail)\n"
    "3 of 3 mutants killed\n"
)
Y_AGREES = (
    "baseline next-react: 2 passed, 0 failed\n"
    "next-react: the count is distinct items: KILLED (1 of 2 tests fail)\n"
    "sveltekit: the count is distinct items: KILLED (2 of 2 tests fail)\n"
    "2 of 2 mutants killed\n"
)


class Counts(unittest.TestCase):
    def tree(self, files: dict[str, str]) -> pathlib.Path:
        root = pathlib.Path(tempfile.mkdtemp(prefix="pw-counts-"))
        self.addCleanup(shutil.rmtree, root, True)
        for rel, text in files.items():
            path = root / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        return root

    def found(self, evidence: str, justfile: str = RECIPE, name: str = "x.txt") -> dict:
        root = self.tree(
            {
                "justfile": justfile,
                "scripts/x_mutations.py": X,
                "scripts/y_mutations.py": Y,
                f"docs/evidence/E14/{name}": evidence,
            }
        )
        return counts.disagreements(root, plan)

    def test_evidence_that_agrees_with_its_script_names_nothing(self):
        self.assertEqual(self.found(AGREES), {})

    def parted(self, z: str = Z, justfile: str = PARTED, z1: str = Z1, z2: str = Z2) -> dict:
        root = self.tree(
            {
                "justfile": justfile,
                "scripts/z_mutations.py": z,
                "docs/evidence/E14/z.txt": z1,
                "docs/evidence/E14/z-2.txt": z2,
            }
        )
        return counts.disagreements(root, plan)

    def test_a_scripts_mutants_split_across_recipes_are_each_counted_in_its_part(self):
        # The integrator's ruling 1 (train-10): `e14-refusal` runs part 1 of
        # its script's mutants and `e14-refusal-2` part 2, each its own count.
        self.assertEqual(self.parted(), {})
        # A part's file that counts the whole script's mutants.
        whole = Z1.replace("2 of 2 mutants killed", "2 of 3 mutants killed")
        self.assertIn(
            "z_mutations.py: counts 3 mutants; its part 1 of 2 holds 2",
            [what for _, _, what in self.parted(z1=whole)["e14-z"]],
        )

    def test_a_scripts_parts_hold_every_mutant_once_each_run_by_a_recipe(self):
        # The floor no mutant falls through: a mutant in no part, a mutant in
        # two, and a part no recipe runs are each named, of each recipe that
        # runs a part.
        gap = Z.replace("PARTS = [(0, 2), (2, 3)]", "PARTS = [(0, 1), (2, 3)]")
        one = Z1.replace("a refusal is not told [core]: KILLED (2 of 4 tests fail)\n", "").replace(
            "2 of 2 mutants killed", "1 of 1 mutants killed"
        )
        self.assertEqual(
            {r: [what for _, _, what in said] for r, said in self.parted(z=gap, z1=one).items()},
            {
                "e14-z": ["z_mutations.py: its parts leave 1 of its 3 mutants to none"],
                "e14-z-2": ["z_mutations.py: its parts leave 1 of its 3 mutants to none"],
            },
        )
        overlap = Z.replace("PARTS = [(0, 2), (2, 3)]", "PARTS = [(0, 2), (1, 3)]")
        two = Z2.replace(
            "an announcer says nothing [browser]",
            "a refusal is not told [core]: KILLED (1 of 6 tests fail)\nan announcer says nothing [browser]",
        ).replace("1 of 1 mutants killed", "2 of 2 mutants killed")
        self.assertEqual(
            {r: [what for _, _, what in said] for r, said in self.parted(z=overlap, z2=two).items()},
            {
                "e14-z": ["z_mutations.py: its parts hold 1 of its 3 mutants twice"],
                "e14-z-2": ["z_mutations.py: its parts hold 1 of its 3 mutants twice"],
            },
        )
        alone = PARTED.split("e14-z-2:")[0]
        self.assertEqual(
            [what for _, _, what in self.parted(justfile=alone)["e14-z"]],
            ["z_mutations.py: part 2 of 2 is run by no recipe"],
        )

    def test_a_count_and_a_verdict_of_a_retired_mutant_are_named(self):
        # As privacy-by-resolution.txt said, of a script that plants three.
        evidence = AGREES.replace(
            "3 of 3 mutants killed\n",
            "the body's label is read by spelling: KILLED (2 of 4 tests fail)\n4 of 4 mutants killed\n",
        )
        self.assertEqual(
            self.found(evidence),
            {
                "e14-x": [
                    ("docs/evidence/E14/x.txt", 6, "x_mutations.py: counts 4 mutants; its script defines 3"),
                    (
                        "docs/evidence/E14/x.txt",
                        5,
                        "x_mutations.py: \"the body's label is read by spelling\" names no mutant of its script",
                    ),
                ]
            },
        )

    def test_a_mutant_named_twice_and_a_verdict_short_are_named(self):
        twice = AGREES.replace(
            "PW1 speaks where PW2 does [server]: KILLED (1 of 4 tests fail)\n",
            "a sink is read [core]: KILLED (1 of 4 tests fail)\n",
        )
        self.assertEqual(
            [what for _, _, what in self.found(twice)["e14-x"]],
            ["x_mutations.py: 'a sink is read [core]' names a mutant named before"],
        )
        short = AGREES.replace("PW1 speaks where PW2 does [server]: KILLED (1 of 4 tests fail)\n", "")
        self.assertEqual(
            [what for _, _, what in self.found(short)["e14-x"]],
            [
                "x_mutations.py: counts 3 mutants above 2 verdicts",
                "x_mutations.py: counts 3 killed; 2 verdicts say KILLED",
            ],
        )

    def test_its_killed_are_the_verdicts_that_say_killed(self):
        evidence = AGREES.replace("[server]: KILLED (1 of 4 tests fail)", "[server]: SURVIVED")
        self.assertEqual(
            [what for _, _, what in self.found(evidence)["e14-x"]],
            ["x_mutations.py: counts 3 killed; 2 verdicts say KILLED"],
        )
        # The control: a survivor counted as one disagrees with nothing here
        # (`evidence_survivors.py` names it).
        self.assertEqual(self.found(evidence.replace("3 of 3", "2 of 3")), {})

    def test_two_scripts_blocks_are_read_by_the_mutants_they_name(self):
        self.assertEqual(self.found(Y_AGREES + AGREES, BOTH, "both.txt"), {})
        # A script the recipe runs, whose mutants no block counts.
        self.assertEqual(
            [what for _, _, what in self.found(AGREES, BOTH, "both.txt")["e14-both"]],
            ["counts none of y_mutations.py's mutants, which the recipe runs"],
        )
        # A script's mutants counted twice.
        self.assertEqual(
            [what for _, _, what in self.found(AGREES + AGREES, RECIPE)["e14-x"]],
            ["x_mutations.py's mutants are counted twice"],
        )

    def test_a_description_is_read_beside_its_words_its_suite_and_separators(self):
        x = ("a sink is read", "core", pathlib.Path("src.rs"), "a", "b")
        self.assertTrue(counts.names("a sink is read", x))
        self.assertTrue(counts.names("a sink is read [core]", x))
        # A suite's tag the script works out from the path, not a word.
        self.assertTrue(counts.names("a sink is read [cargo]", x))
        self.assertTrue(counts.names("V1  a sink is read", ("V1", "a sink is read", *x[2:])))
        # A description inside a longer one names nothing.
        self.assertFalse(counts.names("a sink is read by spelling", x))
        self.assertFalse(counts.names("a sink is read twice [core]", x))
        # One description, two stacks: each verdict names its own.
        react = ("next-react", "the count is distinct items", pathlib.Path("s.ts"), "g", "h")
        svelte = ("sveltekit", "the count is distinct items", pathlib.Path("s.ts"), "i", "j")
        self.assertTrue(counts.names("next-react: the count is distinct items", react))
        self.assertFalse(counts.names("next-react: the count is distinct items", svelte))

    def test_a_disagreement_fails_unless_a_trains_push_records_it_again(self):
        found = {"e14-x": [("x.txt", 1, "?")], "e14-y": [("y.txt", 1, "?")]}
        self.assertEqual(counts.failing(found, {"e14-x"}), ["e14-y"])
        self.assertEqual(counts.failing(found, {"e14-x", "e14-y"}), [])


class PlannedAgain(unittest.TestCase):
    """Which disagreement a train's push records again, as the planner
    judges the tree here: a git tree of its own, with the planner in it."""

    def git(self, *args: str) -> str:
        return subprocess.run(
            ["git", "-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", *args],
            cwd=self.root, check=True, capture_output=True, text=True,
        ).stdout.strip()

    def setUp(self):
        self.root = pathlib.Path(tempfile.mkdtemp(prefix="pw-planned-"))
        self.addCleanup(shutil.rmtree, self.root, True)
        (self.root / "scripts").mkdir()
        shutil.copy(SCRIPTS / "ci_plan.py", self.root / "scripts/ci_plan.py")
        (self.root / "scripts/x_mutations.py").write_text(X)
        (self.root / "src.rs").write_text("a c e\n")
        # One recipe that CI runs, one recorded on the development machine.
        local = next(iter(sorted(plan.LOCAL_ONLY)))
        (self.root / "justfile").write_text(
            RECIPE + RECIPE.replace("e14-x:", f"{local}:").replace("x.txt", "local.txt")
        )
        self.local = local
        self.git("init", "-q")
        self.git("add", ".")
        self.git("commit", "-q", "-m", "a script")
        recorded = self.git("rev-parse", "HEAD")
        lying = f"commit: {recorded}\n" + AGREES.replace("3 of 3", "4 of 4")
        for name in ("x.txt", "local.txt"):
            path = self.root / "docs/evidence/E14" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(lying)
        self.git("add", ".")
        self.git("commit", "-q", "-m", "its evidence")
        self.plan = counts.planner(self.root)

    def again(self) -> set[str]:
        return counts.planned_again(["e14-x", self.local], self.root, self.plan)

    def test_evidence_current_in_the_tree_here_is_recorded_again_by_no_plan(self):
        self.assertEqual(self.again(), set())

    def test_evidence_whose_script_changed_in_the_tree_here_is_planned_again(self):
        # Not committed yet, as `just ci` runs before a commit.
        script = self.root / "scripts/x_mutations.py"
        script.write_text(script.read_text().replace('    ("PW1 speaks', '    # ("PW1 speaks'))
        # The recipe CI runs is planned again; the one recorded on the
        # development machine is not, whatever its evidence says.
        self.assertEqual(self.again(), {"e14-x"})


class Repository(unittest.TestCase):
    def test_the_repositorys_evidence_agrees_with_its_scripts_or_is_planned_again(self):
        found = counts.disagreements()
        self.assertEqual(counts.failing(found, counts.planned_again(sorted(found))), [])


if __name__ == "__main__":
    unittest.main()
