"""`e9-latency` names the machine it measured, its record and its load
(track evidence-integrity-2, the integrator's ruling 3 of 2026-10-15).

Its evidence said "on the machine in docs/environment/macbook.md" wherever it
ran, and the check's latency there is the machine's as much as the
compiler's: a busy machine stretches the wall clock and, less, the checking
thread's CPU time (track evidence-integrity, finding 4). The recipe is run
here as `test_builds_before_suites.py` runs a recipe, by `just` with its
programs stood in for (its measurement is not taken), and its evidence must
name this machine's record by the rule `scripts/record-environment.sh`
writes it by, and the load before and after.
"""

from __future__ import annotations

import pathlib
import platform
import sys
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from test_builds_before_suites import run_full  # noqa: E402


def this_machine() -> str:
    """The record `scripts/record-environment.sh` writes for this host."""
    system = platform.system()
    if system == "Darwin":
        return "macbook"
    if system == "Linux":
        try:
            version = pathlib.Path("/proc/version").read_text()
        except OSError:
            version = ""
        return "pc-wsl" if "microsoft" in version.lower() else "linux"
    return "unknown"


class LatencyNamesItsMachine(unittest.TestCase):
    def test_the_evidence_names_this_machines_record_and_its_load(self):
        status, _, said, evidence = run_full("e9-latency")
        self.assertEqual(status, 0, said)
        text = evidence["docs/evidence/E9/check-latency.txt"]
        record = f"docs/environment/{this_machine()}.md"
        host = next((line for line in text.splitlines() if line.startswith("host: ")), "")
        self.assertTrue(host.endswith(record), host)
        self.assertIn(f"on the machine in\n{record}:", text)
        self.assertRegex(text, r"(?m)^load before: \S")
        self.assertRegex(text, r"(?m)^load after: \S")
        self.assertIn("checking thread's CPU time", text)
        # And the record the recipe writes is the one `record-environment.sh`
        # chooses by the same rule.
        script = (HERE.parent / "record-environment.sh").read_text()
        self.assertIn("grep -qi microsoft /proc/version", script)


if __name__ == "__main__":
    unittest.main()
