#!/usr/bin/env python3
"""Mutation controls for ADR-0243: a block's statements are separated, by `;`
or a line.

Each mutant undoes one piece: the refusal and its repair; each separator,
`;`, `,` and a line; the block's first statement; and each thing a block's
readers read as one: a block after what precedes it, a `return` and the one
statement after it, a clause's head and the rest of its line, which ends at a
block and with the line. Then the names check's half: a clause's head that
is no clause where it is written, refused with what follows it on its line,
unless a separator, a line or a block comes between; and a clause's value,
which a `;` ends and a `,` does not. The tests of each must then fail, among
them the corpus's, whose programs write every one.

Run from the repository root; `just e14-statements-separated` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
NAMES = ROOT / "compiler/pw-core/src/names.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "two statements on one line are not refused",
        GRAMMAR,
        "            if same_line && !block && takes == Takes::Nothing && read && self.at_expression() {\n",
        "            if same_line && !block && takes == Takes::Nothing && read && self.at_expression() && false {\n",
    ),
    (
        "the refusal offers no repair",
        GRAMMAR,
        "                self.error_help(\n"
        '                    "PW0030",\n'
        '                    "two statements on one line are separated by `;`",\n'
        '                    "write `;` between them, or the second on a line of its own",\n'
        "                );\n",
        '                self.error("PW0030", "two statements on one line are separated by `;`");\n',
    ),
    (
        "a block's first statement is taken for a second",
        GRAMMAR,
        "        let mut separated = true;\n",
        "        let mut separated = false;\n",
    ),
    (
        "a line does not separate",
        GRAMMAR,
        "            let same_line = !separated && !self.newline_ahead();\n",
        "            let same_line = !separated;\n",
    ),
    (
        "`;` does not separate",
        GRAMMAR,
        "            separated = comma || semi;\n",
        "            separated = comma || (semi && false);\n",
    ),
    (
        "`,` does not separate",
        GRAMMAR,
        "            separated = comma || semi;\n",
        "            separated = (comma && false) || semi;\n",
    ),
    (
        "a block after what precedes it is a second statement",
        GRAMMAR,
        "            let block = self.at(Kind::LBrace);\n",
        "            let block = false && self.at(Kind::LBrace);\n",
    ),
    (
        "a statement after one read with an error is refused as a second",
        GRAMMAR,
        "            read = self.errors.len() == errors;\n",
        "            read = self.errors.len() == errors || true;\n",
    ),
    (
        "a token no expression begins with is refused as a statement",
        GRAMMAR,
        "            if same_line && !block && takes == Takes::Nothing && read && self.at_expression() {\n",
        "            if same_line && !block && takes == Takes::Nothing && read && (self.at_expression() || true) {\n",
    ),
    (
        "a word alone is not read",
        GRAMMAR,
        "            (Some(t), None) if t.kind == Kind::Ident => Some(&self.src[t.span.clone()]),\n",
        "            (Some(t), None) if t.kind == Kind::Ident && false => Some(&self.src[t.span.clone()]),\n",
    ),
    (
        "a word's path is taken for the word",
        GRAMMAR,
        "        match (read.next(), read.next()) {\n",
        "        match (read.next(), None::<&Token>) {\n",
    ),
    (
        "a `return` takes nothing",
        GRAMMAR,
        '                Some("return") => Takes::One,\n',
        '                Some("return") if false => Takes::One,\n',
    ),
    (
        "a `return` takes the rest of its line",
        GRAMMAR,
        "                _ if takes == Takes::Rest && same_line && !block => Takes::Rest,\n",
        "                _ if (takes == Takes::Rest || takes == Takes::One) && same_line && !block => Takes::Rest,\n",
    ),
    (
        "a clause's head takes nothing",
        GRAMMAR,
        "                Some(word) if heads_a_clause(word) => Takes::Rest,\n",
        "                Some(word) if heads_a_clause(word) && false => Takes::Rest,\n",
    ),
    (
        "a clause's value is one statement",
        GRAMMAR,
        "                _ if takes == Takes::Rest && same_line && !block => Takes::Rest,\n",
        "                _ if takes == Takes::Rest && same_line && !block && false => Takes::Rest,\n",
    ),
    (
        "a clause's value runs past a block",
        GRAMMAR,
        "                _ if takes == Takes::Rest && same_line && !block => Takes::Rest,\n",
        "                _ if takes == Takes::Rest && same_line => Takes::Rest,\n",
    ),
    (
        "a clause's value runs past its line",
        GRAMMAR,
        "                _ if takes == Takes::Rest && same_line && !block => Takes::Rest,\n",
        "                _ if takes == Takes::Rest && !block => Takes::Rest,\n",
    ),
    (
        "any word heads a clause",
        GRAMMAR,
        "    POLICY_KEYWORDS.contains(&word) || STMT_CLAUSE_KEYWORDS.contains(&word)\n",
        "    !word.is_empty() || POLICY_KEYWORDS.contains(&word) || STMT_CLAUSE_KEYWORDS.contains(&word)\n",
    ),
    (
        "a policy's head heads no clause",
        GRAMMAR,
        "    POLICY_KEYWORDS.contains(&word) || STMT_CLAUSE_KEYWORDS.contains(&word)\n",
        "    (POLICY_KEYWORDS.contains(&word) && false) || STMT_CLAUSE_KEYWORDS.contains(&word)\n",
    ),
    (
        "a statement clause's head heads no clause",
        GRAMMAR,
        "    POLICY_KEYWORDS.contains(&word) || STMT_CLAUSE_KEYWORDS.contains(&word)\n",
        "    POLICY_KEYWORDS.contains(&word) || (STMT_CLAUSE_KEYWORDS.contains(&word) && false)\n",
    ),
    (
        "a clause's head that is no clause is not refused",
        NAMES,
        "                && pw_syntax::heads_a_clause(head)\n",
        "                && pw_syntax::heads_a_clause(head)\n                && false\n",
    ),
    (
        "a statement on the next line is taken for the head's",
        NAMES,
        "                && self.same_line(s, n)\n",
        "                && (self.same_line(s, n) || true)\n",
    ),
    (
        "a block after the head is refused",
        NAMES,
        "                && !matches!(self.body.expr(n), Expr::Block { .. })\n",
        "                && !(matches!(self.body.expr(n), Expr::Block { .. }) && false)\n",
    ),
    (
        "a separator between them is not read",
        NAMES,
        "                && self.separators(s, n).is_empty()\n",
        "                && (self.separators(s, n).is_empty() || true)\n",
    ),
    (
        "`;` is not read as a separator",
        NAMES,
        "                .filter(|k| matches!(k, pw_syntax::Kind::Semi | pw_syntax::Kind::Comma))\n",
        "                .filter(|k| matches!(k, pw_syntax::Kind::Comma))\n",
    ),
    (
        "`,` is not read as a separator",
        NAMES,
        "                .filter(|k| matches!(k, pw_syntax::Kind::Semi | pw_syntax::Kind::Comma))\n",
        "                .filter(|k| matches!(k, pw_syntax::Kind::Semi))\n",
    ),
    (
        "a `;` does not end a clause's value",
        NAMES,
        "                            || self.separators(s, v).contains(&pw_syntax::Kind::Semi)\n",
        "                            || (self.separators(s, v).contains(&pw_syntax::Kind::Semi) && false)\n",
    ),
    (
        "a `,` ends a clause's value",
        NAMES,
        "                            || self.separators(s, v).contains(&pw_syntax::Kind::Semi)\n",
        "                            || !self.separators(s, v).is_empty()\n",
    ),

    (
        "the refusal is at the head",
        NAMES,
        "        primary_span: second,\n",
        "        primary_span: head.clone(),\n",
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib", "--",
        "two_statements_on_one_line_are_separated",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "statements_separated", "--test", "checking_source",
        "--test", "every_name_resolves",
    ],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib", "policy::"],
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
