"""PR browser scope must not quietly skip application-level changes."""

import unittest

from scripts.quality_scope import browser_relevant


class BrowserScope(unittest.TestCase):
    def test_app_code_runs_browsers(self):
        for path in (
            "compiler/pw-core/src/tables.rs",
            "runtime/pw-table-memory/src/lib.rs",
            "spikes/own-renderer/e2e/feed.spec.mjs",
            "examples/feed/app.pw",
            "packages/pw-platform-web/src/Clock.pw",
            "pnpm-lock.yaml",
            "Cargo.lock",
        ):
            with self.subTest(path=path):
                self.assertTrue(browser_relevant([path]))

    def test_documents_and_workflows_do_not_force_full_suite(self):
        self.assertFalse(browser_relevant([
            "README.md",
            "docs/STATUS.md",
            ".github/workflows/quality.yml",
            "scripts/tests/test_quality_scope.py",
        ]))

    def test_relevant_script_runs_browsers(self):
        self.assertTrue(browser_relevant(["scripts/ci_plan.py"]))
        self.assertTrue(browser_relevant(["scripts/browser_host_map_mutations.py"]))

    def test_empty_changes_do_not_run_browsers(self):
        self.assertFalse(browser_relevant([]))
