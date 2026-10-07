# ADR-0249: a verification shard sets up only what its recipes need

Status: accepted under the owner's delegation of 2026-10-02, on the owner's
approval of a free speedup to ADR-0245's runs (relayed 2026-10-07). Date:
2026-10-07. Milestone: E14.

## Context

- **A shard spent most of its time setting up.** Shard 3 of the run at
  `c1738ec`: freeing the disk 84 s, the toolchain and caches about 40 s, the
  browsers 54 s, the build 131 s, and its recipes 233 s. Every shard paid
  all of it, whatever its recipes were.
- **Most recipes need neither the browsers nor the build.** Of 211 evidence
  recipes, 56 drive a browser (about a third of the planned work) and 12
  more read what the build makes (the servers, the store's pages, `pw`). The
  rest are tests and mutation controls that `cargo test` what they need.
- **The nightly run was planned in 48 shards**, more than the Free plan's 20
  jobs at once ([limits](https://docs.github.com/en/actions/reference/limits)):
  a shard past them waits, then pays its setup again.

## Research

- **Playwright advises against caching its browsers on CI**: "Caching
  browser binaries is not recommended, since the amount of time it takes to
  restore the cache is comparable to the time it takes to download the
  binaries", and the system dependencies Linux needs cannot be cached
  ([CI](https://playwright.dev/docs/ci)). It keeps them in
  `~/.cache/ms-playwright` on Linux
  ([browsers](https://playwright.dev/docs/browsers)).
- **One build shared by artifact would cost more than it saves.** The
  target directory is about 10 GB; its build takes 131 s, most of it
  restoring the dependency cache, and a mutation script rebuilds its tests
  for each mutant anyway.

## Decision

1. **A shard's recipes are of one kind** (`ci_plan.py`): those that drive a
   browser (and so need the build), those that need the build alone, and the
   rest. Each kind gets shards of its own, as many as its share of the work,
   at least one where it has a recipe.
2. **A shard installs the browsers only for the first kind, and builds only
   for the first two**; the others do neither.
3. **The disk is freed in the background**, while the toolchain, caches and
   node are set up; a build or a recipe waits for it, five minutes at most.
4. **Every run is at most 16 shards**, the nightly's included: with the plan,
   the database job and the three browser jobs, the Free plan's 20.
5. **Browsers are not cached**, on Playwright's advice.

## Acceptance

- **`scripts/tests/test_ci_scripts.py`, 18 tests**, among them: a kind of
  recipe shares its shards with its kind; what a recipe needs is read from
  its lines.
- **The next runs' shards**, set against shard 3's 9 minutes: RUNS.

## Not claimed

- **A shard's recipes are as costly as before.** The setup is what this
  cuts.
