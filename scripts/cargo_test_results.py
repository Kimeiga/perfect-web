"""Pure validation of the pinned Cargo/libtest result contract.

Callers own command execution, fresh output, expected binaries and restoration.
This parser proves only a complete, consistent command result with nonempty aggregate, not provenance
of an arbitrary Rust assertion or a process sandbox boundary.
"""
import re
from typing import NamedTuple


class Result(NamedTuple):
    valid: bool
    passed: int = 0
    failed: int = 0
    failure_names: tuple = ()
    reason: str = ""


def parse(status, output):
    """Refuse unknown exits, build/stop errors and inconsistent binary results."""
    if type(status) is not int or status not in (0, 101) or not isinstance(output, str):
        return Result(False, reason="unexpected Cargo exit or output")
    output = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", output)
    if ("error[" in output or "could not compile" in output
            or re.search(r"^\s*(?:process didn't exit successfully:.*\(signal:|signal: \d+,|memory bound:.*stopped)", output, re.M)):
        return Result(False, reason="build or process-stop error")
    summaries = re.findall(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;", output, re.M)
    if not summaries:
        return Result(False, reason="missing libtest results")
    passed = failed = 0
    for verdict, p, f in summaries:
        p, f = int(p), int(f)
        if (verdict == "ok") != (f == 0):
            return Result(False, reason="empty or contradictory binary result")
        passed += p
        failed += f
    if not passed + failed:
        return Result(False, reason="empty command result")
    if status != (101 if failed else 0):
        return Result(False, reason="Cargo exit contradicts results")
    names = set(re.findall(r"^test (\S+) \.\.\. FAILED\s*$", output, re.M))
    names.update(re.findall(r"^---- (\S+) stdout ----\s*$", output, re.M))
    if names and not failed:
        return Result(False, reason="failure names contradict results")
    return Result(True, passed, failed, tuple(sorted(names)))
