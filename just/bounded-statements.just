# NEXT 4b's recipes (W9): a read is a bounded number of statements on
# PostgreSQL. The root justfile imports this file; an evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other.

# NEXT 4b (ADR-XXXX): every operation of the store's data layer, and every
# page and command of the store, counted on PostgreSQL at the seed and with
# fifty stores more, each count pinned and equal at both
# (tests/bounded_statements.rs); and the mutation controls, each restoring a
# loop that read a store at a time (scripts/bounded_statements_mutations.py).
# It needs a throwaway PostgreSQL database, PW_STORE_DATABASE_URL (or
# PW_FEED_DATABASE_URL); it skips without one, and writes no evidence.
e14-bounded-statements:
    @url="${PW_STORE_DATABASE_URL:-${PW_FEED_DATABASE_URL:-}}"; \
     if [ -z "$url" ]; then \
       echo "e14-bounded-statements: skipped, PW_STORE_DATABASE_URL is not set (a throwaway PostgreSQL database, e.g. postgresql://localhost/pw_store_test)"; \
       exit 0; \
     fi; \
     export PW_STORE_DATABASE_URL="$url"; \
     mkdir -p docs/evidence/E14; \
     { echo "NEXT 4b - a read is a bounded number of statements on PostgreSQL"; echo; \
       echo "produced by: just e14-bounded-statements"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "postgres: $(psql "$url" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; echo; \
       echo "== each operation, and each page and command, counted at the seed and with fifty stores more (tests/bounded_statements.rs)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- tests::bounded_statements:: --nocapture 2>&1 | grep -E '^(test |test result)|panicked at|^  store:|statements per'; \
       echo; echo "== mutation controls (scripts/bounded_statements_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/bounded_statements_mutations.py; \
     } > docs/evidence/E14/bounded-statements.txt; \
     grep -E "^test result|mutants killed|^---- |panicked at|memory bound" docs/evidence/E14/bounded-statements.txt
