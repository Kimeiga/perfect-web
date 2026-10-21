# ADR-0381: a command's send is answered within its deadline, or it is no
# answer. The host's fault, the deadline in three engines, and the mutation
# controls. A failed suite stops the recipe and stays in its evidence.
e14-command-deadline:
    @BUILD_ONLY=1 bash spikes/own-renderer/run.sh > /dev/null
    @cargo build --quiet --locked -p pw-dev-server
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0381 - a command's send is answered within its deadline, or it is no answer"; echo; \
       echo "produced by: just e14-command-deadline"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo "node: $(node --version)"; \
       echo "playwright: $(cd spikes/own-renderer && pnpm exec playwright --version)"; echo; \
       echo "== the host's fault (spikes/own-renderer/server/src/main.rs)"; echo; \
       cargo test --locked -p pw-dev-server --bin pw-dev-server -- --exact \
         tests::a_held_command_connection_is_answered_by_nothing_until_it_is_given_up \
         tests::a_dropped_command_connection_is_answered_by_nothing 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the deadline in three engines (e2e/command-deadline.spec.mjs)"; echo; \
       bash scripts/run_suite.sh -- pnpm exec playwright test e2e/command-deadline.spec.mjs --reporter=line; \
       echo; echo "== mutation controls (scripts/command_deadline_mutations.py)"; echo; \
       python3 scripts/command_deadline_mutations.py; \
     } > docs/evidence/E14/command-deadline.txt
    @grep -E "^test result|passed|failed|mutants killed|^---- |panicked at" docs/evidence/E14/command-deadline.txt
