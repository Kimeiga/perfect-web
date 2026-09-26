#!/usr/bin/env python3
"""Mutation controls for ADR-0071: what a template's blocks and events take.

Each mutant undoes one piece of how an `{#each}`'s collection, an `{#if}` or
`{:else if}` condition, and an event attribute's value are related. The
template-operand tests, or the store's test that every one of its relations
is decided, must then fail.

Run from the repository root; `just e10-template-operands` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "an `{#each}`'s collection is not related",
        VALUES,
        "                other => Outcome::Disagree {\n"
        '                    expected: "List<_>".to_string(),\n'
        "                    actual: self.display(other),\n"
        "                },",
        "                other => {\n"
        "                    let _ = other;\n"
        "                    Outcome::Agree\n"
        "                }",
    ),
    (
        "an `{#each}` over a list is refused",
        VALUES,
        "            let outcome = match &t {\n"
        "                Ty::Builtin(Builtin::List, _) | Ty::Any => Outcome::Agree,",
        "            let outcome = match &t {\n"
        "                Ty::Any => Outcome::Agree,",
    ),
    (
        "an `{#if}`'s condition is not related",
        VALUES,
        '                } if directive.trim_start().starts_with("{#if") => {',
        '                } if false && directive.trim_start().starts_with("{#if") => {',
    ),
    (
        "an `{:else if}`'s condition is not related",
        VALUES,
        '                } => out.extend(self.truth(*c, "{:else if}", false)),',
        "                } => {\n                    let _ = c;\n                }",
    ),
    (
        "a sum type has a truth",
        VALUES,
        "                    .is_some_and(|t| t.variants.is_some()) =>",
        "                    .is_some_and(|t| t.variants.is_some() && false) =>",
    ),
    (
        "no value has a truth",
        VALUES,
        "            _ => Outcome::Agree,\n"
        "        };\n"
        "        Some(ValueRelation {",
        "            _ => Outcome::Disagree {\n"
        "                expected: String::new(),\n"
        "                actual: self.display(&t),\n"
        "            },\n"
        "        };\n"
        "        Some(ValueRelation {",
    ),
    (
        "an `Option` condition is related as agreeing",
        VALUES,
        "            Ty::Builtin(Builtin::Option | Builtin::Result, _) if if_subject => return None,\n",
        "",
    ),
    (
        "an event attribute's value is not related",
        VALUES,
        "                                other => Outcome::Disagree {\n"
        '                                    expected: "a function".to_string(),\n'
        "                                    actual: self.display(&other),\n"
        "                                },",
        "                                other => {\n"
        "                                    let _ = other;\n"
        "                                    Outcome::Agree\n"
        "                                }",
    ),
    (
        "a function is not a function to call",
        VALUES,
        "                                Ty::Builtin(Builtin::Function, _) | Ty::Any => Outcome::Agree,",
        "                                Ty::Any => Outcome::Agree,",
    ),
    (
        "a lambda is not decided",
        VALUES,
        "                            Expr::Lambda { .. } => Outcome::Agree,\n",
        "",
    ),
    (
        "every attribute's value is related as an event's",
        VALUES,
        '                            (a.name.strip_prefix("on:"), &a.value)',
        "                            (Some(a.name.as_str()), &a.value)",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "template_operands"],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "value_relations",
        "the_store_program_is_decided_not_merely_silent",
    ],
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
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
