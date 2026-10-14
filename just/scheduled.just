# The scheduled-commands track's recipes (the integrator's): a command runs
# when the clock passes its time. The root justfile imports this file; an
# evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`,
# and is planned by the verification run like any other.

# Step one: `invoked_by Clock`, and `schedule cmd(args) at when`, `schedule
# cmd(args) after duration` and `unschedule cmd(args)`, statements of a
# command's body (PW65xx). The compiler's tests (tests/scheduled.rs) and the
# mutation controls (scripts/scheduled_mutations.py).
e14-scheduled:
    @mkdir -p docs/evidence/E14
    @{ echo "The scheduled-commands track - a command runs when the clock passes its time"; echo; \
       echo "produced by: just e14-scheduled"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the statements and their refusals (compiler/pw-core/tests/scheduled.rs)"; echo; \
       cargo test --locked -p pw-core --test scheduled 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== mutation controls (scripts/scheduled_mutations.py)"; echo; \
       python3 scripts/scheduled_mutations.py; \
     } > docs/evidence/E14/scheduled.txt
    @grep -E "^test result|mutants killed|panicked at" docs/evidence/E14/scheduled.txt
