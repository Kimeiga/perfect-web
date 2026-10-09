"""`kiokun_inventory.py`: kiokun.com's parity inventory names every route of
the SvelteKit app, and only those, each marked once.

Each test builds a small app's routes in a temporary directory, as
SvelteKit lays them out, and an inventory beside it.
"""

import importlib.util
import pathlib
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


inventory = load("kiokun_inventory")

INVENTORY = """\
| route | kind | status | what it is | evidence |
|---|---|---|---|---|
| `/` | page | **partial** | home | x |
| `/[word]` | page | partial | a word | x |
| `/api/search` | endpoint | missing | search | x |
| `/blog/post` | page | built | a post | x |
"""


class Inventory(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        self.app = pathlib.Path(self.dir.name)
        routes = self.app / "src" / "routes"
        for path, files in {
            "": ["+page.svelte", "+layout.svelte", "+error.svelte"],
            "[word]": ["+page.svelte", "+page.ts"],
            "api": [],
            "api/search": ["+server.ts"],
            "blog": ["+layout.svelte"],
            "blog/post": ["+page.ts"],
        }.items():
            (routes / path).mkdir(parents=True, exist_ok=True)
            for f in files:
                (routes / path / f).write_text("")

    def tearDown(self) -> None:
        self.dir.cleanup()

    def test_a_route_is_a_directory_with_a_page_or_an_endpoint(self) -> None:
        self.assertEqual(
            inventory.app_routes(self.app),
            {"/": "page", "/[word]": "page", "/api/search": "endpoint", "/blog/post": "page"},
        )

    def test_an_inventory_naming_every_route_once_passes(self) -> None:
        problems, counts = inventory.check(INVENTORY, inventory.app_routes(self.app))
        self.assertEqual(problems, [])
        self.assertEqual(counts, {"built": 1, "partial": 2, "missing": 1})

    def test_a_route_the_inventory_leaves_out_is_found(self) -> None:
        text = INVENTORY.replace("| `/api/search` | endpoint | missing | search | x |\n", "")
        problems, _ = inventory.check(text, inventory.app_routes(self.app))
        self.assertEqual(problems, ["/api/search (endpoint) is not in the inventory"])

    def test_a_route_the_app_does_not_have_is_found(self) -> None:
        text = INVENTORY + "| `/game` | page | missing | a game | x |\n"
        problems, _ = inventory.check(text, inventory.app_routes(self.app))
        self.assertEqual(problems, ["line 7: /game is no route of the app"])

    def test_a_route_listed_twice_is_found(self) -> None:
        text = INVENTORY + "| `/` | page | built | home again | x |\n"
        problems, _ = inventory.check(text, inventory.app_routes(self.app))
        self.assertEqual(problems, ["line 7: / is listed twice (first at line 3)"])

    def test_a_row_without_a_status_is_found(self) -> None:
        text = INVENTORY.replace("| missing | search", "| later | search")
        problems, _ = inventory.check(text, inventory.app_routes(self.app))
        self.assertEqual(
            problems, ["line 5: /api/search is marked 'later', not one of built, partial, missing"]
        )

    def test_a_feature_row_is_counted_by_its_status_and_a_route_row_is_not(self) -> None:
        text = INVENTORY + (
            "\n| feature | where | status | what | needs |\n|---|---|---|---|---|\n"
            "| pitch accent | x | missing | x | x |\n"
            "| headword | x | **built** | x | x |\n"
            "| readings | x | partial | x | x |\n"
            "| speech | x | missing | x | x |\n"
        )
        self.assertEqual(
            inventory.feature_counts(text), {"built": 1, "partial": 1, "missing": 2}
        )

    def test_the_script_fails_on_a_problem_and_passes_without(self) -> None:
        adr = self.app / "inventory.md"
        adr.write_text(INVENTORY)
        self.assertEqual(inventory.main(["x", str(adr), str(self.app)]), 0)
        adr.write_text(INVENTORY.replace("| `/[word]` | page | partial | a word | x |\n", ""))
        self.assertEqual(inventory.main(["x", str(adr), str(self.app)]), 1)


class Recipe(unittest.TestCase):
    def test_the_recipe_is_local_since_no_runner_has_the_checkout(self) -> None:
        plan = load("ci_plan")
        self.assertIn("e14-kiokun-inventory", plan.LOCAL_ONLY)
        recipes = (SCRIPTS.parent / "just" / "kiokun.just").read_text()
        self.assertIn("\ne14-kiokun-inventory:\n", recipes)


if __name__ == "__main__":
    unittest.main()
