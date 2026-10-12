# The store-hours track's recipes (docs/PARALLEL.md, "Track store-hours";
# W9): a store's opening hours. The root justfile imports this file; an
# evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`,
# and is planned by the verification run like any other.

# Track `store-hours` (ADR-XXXX): a store's weekly periods in its IANA zone,
# past midnight, special dates and a last-order cut-off; whether it is open,
# by comparison with the platform's clock (ADR-0304), shown on its page, the
# home page, a search and the cart; an order refused while a store of the
# cart is closed or past its last order. The server's tests
# (tests/store_hours.rs), the store in memory and on PostgreSQL; the page
# told at its closing in three engines (e2e/store-search.spec.mjs, the
# store's hours test among the search's, on the search's hosts); and the
# mutation controls (scripts/store_hours_mutations.py). It needs a
# throwaway PostgreSQL database, PW_STORE_DATABASE_URL (or
# PW_FEED_DATABASE_URL); it skips without one, and writes no evidence.
e14-store-hours:
    @url="${PW_STORE_DATABASE_URL:-${PW_FEED_DATABASE_URL:-}}"; \
     if [ -z "$url" ]; then \
       echo "e14-store-hours: skipped, PW_STORE_DATABASE_URL is not set (a throwaway PostgreSQL database, e.g. postgresql://localhost/pw_store_test)"; \
       exit 0; \
     fi; \
     export PW_STORE_DATABASE_URL="$url"; \
     mkdir -p docs/evidence/E14; \
     { echo "Track store-hours - a store's opening hours, and whether it is open by the platform's clock"; echo; \
       echo "produced by: just e14-store-hours"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "postgres: $(psql "$url" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; echo; \
       echo "== the hours served, the store in memory (tests/store_hours.rs)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- tests::store_hours:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the hours served, the store on PostgreSQL (PW_STORE_TEST_LAYER=postgres)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::store_hours:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the store's pages in three engines (e2e/store-search.spec.mjs)"; echo; \
       BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null 2>&1; \
       cargo build --quiet --locked -p pw-dev-server -p kiokun-server; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/store-search.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/store_hours_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/store_hours_mutations.py; \
     } > docs/evidence/E14/store-hours.txt; \
     grep -E "^test result|mutants killed|^---- |panicked at|memory bound|^ +[0-9]+ (passed|failed)" docs/evidence/E14/store-hours.txt
