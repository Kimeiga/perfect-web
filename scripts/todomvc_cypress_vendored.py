#!/usr/bin/env python3
"""**The official suite is vendored unchanged** (track `todomvc`, W7).

`spikes/todomvc-cypress` holds TodoMVC's own unchanged Cypress sources.
Execution uses the explicitly versioned copy in todomvc_cypress_compat.py;
this file verifies the original sources, not unchanged execution. Each file
is held to the git blob upstream has at the pinned commit
(`spikes/todomvc-cypress/UPSTREAM.md`): the SHA-1 of `blob <size>\\0<bytes>`, the
hash `git hash-object` gives. A file edited, reformatted or re-encoded fails
here, whoever's change it was.

    python3 scripts/todomvc_cypress_vendored.py        # exit 1 on any drift

The suite's own package pins Cypress too, and that is held here: 15.21.1, the
reviewed pin. (Upstream's lockfile pins 15.14.2, whose dependency tree carries six
advisories, in `uuid`, `qs` and `extract-zip`, that this one's does not.)
"""

import hashlib
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DIR = ROOT / "spikes/todomvc-cypress"

# github.com/tastejs/todomvc @ ff43b02e59dfa604386bb382034b2cd07c2bcd8a
BLOBS = {
    "cypress/e2e/spec.cy.js": "478bcfb17f407d56b4db19c4bffadcd0aa1e0972",
    "cypress/support/commands.js": "029abdbaf932cd1dded89e79c7943aa2fcab9c36",
    "cypress/support/e2e.js": "8edffddd850ceaa49a30c8b4542df5f02d16676a",
    "cypress/fixtures/example.json": "da87594a2465392b86ba23faaa7ac1d971c32091",
    "tests/knownIssues.js": "808649f49a9cde966e2fb8185eac394a0f975217",
    "cypress.config.js": "9557466f8850fd01fe05406acbc69d2e713fccd1",
    "LICENSE-todomvc.md": "9275d4371020294b6a107e454a0d1819f8b71c31",
}
CYPRESS = "15.21.1"


def blob(data):
    """The git blob hash of `data`."""
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def problems():
    found = []
    for path, want in BLOBS.items():
        file = DIR / path
        if not file.exists():
            found.append(f"{path}: missing")
            continue
        got = blob(file.read_bytes())
        if got != want:
            found.append(f"{path}: blob {got}, upstream's {want}")
    pinned = json.loads((DIR / "package.json").read_text()).get("devDependencies", {})
    if pinned.get("cypress") != CYPRESS:
        found.append(f"package.json: cypress is {pinned.get('cypress')!r}, not {CYPRESS}")
    return found


def main():
    found = problems()
    for p in found:
        print(p)
    if found:
        print(f"todomvc-cypress: {len(found)} problem(s): the suite is not upstream's")
        return 1
    print(f"todomvc-cypress: {len(BLOBS)} files are upstream's, byte for byte; cypress {CYPRESS}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
