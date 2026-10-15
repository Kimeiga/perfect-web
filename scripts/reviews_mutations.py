#!/usr/bin/env python3
"""Mutation controls for track `reviews` (ADR-0341, NEXT's W9): a store
rated through a delivered order, once; what its ratings come to, on its
page, the home page and a search; its reviews, without who gave them.

Each mutant undoes one rule:
- the program (`examples/store/reviews.pw`, `rate.pw`): the mean said from
  nine ratings, or only from eleven; the mean cut, not rounded half up; "1
  rating" said "1 ratings", and many said one; a store no one rated said
  rated; a list's store said another's; an order not delivered linked to
  rate, and its rate page opened; the reader's own open pages not told;
  `MayRate` not required;
- the predicate (`spikes/own-renderer/server/src/reviews.rs`'s `rateable`):
  holding before delivery, for a store not of the order, and for one rated;
- the layer's `#rate`: an order not delivered rated, a store not of the
  order rated, stars past 5 and a comment past 500 characters taken;
- the data: memory and PostgreSQL rating a store twice (each its own key);
  memory's orders sharing one id; memory's ratings not kept; PostgreSQL's
  latest order read as its first;
- the reviews: one naming its order, memory's and PostgreSQL's oldest first
  and with no comment, six shown;
- PostgreSQL's ratings read a store at a time (NEXT 4b's control).

A mutant must fail the development server's reviews tests in memory
(`server`), or on PostgreSQL (`postgres`), narrowed by name to
`tests::reviews::`; the last, `tests::bounded_statements::` on PostgreSQL
(`bounded`). Every `.pw` mutant is a well-typed program.

Run from the repository root; `just e14-reviews` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
REVIEWS_PW = ROOT / "examples/store/reviews.pw"
RATE_PW = ROOT / "examples/store/rate.pw"
REVIEWS_RS = ROOT / "spikes/own-renderer/server/src/reviews.rs"
STORE_DATA = ROOT / "spikes/own-renderer/server/src/store.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a store's mean is said from nine ratings",
        "server",
        REVIEWS_PW,
        "fn fewest_averaged() -> Int !{} {\n    10\n}\n",
        "fn fewest_averaged() -> Int !{} {\n    9\n}\n",
    ),
    (
        "a store's mean waits for an eleventh rating",
        "server",
        REVIEWS_PW,
        "fn fewest_averaged() -> Int !{} {\n    10\n}\n",
        "fn fewest_averaged() -> Int !{} {\n    11\n}\n",
    ),
    (
        "a mean is cut, not rounded half up",
        "server",
        REVIEWS_PW,
        "    (r.stars * 20 + r.count) / (r.count * 2)\n",
        "    (r.stars * 10) / r.count\n",
    ),
    (
        "one rating is said many",
        "server",
        REVIEWS_PW,
        '        "1 rating"\n',
        '        "1 ratings"\n',
    ),
    (
        "many ratings are said one",
        "server",
        REVIEWS_PW,
        '        "{r.count} ratings"\n',
        '        "{r.count} rating"\n',
    ),
    (
        "a store no one rated is said rated",
        "server",
        REVIEWS_PW,
        '        None => "No ratings yet",\n',
        '        None => "0 ratings",\n',
    ),
    (
        "a list's store is said another store's ratings",
        "server",
        REVIEWS_PW,
        "    match List.find(all, r => same_store(r.store, store)) {\n",
        "    match List.get(all, 0) {\n",
    ),
    (
        "an order not delivered links its stores to rate",
        "server",
        REVIEWS_PW,
        "    if o.delivered {\n",
        "    if true {\n",
    ),
    (
        "a store of an order not delivered opens its rate page",
        "server",
        REVIEWS_PW,
        "        None => if delivered {\n",
        "        None => if true {\n",
    ),
    (
        "the reader's own open pages are not told of their rating",
        "server",
        RATE_PW,
        "    emits         UserCartChanged(current_user()), StoreRated(store)\n",
        "    emits         StoreRated(store)\n",
    ),
    (
        "a rating is given whatever its order",
        "server",
        RATE_PW,
        "    requires      SignedIn, MayRate(store)\n",
        "    requires      SignedIn\n",
    ),
    (
        "MayRate holds before the order is delivered",
        "server",
        REVIEWS_RS,
        "    delivered\n        && stores.iter().any(|of| {\n",
        "    (delivered || true)\n        && stores.iter().any(|of| {\n",
    ),
    (
        "MayRate holds for a store not of the order",
        "server",
        REVIEWS_RS,
        "            matches!(val_at(of, &[\"store\"]), Some(Val::String(id)) if id == store)\n",
        "            matches!(val_at(of, &[\"store\"]), Some(Val::String(id)) if !id.is_empty() || id == store)\n",
    ),
    (
        "MayRate holds for a store rated already",
        "server",
        REVIEWS_RS,
        "                && matches!(val_at(of, &[\"given\"]), Some(Val::Option(None)))\n",
        "                && matches!(val_at(of, &[\"given\"]), Some(Val::Option(_)))\n",
    ),
    (
        "the layer rates an order not delivered",
        "server",
        REVIEWS_RS,
        "                if order.status != DELIVERED {\n",
        "                if order.status.is_empty() {\n",
    ),
    (
        "the layer rates a store not of the order",
        "server",
        REVIEWS_RS,
        "                if !order.stores.iter().any(|of| of.store == *store) {\n",
        "                if order.stores.is_empty() {\n",
    ),
    (
        "the layer takes stars past 5",
        "server",
        REVIEWS_RS,
        "            if !(1..=5).contains(stars) {\n",
        "            if !(1..=6).contains(stars) {\n",
    ),
    (
        "the layer takes a comment past 500 characters",
        "server",
        REVIEWS_RS,
        "            if comment.chars().count() > MOST_COMMENT {\n",
        "            if comment.chars().count() > MOST_COMMENT + 1 {\n",
    ),
    (
        "memory rates a store of an order twice",
        "server",
        REVIEWS_RS,
        "    if ratings\n"
        "        .iter()\n"
        "        .any(|r| r.order == rating.order && r.store == rating.store)\n"
        "    {\n"
        "        return false;\n"
        "    }\n",
        "",
    ),
    (
        "PostgreSQL rates a store of an order twice",
        "postgres",
        REVIEWS_RS,
        "             VALUES ($1, $2, $3, $4) ON CONFLICT (order_id, store) DO NOTHING\",\n",
        "             VALUES ($1, $2, $3, $4)\",\n",
    ),
    (
        "memory's orders share one id",
        "server",
        STORE_DATA,
        "        self.orders_made += 1;\n",
        "",
    ),
    (
        "memory's ratings are not kept",
        "server",
        STORE_DATA,
        "        Ok(crate::reviews::mem_rate(&mut self.state().ratings, rating))\n",
        "        Ok(true)\n",
    ),
    (
        "PostgreSQL's latest order is read as its first",
        "postgres",
        REVIEWS_RS,
        "             FROM (SELECT id, status FROM orders WHERE owner = $1 ORDER BY id DESC LIMIT 1) o \\\n",
        "             FROM (SELECT id, status FROM orders WHERE owner = $1 ORDER BY id LIMIT 1) o \\\n",
    ),
    (
        "a review names its order",
        "server",
        REVIEWS_RS,
        "                                (\"comment\".into(), Val::String(comment)),\n",
        "                                (\"comment\".into(), Val::String(comment)),\n"
        "                                (\"order\".into(), Val::S64(id)),\n",
    ),
    (
        "memory lists reviews oldest first",
        "server",
        REVIEWS_RS,
        "        .enumerate()\n        .rev()\n",
        "        .enumerate()\n",
    ),
    (
        "memory lists a rating with no comment as a review",
        "server",
        REVIEWS_RS,
        "        .filter(|(_, r)| r.store == store && !r.comment.is_empty())\n",
        "        .filter(|(_, r)| r.store == store)\n",
    ),
    (
        "PostgreSQL lists reviews oldest first",
        "postgres",
        REVIEWS_RS,
        "             WHERE store = $1 AND comment <> '' ORDER BY id DESC LIMIT $2\",\n",
        "             WHERE store = $1 AND comment <> '' ORDER BY id LIMIT $2\",\n",
    ),
    (
        "PostgreSQL lists a rating with no comment as a review",
        "postgres",
        REVIEWS_RS,
        "             WHERE store = $1 AND comment <> '' ORDER BY id DESC LIMIT $2\",\n",
        "             WHERE store = $1 ORDER BY id DESC LIMIT $2\",\n",
    ),
    (
        "a store's page shows six reviews",
        "server",
        REVIEWS_RS,
        "pub(crate) const REVIEWS_SHOWN: usize = 5;\n",
        "pub(crate) const REVIEWS_SHOWN: usize = 6;\n",
    ),
    (
        "PostgreSQL's ratings are read a store at a time",
        "bounded",
        REVIEWS_RS,
        "    let counted: BTreeMap<String, (i64, i64)> = c\n"
        "        .query(\n"
        "            \"SELECT store, count(*), sum(stars)::bigint FROM order_ratings \\\n"
        "             WHERE store = ANY($1) GROUP BY store\",\n"
        "            &[&stores],\n"
        "        )\n"
        "        .map_err(pg)?\n"
        "        .iter()\n"
        "        .map(|r| (r.get(0), (r.get(1), r.get(2))))\n"
        "        .collect();\n",
        "    let mut counted: BTreeMap<String, (i64, i64)> = BTreeMap::new();\n"
        "    for s in stores {\n"
        "        for r in c\n"
        "            .query(\n"
        "                \"SELECT store, count(*), sum(stars)::bigint FROM order_ratings \\\n"
        "                 WHERE store = $1 GROUP BY store\",\n"
        "                &[s],\n"
        "            )\n"
        "            .map_err(pg)?\n"
        "        {\n"
        "            counted.insert(r.get(0), (r.get(1), r.get(2)));\n"
        "        }\n"
        "    }\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
    "tests::reviews::",
]
BOUNDED = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
    "tests::bounded_statements::",
]
CARGO = {
    "server": (TESTS, {"PW_STORE_TEST_LAYER": "memory", "PW_STORE_DATABASE_URL": ""}),
    "postgres": (TESTS, {"PW_STORE_TEST_LAYER": "postgres"}),
    "bounded": (BOUNDED, {"PW_STORE_TEST_LAYER": "postgres"}),
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, env):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, cwd=ROOT, env={**os.environ, **env}, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    cmd, env = CARGO[suite]
    out, code = bounded(cmd, env)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    if not os.environ.get("PW_STORE_DATABASE_URL"):
        print("FAIL: the postgres suites need PW_STORE_DATABASE_URL, a throwaway database")
        return 1
    for suite in CARGO:
        built, passed, failed = cargo_tests(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    for what, suite, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = cargo_tests(suite)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
