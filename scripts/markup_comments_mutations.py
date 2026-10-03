#!/usr/bin/env python3
"""Mutation controls for ADR-0167: markup text is text, and a comment in
markup is `<!-- -->`.

Each mutant undoes one piece:
- the parser: a `//` in markup content, between elements or inside a run of
  text, read as a comment again; markup text read to the line's end; an
  HTML comment read as an element, ended at its first `>`, quiet when it is
  not closed, or kept as text; an unquoted attribute value ended at its
  first mark, run over `/>`, or read as a comment when it begins with `//`;
- the checker (PW5028): a line that reads as a comment let be, only `//`
  read so, or a stylesheet's own comment refused.

A parser mutant must fail `pw-syntax`'s tests, all of them run; a checker
mutant, or one the page would show, `compiler/pw-core/tests/
markup_comments.rs`.

Run from the repository root; `just e14-markup-comments` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a `//` between elements is a comment again",
        "syntax",
        GRAMMAR,
        "            if depth > 0 {\n                self.markup_text();\n            }\n",
        "",
    ),
    (
        "a `//` inside a run of text is a comment again",
        "syntax",
        GRAMMAR,
        "            // comment, it ran to the line's end, over the closing tag.\n"
        "            self.markup_text();\n",
        "            // comment, it ran to the line's end, over the closing tag.\n",
    ),
    (
        "markup text runs to the line's end",
        "syntax",
        GRAMMAR,
        "        let end = start + rest.find(['<', '{', '}']).unwrap_or(rest.len());\n",
        "        let end = start + rest.find('\\n').unwrap_or(rest.len());\n",
    ),
    (
        "an HTML comment is read as an element",
        "syntax",
        GRAMMAR,
        "                Kind::LAngle if self.src[self.cur_span().start..].starts_with(\"<!--\") => {\n"
        "                    self.markup_comment();\n"
        "                }\n",
        "",
    ),
    (
        "an HTML comment ends at its first `>`",
        "syntax",
        GRAMMAR,
        "        let (end, closed) = match rest.find(\"-->\") {\n"
        "            Some(at) => (start + at + \"-->\".len(), true),\n",
        "        let (end, closed) = match rest.find('>') {\n"
        "            Some(at) => (start + at + \">\".len(), true),\n",
    ),
    (
        "a comment that is not closed is quiet",
        "syntax",
        GRAMMAR,
        "        if !closed {\n"
        "            self.error_help(\n"
        "                \"PW0006\",\n",
        "        if false && !closed {\n"
        "            self.error_help(\n"
        "                \"PW0006\",\n",
    ),
    (
        "a comment in markup is sent as text",
        "core",
        GRAMMAR,
        "                \"close it with `-->`\",\n"
        "            );\n"
        "        }\n"
        "        self.bump();\n",
        "                \"close it with `-->`\",\n"
        "            );\n"
        "        }\n"
        "        self.start(K::Text);\n"
        "        self.bump();\n"
        "        self.finish();\n",
    ),
    (
        "an unquoted value ends at its first mark",
        "syntax",
        GRAMMAR,
        "            .find(|(at, c)| c.is_whitespace() || *c == '>' || rest[*at..].starts_with(\"/>\"))\n",
        "            .find(|(_, c)| !c.is_alphanumeric())\n",
    ),
    (
        "an unquoted value runs over `/>`",
        "syntax",
        GRAMMAR,
        "            .find(|(at, c)| c.is_whitespace() || *c == '>' || rest[*at..].starts_with(\"/>\"))\n",
        "            .find(|(_, c)| c.is_whitespace() || *c == '>')\n",
    ),
    (
        "an unquoted value that begins with `//` is a comment again",
        "syntax",
        GRAMMAR,
        "                .is_some_and(|t| matches!(t.kind, Kind::LineComment | Kind::DocAttr))\n"
        "            {\n"
        "                self.read_unquoted_value();\n"
        "            }\n",
        "                .is_some_and(|t| matches!(t.kind, Kind::LineComment | Kind::DocAttr))\n"
        "                && false\n"
        "            {\n"
        "                self.read_unquoted_value();\n"
        "            }\n",
    ),
    (
        "a line that reads as a comment is let be",
        "core",
        CHECK,
        "                if !(written.starts_with(\"//\") || written.starts_with(\"/*\")) {\n",
        "                if true {\n",
    ),
    (
        "a stylesheet's own comment is refused",
        "core",
        CHECK,
        "                && matches!(tag.to_ascii_lowercase().as_str(), \"style\" | \"script\")\n",
        "                && matches!(tag.to_ascii_lowercase().as_str(), \"script\")\n",
    ),
    (
        "only `//` reads as a comment",
        "core",
        CHECK,
        "                if !(written.starts_with(\"//\") || written.starts_with(\"/*\")) {\n",
        "                if !written.starts_with(\"//\") {\n",
    ),
]

CARGO = {
    "syntax": ["cargo", "test", "--quiet", "--locked", "-p", "pw-syntax"],
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "markup_comments"],
}

# How long one run may take. Past the bound it is killed with everything it
# started, and the run counts as failing.
BOUND = 900


def run(suite):
    """(built, passed, failed) over the suite's tests."""
    p = subprocess.Popen(CARGO[suite], cwd=ROOT, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def main():
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in CARGO:
        built, passed, failed = run(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed")
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
            built, passed, failed = run(suite)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
