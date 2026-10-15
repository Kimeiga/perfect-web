# Track `reviews`'s recipes (NEXT, W9: ratings and reviews of orders and
# stores). The root justfile imports this file; an evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other.

# Track `reviews` (ADR-XXXX): a store rated through a delivered order, once,
# 1 to 5 stars and a comment if the reader likes; what its ratings come to on
# its page, the home page and a search, read for every listed store at once;
# its reviews on its page without their reviewer, which the checker holds
# (PW5001). The server's tests (tests/reviews.rs), the store in memory and on
# PostgreSQL; the ratings' operations, pages and command counted
# (tests/bounded_statements.rs); the store rated in three engines
# (e2e/reviews.spec.mjs, REVIEWS_PORTS); and the mutation controls
# (scripts/reviews_mutations.py). It needs a throwaway PostgreSQL database,
# PW_STORE_DATABASE_URL (or PW_FEED_DATABASE_URL); it skips without one, and
# writes no evidence.
e14-reviews:
    @url="${PW_STORE_DATABASE_URL:-${PW_FEED_DATABASE_URL:-}}"; \
     if [ -z "$url" ]; then \
       echo "e14-reviews: skipped, PW_STORE_DATABASE_URL is not set (a throwaway PostgreSQL database, e.g. postgresql://localhost/pw_store_test)"; \
       exit 0; \
     fi; \
     export PW_STORE_DATABASE_URL="$url"; \
     mkdir -p docs/evidence/E14; \
     { echo "Track reviews - a store rated through a delivered order, once; its reviews without their reviewer"; echo; \
       echo "produced by: just e14-reviews"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "postgres: $(psql "$url" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; echo; \
       echo "== the ratings served, the store in memory (tests/reviews.rs)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- tests::reviews:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the ratings served, the store on PostgreSQL (PW_STORE_TEST_LAYER=postgres)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::reviews:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== each operation, page and command counted, the ratings' among them (tests/bounded_statements.rs)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::bounded_statements:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the store rated in three engines (e2e/reviews.spec.mjs)"; echo; \
       if BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null 2>&1 \
          && cargo build --quiet --locked -p pw-dev-server -p kiokun-server; then \
         (cd spikes/own-renderer && pnpm exec playwright test e2e/reviews.spec.mjs --reporter=line 2>&1) \
           | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
           | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       else \
         echo "FAILED: the build (spikes/own-renderer/run.sh, cargo build) did not finish, so the browser suite did not run"; \
       fi; \
       echo; echo "== mutation controls (scripts/reviews_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/reviews_mutations.py; \
     } > docs/evidence/E14/reviews.txt; \
     grep -E "^test result|mutants killed|^---- |panicked at|memory bound|^FAILED|^ +[0-9]+ (passed|failed)" docs/evidence/E14/reviews.txt
