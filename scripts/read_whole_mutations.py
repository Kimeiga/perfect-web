#!/usr/bin/env python3
"""Mutation controls for ADR-0237: what lowering parses, it reports.

Each mutant undoes one piece: a standalone parse's errors kept by lowering,
for a clause's value, a key clause's, an interface's clauses and an
expression's; moved to their
place in the file; reported by the checker; the rest read as one error; a
missing comma and a missing arrow read past, a bad value its own error
alone; a parse's end named as its own; an error node that is a reported
error not reported again; the help a repair; and the code registered. The
tests of each must then fail.

Run from the repository root; `just e14-read-whole` records the output.
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
CODES = ROOT / "compiler/pw-core/src/codes.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "an optimistic clause's errors are dropped",
        LOWER,
        "            let parsed = pw_syntax::parse_transition_clause(&value);\n"
        "            self.read_errors(&parsed, offset);\n",
        "            let parsed = pw_syntax::parse_transition_clause(&value);\n",
    ),
    (
        "a key clause's errors are dropped",
        LOWER,
        "        let parsed = pw_syntax::parse_expr_list(&value);\n"
        "        self.read_errors(&parsed, offset);\n",
        "        let parsed = pw_syntax::parse_expr_list(&value);\n",
    ),
    (
        "a hole's and a marker's errors are dropped",
        LOWER,
        "        let parsed = pw_syntax::parse_expr(&padded, what);\n"
        "        self.read_errors(&parsed, 0);\n",
        "        let parsed = pw_syntax::parse_expr(&padded, what);\n",
    ),
    (
        "an interface's clauses are not parsed",
        LOWER,
        "        if body.is_none() {\n"
        "            self.clause_syntax(&policies);\n"
        "        }\n",
        "",
    ),
    (
        "an error is left at its place in the clause, not the file",
        LOWER,
        "                span: (e.span.start + offset)..(e.span.end + offset),\n",
        "                span: e.span.clone(),\n",
    ),
    (
        "the checker does not report what lowering parsed",
        CHECK,
        "    let mut out: Vec<Diagnostic> = unit.hir.syntax.iter().cloned().map(syntax_error).collect();\n",
        "    let mut out: Vec<Diagnostic> = Vec::new();\n",
    ),
    (
        "what follows one expression is read without an error",
        GRAMMAR,
        "        self.finish();\n"
        "        self.errors.push(SyntaxError {\n"
        "            code: \"PW0016\",\n",
        "        self.finish();\n"
        "        let _ = (SyntaxError {\n"
        "            code: \"PW0016\",\n",
    ),
    (
        "a hole is not read whole",
        GRAMMAR,
        "    p.rest_unread(&format!(\"{what} is one expression\"), None);\n",
        "    let _ = what;\n",
    ),
    (
        "a value with no comma before it ends the clause",
        GRAMMAR,
        "            let missing = !first && !self.eat(Kind::Comma);\n"
        "            if self.at_eof() {\n",
        "            let missing = !first && !self.eat(Kind::Comma);\n"
        "            if missing || self.at_eof() {\n",
    ),
    (
        "a value with no comma before it is not reported",
        GRAMMAR,
        "            if missing {\n"
        "                let read = self.errors.len() == errors;\n",
        "            if false && missing {\n"
        "                let read = self.errors.len() == errors;\n",
    ),
    (
        "what is not a value where a comma is missing keeps its own errors",
        GRAMMAR,
        "                self.errors.truncate(errors);\n",
        "",
    ),
    (
        "what is not a value where a comma is missing is read as one",
        GRAMMAR,
        "                let read = self.errors.len() == errors;\n",
        "                let read = true;\n",
    ),
    (
        "a transition without its arrow is not read",
        GRAMMAR,
        "        if !self.at_eof() && !self.at(Kind::Comma) {\n"
        "            self.expr(0);\n",
        "        if arrow && !self.at_eof() && !self.at(Kind::Comma) {\n"
        "            self.expr(0);\n",
    ),
    (
        "an arrow with no transition after it is no error",
        GRAMMAR,
        "        } else if arrow {\n"
        "            let found = self.found();\n",
        "        } else if false {\n"
        "            let found = self.found();\n",
    ),
    (
        "a standalone parse's end is the file's",
        GRAMMAR,
        "            Kind::Eof => self.end.clone(),\n",
        "            Kind::Eof => \"end of file\".to_string(),\n",
    ),
    (
        "an error node a reported parse error left is reported again",
        CHECK,
        "            .any(|e| span.start <= e.span.start && e.span.start <= span.end)\n",
        "            .any(|_| false)\n",
    ),
    (
        "the parser's help is no repair",
        CHECK,
        "        repairs: e\n"
        "            .help\n",
        "        repairs: None::<String>\n",
    ),
    (
        "the code is not registered",
        CODES,
        "    READ_WHOLE = \"PW0016\" / read_whole / 1, Syntax,\n",
        "    READ_WHOLE = \"PW0018\" / read_whole / 1, Syntax,\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "read_whole"],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib", "--",
        "what_a_standalone_parse_leaves_is_one_error_over_it",
        "a_value_with_no_comma_before_it_is_reported_and_read",
        "an_optimistic_clause_without_its_arrow_reads_its_transition",
    ],
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
