#!/usr/bin/env python3
"""Conservatively decide whether a PR's changed paths require full browser suites.

Print the GitHub Actions output 'browsers=true|false'. Unknown application
files are not assumed safe. Research and documentation changes stay cheap.
"""

import argparse
import sys


BROWSER_PREFIXES = (
    "compiler/",
    "runtime/",
    "examples/",
    "packages/",
    "spikes/own-renderer/",
    "spikes/pw-to-marko/",
    "spikes/browser/",
)

BROWSER_EXACT = {
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "rust-toolchain.toml",
}

BROWSER_SCRIPT_PREFIXES = (
    "scripts/browser_",
    "scripts/run_suite",
    "scripts/ci_recipes",
    "scripts/ci_plan",
    "scripts/keep_traces",
)


def browser_relevant(paths: list[str]) -> bool:
    """Prefer an unnecessary browser run to missing an application change."""
    return any(
        path in BROWSER_EXACT
        or path.startswith(BROWSER_PREFIXES)
        or path.startswith(BROWSER_SCRIPT_PREFIXES)
        for path in paths
        if path
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--browser", action="store_true", required=True)
    parser.parse_args()
    print(f"browsers={str(browser_relevant(sys.stdin.read().splitlines())).lower()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
