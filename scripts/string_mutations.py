#!/usr/bin/env python3
"""Mutation controls for ADR-0049: a string's escapes are the language's.

Each mutant undoes one rule of the string decoder, one place that reads a
string through it, or one backend's encoding of the value, and the escape
tests must then fail.

Run from the repository root; `just e10-strings` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
STRINGS = ROOT / "compiler/pw-syntax/src/strings.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
BACKEND = ROOT / "compiler/pw-core/src/backend/lower.rs"
KOKA = ROOT / "compiler/pw-core/src/koka.rs"
MARKO = ROOT / "compiler/pw-core/src/marko.rs"
HANDLERS = ROOT / "compiler/pw-core/src/backend/js.rs"

B = "\\"  # one backslash, so the anchors below read as the Rust does

# (what is undone, file, anchor, replacement)
MUTANTS = [
    ("`\\n` is an `n`", STRINGS, "'n' => text.push('" + B + "n'),", "'n' => text.push('n'),"),
    ("`\\t` is a `t`", STRINGS, "'t' => text.push('" + B + "t'),", "'t' => text.push('t'),"),
    ("`\\r` is dropped", STRINGS, "'r' => text.push('" + B + "r'),", "'r' => {}"),
    (
        "`\\\\` keeps both backslashes",
        STRINGS,
        "'" + B + B + "' => text.push('" + B + B + "'),",
        "'" + B + B + "' => text.push_str(\"" + B + B + B + B + "\"),",
    ),
    ("`\\\"` is dropped", STRINGS, "'\"' => text.push('\"'),", "'\"' => {}"),
    (
        "`\\{` keeps its backslash",
        STRINGS,
        "'{' => text.push('{'),",
        "'{' => text.push_str(\"" + B + B + "{\"),",
    ),
    (
        "`\\u{..}` names the replacement character for a surrogate",
        STRINGS,
        "    char::from_u32(n).ok_or_else(|| {",
        "    char::from_u32(n).or(Some('" + B + "u{FFFD}')).ok_or_else(|| {",
    ),
    (
        "an escape the language does not define is kept",
        STRINGS,
        """                    other => {
                        return Err(StringError {
                            at,
                            message: format!(""",
        """                    other => {
                        text.push(other);
                        continue;
                        #[allow(unreachable_code)]
                        return Err(StringError {
                            at,
                            message: format!(""",
    ),
    (
        "an empty hole is text",
        STRINGS,
        "                if source.trim().is_empty() {",
        "                if false {",
    ),
    (
        "a triple-quoted string decodes its escapes",
        STRINGS,
        """    if let Some(raw) = token
        .strip_prefix("\\"\\"\\"")""",
        """    if let Some(raw) = None::<&str>
        .filter(|_| token.starts_with("\\"\\"\\""))""",
    ),
    (
        "the grammar refuses no escape",
        GRAMMAR,
        """                if self.at(Kind::Str)
                    && let Err(e) = crate::strings::pieces(self.cur_text())""",
        """                if false
                    && let Err(e) = crate::strings::pieces(self.cur_text())""",
    ),
    (
        "the lowering finds holes by scanning for braces",
        LOWER,
        "if let Ok(pieces) = pw_syntax::strings::pieces(&s) {",
        # An escaped brace read as a plain one, as a scan for `{` reads it.
        "if let Ok(pieces) = pw_syntax::strings::pieces(&s.replace(\"" + B + B + "{\", \"{\")) {",
    ),
    (
        "the component's text keeps a line feed's escape",
        BACKEND,
        "                        value: Const::Str(t),",
        "                        value: Const::Str(t.replace('" + B + "n', \"" + B + B + "n\")),",
    ),
    (
        "Koka is given the token",
        KOKA,
        "            Literal::Str(_) => koka_string(l)?,",
        "            Literal::Str(s) => s.clone(),",
    ),
    (
        "Marko is given the interpolation's token",
        MARKO,
        """            format!("({})", out.join(" + "))""",
        """            let _ = out;
            text.clone()""",
    ),
    (
        "a handler sends the token",
        HANDLERS,
        "            Some(v) => Encoding::Encoded(json(&v)),",
        "            Some(_) => Encoding::Encoded(s.clone()),",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib", "strings"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "string_escapes"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "handlers"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "strings"],
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
