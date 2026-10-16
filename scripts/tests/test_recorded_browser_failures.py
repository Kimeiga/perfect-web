"""`recorded_browser_failures.py`: the browser failures and failed controls
the evidence recorded while their recipes passed, through every version the
history holds (track evidence-integrity-2). Read on a repository of its own:
a failure recorded and recorded again green, one recorded on a side branch
and merged, one still committed, and the controls that are not failures.
"""

from __future__ import annotations

import importlib.util
import contextlib
import io
import os
import pathlib
import subprocess
import tempfile
import unittest
from unittest import mock

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent


def load():
    spec = importlib.util.spec_from_file_location("recorded_browser_failures", SCRIPTS / "recorded_browser_failures.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def evidence(recipe: str, browser: list[str], controls: list[str] | None = None) -> str:
    out = [f"Title\n\nproduced by: just {recipe}\ncommit: {'a' * 40}\n", "== the server", "test result: ok. 2 passed; 0 failed"]
    out += ["== the feed in three engines (e2e/feed.spec.mjs)", *browser]
    if controls:
        out += ["== the controls", *controls]
    out += ["== mutation controls", "a mutant: KILLED (3 of 3 tests fail)", "1 of 1 mutants killed"]
    return "\n".join(out) + "\n"


FAILED = ["  1) [webkit] › e2e/feed.spec.mjs:419:1 › Load more", "    Error: expect(x)", "  1 failed", "  2 did not run", "  9 passed (1.0m)"]
GREEN = ["  12 passed (1.0m)", "  1 skipped"]


class RecordedBrowserFailures(unittest.TestCase):
    def setUp(self):
        self.root = pathlib.Path(tempfile.mkdtemp(prefix="pw-recorded-failures-"))
        self.env = dict(os.environ, GIT_AUTHOR_DATE="2026-10-09T12:00:00", GIT_COMMITTER_DATE="2026-10-09T12:00:00")
        self.git("init", "-q", "-b", "master")
        self.git("config", "user.email", "t@example.com")
        self.git("config", "user.name", "t")

    def git(self, *args: str) -> str:
        return subprocess.run(["git", *args], cwd=self.root, env=self.env, capture_output=True, text=True, check=True).stdout

    def commit(self, files: dict[str, str], message: str) -> str:
        for rel, text in files.items():
            (self.root / rel).parent.mkdir(parents=True, exist_ok=True)
            (self.root / rel).write_text(text)
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)
        return self.git("rev-parse", "HEAD").strip()

    def test_each_failure_is_named_once_at_its_first_commit_and_whether_it_stands(self):
        feed = "docs/evidence/E14/feed.txt"
        first = self.commit({feed: evidence("e14-feed", FAILED)}, "feed, failed")
        self.commit({"README": "x\n"}, "no evidence")
        again = self.commit({feed: evidence("e14-feed", FAILED) + "\n"}, "feed, failed again, the same lines")
        self.commit({feed: evidence("e14-feed", GREEN)}, "feed, green")
        # A side branch records a failed control, and is merged.
        self.git("checkout", "-q", "-b", "track/x")
        side = self.commit(
            {"docs/evidence/E14/harness-T12.txt": evidence("e14-harness T12", GREEN, ["T12 pleris: negative ok, unsafe FAIL, harness ok"])},
            "harness",
        )
        self.git("checkout", "-q", "master")
        self.git("merge", "-q", "--no-ff", "-m", "merge", "track/x")
        merge = self.git("rev-parse", "HEAD").strip()
        # And one still committed.
        navigate = "docs/evidence/E14/navigate.txt"
        still = self.commit({navigate: evidence("e14-navigate", ["  ✘  1 [firefox] › e2e/navigate.spec.mjs:134:1 › x", "  1 failed"])}, "navigate")
        module = load()
        entries, read = module.found(self.root)
        got = [(e["path"], e["first"][0], e["last"][0], e["recipe"], e["now"], e["lines"]) for e in entries]
        self.assertEqual(
            got,
            [
                (
                    feed,
                    first,
                    again,
                    "just e14-feed",
                    False,
                    ["1) [webkit] › e2e/feed.spec.mjs:419:1 › Load more", "1 failed", "2 did not run"],
                ),
                # First on its branch, last in the merge that took it.
                ("docs/evidence/E14/harness-T12.txt", side, merge, "just e14-harness T12", True, ["T12 pleris: negative ok, unsafe FAIL, harness ok"]),
                (navigate, still, still, "just e14-navigate", True, ["✘  1 [firefox] › e2e/navigate.spec.mjs:134:1 › x", "1 failed"]),
            ],
        )
        self.assertGreaterEqual(read, 5)

    def test_a_committed_report_does_not_become_a_new_failed_suite(self):
        feed = "docs/evidence/E14/feed.txt"
        self.commit({feed: evidence("e14-feed", FAILED)}, "failed suite")
        module = load()
        scan = module.found
        output = io.StringIO()
        with mock.patch.object(module, "found", side_effect=lambda: scan(self.root)):
            with contextlib.redirect_stdout(output):
                self.assertEqual(module.main(), 0)
        self.commit(
            {"docs/evidence/E14/evidence-integrity-2.txt": output.getvalue()},
            "record the historical failure census",
        )
        entries, _ = scan(self.root)
        self.assertEqual([entry["path"] for entry in entries], [feed])

    def test_a_green_run_a_skip_a_mutant_and_a_cargo_failure_are_no_browser_failure(self):
        # The control: counts of nothing failed, a skip, a mutation script's
        # KILLED, and `cargo test`'s own summary line.
        module = load()
        lines = ["  0 failed", "  3 skipped", "  12 passed (1.0m)", "test result: FAILED. 0 passed; 11 failed;", "x: KILLED (2 failed)"]
        self.assertEqual(module.failures(lines), [])
        self.assertEqual(module.failures(["  1 flaky"]), ["1 flaky"])
        self.assertEqual(module.failures(["  1 interrupted"]), ["1 interrupted"])


if __name__ == "__main__":
    unittest.main()
