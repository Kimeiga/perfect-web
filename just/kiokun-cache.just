# ADR-0358: private SQLite disk-cache concurrency/publication/lifetime.
e14-kiokun-cache:
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0358 - Kiokun cache publication and reader lifetime"; echo; \
       echo "produced by: just e14-kiokun-cache"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       cargo test --quiet --locked -p pw-dev-server --bin pw-dev-server kiokun::search::cache_tests -- --test-threads=4; \
       python3 scripts/kiokun_cache_mutations.py; \
     } > docs/evidence/E14/kiokun-cache.txt 2>&1
    @grep -E '^test result|mutants killed|^baseline:' docs/evidence/E14/kiokun-cache.txt
