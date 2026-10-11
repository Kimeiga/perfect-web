# The store-search track's recipes (ADR-0253, docs/PARALLEL.md, W9): the
# DoorDash store's customer search. The track adds its recipes here and
# nowhere else; the root justfile imports this file, and its settings and
# variables (`set shell`, `PATH`) apply here as there. An evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other (scripts/ci_plan.py reads this file too).

# Track `store-search` (ADR-XXXX): the stores and their dishes that match a
# reader's text, matched in the store's data layer to one meaning in memory
# and on PostgreSQL (search.rs's own tests: the same fixture and table on
# both, and every character lowered alike); the search page served
# (tests/store_search.rs), the store in memory and on PostgreSQL; the page in
# three engines (e2e/store-search.spec.mjs); and the mutation controls
# (scripts/store_search_mutations.py). It needs a throwaway PostgreSQL
# database, PW_STORE_DATABASE_URL (or PW_FEED_DATABASE_URL), each test in a
# schema of its own, as e14-store-postgres does; it skips without one, and
# writes no evidence.
e14-store-search:
    @url="${PW_STORE_DATABASE_URL:-${PW_FEED_DATABASE_URL:-}}"; \
     if [ -z "$url" ]; then \
       echo "e14-store-search: skipped, PW_STORE_DATABASE_URL is not set (a throwaway PostgreSQL database, e.g. postgresql://localhost/pw_store_test)"; \
       exit 0; \
     fi; \
     export PW_STORE_DATABASE_URL="$url"; \
     mkdir -p docs/evidence/E14; \
     { echo "Track store-search - the stores and their dishes, searched in the store's data layer to one meaning"; echo; \
       echo "produced by: just e14-store-search"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "postgres: $(psql "$url" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; echo; \
       echo "== the search's meaning, in memory and on PostgreSQL (search.rs)"; echo; \
       cargo test --locked -p pw-dev-server -- search::tests:: --nocapture 2>&1 | grep -E '^(test |test result)|panicked at|Unicode'; \
       echo; echo "== the search page served, the store in memory (tests/store_search.rs)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- tests::store_search:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the search page served, the store on PostgreSQL (PW_STORE_TEST_LAYER=postgres)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::store_search:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the search page in three engines (e2e/store-search.spec.mjs)"; echo; \
       BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null 2>&1; \
       cargo build --quiet --locked -p pw-dev-server -p kiokun-server; \
       (cd spikes/own-renderer && pnpm exec playwright test e2e/store-search.spec.mjs --reporter=line 2>&1) \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' \
         | grep -E "^ +[0-9]+\) |Error:|^ +[0-9]+ (passed|failed|flaky|skipped|interrupted|did not run)" || true; \
       echo; echo "== mutation controls (scripts/store_search_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/store_search_mutations.py; \
     } > docs/evidence/E14/store-search.txt; \
     grep -E "^test result|mutants killed|^---- |panicked at|memory bound|^ +[0-9]+ (passed|failed)" docs/evidence/E14/store-search.txt
