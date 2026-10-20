"""Guide-local discovery/process ownership around the shared unittest protocol.

All terminal result classification belongs to unittest_protocol. This module
owns fresh files, controlled imports, bounded child lifetime and retained logs.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import NamedTuple

import unittest_protocol

ROOT = Path(__file__).resolve().parent.parent
PRODUCER = ROOT / "scripts/unittest_producer.py"
DEFAULT_TIMEOUT = 60


class PythonRun(NamedTuple):
    result: unittest_protocol.Result
    output: str
    directory: Path
    status: object


class DiscoveryRefused(ValueError):
    pass


def _directory(evidence_root, label):
    if evidence_root is not None:
        evidence_root = Path(evidence_root)
        evidence_root.mkdir(parents=True, exist_ok=True)
    return Path(tempfile.mkdtemp(prefix="pw-guide-" + label + "-", dir=evidence_root))


def _launch(directory, command, search_dirs, timeout):
    env = dict(os.environ)
    env.pop("KIOKUN_DATA", None)
    env.pop("KIOKUN_APP", None)
    env["PYTHONPATH"] = os.pathsep.join(str(Path(p)) for p in (*search_dirs, ROOT))
    # -B prevents writes; a fresh prefix also prevents reading a prior mutant's
    # timestamp/size cache. No existing cache is deleted or trusted.
    env["PYTHONPYCACHEPREFIX"] = str(directory / "fresh-bytecode")
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    timed_out = False
    with (directory / "stdout.txt").open("w") as stdout, (directory / "stderr.txt").open("w") as stderr:
        child = subprocess.Popen(command, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, text=True)
        try:
            actual_status = child.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            child.terminate()
            try:
                actual_status = child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                actual_status = child.wait()
    status = None if timed_out else actual_status
    output = (directory / "stdout.txt").read_text(errors="replace") + (directory / "stderr.txt").read_text(errors="replace")
    metadata = {"command": list(map(str, command)), "cwd": str(ROOT), "pid": child.pid,
                "actual_status": actual_status, "validated_status": status,
                "timed_out": timed_out, "child_reaped": True,
                "deployment_env_absent": True, "fresh_bytecode_prefix": env["PYTHONPYCACHEPREFIX"]}
    (directory / "process.json").write_text(json.dumps(metadata, indent=2) + "\n")
    return status, output


def _read_report(path):
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return ""


_NATIVE_DISCOVERY = """import json,sys,unittest
loader=unittest.TestLoader()
suite=loader.discover(sys.argv[1],pattern=sys.argv[2])
def walk(node):
 for item in node:
  if isinstance(item,unittest.TestSuite):yield from walk(item)
  else:yield item.id()
ids=sorted(walk(suite))
if loader.errors:raise RuntimeError('native discovery failed: '+repr(loader.errors))
print(json.dumps(ids))
"""


def freeze(selector, search_dir, pattern, expected_count=None, evidence_root=None, timeout=DEFAULT_TIMEOUT):
    """Compare native discover and producer inventory once, before mutation."""
    search_dir = Path(search_dir)
    native_dir = _directory(evidence_root, "native-inventory")
    status, output = _launch(native_dir, [sys.executable, "-B", "-c", _NATIVE_DISCOVERY, str(search_dir), pattern], [search_dir], timeout)
    try:
        native = unittest_protocol.strict_json((native_dir / "stdout.txt").read_text())
    except (OSError, ValueError, TypeError, RecursionError) as error:
        raise DiscoveryRefused(f"native inventory refused: {error}; evidence {native_dir}\n{output}") from error
    if (status != 0 or not isinstance(native, list) or not native
            or any(type(item) is not str for item in native) or native != sorted(set(native))):
        raise DiscoveryRefused(f"invalid native inventory; evidence {native_dir}\n{output}")
    inventory_dir = _directory(evidence_root, "producer-inventory")
    report = inventory_dir / "inventory.json"
    assert not report.exists()
    status, output = _launch(inventory_dir, [sys.executable, "-B", str(PRODUCER), "--report", str(report), "--inventory", selector], [search_dir], timeout)
    try:
        inventory = unittest_protocol.strict_json(_read_report(report))
    except (ValueError, TypeError, RecursionError) as error:
        raise DiscoveryRefused(f"producer inventory refused: {error}; evidence {inventory_dir}\n{output}") from error
    if (status != 0 or not isinstance(inventory, dict) or set(inventory) != {"schema", "ids"}
            or type(inventory["schema"]) is not int or inventory["schema"] != unittest_protocol.SCHEMA
            or inventory["ids"] != native
            or (expected_count is not None and len(native) != expected_count)):
        raise DiscoveryRefused(f"producer/native frozen inventory mismatch; evidence {inventory_dir}\n{output}")
    (inventory_dir / "frozen.json").write_text(json.dumps(native, indent=2) + "\n")
    return native


def run(selector, expected, search_dir, evidence_root=None, timeout=DEFAULT_TIMEOUT):
    """Launch the canonical producer; delegate every terminal proof decision."""
    directory = _directory(evidence_root, "result")
    expected_file = directory / "expected.json"
    expected_file.write_text(json.dumps(expected, indent=2) + "\n")
    report = directory / "result.json"
    assert not report.exists()
    command = [sys.executable, "-B", str(PRODUCER), "--report", str(report), "--expected", str(expected_file), selector]
    status, output = _launch(directory, command, [search_dir], timeout)
    text = _read_report(report)
    outcome = unittest_protocol.validate_result(status, text, expected)
    summary = {"valid": outcome.valid, "reason": outcome.reason, "passed": outcome.passed,
               "assertion_failed": outcome.assertion_failed, "body_errors": outcome.body_errors,
               "executed": outcome.executed, "report_sha256": hashlib.sha256(text.encode()).hexdigest()}
    (directory / "validated.json").write_text(json.dumps(summary, indent=2) + "\n")
    output += "\nPython structured receipt: " + str(directory) + "\n" + json.dumps(summary) + "\n"
    return PythonRun(outcome, output, directory, status)