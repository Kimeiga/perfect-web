#!/usr/bin/env python3
"""Whole controls for the shared executable-program validation authority.

Every baseline command must build, execute tests and succeed. Source restores
on signals and failures. A bound stop fails the campaign; it never kills a
mutant by timeout. No filter is implemented: this runs every listed control.
"""
import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CLI = ROOT / "compiler/pw-cli/src/main.rs"
PROGRAM = ROOT / "compiler/pw-core/src/program.rs"
BUILD = ROOT / "compiler/pw-core/src/build.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
WIT = ROOT / "compiler/pw-core/src/wit.rs"

MUTANTS = [
    ('CLI check uses only partial-program semantics', CLI, 'pw_core::check::check_program_sources(&all)', 'pw_core::check::check_sources(&all)'),
    ('CLI build discards a complete validation failure', CLI, '    let build = match pw_core::program::validate_sources(&sources).into_build() {\n        Ok(b) => b,\n        Err(e) => {\n            eprintln!("pw: {e}");\n            return ExitCode::FAILURE;\n        }\n    };\n', '    let build = match pw_core::program::validate_sources(&sources).into_build() {\n        Ok(b) => b,\n        Err(e) => {\n            eprintln!("pw: {e}");\n            return ExitCode::SUCCESS;\n        }\n    };\n'),
    ('support refusals disappear from check diagnostics', PROGRAM, '    for refusal in refusals {', '    for refusal in refusals.into_iter().take(0) {'),
    ('depended placeholders are accepted', BUILD, '                .filter(|target| placeholders.contains(target))', '                .filter(|_| false)'),
    ('page-plan refusals disappear', BUILD, '        for p in &self.pages {', '        for p in self.pages.iter().take(0) {'),
    ('blocked renderer parts are accepted', BUILD, '        for blocked in t.blocked() {', '        for blocked in t.blocked().into_iter().take(0) {'),
    ('WIT collision ownership points at the wrong file', PROGRAM, 'crate::wit::WitError::Collision { second, .. } => second.clone(),', 'crate::wit::WitError::Collision { .. } => String::new(),'),
    ('a proof applies to other HIR of equal length', LOWER, '                    .all(|(unit, hir)| std::ptr::eq(&unit.hir, *hir)),', '                    .all(|_| true),'),
    ('support diagnostics have no responsible source span', PROGRAM, 'primary_span: span', 'primary_span: 0..0'),
    ('placeholder stage metadata names a different stage', PROGRAM, 'Self::Placeholder => "backend_placeholder",', 'Self::Placeholder => "backend_component",'),
    ('a partial inspection plan is written', BUILD, '        if !refused.is_empty() {', '        if false && !refused.is_empty() {'),
    ('early WIT failure erases independent page refusal', BUILD, 'early.extend(build.support_refusals());', ''),
    ('an unmappable export loses source ownership', WIT, 'LocatedError::at(error, declaration)', 'LocatedError::from(error)'),
    ('public report data can mint a validated plan', PROGRAM, '(true, Ok(build)) => Plan::Validated(Executable { build }),', '(_, Ok(build)) => Plan::Validated(Executable { build }),'),
    ('a caller can mint a proof from a foreign context', LOWER, "impl<'a> Checked<'a> {", "impl<'a> Checked<'a> {\n    pub fn of(units: &'a [crate::check::Unit], cx: Context<'a>) -> Self {\n        Self { cx, units, pages: std::cell::OnceCell::new() }\n    }"),
    ('syntax refusal omits clean sibling diagnostics', PROGRAM, 'if parsed.iter().any(|file| !file.ok()) {', 'if false && parsed.iter().any(|file| !file.ok()) {'),
    ('a page failure hides its signal failure', BUILD, '(Err(why), Err(_)) => early.push(', '(Err(why), Err(_)) => Vec::<crate::program::Refusal>::new().push('),
 ]
COMMANDS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--doc", "backend::lower::Checked"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib", "backend::lower::checked_input_tests"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "check_is_build"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-cli", "--test", "check_is_build"],
]
BOUND = 180

def run_tests():
    built, passed, failed, stopped = True, 0, 0, False
    outputs = []
    for command in COMMANDS:
        child = subprocess.Popen(command, cwd=ROOT, start_new_session=True,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            out, _ = child.communicate(timeout=BOUND)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            out, _ = child.communicate()
            stopped = True
        outputs.append(out)
        summaries = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        count = sum(int(p) + int(f) for p, f in summaries)
        if not summaries or not count:
            built = False
        passed += sum(int(p) for p, _ in summaries)
        failed += sum(int(f) for _, f in summaries)
        if child.returncode != 0 and not any(int(f) for _, f in summaries):
            built = False
    return built, passed, failed, stopped, outputs

def main():
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed, stopped, outputs = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or not passed or failed or stopped:
        print("FAIL: baseline did not build, execute tests and succeed")
        print("\n".join(outputs))
        return 1
    survivors = stops = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}", flush=True)
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed, stopped, outputs = run_tests()
        finally:
            path.write_text(original)
        if stopped:
            verdict = "BOUND STOP (campaign fails; not a kill)"
            stops += 1
        elif not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)
    print(f"{len(MUTANTS) - survivors - stops} of {len(MUTANTS)} mutants killed; {stops} bound stops", flush=True)
    return 1 if survivors or stops else 0

if __name__ == "__main__":
    sys.exit(main())
