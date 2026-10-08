#!/usr/bin/env python3
"""Mutation controls for ADR-0138: a handler is given its event.

Each mutant undoes one part: a lambda's written parameter type kept, an
event's record typing its handler's parameter, the checks on a handler's
parameter and an event's modifiers, the module decoding the event, the
manifest saying which event a part handles, a typed arrow parameter list
parsing, and an element's handlers written as one attribute. The tests in
`events.rs`, pw-syntax's grammar and pw-render's `properties.rs` must then
fail.

Run from the repository root; `just e14-events` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CORE = ROOT / "compiler/pw-core/src"
TEMPLATE = CORE / "template_ir.rs"
ANNOTATIONS = CORE / "annotations.rs"
VALUES = CORE / "values.rs"
LOWER = CORE / "lower.rs"
BACKEND = CORE / "backend/lower.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a lambda's written parameter type is dropped",
        LOWER,
        "                    b.param_types.insert(pat, ty);\n",
        "                    let _ = ty;\n",
    ),
    (
        "an on: handler's parameter is not typed by its event",
        VALUES,
        "                locals\n"
        "                    .entry(Binder::Pattern(*p))\n"
        "                    .or_insert_with(|| Ty::of(t));\n",
        "                let _ = t;\n",
    ),
    (
        "a handler's written event type is not compared",
        ANNOTATIONS,
        "                        .filter(|t| !t.same_as(expected))\n",
        "                        .filter(|_| false)\n",
    ),
    (
        "a handler may take several parameters",
        ANNOTATIONS,
        "                    many => Some(format!(\"one taking {} parameters\", many.len())),\n",
        "                    _many => None,\n",
    ),
    (
        "an unknown modifier is accepted",
        ANNOTATIONS,
        "                .filter(|m| !crate::hir::EVENT_MODIFIERS.contains(m))\n",
        "                .filter(|_| false)\n",
    ),
    (
        "the handler does not bind its event",
        BACKEND,
        "        if let Some(name) = name {\n            f.locals.insert(name, v);\n        }\n",
        "        let _ = name;\n",
    ),
    (
        "the manifest does not say which event",
        TEMPLATE,
        "                    event: match p {\n                        Part::Event { event, .. } => event.clone(),\n",
        "                    event: match p {\n                        Part::Event { .. } => String::new(),\n",
    ),
    (
        "a modifier is not carried",
        TEMPLATE,
        "                modifiers: modifiers.iter().map(|m| m.to_string()).collect(),\n",
        "                modifiers: Vec::new(),\n",
    ),
    (
        "an element's handlers are lowered in source order",
        TEMPLATE,
        "        attrs.iter().partition(|a| a.event().is_some());\n",
        "        attrs.iter().partition(|a| a.event().is_some() && false);\n",
    ),
    (
        "a typed arrow parameter list does not parse",
        GRAMMAR,
        "            Kind::LParen if self.nth_is(1, Kind::Ident) && self.nth_is(2, Kind::Colon) => {\n",
        "            Kind::LParen if self.nth_is(1, Kind::Ident) && self.nth_is(2, Kind::Colon) && false => {\n",
    ),
    (
        "an element's handlers write an attribute each",
        RENDER,
        "                    && o == owner\n",
        "                    && o != owner\n",
    ),
    (
        "one name captured from two places is guessed between",
        RENDER,
        "            if paths.get(path).is_some_and(|seen| *seen != at) {\n",
        "            if paths.get(path).is_some_and(|seen| *seen != at) && false {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "events"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib", "grammar"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "properties"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if m is None:
            built = False
            continue
        passed += int(m.group(1))
        failed += int(m.group(2))
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run_tests()
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
            built, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
