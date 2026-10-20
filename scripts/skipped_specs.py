#!/usr/bin/env python3
"""**Every spec an optional build can leave out runs where that build is
made** (the integrator's ruling of 2026-10-19, train-11).

A spec whose optional build (the feed's, the keyed store's, kiokun's, the
guestbook's) cannot be served is left out of a browser run and said, not
refused (`spikes/own-renderer/host-metadata.mjs`, `unservedSpecs`): CI's
browser jobs build neither kiokun's nor the guestbook's. So that nothing
goes unrun everywhere, for each such spec and each optional build it needs,
at least one recipe both makes the build (runs its prerequisite,
`spikes/own-renderer/<script>`) and runs the spec: a suite of
`spikes/own-renderer` that names it, or names no spec and so runs every
one; or a mutation script the recipe runs that runs Playwright and names
it. A spec whose tests name their own hosts (`annotations`) needs the
builds of the roles they annotate.

    python3 scripts/skipped_specs.py

prints each spec no recipe runs where its build is made, and exits 1 where
there is one. `scripts/tests/test_skipped_specs.py` holds it in `just ci`.
"""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import ci_plan  # noqa: E402

SUITE = "spikes/own-renderer"
REGISTRY = ROOT / SUITE / "host-registry.json"
# A test's own host, as a spec whose tests name their hosts annotates it
# (`host-metadata.mjs`, `hostAnnotationsIn`).
ANNOTATION = re.compile(r'\{\s*type:\s*"pw-host",\s*description:\s*"([^"]+)"\s*\}')


def skippable(registry, root=ROOT):
    """Each spec an optional build can leave out, with those builds, by
    name: the builds of its registered roles, or of the roles its tests
    annotate, other than the required one's and those with no build."""
    required = registry["roles"]["base"]["build"]
    optional = {
        name
        for name, build in registry["builds"].items()
        if name != required and build["dist"] is not None
    }
    out = {}
    for spec, entry in sorted(registry["specs"].items()):
        roles = entry.get("roles", [])
        if entry.get("annotations") is True:
            roles = ANNOTATION.findall((root / SUITE / "e2e" / spec).read_text())
        builds = sorted({registry["roles"][role]["build"] for role in roles} & optional)
        if builds:
            out[spec] = builds
    return out


def runs(body, spec, root=ROOT):
    """Whether a recipe's body runs `spec`: a suite of the suite's directory
    that names it or names no spec; or a mutation script the recipe runs
    that runs Playwright and names it."""
    for directory, command in ci_plan.suite_runs(body):
        if directory == SUITE and (spec in command or ".spec.mjs" not in command):
            return True
    for script in sorted(set(re.findall(r"scripts/([a-z0-9_]+_mutations\.py)", body))):
        try:
            text = (root / "scripts" / script).read_text()
        except OSError:
            continue
        if spec in text and re.search(r'"playwright",\s*"test"|playwright\s+test', text):
            return True
    return False


def makes(body, prerequisite):
    """Whether a recipe's body makes a build: runs its prerequisite."""
    return f"{SUITE}/{prerequisite}" in body


def unrun(registry, bodies, root=ROOT):
    """Each spec and build no one recipe both makes and runs, said."""
    problems = []
    for spec, builds in skippable(registry, root).items():
        for name in builds:
            build = registry["builds"][name]
            if not any(
                makes(body, build["prerequisite"]) and runs(body, spec, root)
                for body in bodies.values()
            ):
                problems.append(
                    f"e2e/{spec} needs {build['dist']}: no recipe runs "
                    f"{build['prerequisite']} and the spec"
                )
    return problems


def main():
    problems = unrun(json.loads(REGISTRY.read_text()), ci_plan.bodies())
    for problem in problems:
        print(problem)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
