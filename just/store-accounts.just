# The store-accounts track's recipes (ADR-0253, docs/PARALLEL.md, W8's plan):
# the DoorDash store's customer side. The track adds its recipes here and
# nowhere else; the root justfile imports this file, and its settings and
# variables (`set shell`, `PATH`) apply here as there. An evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other (scripts/ci_plan.py reads this file too).

# Track `store-accounts` (ADR-XXXX), milestone 1: a cart and an order a
# user's. The compiler's `user` visibility and a record's scope its producers'
# joined (tests/user_visibility.rs, tests/boundary_matrix.rs, boundary.rs's
# own); the corpus (R-063, A-033, R-064); the server's (tests/store_accounts.rs
# and store.rs's joins), in memory and with the store on PostgreSQL; identity's
# sign-in, which tells the store of a guest signed in; telling by principal,
# which the feed's notifications share; milestone 2's delivery addresses
# (tests/addresses.rs, places.rs's own); and the mutation controls
# (scripts/store_accounts_mutations.py). It needs a throwaway
# PostgreSQL database, PW_STORE_DATABASE_URL (or PW_FEED_DATABASE_URL), each
# test in a schema of its own, as e14-store-postgres does.
e14-store-accounts:
    @url="${PW_STORE_DATABASE_URL:-${PW_FEED_DATABASE_URL:-}}"; \
     if [ -z "$url" ]; then \
       echo "e14-store-accounts: skipped, PW_STORE_DATABASE_URL is not set (a throwaway PostgreSQL database, e.g. postgresql://localhost/pw_store_test)"; \
       exit 0; \
     fi; \
     export PW_STORE_DATABASE_URL="$url"; \
     export PW_FEED_DATABASE_URL="${PW_FEED_DATABASE_URL:-$url}"; \
     mkdir -p docs/evidence/E14; \
     { echo "Track store-accounts - a cart and an order a user's (milestone 1), delivery addresses (milestone 2)"; echo; \
       echo "produced by: just e14-store-accounts"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "postgres: $(psql "$url" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; echo; \
       echo "== the compiler: \`user\`, and a record's scope its producers' joined"; echo; \
       cargo test --locked -p pw-core --test user_visibility 2>&1 | grep -E '^(test |test result)|panicked at'; \
       cargo test --locked -p pw-core --test boundary_matrix 2>&1 | grep -E '^(test |test result)|panicked at'; \
       cargo test --locked -p pw-core --lib boundary 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the corpus (corpus-check; R-063, A-033, R-064)"; echo; \
       cargo run --quiet --locked -p corpus-check -- examples 2>&1 | tail -5; \
       echo; echo "== the server, the store in memory (tests/store_accounts.rs, store.rs's joins, tests/sign_in.rs, tests/addresses.rs, places.rs)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- tests::store_accounts:: store::joins:: tests::sign_in:: tests::notifications:: a_sessions_own_change_reaches_no_other_session tests::addresses:: places:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the server, the store on PostgreSQL (PW_STORE_TEST_LAYER=postgres)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::store_accounts:: store::joins:: tests::addresses:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== mutation controls (scripts/store_accounts_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/store_accounts_mutations.py; \
     } > docs/evidence/E14/store-accounts.txt; \
     grep -E "^test result|mutants killed|^---- |panicked at|memory bound" docs/evidence/E14/store-accounts.txt
