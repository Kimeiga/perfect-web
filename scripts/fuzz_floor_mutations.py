#!/usr/bin/env python3
"""Mutation controls for track fuzz-floor: the coverage-guided fuzzer
(`tools/fuzz`) measures, or it fails; it never reports "0 covered regions ...
0 finding(s)" and exits 0 (found 2026-10-20, on the PC: it looked for
`llvm-profdata` at one machine's own path).

Each mutant undoes one piece:
1. where the tool is: at one machine's path and not the active toolchain's
   (the sysroot fixed, the host cut to its first component, a wrong layout,
   the sysroot untrimmed); the host read from the wrong line; the override
   (`PW_LLVM_BIN_DIR`) ignored, or an empty one taken; a tool taken to be
   there that is not, or to run that does not (it fails, or cannot be
   started);
2. what a profile says: a function's count read from `Function name:` (the
   first version's mistake, which measured 69 regions for a compiler), a zero
   count counted, block counts not read or a zero one counted;
3. a measurement that did not happen is not "no new regions": a tool that
   cannot be run, a tool that fails, and a listing of no function are read as
   nothing covered; a run that cannot be measured is read as covering
   nothing;
4. the floor: a target that covered nothing passes, the floor names the
   wrong targets or is not asked, a finding does not end the run with status
   1, the run's error ends with status 0, the checks before the build are
   skipped, an argument it does not know is ignored, a count that is no
   number says nothing of it;
4b. the campaign (an overnight run, one process a target): the time limit is
   ignored, may end a run before its first input, or ends one it should not;
   a target says where it is at every input, or never, and does not say a
   finding, its start or its end; a failure is told from another by the
   input's length, or every one is a finding of its own; the profile of a run
   that failed, or survived, is kept (a campaign that fails often fills the
   disk); an input that does not end is not killed, its limit is ignored, it
   is called an abort, or a panic is told from an abort by another status;
   `--seconds`, `--hang` and `--only` take their values and ignore them, or
   the limits leave one out; `--only` names nothing, accepts a target that
   does not exist, or a target not named runs; the progress line's rate or
   time is in other units;
5. `just doctor` names the component (`scripts/doctor.sh`): it does not look,
   it never finds it, it does not count it among the missing; and
   `rust-toolchain.toml` lists it.

Each runs the suite that holds it: `cargo test -p pw-fuzz` (the unit tests of
`tools/fuzz/src/tests.rs`, which use the real `llvm-profdata` once, and the
program run as a person runs it, `tools/fuzz/tests/floor.rs`), or the Python
tests of `scripts/tests/test_doctor_llvm_tools.py`. At least one test must
fail.

Run from the repository root; `just e14-fuzz-floor` records the output. The
sources are restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
FUZZ = ROOT / "tools/fuzz/src/main.rs"
DOCTOR = ROOT / "scripts/doctor.sh"
TOOLCHAIN = ROOT / "rust-toolchain.toml"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    # 1. Where the tool is.
    (
        "the sysroot is a fixed path",
        "fuzz",
        FUZZ,
        "    Ok(Path::new(sysroot.trim())\n",
        '    Ok(Path::new(&sysroot[..0]).join("/usr")\n',
    ),
    (
        "the host is cut to its first component",
        "fuzz",
        FUZZ,
        "        .join(host)\n",
        "        .join(host.split('-').next().unwrap_or(host))\n",
    ),
    (
        "the layout under the sysroot is another",
        "fuzz",
        FUZZ,
        '        .join("lib/rustlib")\n',
        '        .join("lib/rust")\n',
    ),
    (
        "the sysroot is not trimmed of its newline",
        "fuzz",
        FUZZ,
        "    Ok(Path::new(sysroot.trim())\n",
        "    Ok(Path::new(&sysroot)\n",
    ),
    (
        "the host is read from the wrong line",
        "fuzz",
        FUZZ,
        '.find_map(|line| line.strip_prefix("host: "))',
        '.find_map(|line| line.strip_prefix("release: "))',
    ),
    (
        "the override is ignored",
        "fuzz",
        FUZZ,
        "    if let Some(dir) = over_ride.filter(|d| !d.is_empty()) {\n",
        "    if let Some(dir) = over_ride.filter(|_| false) {\n",
    ),
    (
        "an empty override is taken",
        "fuzz",
        FUZZ,
        "    if let Some(dir) = over_ride.filter(|d| !d.is_empty()) {\n",
        "    if let Some(dir) = over_ride.filter(|_| true) {\n",
    ),
    (
        "a tool that is not there is taken to be",
        "fuzz",
        FUZZ,
        "    if !path.is_file() {\n",
        "    if false {\n",
    ),
    (
        "a tool that fails is taken to run",
        "fuzz",
        FUZZ,
        "        Ok(out) if out.status.success() => Ok(path),\n",
        "        Ok(out) if out.status.code().is_some() => Ok(path),\n",
    ),
    (
        "a tool that cannot be started is taken to run",
        "fuzz",
        FUZZ,
        '        Err(e) => Err(format!("cannot run {}: {e}", path.display())),\n',
        "        Err(_) => Ok(path),\n",
    ),
    # 2. What a profile says.
    (
        "a function's count is read from `Function name:`",
        "fuzz",
        FUZZ,
        'strip_prefix("Function count: ")',
        'strip_prefix("Function name: ")',
    ),
    (
        "a function that never ran counts",
        "fuzz",
        FUZZ,
        "            && n.trim().parse::<u64>().unwrap_or(0) > 0\n",
        "            && n.trim().parse::<u64>().is_ok()\n",
    ),
    (
        "block counts are not read",
        "fuzz",
        FUZZ,
        'strip_prefix("Block counts: [")',
        'strip_prefix("Block count: [")',
    ),
    (
        "a block that never ran counts",
        "fuzz",
        FUZZ,
        "                if c.trim().parse::<u64>().unwrap_or(0) > 0 {\n",
        "                if c.trim().parse::<u64>().is_ok() {\n",
    ),
    # 3. A measurement that did not happen is not "nothing new".
    (
        "a tool that cannot be run is read as no regions",
        "fuzz",
        FUZZ,
        '        Err(e) => return Err(format!("cannot run {}: {e}", profdata.display())),\n',
        "        Err(_) => return Ok(Vec::new()),\n",
    ),
    (
        "a tool that fails is read as no regions",
        "fuzz",
        FUZZ,
        "    if !out.status.success() {\n        return Err(format!(\n"
        '            "{} could not read {}: {}\\n  was the harness built with -C instrument-coverage?",\n'
        "            profdata.display(),\n            profraw.display(),\n            first_line(&out.stderr)\n"
        "        ));\n    }\n    let found",
        "    let found",
    ),
    (
        "a listing of no function is accepted",
        "fuzz",
        FUZZ,
        "    if found.is_empty() {\n",
        "    if false {\n",
    ),
    (
        "a run that cannot be measured covers nothing",
        "fuzz",
        FUZZ,
        "        let new = regions(profdata, &profraw)?\n",
        "        let new = regions(profdata, &profraw).unwrap_or_default()\n",
    ),
    # 4. The floor.
    (
        "a target that covered nothing passes the floor",
        "fuzz",
        FUZZ,
        "        .filter(|(_, o)| o.regions == 0)\n",
        "        .filter(|(_, o)| o.regions == usize::MAX)\n",
    ),
    (
        "the floor names the targets that covered something",
        "fuzz",
        FUZZ,
        "        .filter(|(_, o)| o.regions == 0)\n",
        "        .filter(|(_, o)| o.regions > 0)\n",
    ),
    (
        "the floor is not asked",
        "fuzz",
        FUZZ,
        "    floor(&outcomes)?;\n",
        "    let _ = floor(&outcomes);\n",
    ),
    (
        "a finding does not end the run with status 1",
        "fuzz",
        FUZZ,
        "    Ok(if findings > 0 { 1 } else { 0 })\n",
        "    Ok(0)\n",
    ),
    (
        "a run that measured nothing ends with status 0",
        "fuzz",
        FUZZ,
        "            std::process::exit(2);\n",
        "            std::process::exit(0);\n",
    ),
    (
        "the checks before the build are skipped",
        "fuzz",
        FUZZ,
        "    let profdata = llvm_tool(&root, PROFDATA)?;\n",
        "    let profdata = PathBuf::from(PROFDATA);\n",
    ),
    (
        "an argument it does not know is ignored",
        "fuzz",
        FUZZ,
        '            other => return Err(format!("unknown argument {other:?}\\n{USAGE}")),\n',
        "            _ => {}\n",
    ),
    (
        "a count that is no number says nothing of it",
        "fuzz",
        FUZZ,
        '        .map_err(|_| format!("{flag} wants a whole number, not {text:?}\\n{USAGE}"))\n',
        "        .map_err(|_| USAGE.to_string())\n",
    ),
    # 4b. The campaign: what ends a run, what it says, what a finding is.
    (
        "the time limit is ignored",
        "fuzz",
        FUZZ,
        "limits.time.is_none_or(|t| began.elapsed() < t)",
        "true",
    ),
    (
        "a time limit may end a run before its first input",
        "fuzz",
        FUZZ,
        "(execs == 0 || limits.time.is_none_or(|t| began.elapsed() < t))",
        "limits.time.is_none_or(|t| began.elapsed() < t)",
    ),
    (
        "a time limit not yet reached ends the run",
        "fuzz",
        FUZZ,
        "|t| began.elapsed() < t",
        "|t| began.elapsed() >= t",
    ),
    (
        "a target says where it is at every input",
        "fuzz",
        FUZZ,
        "        if said.elapsed() >= limits.say_every {\n",
        "        if true {\n",
    ),
    (
        "a target never says where it is",
        "fuzz",
        FUZZ,
        "        if said.elapsed() >= limits.say_every {\n",
        "        if false {\n",
    ),
    (
        "a finding is not said when it is found",
        "fuzz",
        FUZZ,
        "                say(&line);\n                findings.push(finding);\n",
        "                findings.push(finding);\n",
    ),
    (
        "the end of a target is not said",
        "fuzz",
        FUZZ,
        '    say(&format!("done {line}"));\n',
        "",
    ),
    (
        "the start of a target is not said",
        "fuzz",
        FUZZ,
        '    say(&format!(\n        "[{}] starting with {} seed(s)",\n        target.name,\n        queue.len()\n    ));\n',
        "",
    ),
    (
        "a failure is told from another by the input's length too",
        "fuzz",
        FUZZ,
        '            if failures.insert(format!("{}: {}", failure.kind, failure.why)) {\n',
        '            if failures.insert(format!("{}: {} {}", failure.kind, failure.why, input.len())) {\n',
    ),
    (
        "every failure is a finding of its own",
        "fuzz",
        FUZZ,
        '            if failures.insert(format!("{}: {}", failure.kind, failure.why)) {\n',
        '            if { failures.insert(format!("{}: {}", failure.kind, failure.why)); true } {\n',
    ),
    (
        "the profile of a run that failed is kept",
        "fuzz",
        FUZZ,
        "            // campaign that fails often would fill the disk with them.\n"
        "            let _ = std::fs::remove_file(&profraw);\n",
        "            // campaign that fails often would fill the disk with them.\n",
    ),
    (
        "the profile of a run that survived is kept",
        "fuzz",
        FUZZ,
        "            .collect::<Vec<_>>();\n        let _ = std::fs::remove_file(&profraw);\n",
        "            .collect::<Vec<_>>();\n",
    ),
    (
        "an input that does not end is not killed",
        "fuzz",
        FUZZ,
        "                let _ = child.kill();\n",
        "",
    ),
    (
        "the hang limit is ignored",
        "fuzz",
        FUZZ,
        "            Ok(None) if began.elapsed() >= run.hang => {\n",
        "            Ok(None) if false => {\n",
    ),
    (
        "a hang is called an abort",
        "fuzz",
        FUZZ,
        '                    kind: "hang",\n',
        '                    kind: "abort",\n',
    ),
    (
        "a panic is told from an abort by another status",
        "fuzz",
        FUZZ,
        "        kind: if status.code() == Some(101) {\n",
        "        kind: if status.code() == Some(1) {\n",
    ),
    (
        "--seconds takes its value and ignores it",
        "fuzz",
        FUZZ,
        '            "--seconds" => options.seconds = Some(whole(flag, value("a number of seconds")?)?),\n',
        '            "--seconds" => {\n                value("a number of seconds")?;\n            }\n',
    ),
    (
        "--hang takes its value and ignores it",
        "fuzz",
        FUZZ,
        '            "--hang" => options.hang = Some(whole(flag, value("a number of seconds")?)?),\n',
        '            "--hang" => {\n                value("a number of seconds")?;\n            }\n',
    ),
    (
        "--only splits its names at another character",
        "fuzz",
        FUZZ,
        "                    .split(',')\n",
        "                    .split(';')\n",
    ),
    (
        "the limits leave out --seconds",
        "fuzz",
        FUZZ,
        "        time: options.seconds.map(Duration::from_secs),\n",
        "        time: None,\n",
    ),
    (
        "the limits leave out --hang",
        "fuzz",
        FUZZ,
        "        hang: options.hang.map_or(HANG, Duration::from_secs),\n",
        "        hang: HANG,\n",
    ),
    (
        "the limits leave out --iterations",
        "fuzz",
        FUZZ,
        "        execs: options.iterations,\n",
        "        execs: 2000,\n",
    ),
    (
        "--only names nothing: every target runs",
        "fuzz",
        FUZZ,
        "    if only.is_empty() {\n        return Ok(all);\n    }\n",
        "    if true {\n        return Ok(all);\n    }\n",
    ),
    (
        "a target that does not exist is accepted",
        "fuzz",
        FUZZ,
        "        .find(|n| !all.iter().any(|t| t.name == n.as_str()))\n",
        "        .find(|_| false)\n",
    ),
    (
        "a target not named runs",
        "fuzz",
        FUZZ,
        "        .filter(|t| only.iter().any(|n| n == t.name))\n",
        "        .filter(|_| true)\n",
    ),
    (
        "the rate is seconds an execution",
        "fuzz",
        FUZZ,
        "    let rate = execs as f64 / elapsed.as_secs_f64().max(0.001);\n",
        "    let rate = elapsed.as_secs_f64() / execs.max(1) as f64;\n",
    ),
    (
        "the time is in minutes",
        "fuzz",
        FUZZ,
        "        elapsed.as_secs()\n    )\n}\n",
        "        elapsed.as_secs() / 60\n    )\n}\n",
    ),
    # 5. The doctor and the toolchain file.
    (
        "the doctor takes any toolchain to have the component",
        "doctor",
        DOCTOR,
        '        *llvm-tools*) ok "llvm-tools" "installed (llvm-profdata, for pw-fuzz)" ;;\n',
        '        *) ok "llvm-tools" "installed (llvm-profdata, for pw-fuzz)" ;;\n',
    ),
    (
        "the doctor never finds the component",
        "doctor",
        DOCTOR,
        "        *llvm-tools*) ok",
        "        *llvm-toolz*) ok",
    ),
    (
        "the doctor does not count the component among the missing",
        "doctor",
        DOCTOR,
        "            missing=$((missing + 1))\n            ;;\n    esac\n",
        "            ;;\n    esac\n",
    ),
    (
        "the toolchain file does not list the component",
        "doctor",
        TOOLCHAIN,
        'components = ["rustfmt", "clippy", "llvm-tools"]\n',
        'components = ["rustfmt", "clippy"]\n',
    ),
]

UNITTEST = [sys.executable, "-m", "unittest"]
SUITES = {
    "fuzz": ["cargo", "test", "--quiet", "--locked", "-p", "pw-fuzz"],
    "doctor": UNITTEST + ["scripts/tests/test_doctor_llvm_tools.py"],
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1800


def counted(out):
    """(built, passed, failed) as a suite's output says: `unittest`'s `Ran`
    and `OK (...)` or `FAILED (...)` lines (a skipped test neither passed nor
    failed), or cargo's `test result:` lines."""
    ran = [int(n) for n in re.findall(r"^Ran (\d+) tests? in", out, re.M)]
    if ran:
        ended = re.findall(r"^(?:OK|FAILED) \((.*)\)$", out, re.M)
        failed = sum(int(n) for line in ended for n in re.findall(r"(?:failures|errors)=(\d+)", line))
        skipped = sum(int(n) for line in ended for n in re.findall(r"skipped=(\d+)", line))
        return True, sum(ran) - failed - skipped, failed
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def run_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    p = subprocess.Popen(SUITES[suite], cwd=ROOT, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return True, 0, 1
    return counted(out)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the sources are still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in SUITES:
        built, passed, failed = run_tests(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, suite, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests(suite)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
