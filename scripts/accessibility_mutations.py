#!/usr/bin/env python3
"""Mutation controls for ADR-0182: keyboard and screen-reader semantics
remain valid (charter §15.6 test 14, §17.4).

Each mutant undoes one piece:
- the page's shell: no viewport, on the store's page or on the page that
  is not found; no language;
- the runtime: a part's text written again when it holds it already; an
  attribute set again when it holds its value;
- the store's markup: a category's heading a level down; every Add named
  "Add"; the cart's section named by an id nothing has; the cart's count no
  live region; a control first in the keyboard's order;
- the store's stylesheet: an item's description too faint to read; focus
  not shown; the page wider than a phone; a cart line that moves in.

Every mutant must fail `e2e/accessibility.spec.mjs` in Chromium, against a
build of the mutated source: the page (`run.sh`) and the server.

Run from the repository root; `just e14-accessibility` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RUNTIME = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"
APP = ROOT / "examples/store/app.pw"

STYLE = (
    'const STYLE: &str = "#menu li { content-visibility: auto; contain-intrinsic-size: auto 42px; }";\n'
)


def styled(rule):
    """The store's stylesheet with one more rule."""
    return STYLE.replace(' }";', f' }} {rule}";')


# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the store's page has no viewport",
        SERVER,
        r"""         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Store</title>""",
        r"""         <title>Store</title>""",
    ),
    (
        "the page that is not found has no viewport",
        SERVER,
        r"""     <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
     <title>Not found</title>""",
        r"""     <title>Not found</title>""",
    ),
    (
        "the store's page says no language",
        SERVER,
        r"""        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Store</title>""",
        r"""        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Store</title>""",
    ),
    (
        "a part's text is written again when it holds it already",
        RUNTIME,
        "  if (holdsText(r, text)) return true;\n",
        "",
    ),
    (
        "an attribute is set again when it holds its value",
        RUNTIME,
        "  if (element.getAttribute(op.name) !== value) element.setAttribute(op.name, value);\n",
        "  element.setAttribute(op.name, value);\n",
    ),
    (
        "a category's heading is a level down",
        APP,
        "<h2>{section.category.name}</h2>",
        "<h3>{section.category.name}</h3>",
    ),
    (
        "every Add is named Add",
        APP,
        'aria-label="Add {item.name}"',
        'aria-label="Add"',
    ),
    (
        "the cart's section is named by an id nothing has",
        APP,
        '<section aria-labelledby="cart-heading">',
        '<section aria-labelledby="cart-title">',
    ),
    (
        "the cart's count is no live region",
        APP,
        '<p aria-live="polite" aria-atomic="true">Items in cart:',
        "<p>Items in cart:",
    ),
    (
        "a control is first in the keyboard's order",
        APP,
        '                    id="clear-cart"\n',
        '                    id="clear-cart"\n                    tabindex="1"\n',
    ),
    (
        "an item's description is too faint to read",
        SERVER,
        STYLE,
        styled("#menu li p { color: #999; }"),
    ),
    (
        "focus is not shown",
        SERVER,
        STYLE,
        styled("button:focus-visible { outline: none; }"),
    ),
    (
        "the page is wider than a phone",
        SERVER,
        STYLE,
        styled("main { min-width: 600px; }"),
    ),
    (
        "a cart line moves in",
        SERVER,
        STYLE,
        styled("#cart-lines li { animation: pw-in 0.2s; } @keyframes pw-in { from { opacity: 0; } }"),
    ),
]

BROWSER = ["e2e/accessibility.spec.mjs", "--project=chromium"]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, **kw):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True, **kw)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def build():
    """The page and the server the suite runs, as the source says."""
    page = subprocess.run(["bash", "spikes/own-renderer/run.sh"], cwd=ROOT,
                          env={**os.environ, "BUILD_ONLY": "1"}, capture_output=True, text=True)
    server = subprocess.run(["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"],
                            cwd=ROOT, capture_output=True, text=True)
    return page.returncode == 0 and server.returncode == 0


def browser_tests():
    """(built, passed, failed) for the spec, after a build."""
    if not build():
        return False, 0, 0
    out, code = bounded(
        ["pnpm", "exec", "playwright", "test", *BROWSER, "--reporter=line"],
        cwd=ROOT / "spikes/own-renderer",
    )
    if code is None:
        return True, 0, 1
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


def main():
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = browser_tests()
    print(f"baseline (browser): {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        return 1

    survivors = 0
    try:
        for what, path, anchor, replacement in MUTANTS:
            original = path.read_text()
            if original.count(anchor) != 1:
                print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
                survivors += 1
                continue
            try:
                path.write_text(original.replace(anchor, replacement, 1))
                built, passed, failed = browser_tests()
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
    finally:
        # The page and the server as the source says, whatever a mutant
        # left built.
        build()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
