# NEXT 4c's first step (track `render-merge`): within one render the host
# makes each identical operation call once, in one snapshot, and on
# PostgreSQL in a bounded few seats. The root justfile imports this file; an
# evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and
# is planned by the verification run like any other.

# NEXT 4c (ADR-XXXX): the render merge's own tests and the served ones, the
# store in memory and on PostgreSQL, the feed on PostgreSQL
# (render_merge.rs, tests/render_merge.rs): each identical call once; one
# snapshot, and another render's read only where nothing invalidated it
# since the render began; on PostgreSQL a bounded few seats, every one given
# back, no render holding one while it waits for the table; pw-resource's
# reads as of an epoch (tests/read_as_of.rs); NEXT 4b's counts as re-pinned,
# the commit's re-reads 21 -> 16 and rate_order's 42 -> 34
# (tests/bounded_statements.rs); and the mutation controls, each undoing one
# rule (scripts/render_merge_mutations.py). It needs a throwaway PostgreSQL
# database, PW_STORE_DATABASE_URL and PW_FEED_DATABASE_URL (either names
# both); it skips without one, and writes no evidence.
e14-render-merge:
    @url="${PW_STORE_DATABASE_URL:-${PW_FEED_DATABASE_URL:-}}"; \
     if [ -z "$url" ]; then \
       echo "e14-render-merge: skipped, PW_STORE_DATABASE_URL is not set (a throwaway PostgreSQL database, e.g. postgresql://localhost/pw_store_test)"; \
       exit 0; \
     fi; \
     export PW_STORE_DATABASE_URL="${PW_STORE_DATABASE_URL:-$url}" PW_FEED_DATABASE_URL="${PW_FEED_DATABASE_URL:-$url}"; \
     mkdir -p docs/evidence/E14; \
     { echo "NEXT 4c - one render's identical calls made once, in one snapshot, in a bounded few seats"; echo; \
       echo "produced by: just e14-render-merge"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; \
       echo "postgres: $(psql "$url" -Atc 'SHOW server_version' 2>/dev/null || echo 'unknown (psql is not on PATH)')"; echo; \
       echo "== the merge and the snapshot, the store in memory (PW_STORE_TEST_LAYER=memory)"; echo; \
       PW_STORE_TEST_LAYER=memory cargo test --locked -p pw-dev-server -- render_merge:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the same, the store on PostgreSQL, and NEXT 4b's counts as re-pinned (PW_STORE_TEST_LAYER=postgres)"; echo; \
       PW_STORE_TEST_LAYER=postgres cargo test --locked -p pw-dev-server -- render_merge:: tests::bounded_statements:: --nocapture 2>&1 | grep -E '^(test |test result)|panicked at|^(place|rate)_order: read'; \
       echo; echo "== pw-resource: a read as of an epoch (tests/read_as_of.rs)"; echo; \
       cargo test --locked -p pw-resource --test read_as_of 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== mutation controls (scripts/render_merge_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/render_merge_mutations.py; \
     } > docs/evidence/E14/render-merge.txt; \
     grep -E "^test result|mutants killed|^---- |panicked at|memory bound" docs/evidence/E14/render-merge.txt
