"""The killers of a mutant, read from a `cargo test --no-fail-fast` run
(scripts/mutant_killers.py)."""

import importlib.util
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
spec = importlib.util.spec_from_file_location("mutant_killers", ROOT / "scripts/mutant_killers.py")
killers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(killers)

RUN = """\
    Finished `test` profile [unoptimized] target(s) in 0.31s
     Running unittests src/lib.rs (target/debug/deps/pw_core-0a1b)

running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/labels.rs (target/debug/deps/labels-2c3d)

running 2 tests
test a_label_names_the_first_element_with_its_id ... FAILED
test hidden_inputs_need_no_label ... ok

failures:

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/routes.rs (target/debug/deps/routes-4e5f)

running 5 tests
.....
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
"""


class Killers(unittest.TestCase):
    def test_a_failing_result_is_its_targets(self):
        self.assertEqual(killers.targets_failed(RUN), (["labels"], True))

    def test_cargos_closing_account_is_read_first(self):
        closing = RUN + "error: 2 targets failed:\n    `-p pw-core --test labels`\n    `-p pw-core --lib`\n"
        self.assertEqual(killers.targets_failed(closing), (["labels", "--lib"], True))

    def test_a_green_run_has_no_killers(self):
        green = RUN.replace("test result: FAILED. 1 passed; 1 failed", "test result: ok. 2 passed; 0 failed")
        self.assertEqual(killers.targets_failed(green), ([], True))

    def test_a_run_that_built_nothing_ran_nothing(self):
        # A mutant that does not build: no result, and so no killer named.
        self.assertEqual(killers.targets_failed("error[E0308]: mismatched types\n"), ([], False))


if __name__ == "__main__":
    unittest.main()
