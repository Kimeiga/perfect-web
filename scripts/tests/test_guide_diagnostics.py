"""`guide_diagnostics.py`: the guide's reference of codes is the registry's.

ADR-0343 (a guide whose examples compile). The committed page is what the
script writes from `codes.rs` today, so a code added, renamed or reworded
without the page written again fails `just evidence-gates`. The script's
reading of the registry is tested on a registry of its own, with its
controls.
"""

import importlib.util
import pathlib
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


guide = load("guide_diagnostics")

REGISTRY = '''
codes! {
    // --- syntax (PW00xx) ---

    EXPECTED = "PW0001" / expected / 1, Syntax, "the parser expected a different token here";
    // ADR-0322: `a | b & c` was `(a | b) & c` (ADR-0322 again).
    LOGICAL_MIXED = "PW0031" / logical_mixed / 1, Syntax,
        "`|` and `&` mixed in one chain are parenthesized";

    // ADR-0098: written twice.
    DECLARED_TWICE = "PW0028" / declared_twice / 1, Resolution,
        "a \\"name\\" is written once";
    UNCITED = "PW0029" / uncited / 1, Resolution, "a sentence";
}
'''


class Reading(unittest.TestCase):
    def test_each_entry_with_its_invariant_and_the_adrs_its_comment_cites(self):
        found = guide.entries(REGISTRY)
        self.assertEqual(
            found,
            [
                ("PW0001", "expected", 1, "Syntax", "the parser expected a different token here", []),
                ("PW0031", "logical_mixed", 1, "Syntax",
                 "`|` and `&` mixed in one chain are parenthesized", ["0322"]),
                ("PW0028", "declared_twice", 1, "Resolution", 'a "name" is written once', ["0098"]),
                # A comment belongs to the entry right after it, not the next.
                ("PW0029", "uncited", 1, "Resolution", "a sentence", []),
            ],
        )

    def test_a_line_that_is_no_entry_is_refused_not_skipped(self):
        broken = REGISTRY.replace('    UNCITED = "PW0029"', '    UNCITED = PW0029')
        with self.assertRaises(ValueError):
            guide.entries(broken)

    def test_a_retired_code_with_its_reason_continued_across_lines(self):
        # As codes.rs writes it: the tuple across lines, a trailing comma,
        # and the reason continued with `\` at a line's end.
        source = (
            'pub const RETIRED: &[(&str, &str)] = &[(\n'
            '    "PW5102",\n'
            '    "a key must separate the build. \\\n'
            '     Retired 2026-08-06: the compiler injects it.",\n'
            ')];\n'
        )
        self.assertEqual(
            guide.retired(source),
            [("PW5102", "a key must separate the build. Retired 2026-08-06: the compiler injects it.")],
        )

    def test_a_pipe_in_a_cell_is_escaped(self):
        self.assertEqual(guide.cell("`|` and `&`"), "`\\|` and `&`")

    def test_an_owner_without_a_title_is_written_from_its_name(self):
        self.assertEqual(guide.title("NewTrackOwner"), "New track owner")
        self.assertEqual(guide.title("Syntax"), "Syntax")


class Current(unittest.TestCase):
    def test_the_committed_page_is_what_the_registry_writes(self):
        committed = guide.PAGE.read_text()
        self.assertEqual(
            committed,
            guide.page(),
            "docs/guide/diagnostics.md is not current: run python3 scripts/guide_diagnostics.py",
        )

    def test_every_registered_code_has_a_row(self):
        page = guide.page()
        for code, *_ in guide.entries(guide.CODES.read_text()):
            self.assertIn(f"| {code} |", page)


if __name__ == "__main__":
    unittest.main()
