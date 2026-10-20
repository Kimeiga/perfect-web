"""Exercise real just recipes with deterministic producers in an isolated tree.

These tests verify exit-status propagation, not compiler or browser behavior.
Only the producers are replaced. just parses and executes the checked-in recipes.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
JUSTFILE = ROOT / "justfile"

# Expected producer invocations. Checking the count prevents an unrelated setup
# failure from making a failure-propagation assertion pass accidentally.
RECIPES = {
    "resume-matrix": 1,
    "platform": 1,
    "each-typing": 1,
    "golden-marko": 1,
    "golden-pw": 1,
    "golden-both": 2,
    "e8-contracts": 3,
    "e8-placement": 1,
    "e8-binding": 4,
    "e8-wit": 2,
    "e9-latency": 1,
    "e9-oracle": 2,
    "one-parser": 2,
    "materialize": 3,
    "robustness": 1,
    "generality": 1,
    "evidence-corpus": 1,
    # The two guests' builds, then the audit's tests.
    "e8-host": 3,
}

PRODUCER = r'''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

log = Path(os.environ["GATE_TEST_LOG"])
previous = log.read_text().splitlines() if log.exists() else []
with log.open("a") as stream:
    stream.write(json.dumps(sys.argv[1:]) + "\n")
call = len(previous) + 1
# A failing producer may print a successful sub-suite before failing later.
# Neither grep matches nor reassuring text is the authoritative exit status.
for index in range(int(os.environ.get("GATE_TEST_LINES", "1"))):
    print(f"test result: ok. fixture {index}")
print("test fixture ... ok")
print("platform contract: fixture")
print("check-latency fixture")
print("  corpus fixture")
print("world fixture {}")
print("    record fixture {}")
if call == int(os.environ.get("GATE_TEST_FAIL_AT", "0")):
    print("injected producer failure", file=sys.stderr)
    sys.exit(17)
'''


# Only the optional hours cases install this private browser stand-in. It
# logs the real recipe's argv; the shell wrapper retains its actual native
# status separately from Just's own failure status.
HOURS_BROWSER = r'''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

with Path(os.environ["GATE_TEST_LOG"]).open("a") as stream:
    stream.write(json.dumps(sys.argv[1:]) + "\n")
print(os.environ["GATE_HOURS_OUTPUT"])
sys.exit(int(os.environ["GATE_HOURS_STATUS"]))
'''
HOURS_WRAPPER = r'''#!/usr/bin/env bash
python3 "$GATE_HOURS_CHILD" "$@"
status=$?
printf '%s\n' "$status" > "$GATE_HOURS_STATUS_FILE"
exit "$status"
'''


def executable(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)
    path.chmod(0o755)


class EvidenceGates(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        found = os.environ.get("JUST") or shutil.which("just")
        if not found:
            raise RuntimeError("just is required; these gate tests must not silently skip")
        cls.just = str(Path(found).resolve())

    def run_recipe(
        self,
        recipe: str,
        *,
        fail_at: int = 0,
        lines: int = 1,
        source: str | None = None,
        hours_import_source: str | None = None,
        hours_browser: tuple[int, str] | None = None,
    ) -> tuple[subprocess.CompletedProcess[str], list[list[str]], dict[str, str]]:
        with tempfile.TemporaryDirectory(prefix="pleris gate tests ") as directory:
            root = Path(directory)
            (root / "justfile").write_text(JUSTFILE.read_text() if source is None else source)
            # The files it imports (ADR-0253), where they are: a test's own
            # source is the justfile changed, its imports with it.
            if (JUSTFILE.parent / "just").is_dir():
                shutil.copytree(JUSTFILE.parent / "just", root / "just")
            if hours_import_source is not None:
                (root / "just/store-hours.just").write_text(hours_import_source)
            bin_dir = root / ".toolchain/prefix/bin"
            executable(bin_dir / "cargo", PRODUCER)
            executable(bin_dir / "git", "#!/usr/bin/env bash\nexit 0\n")
            if hours_browser is not None:
                executable(bin_dir / "pnpm", HOURS_WRAPPER)
                executable(bin_dir / "hours-browser-child.py", HOURS_BROWSER)
                executable(bin_dir / "rustc", "#!/usr/bin/env bash\necho 'rustc stand-in'\n")
                executable(bin_dir / "psql", "#!/usr/bin/env bash\necho 'postgres stand-in'\n")
                # The following stage is a private producer, never a real
                # mutation campaign. Its empty argv identifies its call.
                executable(root / "scripts/store_hours_mutations.py", PRODUCER)
                # The recipe runs its suite through the real runner
                # (ADR-0349's `run_suite.sh`, merged on train-10), whose
                # status these cases hold the recipe to: copied, not
                # stood in for.
                shutil.copy(ROOT / "scripts/run_suite.sh", root / "scripts/run_suite.sh")
            for name in ("pw-to-marko", "own-renderer"):
                executable(
                    root / "spikes" / name / "run.sh",
                    f'#!/usr/bin/env bash\nexec cargo renderer {name}\n',
                )
            # `e8-host`'s own script, its guests' directories, and the built
            # minimal guest it refuses to run without: the producer builds
            # nothing.
            audit = root / "spikes/wasmtime-component"
            audit.mkdir(parents=True, exist_ok=True)
            shutil.copy(ROOT / "spikes/wasmtime-component/audit.sh", audit / "audit.sh")
            (audit / "guest").mkdir()
            minimal = audit / "guest-minimal/target/wasm32-wasip2/release/spike_wasmtime_guest_minimal.wasm"
            minimal.parent.mkdir(parents=True)
            minimal.write_bytes(b"")
            for name in ("E8", "E9", "E2D"):
                (root / "docs/evidence" / name).mkdir(parents=True)
            (root / "runtime/pw-materialize/tests").mkdir(parents=True)
            (root / "tools").mkdir()
            (root / "tools/binding-report.py").write_text('print("binding report completed")\n')
            (root / "docs/evidence/E2D/corpus-enforcement.txt").write_text("1/1 enforce their declared invariant\n")
            env = os.environ.copy()
            # Do not let a caller's shell initialization change the tested shell.
            for name in ("BASH_ENV", "ENV", "SHELLOPTS", "BASHOPTS", "JUST_SHELL", "JUST_SHELL_ARG"):
                env.pop(name, None)
            env.update({
                "GATE_TEST_LOG": str(root / "calls.jsonl"),
                "GATE_TEST_FAIL_AT": str(fail_at),
                "GATE_TEST_LINES": str(lines),
                "NO_COLOR": "1",
            })
            if hours_browser is not None:
                status, output = hours_browser
                env.update({
                    "PW_STORE_DATABASE_URL": "postgresql://stand-in/hours",
                    "GATE_HOURS_CHILD": str(bin_dir / "hours-browser-child.py"),
                    "GATE_HOURS_STATUS": str(status),
                    "GATE_HOURS_OUTPUT": output,
                    "GATE_HOURS_STATUS_FILE": str(root / "docs/evidence/E14/hours-browser-status.txt"),
                })
            result = subprocess.run(
                [self.just, "--justfile", str(root / "justfile"), recipe],
                cwd=root,
                env=env,
                text=True,
                capture_output=True,
                timeout=15,
                check=False,
            )
            log = root / "calls.jsonl"
            calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            reports = {
                str(path.relative_to(root)): path.read_text()
                for path in (root / "docs/evidence").rglob("*.txt")
            }
            return result, calls, reports

    def assert_hours_browser_result(self, status: int, output: str, succeeds: bool) -> None:
        result, calls, reports = self.run_recipe(
            "e14-store-hours", hours_browser=(status, output)
        )
        prefix = [
            ["test", "--locked", "-p", "pw-dev-server", "--", "tests::store_hours::"],
            ["test", "--locked", "-p", "pw-dev-server", "--", "tests::store_hours::"],
            ["renderer", "own-renderer"],
            ["build", "--quiet", "--locked", "-p", "pw-dev-server", "-p", "kiokun-server"],
            ["exec", "playwright", "test", "e2e/store-search.spec.mjs", "--reporter=line"],
        ]
        self.assertEqual(calls[:len(prefix)], prefix, result.stdout + result.stderr)
        self.assertEqual(reports["docs/evidence/E14/hours-browser-status.txt"], f"{status}\n")
        if succeeds:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(calls, prefix + [[]], "the private following stage must run once")
        else:
            self.assertNotEqual(result.returncode, 0, "browser failure/no result was swallowed")
            self.assertEqual(calls, prefix, "another producer ran after the browser refusal")
            self.assertNotIn("== mutation controls", reports["docs/evidence/E14/store-hours.txt"])

    def test_hours_browser_pass_reaches_the_private_following_stage(self) -> None:
        self.assert_hours_browser_result(0, "\x1b[32m  7 passed (0.1s)\x1b[0m", True)

    def test_hours_browser_nonzero_with_pass_summary_stops_recipe(self) -> None:
        self.assert_hours_browser_result(42, "\x1b[32m  7 passed (0.1s)\x1b[0m", False)

    def test_hours_browser_failed_summary_stops_recipe(self) -> None:
        self.assert_hours_browser_result(1, "  1 failed (0.1s)", False)

    def test_hours_browser_missing_summary_stops_recipe(self) -> None:
        self.assert_hours_browser_result(0, "no browser result summary", False)

    def test_successful_producers_keep_every_recipe_usable(self) -> None:
        for recipe, count in RECIPES.items():
            with self.subTest(recipe=recipe):
                result, calls, _ = self.run_recipe(recipe)
                self.assertEqual(len(calls), count, result.stderr)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_each_failed_producer_stops_its_recipe(self) -> None:
        for recipe, count in RECIPES.items():
            for fail_at in range(1, count + 1):
                with self.subTest(recipe=recipe, fail_at=fail_at):
                    result, calls, _ = self.run_recipe(recipe, fail_at=fail_at)
                    self.assertGreater(len(calls), 0, result.stderr)
                    self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertEqual(len(calls), fail_at, "another producer ran after failure")
                    self.assertNotIn("ci: OK", result.stdout)

    def test_grouped_failure_does_not_write_post_test_claims(self) -> None:
        forbidden = {
            "e8-placement": "nothing listed means they were reproduced byte for byte",
            "e8-binding": "binding report completed",
            "e9-latency": "Median of 7 runs",
            "e9-oracle": "And the divergences",
        }
        for recipe, claim in forbidden.items():
            with self.subTest(recipe=recipe):
                result, calls, reports = self.run_recipe(recipe, fail_at=1)
                self.assertEqual(len(calls), 1, result.stderr)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn(claim, "\n".join(reports.values()))

    def test_renderer_failure_prevents_combined_pass_table(self) -> None:
        result, calls, _ = self.run_recipe("golden-both", fail_at=1)
        self.assertEqual(len(calls), 1, result.stderr)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("server HTML", result.stdout)

    def test_summary_drains_large_output_without_sigpipe(self) -> None:
        result, calls, _ = self.run_recipe("resume-matrix", lines=20000)
        self.assertEqual(len(calls), 1, result.stderr)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout.count("test result:"), 3)

    def test_failure_after_large_output_is_not_lost(self) -> None:
        result, calls, _ = self.run_recipe("resume-matrix", lines=20000, fail_at=1)
        self.assertEqual(len(calls), 1, result.stderr)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("17", result.stderr, "the producer failure, not SIGPIPE, must reach just")

    def test_a_failed_audit_prints_its_tests_output(self) -> None:
        # ADR-0026, amended 2026-10-13: the tests are the third call.
        result, calls, reports = self.run_recipe("e8-host", fail_at=3)
        self.assertEqual(len(calls), 3, result.stderr)
        self.assertNotEqual(result.returncode, 0)
        # The guests' builds print theirs; the tests' is the third.
        self.assertEqual(result.stdout.count("test fixture ... ok"), 3, result.stdout)
        self.assertIn("injected producer failure", result.stdout)
        self.assertIn("exit 17", result.stderr)
        self.assertNotIn("docs/evidence/E8/artifact-audit.txt", reports)
        # Control: tests that pass write the evidence, with their lines.
        result, calls, reports = self.run_recipe("e8-host")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("test fixture ... ok", reports["docs/evidence/E8/artifact-audit.txt"])

    def test_control_without_pipefail_reproduces_false_success(self) -> None:
        source = JUSTFILE.read_text()
        start = source.index("set shell :=")
        end = source.index("\n", start)
        source = source[:start] + 'set shell := ["bash", "-euc"]' + source[end:]
        result, calls, _ = self.run_recipe("each-typing", fail_at=1, source=source)
        self.assertEqual(len(calls), 1, result.stderr)
        self.assertEqual(result.returncode, 0, "negative control no longer reproduces the hole")

    def test_control_without_errexit_continues_group_after_failure(self) -> None:
        source = JUSTFILE.read_text()
        start = source.index("set shell :=")
        end = source.index("\n", start)
        source = source[:start] + 'set shell := ["bash", "-uo", "pipefail", "-c"]' + source[end:]
        result, calls, reports = self.run_recipe("e8-binding", fail_at=1, source=source)
        self.assertEqual(len(calls), 4, result.stderr)
        self.assertEqual(result.returncode, 0, "negative control no longer reproduces the hole")
        self.assertIn("binding report completed", "\n".join(reports.values()))


if __name__ == "__main__":
    unittest.main()
