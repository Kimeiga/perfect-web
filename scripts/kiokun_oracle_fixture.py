#!/usr/bin/env python3
"""Write an oracle's answers as a fixture CI holds the rewrite to.

The kiokun track's oracles run kiokun.com's own code locally (the
integrator's ruling of 2026-10-09). CI has neither that code nor the owner's
data, so it holds the rewrite to the oracle's answers for the repository's
committed sample, written here with what made them: the command, kiokun.com's
commit, this repository's commit, and Node's version. The answers are data,
not kiokun.com's code.

usage: kiokun_oracle_fixture.py ANSWERS OUT COMMAND KIOKUN_COMMIT PLERIS_COMMIT NODE
"""

import json
import sys


def main(argv: list[str]) -> int:
    if len(argv) != 7:
        print(
            "usage: kiokun_oracle_fixture.py ANSWERS OUT COMMAND KIOKUN_COMMIT PLERIS_COMMIT NODE",
            file=sys.stderr,
        )
        return 2
    answers, out, command, kiokun, pleris, node = argv[1:]
    with open(answers, encoding="utf-8") as f:
        oracle = json.load(f)
    fixture = {
        "produced_by": command,
        "kiokun_commit": kiokun,
        "pleris_commit": pleris,
        "node": node,
        "oracle": oracle,
    }
    with open(out, "w", encoding="utf-8") as f:
        json.dump(fixture, f, ensure_ascii=False, indent=1, sort_keys=True)
        f.write("\n")
    print(f"fixture: {out}: {oracle['answered']} answers, named {oracle['named']}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
