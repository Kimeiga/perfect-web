#!/usr/bin/env python3
"""Mutation controls for ADR-0242: an `{#each}`'s head is read once, by the
grammar.

Each mutant undoes one piece: the head parsed and its errors reported; the
grammar's recovery before `as` and before the key, and its key; and each
reader reading the head's parts, not the directive's text: name resolution,
lexical scope, the names check, the template IR, Marko, and the keyless and
mis-keyed loop rules. The tests of each must then fail.

Run from the repository root; `just e14-each-heads` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
RESOLVE = ROOT / "compiler/pw-core/src/resolve.rs"
LEXICAL = ROOT / "compiler/pw-core/src/lexical.rs"
NAMES = ROOT / "compiler/pw-core/src/names.rs"
MARKO = ROOT / "compiler/pw-core/src/marko.rs"
TEMPLATE_IR = ROOT / "compiler/pw-core/src/template_ir.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "an each head is not read",
        LOWER,
        "                let each = open.as_ref().and_then(|o| self.each_head(o));\n",
        "                let each: Option<crate::hir::EachHead> = None;\n",
    ),
    (
        "an each head's errors are dropped",
        LOWER,
        "        let parsed = pw_syntax::parse_each_head(&padded);\n"
        "        self.read_errors(&parsed, 0);\n",
        "        let parsed = pw_syntax::parse_each_head(&padded);\n",
    ),
    (
        "two words for a list leave the name unread",
        GRAMMAR,
        "            while !p.at_eof() && !p.at_kw(\"as\") {\n",
        "            while !p.at_eof() {\n",
    ),
    (
        "what stands before the key swallows it",
        GRAMMAR,
        "                    while !p.at_eof() && !p.at(Kind::LParen) {\n",
        "                    while !p.at_eof() {\n",
    ),
    (
        "a key is not read",
        GRAMMAR,
        "                if p.eat(Kind::LParen) {\n"
        "                    p.expr(0);\n"
        "                    p.expect(Kind::RParen, \"to close the key\");\n",
        "                if false && p.eat(Kind::LParen) {\n"
        "                    p.expr(0);\n"
        "                    p.expect(Kind::RParen, \"to close the key\");\n",
    ),
    (
        "a keyless list is not refused",
        CHECK,
        "                if each.key.is_some() {\n",
        "                if true {\n",
    ),
    (
        "a loop's key is refused at its block",
        CHECK,
        "            primary_span: key_span.clone(),\n",
        "            primary_span: body.node_span(n),\n",
    ),
    (
        "a row's name is not a local binding",
        RESOLVE,
        "                    out.insert(each.binder.clone());\n",
        "                    let _ = each;\n",
    ),
    (
        "a row's name is not in lexical scope",
        LEXICAL,
        "                        each.as_ref().map(|e| (e.binder.clone(), e.list.clone()))\n",
        "                        None::<(String, String)>.filter(|_| each.is_some())\n",
    ),
    (
        "the names check reads no head",
        NAMES,
        "                    each.as_ref().map(|e| (e.list.clone(), e.binder.clone()))\n",
        "                    None::<(String, String)>.filter(|_| each.is_some())\n",
    ),
    (
        "Marko reads no head",
        MARKO,
        "            let each = each.as_ref().map(Each::of).ok_or_else(|| {\n",
        "            let each = each.as_ref().filter(|_| false).map(Each::of).ok_or_else(|| {\n",
    ),
    (
        "the template IR reads no head",
        TEMPLATE_IR,
        "    if let Some(each) = each {\n",
        "    if let Some(each) = each.filter(|_| false) {\n",
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "each_heads", "--test", "every_name_resolves", "--test", "lexical_scope",
        "--test", "each_typing", "--test", "marko_adapter", "--test", "template_blocks",
        "--test", "keyed", "--test", "nested_lists",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib", "--",
        "an_each_head_is_its_list_its_name_and_its_key",
    ],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib", "resolve::"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
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
