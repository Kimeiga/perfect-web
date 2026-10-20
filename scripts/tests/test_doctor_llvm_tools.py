"""`scripts/doctor.sh`, `rust-toolchain.toml` and the rustup component the
fuzzer's coverage needs (track fuzz-floor, 2026-10-20).

`llvm-tools` is where `llvm-profdata` comes from, and `pw-fuzz` and its tests
read coverage with it. The toolchain file lists it, so that a fresh install
(a CI runner's) has it; `just doctor` says so on a machine without it, so
that the first sign is not a test that fails.

The doctor is run with a stand-in `rustup` first on `PATH` that lists the
components it is told to; the rest of what the doctor checks is the machine's
own, and not looked at here.
"""

import os
import pathlib
import re
import shlex
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
DOCTOR = ROOT / "scripts/doctor.sh"


def doctor(installed: list[str]) -> subprocess.CompletedProcess:
    """The doctor, on a machine whose `rustup component list --installed`
    lists `installed`."""
    with tempfile.TemporaryDirectory(prefix="pw-doctor-") as tmp:
        rustup = pathlib.Path(tmp) / "rustup"
        rustup.write_text(
            "#!/bin/sh\n"
            'case "$*" in\n'
            f"  \"component list --installed\") printf '%s\\n' {shlex.quote(chr(10).join(installed))} ;;\n"
            '  "target list --installed") echo wasm32-wasip2 ;;\n'
            "esac\n"
        )
        rustup.chmod(0o755)
        env = dict(os.environ, PATH=f"{tmp}{os.pathsep}{os.environ['PATH']}")
        return subprocess.run(
            ["bash", str(DOCTOR)], cwd=ROOT, env=env, capture_output=True, text=True, check=False
        )


def llvm_tools_lines(done: subprocess.CompletedProcess) -> list[str]:
    return [line for line in done.stdout.splitlines() if "llvm-tools" in line]


def missing(done: subprocess.CompletedProcess) -> int:
    """How many required items the doctor says are missing."""
    found = re.search(r"doctor: (\d+) required item\(s\) missing", done.stdout)
    return int(found.group(1)) if found else 0


WITH = ["cargo-x86_64-unknown-linux-gnu", "llvm-tools-x86_64-unknown-linux-gnu", "rustc-x86_64-unknown-linux-gnu"]
WITHOUT = ["cargo-x86_64-unknown-linux-gnu", "clippy-x86_64-unknown-linux-gnu", "rustc-x86_64-unknown-linux-gnu"]


class DoctorLlvmTools(unittest.TestCase):
    def test_a_toolchain_with_the_component_is_said_to_have_it(self) -> None:
        lines = llvm_tools_lines(doctor(WITH))
        self.assertEqual(len(lines), 1, lines)
        self.assertIn("installed", lines[0])
        self.assertNotIn("MISSING", lines[0])

    def test_the_component_by_its_old_name_is_the_component(self) -> None:
        lines = llvm_tools_lines(doctor(["llvm-tools-preview-x86_64-unknown-linux-gnu"]))
        self.assertEqual(len(lines), 1, lines)
        self.assertIn("installed", lines[0])

    def test_a_toolchain_without_it_is_said_to_miss_it_and_how_to_get_it(self) -> None:
        lines = llvm_tools_lines(doctor(WITHOUT))
        self.assertEqual(len(lines), 2, lines)
        self.assertIn("MISSING", lines[0])
        self.assertIn("pw-fuzz", lines[0])
        self.assertIn("rustup component add llvm-tools", lines[1])

    def test_a_missing_component_is_one_more_required_item_missing(self) -> None:
        # The machine's own gaps are the same in both runs.
        self.assertEqual(missing(doctor(WITHOUT)), missing(doctor(WITH)) + 1)


class ToolchainFile(unittest.TestCase):
    def test_the_pinned_toolchain_lists_llvm_tools(self) -> None:
        text = (ROOT / "rust-toolchain.toml").read_text()
        found = re.search(r"^components\s*=\s*\[(.*?)\]", text, re.M | re.S)
        self.assertIsNotNone(found, "rust-toolchain.toml lists no components")
        names = re.findall(r'"([^"]+)"', found.group(1))
        self.assertIn("llvm-tools", names)
        # What it listed before is still listed.
        self.assertIn("rustfmt", names)
        self.assertIn("clippy", names)


if __name__ == "__main__":
    unittest.main()
