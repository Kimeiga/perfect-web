# The bounded-checker track's recipes (4c, a read is bounded by
# construction; the design note on `track/bounded-reads`). The root justfile
# imports this file; an evidence recipe is `e14-<name>:`, writes
# `docs/evidence/E14/<name>.txt`, and is planned by the verification run like
# any other.

# 4c's checker: a read keyed by its loop's element refused (PW0405), a set
# read on a source that cannot batch warned (PW0406), a keyed read's `set`
# held to its shape (PW0335), a source's `sets`. The rules' tests
# (tests/bounded_reads.rs, the store, the feed and kiokun-site among them),
# the corpus's (corpus C24: R-070..R-072, A-037, the generality witnesses),
# the reference programs checked, and the mutation controls
# (scripts/bounded_checker_mutations.py).
e14-bounded-reads:
    @mkdir -p docs/evidence/E14
    @{ echo "4c - a read is bounded by construction (PW0405, PW0406)"; echo; \
       echo "produced by: just e14-bounded-reads"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the rules (compiler/pw-core/tests/bounded_reads.rs, and bounded's own)"; echo; \
       cargo test --locked -p pw-core --test bounded_reads 2>&1 | grep -E '^(test |test result)|panicked at'; \
       cargo test --locked -p pw-core --lib bounded:: 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the corpus: each fixture its own defect, and the generality witnesses (checking_source.rs, generality.rs)"; echo; \
       cargo test --locked -p pw-core --test checking_source --test generality 2>&1 | grep -E '^test result|^---- |panicked at'; \
       echo; echo "== the reference programs: the store, the accepted corpus, the feed, kiokun-site"; echo; \
       cargo run --quiet --locked -p pw-cli -- check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/store/*.pw 2>&1 | tail -1; \
       cargo run --quiet --locked -p pw-cli -- check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/domain.pw examples/lib/*.pw examples/accepted/*.pw 2>&1 | tail -1; \
       cargo run --quiet --locked -p pw-cli -- check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/feed/*.pw 2>&1 | tail -1; \
       NO_COLOR=1 cargo run --quiet --locked -p pw-cli -- check packages/pw-std/*.pw packages/pw-platform-web/*.pw examples/kiokun/Shards.pw examples/kiokun/dictionary.pw examples/kiokun-site/*.pw 2>&1 \
         | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' | grep -E '^(error|warning)|^pw check'; \
       echo; echo "== mutation controls (scripts/bounded_checker_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/bounded_checker_mutations.py; \
     } > docs/evidence/E14/bounded-reads.txt
    @grep -E "^test result|no diagnostics|^pw check|mutants killed|^---- |panicked at" docs/evidence/E14/bounded-reads.txt
