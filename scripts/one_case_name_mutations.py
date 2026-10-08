#!/usr/bin/env python3
"""Mutation controls for ADR-0206: a case has one name where values are
rendered, its WIT case's.

Each mutant names one of the language's four cases as the template did
before, `Some` for `some`:
- the template's arms;
- the renderer's stream outcome;
- each host's option and result.

Every mutant must fail `one_name_for_a_case.rs`, the renderer's stream
tests, or a host's own tests.

Run from the repository root; `just e14-one-case-name` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
IR = ROOT / "compiler/pw-core/src/template_ir.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
KIOKUN = ROOT / "spikes/kiokun/server/src/app.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the template names the language's four as Pleris does",
        IR,
        "            let case = crate::wit::ident(arm.short());\n",
        "            let case = match arm.short() {\n"
        "                c @ (\"Some\" | \"None\" | \"Ok\" | \"Err\") => c.to_string(),\n"
        "                c => crate::wit::ident(c),\n"
        "            };\n",
    ),
    (
        "a stream's outcome is named as Pleris names it",
        RENDER,
        "                case: if why.is_some() { \"some\" } else { \"none\" }.to_string(),\n",
        "                case: if why.is_some() { \"Some\" } else { \"None\" }.to_string(),\n",
    ),
    (
        "a host names an option as Pleris does",
        SERVER,
        "            case: if v.is_some() { \"some\" } else { \"none\" }.to_string(),\n",
        "            case: if v.is_some() { \"Some\" } else { \"None\" }.to_string(),\n",
    ),
    (
        "a host names a result as Pleris does",
        SERVER,
        "                Ok(v) => (\"ok\", v),\n                Err(v) => (\"err\", v),\n"
        "            };\n            Value::Variant {\n",
        "                Ok(v) => (\"Ok\", v),\n                Err(v) => (\"Err\", v),\n"
        "            };\n            Value::Variant {\n",
    ),
    (
        "kiokun's host names an option as Pleris does",
        KIOKUN,
        "        Val::Option(Some(inner)) => case(\"some\", Some(inner))?,\n",
        "        Val::Option(Some(inner)) => case(\"Some\", Some(inner))?,\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "one_name_for_a_case"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "streams"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
    ["cargo", "test", "--quiet", "--locked", "-p", "kiokun-server"],
]


def run():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            if "error[" in out or "could not compile" in out:
                built = False
            else:
                failed += 1
            continue
        for p, f in found:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1
    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)
    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
