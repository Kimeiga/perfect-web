# The committed components' freshness (train-11, the integrator's ruling of
# 2026-10-19). The root justfile imports this file; an evidence recipe is
# `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and is planned by the
# verification run like any other.

# Every component the store's build emits is committed in docs/evidence/E10/,
# byte for byte, and none it no longer emits: the list the build's, never one
# kept beside it (compiler/pw-core/tests/evidence_is_current.rs; `just
# e10-component` commits them); a component the build adds fails until it is
# committed current; and the mutation controls, each undoing one part of the
# check (scripts/committed_components_mutations.py).
e14-committed-components:
    @mkdir -p docs/evidence/E14
    @{ echo "Train-11 - every component the store's build emits is committed current"; echo; \
       echo "produced by: just e14-committed-components"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the committed evidence (compiler/pw-core/tests/evidence_is_current.rs)"; echo; \
       cargo test --locked -p pw-core --test evidence_is_current 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== mutation controls (scripts/committed_components_mutations.py)"; echo; \
       python3 scripts/committed_components_mutations.py; \
     } > docs/evidence/E14/committed-components.txt
    @grep -E "^test result|mutants killed|^---- |panicked at" docs/evidence/E14/committed-components.txt
