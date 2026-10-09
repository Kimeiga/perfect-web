#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: a page at another address of the page
answers a redirect there.

Each mutant undoes one piece:
- the clause: how it moves read, a page's alone, never the absent case, a
  case a query the page reads can answer, the one parameter of the route,
  and a value of its type;
- the plan: carrying the move, on the binding whose query can answer it;
- the host: a case the page names read as a move and no other, 308 and 307,
  the route's hole filled encoded, the query kept, kept by no cache, and a
  move to itself or to no segment refused.

Every mutant must fail `compiler/pw-core/tests/redirects.rs` or the
development server's `tests::redirects`.

Run from the repository root; `just e14-redirects` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ROUTES = ROOT / "compiler/pw-core/src/routes.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
VALUES = ROOT / "compiler/pw-core/src/page_values.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "every move is permanent",
        ROUTES,
        '        "temporary" => false,\n',
        '        "temporary" => true,\n',
    ),
    (
        "the clause is any declaration's",
        CHECK,
        "        if decl.kind != DeclKind::Page {\n"
        "            out.push(refused(\n"
        "                format!(\n"
        '                    "`redirect_on {value}` is a page\'s: `{}` is not one",\n',
        "        if false {\n"
        "            out.push(refused(\n"
        "                format!(\n"
        '                    "`redirect_on {value}` is a page\'s: `{}` is not one",\n',
    ),
    (
        "the case that means absent is a move too",
        CHECK,
        "            && absent_case == case\n",
        "            && absent_case == case\n            && false\n",
    ),
    (
        "a case no query the page reads answers is a move",
        CHECK,
        "        if !answers {\n"
        "            out.push(refused(\n"
        "                format!(\n"
        '                    "`redirect_on {value}`: no query `{}` reads can answer it",\n',
        "        if false {\n"
        "            out.push(refused(\n"
        "                format!(\n"
        '                    "`redirect_on {value}`: no query `{}` reads can answer it",\n',
    ),
    (
        "a route's first parameter is filled, whatever it carries",
        CHECK,
        "        let [hole] = holes.as_slice() else {\n",
        "        let Some(hole) = holes.first() else {\n",
    ),
    (
        "any one value fills the route",
        CHECK,
        "            ([one], Some(param)) => one.resolved().is_some_and(|t| t.same_as(param)),\n",
        "            ([_], Some(_)) => true,\n",
    ),
    (
        "the plan carries no move",
        VALUES,
        "                    Some(Redirect {\n"
        "                        case: crate::wit::ident(case),\n"
        "                        permanent: *permanent,\n"
        "                    })\n",
        "                    None\n",
    ),
    (
        "every binding carries the move",
        VALUES,
        "                Some((ty, case, permanent))\n"
        "                    if crate::routes::error_of(sigs, *resource) == Some(*ty) =>\n",
        "                Some((_, case, permanent)) =>\n",
    ),
    (
        "a case the page does not name is read as a move",
        SERVER,
        '            && binding["redirect"]["case"] == case.as_str()\n',
        '            && (binding["redirect"]["case"] == case.as_str() || case == "moved")\n',
    ),
    (
        "a move is answered with a status that may change the method",
        SERVER,
        "        if permanent { 308 } else { 307 },\n",
        "        if permanent { 301 } else { 302 },\n",
    ),
    (
        "the route's hole is filled as written",
        SERVER,
        "        &pw_render::escape::url_component(to),\n",
        "        to,\n",
    ),
    (
        "the address's query is dropped",
        SERVER,
        "    if !query.is_empty() {\n        location.push('?');\n",
        "    if false {\n        location.push('?');\n",
    ),
    (
        "a move may be kept by a cache",
        SERVER,
        '        &format!("{PRIVATE}{cookie}location: {location}\\r\\n"),\n',
        '        &format!("{cookie}location: {location}\\r\\n"),\n',
    ),
    (
        "a move to the address asked for is answered",
        SERVER,
        "    if params.get(*hole).map(String::as_str) == Some(to) {\n",
        "    if false {\n",
    ),
    (
        "a value no segment can carry is answered",
        SERVER,
        '    if to.is_empty() || to == "." || to == ".." {\n',
        "    if to.is_empty() {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "redirects"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "tests::redirects"],
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
        if r.returncode != 0 and not any(int(f) for _, f in found):
            failed += 1
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
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
