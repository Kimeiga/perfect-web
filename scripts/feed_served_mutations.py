#!/usr/bin/env python3
"""Mutation controls for ADR-0220: the feed reference app, served in
browsers by the host that serves the store.

Each mutant undoes one piece: the drain that regenerated the store's cart
for every program, which told the feed's page to reload for ever; the
style each data layer gives its pages, on the feed's and on the store's;
and a guest named for its session. The tests of each must then fail.

The browser suite's `e2e/feed.spec.mjs` runs the feed in three engines.

Run from the repository root; `just e14-feed` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
STORE = ROOT / "spikes/own-renderer/server/src/store.rs"
FEED = ROOT / "spikes/own-renderer/server/src/feed.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the drain regenerates the store's cart for every program",
        SERVER,
        "        if !self.data.session_entry() {\n"
        "            return;\n"
        "        }\n"
        "        let key = self.cart_key(session);\n",
        "        let key = self.cart_key(session);\n",
    ),
    (
        "every page carries the store's style",
        SERVER,
        "            server.data.style(),\n",
        "            STYLE,\n",
    ),
    (
        "the store's page carries no style",
        STORE,
        "    fn style(&self) -> &'static str {\n        crate::STYLE\n    }\n",
        "    fn style(&self) -> &'static str {\n        \"\"\n    }\n",
    ),
    (
        "a page with no style is written an empty one",
        SERVER,
        '        "" => String::new(),\n        css => format!("<style>{css}</style>\\n"),\n',
        '        "" => "<style></style>\\n".to_string(),\n        css => format!("<style>{css}</style>\\n"),\n',
    ),
    (
        "a guest is You to every reader",
        FEED,
        '        (format!("@{session}"), format!("Guest {session}"))\n',
        '        let _ = session;\n        ("@you".to_string(), "You".to_string())\n',
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "the_feeds_page_opening_its_stream_is_not_told_to_reload",
        "a_page_carries_its_data_layers_style_and_no_other",
        "a_guests_post_names_the_guest_to_every_reader",
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
