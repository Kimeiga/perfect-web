# ADR-0278: a recipe run against a database is a shard of its own

Status: accepted under the owner's delegation of 2026-10-02, on the
messages track's finding (W4, relayed 2026-10-08). Date: 2026-10-08.
Milestone: E14. Amends ADR-0246 (the database job) and ADR-0249 (a run's
shards).

## Context

- **The database job outlasted its limit.** W4's verification run
  37831080654 at `c675180` ran the job's four planned recipes in series,
  against one PostgreSQL: `e14-identity` in 1958 s, `e14-messages` in
  2695 s (25 of 25 mutants killed, 9 of 9 browser tests), and then its
  `timeout-minutes: 120` cancelled it inside `e14-notifications`.
  `e14-uploads` never ran, and the run's summary failed for it. Every other
  job was green.
- **In series, the job's time is the sum of its recipes'.** Each track has
  added one: the feed's own (ADR-0246), identity's (ADR-0258), uploads'
  (ADR-0260), notifications' (ADR-0270) and now messages'. A higher limit
  only moves the day it is reached again.
- **Those recipes shared one database**, each run after the last on what
  the one before it left.

## Research

- **GitHub's limits** ([limits](https://docs.github.com/en/actions/reference/limits)):
  "Each job in a workflow can run for up to 6 hours of execution time"; the
  Free plan runs 20 jobs at once on standard runners; a matrix makes at
  most 256 jobs a run.
- **A service can be given to some of a matrix's jobs.** "If
  `jobs.<job_id>.services.<service_id>.image` is assigned an empty string,
  the service will not start"
  ([workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)),
  and `jobs.<job_id>.services` reads the `matrix` context
  ([contexts](https://docs.github.com/en/actions/reference/workflows-and-actions/contexts)).
- **More jobs cost nothing here but their setup.** The repository is
  public, and "GitHub Actions usage is free for self-hosted runners and for
  public repositories that use standard GitHub-hosted runners"
  ([billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions)).

## Decision

1. **Each recipe run against a database (`NEEDS_DATABASE`) is a shard of
   its own**, beside a PostgreSQL of its own: `postgres:18.6`, a throwaway
   database each run, as the database job's was. Its time is its recipe's,
   and its database no other recipe's.
2. **Such a shard is a shard like any other.** The `recipes` job starts
   the PostgreSQL only for a shard that has `"database": true` (the image
   is empty for the rest), and names it in `PW_FEED_DATABASE_URL`, which a
   shard without one leaves unset. Such a shard sets up what its recipe
   needs, as any shard does (ADR-0249, ADR-0258), and is held to the
   shards' 345 minutes. The `database` job is gone, and the plan's
   `database`, `database_browsers` and `database_build` outputs with it.
3. **They count among the run's shards.** A run is 17 shards and the three
   browser jobs, the Free plan's 20 at once: ADR-0249's 16, and the
   database job's one. A track's push is 9 (its 8 and the database job's
   one). While the shards go round, each recipe on a database has a shard
   of its own, leaving at least one for the rest. Past that, they share
   theirs, dealt as any recipes are, and the rest share the one left.
4. **The messages track keeps its browser mutants.** The mutation controls
   are the evidence, and splitting the job is what fixes its time.

## Acceptance

- **`scripts/tests/test_ci_scripts.py`, 22 tests**, four of them on this
  ruling:
  - a recipe run against a database is a shard of its own;
  - a shard with a database sets up what its recipe needs;
  - each recipe run against a database has a database of its own;
  - recipes run against a database share when the shards run out.
- **The plan of every recipe** (`ci_plan.py --shards 17 --all`): 17
  shards, four of them on PostgreSQL with one recipe each (`e14-uploads`,
  `e14-notifications`, `e14-identity`, `e14-feed-postgres`).
- **The first verification run with it**: the messages track's, after it
  takes this commit.

## Not claimed

- **The other shards carry more.** With four recipes on a database, the
  nightly run deals the rest into 13 shards where it had 16. With
  messages', it is 12, each about a third more work than before.
- **One recipe's own time is not cut.** `e14-messages`' 45 minutes are its
  shard's, and the 6-hour limit still bounds one recipe.
- **Twenty at once is the Free plan's figure as GitHub states it.** The
  limits page does not say that a job past it waits rather than being
  cancelled, so the plan stays within it.
