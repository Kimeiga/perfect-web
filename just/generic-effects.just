# ADR-XXXX: generic results retain their member types, effects and labels.
e14-generic-effects:
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-XXXX - generic/member effects"; echo; \
       echo "produced by: just e14-generic-effects"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       cargo test --locked -p pw-core --test generic_effects --test generality --test checking_source; \
       python3 scripts/generic_effects_mutations.py; \
     } > docs/evidence/E14/generic-effects.txt 2>&1
    @grep -E '^test result|mutants killed|^baseline:' docs/evidence/E14/generic-effects.txt
