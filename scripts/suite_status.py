#!/usr/bin/env python3
"""**A browser suite's status is its recipe's** (track evidence-integrity-2,
the integrator's ruling 6 of 2026-10-15).

Until then 81 of the 83 recipes that ran a browser suite ended its pipeline
`|| true`: a failed test failed neither its recipe nor the run, and the
evidence said "1 failed" in a green run (`e14-navigate`'s on 2026-10-10,
`e14-feed`'s on 2026-10-09). Each now runs it through `scripts/run_suite.sh`,
which records the suite's output and exits with its status, and this gate
holds every recipe to that.

It reads the justfile and each file it imports, recipe by recipe, a line
continued by `\\` as one, as shell words and operators, and finds each
browser-suite invocation: each `playwright test`, and each `bench.mjs
controls`, which runs the benchmark's grade suite in a browser. One is
refused where:
- a `playwright test` is not run by `scripts/run_suite.sh` (the helper, the
  suite's command after its `--`);
- its status is swallowed: the command, or a subshell, group, loop or `if`
  that holds it, is followed by `||`, `&&`, `|` or `&`, or by `; true` or
  `; :`; it is a condition of `if`, `elif`, `while` or `until`; it is
  negated by `!`; it is inside a command substitution;
- its recipe turns errexit off (`set +e`, `set +o errexit`), is a script of
  its own (a shebang, which `just` runs without the justfile's shell), or
  runs the line ignoring its status (`just`'s `-` prefix);
- or the justfile's shell is not `bash` with `-e` and `pipefail`.

Non-vacuity: as many invocations recognised as there are occurrences of
`playwright test` in the recipes' lines, and at least one. A known failure
stays a job's `--known` at the fetch (ADR-0281), never a recipe that
swallows its suite's status.

    python3 scripts/suite_status.py        # exits 1 on any refusal
"""

from __future__ import annotations

import dataclasses
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import ci_plan  # noqa: E402

HELPER = "scripts/run_suite.sh"

# The words that make a command a condition: its status is read, not raised.
CONDITIONS = {"if", "elif", "while", "until"}

# The words after which a command starts.
STARTS_AFTER = {"then", "do", "else", "elif", "if", "while", "until", "!", "{"}

# The words that close a compound command, by the words that open it.
CLOSERS = {"}": ("{",), "done": ("for", "while", "until"), "fi": ("if",), "esac": ("case",)}

ASSIGNMENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")
REDIRECTION = re.compile(r"^\d*(>>?|<)(&\d+|&-)?$")


@dataclasses.dataclass
class Invocation:
    """A browser-suite invocation: where it is, what it runs, and why it is
    refused (none where it is not)."""

    part: str
    line: int
    recipe: str
    command: str
    kind: str  # "playwright" or "bench"
    problems: list[str]


@dataclasses.dataclass
class Token:
    kind: str  # "word" or "op"
    text: str


def logical_lines(text: str) -> list[tuple[int, str, str]]:
    """Each recipe's lines, a line ending in `\\` joined to the next, as
    (first line's number, recipe, text). Lines outside a recipe are not
    returned."""
    out: list[tuple[int, str, str]] = []
    recipe = None
    pending: tuple[int, str] | None = None
    for number, line in enumerate(text.splitlines(), 1):
        header = ci_plan.HEADER.match(line)
        if header and not line.startswith((" ", "\t")):
            recipe, pending = header.group(1), None
            continue
        if recipe is None:
            continue
        if line.strip() and not line.startswith((" ", "\t")):
            recipe, pending = None, None
            continue
        start, joined = pending if pending is not None else (number, "")
        joined = f"{joined} {line.strip()}".strip()
        if joined.endswith("\\"):
            pending = (start, joined[:-1].rstrip())
            continue
        pending = None
        if joined:
            out.append((start, recipe, joined))
    return out


OPERATORS = ("&&", "||", ";;", "$(", ";", "|", "&", "(", ")", "`")


def tokens(text: str) -> list[Token]:
    """`text` as shell words and operators: a quoted part kept within its
    word, `$(`, `(`, `)` and a backtick as operators of their own, a
    redirection (`>`, `2>&1`) as a word, a just interpolation (`{{RUNS}}`)
    within its word."""
    out: list[Token] = []
    word = ""
    i, quote = 0, None

    def end_word() -> None:
        nonlocal word
        if word:
            out.append(Token("word", word))
            word = ""

    while i < len(text):
        c = text[i]
        if quote:
            word += c
            if c == "\\" and quote == '"' and i + 1 < len(text):
                word += text[i + 1]
                i += 2
                continue
            if c == quote:
                quote = None
            i += 1
            continue
        if c in "\"'":
            quote = c
            word += c
            i += 1
            continue
        if c == "\\" and i + 1 < len(text):
            word += text[i : i + 2]
            i += 2
            continue
        if text.startswith("{{", i):
            end = text.find("}}", i)
            end = len(text) if end < 0 else end + 2
            word += text[i:end]
            i = end
            continue
        redirection = re.match(r"\d*(>>?|<)(&\d+|&-)?", text[i:]) if (c in "<>" or c.isdigit()) and not word else None
        if redirection and redirection.group(1):
            end_word()
            out.append(Token("word", redirection.group(0)))
            i += len(redirection.group(0))
            continue
        op = next((o for o in OPERATORS if text.startswith(o, i)), None)
        if op is not None:
            end_word()
            out.append(Token("op", op))
            i += len(op)
            continue
        if c.isspace():
            end_word()
            i += 1
            continue
        word += c
        i += 1
    end_word()
    return out


def pairs(toks: list[Token]) -> dict[int, int]:
    """Each `(`, `$(` and opening backtick, by its token, with the token
    that closes it."""
    out: dict[int, int] = {}
    stack: list[tuple[str, int]] = []
    for i, t in enumerate(toks):
        if t.kind != "op":
            continue
        if t.text == "`":
            if stack and stack[-1][0] == "`":
                out[stack.pop()[1]] = i
            else:
                stack.append(("`", i))
        elif t.text in ("(", "$("):
            stack.append((t.text, i))
        elif t.text == ")" and stack and stack[-1][0] in ("(", "$("):
            out[stack.pop()[1]] = i
    return out


def commands(toks: list[Token]) -> list[tuple[int, int]]:
    """Each simple command, by its first and last token: a substitution
    within it (`$(...)`, backticks) is part of it, and its own commands are
    commands too."""
    closing = pairs(toks)
    out: list[tuple[int, int]] = []
    start = None
    at_start = True
    i = 0
    while i < len(toks):
        t = toks[i]
        if t.kind == "op" and t.text in ("$(", "`") and i in closing:
            if start is None:
                start = i
            # Its inside: commands of their own.
            inner = commands(toks[i + 1 : closing[i]])
            out.extend((a + i + 1, b + i + 1) for a, b in inner)
            i = closing[i] + 1
            at_start = False
            continue
        if t.kind == "op":
            if start is not None:
                out.append((start, i - 1))
                start = None
            at_start = True
            i += 1
            continue
        if start is None and at_start and (t.text in STARTS_AFTER or t.text in CLOSERS or t.text in ("fi", "done")):
            i += 1
            continue
        if start is None:
            start = i
        at_start = False
        i += 1
    if start is not None:
        out.append((start, len(toks) - 1))
    return sorted(set(out))


def compounds(toks: list[Token]) -> list[tuple[str, int, int]]:
    """Each subshell, substitution, brace group, loop, `if` and `case`, by
    its kind and its first and last token."""
    spans: list[tuple[str, int, int]] = []
    stack: list[tuple[str, int]] = []
    at_start = True
    for i, t in enumerate(toks):
        if t.kind == "op":
            if t.text in ("(", "$("):
                stack.append((t.text, i))
            elif t.text == "`":
                if stack and stack[-1][0] == "`":
                    kind, opened = stack.pop()
                    spans.append((kind, opened, i))
                else:
                    stack.append(("`", i))
            elif t.text == ")":
                if stack and stack[-1][0] in ("(", "$("):
                    kind, opened = stack.pop()
                    spans.append((kind, opened, i))
                at_start = False
                continue
            at_start = True
            continue
        if at_start and t.text in ("{", "for", "while", "until", "if", "case"):
            stack.append((t.text, i))
            at_start = t.text in ("{", "while", "until", "if")
            continue
        if at_start and t.text in CLOSERS:
            if stack and stack[-1][0] in CLOSERS[t.text]:
                kind, opened = stack.pop()
                spans.append((kind, opened, i))
            at_start = False
            continue
        at_start = t.text in STARTS_AFTER or (at_start and ASSIGNMENT.match(t.text) is not None)
    return spans


def swallowed(toks: list[Token], first: int, last: int) -> list[str]:
    """What swallows the status of what spans tokens `first` to `last`, at
    its own level: what follows it, and what stands before it."""
    problems = []
    j = last + 1
    # A redirection after it is its own: `{ ...; } > file`.
    while j < len(toks) and toks[j].kind == "word" and REDIRECTION.match(toks[j].text):
        j += 2
    if j < len(toks):
        t = toks[j]
        if t.kind == "op" and t.text in ("||", "&&", "|", "&"):
            problems.append(f"is followed by `{t.text}`")
        elif t.kind == "op" and t.text == ";" and j + 1 < len(toks) and toks[j + 1].text in ("true", ":"):
            problems.append(f"is followed by `; {toks[j + 1].text}`")
    k = first - 1
    while k >= 0 and toks[k].kind == "word" and ASSIGNMENT.match(toks[k].text):
        k -= 1
    if k >= 0 and toks[k].kind == "word" and toks[k].text == "!":
        problems.append("is negated by `!`")
    # A condition runs from its keyword to `then` or `do`, at this level.
    depth = 0
    for k in range(first - 1, -1, -1):
        t = toks[k]
        if t.kind == "op" and t.text == ")":
            depth += 1
        elif t.kind == "op" and t.text in ("(", "$("):
            if depth == 0:
                break
            depth -= 1
        elif depth == 0 and t.kind == "word" and t.text in ("then", "do", "else", "{", "}", "fi", "done"):
            break
        elif depth == 0 and t.kind == "word" and t.text in CONDITIONS and (k == 0 or toks[k - 1].kind == "op" or toks[k - 1].text in STARTS_AFTER):
            problems.append(f"is a condition of `{t.text}`")
            break
    return problems


def judged(toks: list[Token], first: int, last: int) -> list[str]:
    """What swallows the status of the command at tokens `first` to `last`:
    its own place, and that of each compound command holding it."""
    problems = [f"its status is swallowed: it {p}" for p in swallowed(toks, first, last)]
    names = {"(": "subshell", "{": "group", "for": "loop", "while": "loop", "until": "loop", "if": "`if`", "case": "`case`"}
    for kind, opened, closed in compounds(toks):
        if not (opened < first and last <= closed):
            continue
        if kind in ("$(", "`"):
            problems.append("its status is lost: it runs inside a command substitution")
            continue
        problems += [f"its status is swallowed: the {names[kind]} holding it {p}" for p in swallowed(toks, opened, closed)]
    return problems


def is_helper(words: list[str]) -> bool:
    """Whether the command runs `scripts/run_suite.sh`, by `bash` or by its
    path, after any assignments."""
    rest = list(words)
    while rest and ASSIGNMENT.match(rest[0]):
        rest.pop(0)
    if len(rest) > 1 and rest[0] in ("bash", "sh"):
        rest.pop(0)
    path = rest[0].removeprefix("./") if rest else ""
    return path == HELPER or path.endswith("/" + HELPER)


def shell_settings(root: pathlib.Path) -> list[str]:
    """What is wrong with the justfile's shell, for a suite's status."""
    text = (root / "justfile").read_text()
    m = re.search(r"^set shell\s*:=\s*\[(.*)\]\s*$", text, re.M)
    if not m:
        return ["the justfile names no shell: `just` runs `sh -cu`, without errexit"]
    words = re.findall(r'"([^"]*)"', m.group(1))
    problems = []
    if not words or pathlib.Path(words[0]).name != "bash":
        problems.append(f"the justfile's shell is {words[:1]}, not bash")
    flags = [w[1:] for w in words[1:] if re.match(r"^-[a-z]+$", w)]
    if not any("e" in f for f in flags):
        problems.append("the justfile's shell runs without `-e`: a failed suite would not stop its recipe")
    if "pipefail" not in words:
        problems.append("the justfile's shell runs without `pipefail`")
    return problems


ERREXIT_OFF = re.compile(r"\bset\s+(\+[a-zA-Z]*e[a-zA-Z]*\b|\+o\s+errexit\b)")


def invocations(root: pathlib.Path = ROOT) -> tuple[list[Invocation], int]:
    """Every browser-suite invocation in the justfile's parts, judged, and
    how many times `playwright test` occurs in the recipes' lines."""
    found: list[Invocation] = []
    occurrences = 0
    settings = shell_settings(root)
    for part in ci_plan.justfile_parts(root):
        lines = logical_lines((root / part).read_text())
        by_recipe: dict[str, list[str]] = {}
        for _, recipe, line in lines:
            by_recipe.setdefault(recipe, []).append(line)
        for number, recipe, line in lines:
            if line.startswith("#") and not line.startswith("#!"):
                continue
            occurrences += len(re.findall(r"\bplaywright\s+test\b", line))
            prefix = re.match(r"^[@-]*", line).group(0)
            toks = tokens(line[len(prefix):])
            closing = pairs(toks)
            for first, last in commands(toks):
                text = " ".join(t.text for t in toks[first : last + 1])
                # Its own words, not those of a substitution within it.
                words, i = [], first
                while i <= last:
                    if toks[i].kind == "op" and i in closing:
                        i = closing[i] + 1
                        continue
                    words.append(toks[i].text)
                    i += 1
                # By its words, not a quoted mention: a suite hidden in a
                # string (`bash -c "..."`) is not recognised, and the count
                # of occurrences, which reads the text, refuses it.
                pairs_of = list(zip(words, words[1:]))
                playwright = any(a.endswith("playwright") and b == "test" for a, b in pairs_of)
                bench = any(a.endswith("bench.mjs") and b == "controls" for a, b in pairs_of)
                if not (playwright or bench):
                    continue
                problems = list(settings)
                if playwright:
                    if not is_helper(words):
                        problems.append(f"`playwright test` is not run by `{HELPER}`")
                    elif "--" not in words or words.index("--") > next(
                        k for k, w in enumerate(words) if w.endswith("playwright")
                    ):
                        problems.append(f"`playwright test` is not the command `{HELPER}` runs, after its `--`")
                problems += judged(toks, first, last)
                if "-" in prefix:
                    problems.append("its line is run with `-`: `just` ignores its status")
                if by_recipe[recipe][0].startswith("#!"):
                    problems.append("its recipe is a script of its own (a shebang), not run by the justfile's shell")
                if any(ERREXIT_OFF.search(l) for l in by_recipe[recipe]):
                    problems.append("its recipe turns errexit off (`set +e`)")
                found.append(
                    Invocation(part, number, recipe, text, "playwright" if playwright else "bench", sorted(set(problems)))
                )
    return found, occurrences


def report(root: pathlib.Path = ROOT) -> tuple[list[str], str]:
    """Each refusal, and the summary line."""
    found, occurrences = invocations(root)
    refusals = [f"{i.part}:{i.line} ({i.recipe}): {p}: {i.command[:120]}" for i in found for p in i.problems]
    recognised = sum(1 for i in found if i.kind == "playwright" and not i.problems)
    if occurrences == 0:
        refusals.append("no `playwright test` in the recipes' lines: the gate read nothing")
    elif recognised != occurrences:
        refusals.append(f"{occurrences - recognised} of {occurrences} `playwright test` not run by `{HELPER}` with their status their recipe's")
    summary = (
        f"suite status: {occurrences} `playwright test` in the recipes' lines, {recognised} run by `{HELPER}` "
        f"with their status their recipe's; {sum(1 for i in found if i.kind == 'bench')} `bench.mjs controls`; "
        f"{len(refusals)} refused"
    )
    return refusals, summary


def main() -> int:
    refusals, summary = report()
    for r in refusals:
        print(f"REFUSED: {r}")
    print(summary)
    return 1 if refusals else 0


if __name__ == "__main__":
    sys.exit(main())
