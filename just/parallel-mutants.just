# The parallel-mutants track's recipes (ADR-0253, docs/PARALLEL.md): a
# mutation script's mutants run in parallel slots (ADR-0357). The track adds
# its recipes here and nowhere else; the root justfile imports this file,
# and its settings and variables (`set shell`, `PATH`) apply here as there.
# An evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`,
# and is planned by the verification run like any other (scripts/ci_plan.py
# reads this file too).

# ADR-0357: a mutation script's mutants run in parallel slots. The slots'
# tests (a toy script of the scripts' shape, in order and in 3 slots, saying
# the same, its control surviving in both; the tree never mutated; each
# slot its own; a red baseline; a run stopped from outside; two runs at
# once), and the mutation controls (scripts/mutation_slots_mutations.py),
# run as `PW_MUTANT_SLOTS` says, as every script's are.
e14-parallel-mutants:
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0357 - a mutation script's mutants run in parallel slots"; echo; \
       echo "produced by: just e14-parallel-mutants"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "python: $(python3 --version)"; echo "platform: $(uname -sm)"; echo "slots: ${PW_MUTANT_SLOTS:-1}"; echo; \
       echo "== the slots (scripts/tests/test_mutation_slots.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_mutation_slots.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== mutation controls (scripts/mutation_slots_mutations.py)"; echo; \
       python3 scripts/mutation_slots_mutations.py; \
     } > docs/evidence/E14/parallel-mutants.txt
    @grep -E "^Ran |^OK|^FAILED|mutants killed" docs/evidence/E14/parallel-mutants.txt
