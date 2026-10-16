# The each-head-effects track's recipes (ADR-0345): an `{#each}`'s list and
# key are expressions every walker of a view reads. The root justfile
# imports this file; an evidence recipe is `e14-<name>:`, writes
# `docs/evidence/E14/<name>.txt`, and is planned by the verification run
# like any other.

# ADR-0345: the holes a loop's head left and the neighbours that stay
# accepted (tests/each_head_effects.rs); every analysis at every position a
# view holds an expression at (tests/view_positions.rs); the readers whose
# tests the change reached; the corpus at C26; and the mutation controls
# (scripts/each_head_effects_mutations.py).
e14-each-head-effects:
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0345 - an each head's list and key are expressions every walker of a view reads"; echo; \
       echo "produced by: just e14-each-head-effects"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       echo "== the holes, and the neighbours that stay accepted (compiler/pw-core/tests/each_head_effects.rs)"; echo; \
       cargo test --locked -p pw-core --test each_head_effects 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== every analysis at every position (compiler/pw-core/tests/view_positions.rs)"; echo; \
       cargo test --locked -p pw-core --test view_positions 2>&1 | grep -E '^(test |test result)|panicked at'; \
       echo; echo "== the readers the change reached (each_heads, template_values, template_text, dialogs, marko_adapter, lexical_scope, every_name_resolves, session_to_browser)"; echo; \
       cargo test --locked -p pw-core --test each_heads --test template_values --test template_text --test dialogs --test marko_adapter --test lexical_scope --test every_name_resolves --test session_to_browser 2>&1 | grep -E '^test result|^---- |panicked at'; \
       echo; echo "== the corpus at C26 (corpus-check, corpus_history.rs, generality.rs, checking_source.rs)"; echo; \
       cargo run --quiet --locked -p corpus-check -- examples 2>&1 | tail -5; \
       cargo test --locked -p pw-core --test corpus_history --test generality --test checking_source 2>&1 \
         | grep -E '^test result|panicked at|generality-tested'; \
       echo; echo "== mutation controls (scripts/each_head_effects_mutations.py)"; echo; \
       CARGO_INCREMENTAL=0 python3 scripts/each_head_effects_mutations.py; \
     } > docs/evidence/E14/each-head-effects.txt
    @grep -E "^test result|mutants killed|^---- |panicked at" docs/evidence/E14/each-head-effects.txt
