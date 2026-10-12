#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: a related note is shown in its own file.

Each mutant undoes one piece:
- the checker: a note's unit not resolved to its file; a note in the
  diagnostic's own file named as another's;
- the detectors: PW5046's note on the layout, PW0338's on each press,
  PW5106's on a reader, PW0351's and PW0341's on the first declaration,
  each read in the diagnostic's own file again;
- `pw check`: a span outside its file annotated (which stops it), a note in
  another file read in the diagnostic's own, and a note in another file not
  shown.

A checker or detector mutant must fail `compiler/pw-core/tests/
related_files.rs`; a `pw check` mutant, `compiler/pw-cli`'s tests, its unit
tests and `tests/related_files.rs`, which runs the binary.

Run from the repository root; `just e14-related-files` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DIAGNOSTICS = ROOT / "compiler/pw-core/src/diagnostics.rs"
LAYOUTS = ROOT / "compiler/pw-core/src/layouts.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
PREDICATES = ROOT / "compiler/pw-core/src/predicates.rs"
ROUTES = ROOT / "compiler/pw-core/src/routes.rs"
CLI = ROOT / "compiler/pw-cli/src/main.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a note's unit is not resolved to its file",
        "core",
        DIAGNOSTICS,
        "                r.file = (unit != own).then(|| paths[unit].to_string());\n",
        "                let _ = (unit, own, paths);\n",
    ),
    (
        "a note in the diagnostic's own file names it as another's",
        "core",
        DIAGNOSTICS,
        "                r.file = (unit != own).then(|| paths[unit].to_string());\n",
        "                r.file = Some(paths[unit].to_string());\n",
    ),
    (
        "PW5046's note on the layout is read in the page's file",
        "core",
        LAYOUTS,
        "                related: vec![Related::in_unit(\n                    def.unit,\n",
        "                related: vec![Related::in_unit(\n                    unit,\n",
    ),
    (
        "PW0338's note on a press is read in the command's file",
        "core",
        CHECK,
        '                    Related::in_unit(*at, span.clone(), "a press sends it from here".to_string())\n',
        '                    Related::in_unit(unit + 0 * *at, span.clone(), "a press sends it from here".to_string())\n',
    ),
    (
        "PW5106's note on a reader is read in the command's file",
        "core",
        CHECK,
        "            related.push(Related::in_unit(\n                r.unit,\n                r.span.clone(),\n"
        "                format!(\"`{}` reads `{}`\", r.name, shared.join(\"`, `\")),\n",
        "            related.push(Related::in_unit(\n                unit,\n                r.span.clone(),\n"
        "                format!(\"`{}` reads `{}`\", r.name, shared.join(\"`, `\")),\n",
    ),
    (
        "PW0351's note on the first declaration is read in the second's file",
        "core",
        PREDICATES,
        "                    related: vec![Related::in_unit(\n                        *at,\n",
        "                    related: vec![Related::in_unit(\n                        unit + 0 * *at,\n",
    ),
    (
        "PW0341's note on the first page is read in the second's file",
        "core",
        ROUTES,
        "                        related: vec![Related::in_unit(\n                            *at,\n",
        "                        related: vec![Related::in_unit(\n                            unit + 0 * *at,\n",
    ),
    (
        "`pw check` annotates a span outside its file",
        "cli",
        CLI,
        "    span.start <= span.end\n        && span.end <= source.len()\n",
        "    true || span.start <= span.end\n        && span.end <= source.len()\n",
    ),
    (
        "`pw check` reads a note in another file in the diagnostic's own",
        "cli",
        CLI,
        "                None if readable(source, &r.span) => {\n",
        "                _ if readable(source, &r.span) => {\n",
    ),
    (
        "`pw check` does not show a note in another file",
        "cli",
        CLI,
        "        for other in elsewhere {\n            group = group.element(other);\n        }\n",
        "        for other in elsewhere {\n            let _ = other;\n        }\n",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "related_files"],
    "cli": ["cargo", "test", "--quiet", "--locked", "-p", "pw-cli"],
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, **kw):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True, **kw)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(CARGO[suite], cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in CARGO:
        built, passed, failed = cargo_tests(suite)
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
            built, passed, failed = cargo_tests(suite)
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
