#!/usr/bin/env python3
"""Mutation controls for ADR-0265: a form goes where something answers it.

Each mutant undoes one piece: a form checked at all, its method read, `get`
where it states none, its method's case, a link reaching what answers a
`get` and not pages alone, the relying party's routes and an upload's
answered, a page answering a `get` alone, a file form left to PW5603, and
the development server's identity answering `/sign-out` by its method. The
tests of `compiler/pw-core/tests/form_routes.rs` and the server's
`the_relying_party_answers_the_routes_the_compiler_knows` must then fail.

Run from the repository root; `just e14-form-routes` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ROUTES = ROOT / "compiler/pw-core/src/routes.rs"
IDENTITY = ROOT / "spikes/own-renderer/server/src/identity.rs"

METHOD = (
    '    let method = value("method").map_or_else(|| "GET".to_string(), '
    '|(m, _)| m.to_ascii_uppercase());\n'
)

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a form is not checked",
        ROUTES,
        "                form(body, decl, at.clone(), attrs, children, table, out);\n",
        "",
    ),
    (
        "a form's method is not read",
        ROUTES,
        METHOD,
        '    let method = "GET".to_string();\n',
    ),
    (
        "a form states `post` where it states none",
        ROUTES,
        METHOD,
        METHOD.replace('|| "GET"', '|| "POST"'),
    ),
    (
        "a form's method is read in its case",
        ROUTES,
        METHOD,
        METHOD.replace("m.to_ascii_uppercase()", "m"),
    ),
    (
        "a link reaches pages alone",
        ROUTES,
        '                if !is_internal(target) || table.answers("GET", target) {\n',
        "                if !is_internal(target) || table.pages.iter().any(|r| matches(r, target)) {\n",
    ),
    (
        "the relying party's routes are not answered",
        ROUTES,
        "    answered.extend(RELYING_PARTY.iter().map(|(m, r)| (*m, r.to_string())));\n",
        "",
    ),
    (
        "what an upload serves is not answered",
        ROUTES,
        '        answered.insert(("GET", format!("{}/{{key}}", u.serves)));\n',
        "",
    ),
    (
        "a page answers a post",
        ROUTES,
        '        pages.iter().map(|r| ("GET", r.clone())).collect();\n',
        '        pages.iter().flat_map(|r| [("GET", r.clone()), ("POST", r.clone())]).collect();\n',
    ),
    (
        "a form that sends a file is checked here too",
        ROUTES,
        '    if sends_file || value("enctype").is_some() || to_upload || !is_internal(&target) {\n',
        "    if to_upload || !is_internal(&target) {\n",
    ),
    (
        "the development server's identity signs out on a get",
        IDENTITY,
        '            ("POST", "/sign-out") => Some(self.sign_out(session)),\n',
        '            ("POST" | "GET", "/sign-out") => Some(self.sign_out(session)),\n',
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "form_routes"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--", "the_relying_party_answers"],
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
