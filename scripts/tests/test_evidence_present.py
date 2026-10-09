"""`evidence_present.py`: every recipe's evidence is in the repository.

`master` is held to it on its CI; a branch's new recipe is recorded at its
merge, so a branch is not. The reading of recipes is tested on a tree of its
own, with its control.
"""

import importlib.util
import os
import pathlib
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


present = load("evidence_present")

JUSTFILE = """# A recipe that writes its evidence.
e14-written:
    @mkdir -p docs/evidence/E14
    @{ echo "a"; \\
       python3 scripts/x.py; \\
     } > docs/evidence/E14/written.txt
    @grep -E "killed" docs/evidence/E14/written.txt

# One whose evidence was never recorded.
e14-unrecorded arg="":
    @{ echo "b"; } > docs/evidence/E14/unrecorded.txt

# One that writes none.
check:
    @echo nothing
"""


class Present(unittest.TestCase):
    def tree(self):
        root = pathlib.Path(tempfile.mkdtemp(prefix="pw-evidence-"))
        (root / "just").mkdir()
        (root / "justfile").write_text(JUSTFILE)
        (root / "just" / "track.just").write_text(
            "e14-track:\n    @echo c > docs/evidence/E14/track.txt\n"
            "\ne14-task TASK:\n    @echo d > docs/evidence/E14/task-{{TASK}}.txt\n"
        )
        (root / "docs/evidence/E14").mkdir(parents=True)
        (root / "docs/evidence/E14/task-T01.txt").write_text("d\n")
        (root / "docs/evidence/E14/written.txt").write_text("a\n")
        return root

    def test_each_recipe_is_read_with_where_it_writes(self):
        found = sorted(present.recipes(self.tree()))
        self.assertEqual(
            found,
            [
                ("justfile", "e14-unrecorded", "docs/evidence/E14/unrecorded.txt"),
                ("justfile", "e14-written", "docs/evidence/E14/written.txt"),
                ("track.just", "e14-task", "docs/evidence/E14/task-{{TASK}}.txt"),
                ("track.just", "e14-track", "docs/evidence/E14/track.txt"),
            ],
        )

    def test_a_recipe_whose_evidence_is_not_in_the_tree_is_named(self):
        self.assertEqual(
            present.missing(self.tree()),
            [
                ("justfile", "e14-unrecorded", "docs/evidence/E14/unrecorded.txt"),
                ("track.just", "e14-track", "docs/evidence/E14/track.txt"),
            ],
        )

    @unittest.skipUnless(
        os.environ.get("GITHUB_REF") == "refs/heads/master",
        "a branch's recipes are recorded at its merge; master's CI holds them",
    )
    def test_every_recipe_on_master_has_its_evidence(self):
        self.assertEqual(present.missing(), [])
