# Inferred effect rows converge before purity checks or capability contracts.
e14-effect-convergence:
    @mkdir -p docs/evidence/E14
    @report="$(mktemp docs/evidence/E14/.effect-convergence.XXXXXX)"; \
     trap 'status=$?; if [ "$status" -ne 0 ]; then cat "$report" >&2; fi; rm -f "$report"; exit "$status"' EXIT; \
     { echo "Effect propagation reaches a fixed point before checking or granting"; echo; \
       echo "produced by: just e14-effect-convergence"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       cargo test --quiet --locked -p pw-core --test effect_convergence -- --ignored --nocapture; \
       python3 scripts/effect_convergence_mutations.py; \
     } > "$report" 2>&1; \
     mv "$report" docs/evidence/E14/effect-convergence.txt
    @cat docs/evidence/E14/effect-convergence.txt
