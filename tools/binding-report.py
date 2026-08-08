#!/usr/bin/env python3
"""Print the binding support of every export in a contracts artifact.

Reads `docs/evidence/E8/component-contracts.json` — compiler output, checked in
so the host is tested against what the compiler actually emits. Used by
`just e8-binding`; the report is evidence, not a check.
"""

import json
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: binding-report.py <component-contracts.json>", file=sys.stderr)
        return 2
    with open(sys.argv[1]) as f:
        contracts = json.load(f)
    for c in contracts:
        for e in c["exports"]:
            b = e.get("binding", {})
            local = b.get("local", "?")
            remote = b.get("remote", {})
            kind = remote.get("kind", "?")
            line = f"  {c['component_id']:24} {e['name']:14} local={local:8} remote={kind}"
            print(line)
            # Every position that refused or could not be decided, because a
            # count alone says an edge is not remotable without saying what
            # would have to change.
            for p in remote.get("positions", []):
                ty = p.get("ty") or "(undetermined)"
                print(f"      {p['position']}: {ty} — {p['reason']}")
            # An obligation is a yes with a condition: the binding must prove
            # it, and the compiler names exactly what.
            for o in remote.get("obligations", []):
                ty = o.get("ty") or "(undetermined)"
                print(
                    f"      {o['position']}: {ty} — binding must preserve "
                    f"{o['principal']}"
                )
    return 0


if __name__ == "__main__":
    sys.exit(main())
