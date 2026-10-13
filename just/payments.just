# The payments track's recipes (NEXT 6c, the integrator's): a payment at the
# deployment's provider, settled only by the provider's word. The root
# justfile imports this file; an evidence recipe is `e14-<name>:`, writes
# `docs/evidence/E14/<name>.txt`, and is planned by the verification run
# like any other.

# NEXT 6c, step six: a command the platform invokes, which no reader presses
# (`invoked_by Payments`, PW64xx). The compiler's tests
# (tests/invoked_by.rs), the server's (payments::invoked_tests), and the
# mutation controls (scripts/invoked_by_mutations.py).
e14-invoked-by:
    @mkdir -p docs/evidence/E14
    @{ echo "NEXT 6c - a command the platform invokes, which no reader presses"; echo; \
       echo "produced by: just e14-invoked-by"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the clause and its refusals (compiler/pw-core/tests/invoked_by.rs)"; echo; \
       cargo test --locked -p pw-core --test invoked_by 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== a reader's press refused (spikes/own-renderer/server/src/payments.rs)"; echo; \
       cargo test --locked -p pw-dev-server -- invoked_tests 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== mutation controls (scripts/invoked_by_mutations.py)"; echo; \
       python3 scripts/invoked_by_mutations.py; \
     } > docs/evidence/E14/invoked-by.txt
    @grep -E "^test result|mutants killed|panicked at" docs/evidence/E14/invoked-by.txt
