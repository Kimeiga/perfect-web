# ADR-0290: a verification run is dealt by the seconds its recipes last took

Status: accepted under the owner's delegation of 2026-10-02, on a finding of
the integrator's of 2026-10-09 (the nightly cancelled). Date: 2026-10-09.
Milestone: E14; the verification run (ADR-0245, ADR-0249, ADR-0278,
ADR-0281). Amends ADR-0249's shards of a kind and ADR-0278's shard each.

## Context

- **The nightly of 2026-10-08 did not finish** (run 37785652028). Two of its
  17 recipe shards were cancelled at the job's 345 minutes, one with 6 of
  its 12 recipes run, the other with 9 of 12, while one ended after 41
  minutes. Its recipes took 38.7 runner-hours in all.
- **The plan dealt by mutants.** A recipe's cost was itself and the mutants
  its scripts plant. Over the 213 recipes that night timed, that count and
  the recipe's seconds correlate at 0.43: a mutant that runs one test file
  and one that runs a crate's 1,316 tests counted alike. The plan replayed
  on the seconds measured puts 437 minutes in its longest shard.
- **Shards of a kind kept apart** (ADR-0249): a recipe that drives a browser
  shared shards only with its kind, and so did one that reads the build,
  and one that needs neither. That night, setting up took a plain shard 0.6
  to 1.0 minutes before its first recipe, and one that builds, or installs
  the browsers and builds, 2.5 to 6.5: small beside an hour's imbalance.
- **A shard each for the recipes run against a database** (ADR-0278): the
  five there are now would hold five shards of seventeen. The three that
  ran that night took 12 to 29 minutes each.
- **Each shard already writes each recipe's seconds** to its `results.json`
  as the recipe ends, and the run uploads it with the evidence.

## Decision

1. **A recipe's cost is the seconds it last took**, in a run that ran it to
   its end (exit 0), kept in `scripts/ci_seconds.json`.
   - Every evidence fetch keeps the seconds of each recipe its run ran
     (`evidence_fetch.py`), over the ones kept before; every other recipe's
     stay as they were.
   - `evidence_fetch.py <run> --times-only` keeps them from any completed
     run, one that failed or was cancelled among them, and copies no
     evidence: a recipe's time is what it took, whatever its shard's others
     did. The file starts from the nightly above: 203 recipes.
   - A recipe not yet measured costs itself and its mutants, each at the
     median of what a measured recipe took one (24 seconds that night,
     `RATE` where none is measured).
2. **Each recipe, the longest first, goes to the shard where it ends
   soonest**, what it adds to that shard's setup counted (`SETUP`, that
   night's measure). A shard still installs browsers, or builds, only for a
   recipe dealt it that needs them (ADR-0249), and a recipe that needs less
   may run where more is set up.
3. **The recipes run against a database keep shards of their own**, each
   with a PostgreSQL of its own (ADR-0278), and no other recipe runs there:
   such a shard names its database to every recipe it runs, so a recipe's
   evidence would depend on where it was dealt. They take as many shards as
   end the run soonest, the fewest of those.

## Acceptance

- **`scripts/tests/test_ci_scripts.py`, 34 tests.** New or rewritten on
  this ruling:
  - a recipe costs the seconds it last took, and one not yet measured, its
    mutants at the measured median;
  - a run is planned by the seconds kept: of three recipes in two shards,
    whichever took longest is alone;
  - a recipe needing less may run where more is set up, and what a recipe
    adds to a shard's setup counts;
  - the recipes run against a database take the shards that end the run
    soonest: long ones a shard each, short ones one between them, the
    fewest where more would end the run no sooner;
  - a recipe that ran to its end has its seconds kept, over its last, every
    other recipe's as it was, from each shard and from a lone shard.
- **`scripts/dealt_by_time_mutations.py`, 8 mutants**, 8 killed here.
- **The nightly replayed** on the seconds kept: 17 shards, sixteen ending
  at about 152 minutes and the database's at 87, where its own plan put
  437 in one.
- **The first nightly dealt by it**, named in NEXT when it has run.

## Not claimed

- **A recipe's seconds are its last run's**, not a mean. A shared runner's
  speed varies from run to run, and a change that adds a mutant is dealt
  by the time before it until the recipe runs again.
- **`SETUP` is one night's measure**, as `RATE` is.
- **The run's length is not bounded.** Nothing here stops the recipes from
  outgrowing 17 shards of 345 minutes; it puts that off until the
  recipes take about 97 runner-hours.

## Alternatives

- **Weigh mutants by kind** (a browser recipe's more): a fit to one night,
  and the time of a mutant still depends on the tests it runs.
- **Read the seconds at plan time** from the last nightly's artifacts: the
  plan would need the API and artifacts within their retention, and two
  runs of one commit could be planned apart. The kept file is read the same
  by every run of a commit.
- **More shards**: 17 recipe shards and the three browser jobs are the Free
  plan's 20 at once.
- **Any recipe in a database's shard**: see decision 3.

## Amended, 2026-10-09: a recipe not yet measured, at its kind's upper quartile

Found by `e14-refusal`'s first run (verify 37942903849): planned at the
median, 24 s a mutant, 31 units, about 12 minutes, it took about 420 s a
mutant. It was dealt after three other recipes, one of which, `e14-not-found`,
took 7,012 s where 4,260 were kept, and the shard reached its 345-minute
bound with 27 of the 30 mutants killed and none surviving. The run was lost
whole for want of a better first guess.

What a unit costs, over the 233 recipes kept that day, spans two orders of
magnitude: a median of 25 s, an upper quartile of 109, 407 at the ninetieth
percentile and 1,040 at most. By what a recipe's work runs, its kind: 144
`core` recipes, median 14 s a unit and upper quartile 27; 25 `host`
recipes, 287 and 483; 64 `browser` recipes, 142 and 408.

- **A recipe not yet measured costs its units at the upper quartile of the
  measured recipes of its kind** (`ci_plan.kind`): `browser` where it, or a
  mutation script it runs, drives Playwright; `host` where they run the
  development server's tests; `core` otherwise. Where none of its kind is
  measured, every recipe's upper quartile; where none is, `RATE`.
- **The upper quartile, not the median**, since the two errors do not cost
  the same: a recipe planned short runs its shard past the bound and the
  run is made again whole, hours; one planned long ends its shard early.
  `e14-refusal` at its kind's quartile is 31 × 408 s, 3.5 hours, what it
  took.
- **Not the alternative this ADR refused**, mutants weighed by kind for
  every recipe: a measured recipe still costs the seconds it last took, and
  the kinds' rates are read from the kept seconds at each plan, not fitted
  to one night.

`scripts/tests/test_ci_scripts.py`:
`test_a_recipe_not_yet_measured_costs_what_its_kind_does`.

## Amended, 2026-10-10: a run is dealt so that no shard ends past a ceiling under its limit

Found by `track/command-order`'s verify 38015617597: its shard 7 was dealt
13,959 s, its two longest recipes then took twice what they last had
(`e14-connection-faults` 10,364 s, `e14-invariants` 9,474 s: mutants that
hang under the command order's wait, killed at their bound), and the shard was
stopped at verify.yml's `timeout-minutes`, 345 minutes. A stopped shard is a
recipe shard that failed: nothing of the run is fetched (ADR-0281), and it is
made again whole. And `track/soft-navigation`'s verify was dealt 21,900 s into
five of its nine shards, past the limit before any overran: a track is dealt
into 9 shards (ADR-0253), and a track that touches the runtime now reaches
recipes of 184,111 s.

- **No shard of more than one recipe is dealt past `CEILING`**, 13,800 s, two
  thirds of the limit: measured shards ended up to half again past their
  recorded seconds. Where a run's asked number of shards would deal one past
  it, the run is dealt into the fewest more that do not, up to `MOST`, 40.
- **A recipe longer than the ceiling by itself** is dealt a shard of its own
  and let be: no number of shards ends it sooner. `e14-command-answers` took
  15,426 s and `e14-cart-lines` 14,021 in that run; each is the mutation
  narrowing's to shorten (NEXT, infrastructure).
- **The jobs past the Free plan's 20 wait for one to end**, which costs a run
  less than a stopped shard does. `track/soft-navigation`'s recipes, dealt
  again: 14 shards, two of one long recipe each, the rest within 13,637 s.
- The ceiling is held under the limit by a test that reads verify.yml.

`scripts/tests/test_ci_scripts.py`: four tests;
`scripts/dealt_by_time_mutations.py`: four more mutants, 17 of 17.
