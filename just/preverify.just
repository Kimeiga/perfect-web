# The preverify track's recipes (ADR-0253, docs/PARALLEL.md): a train is
# pre-verified on this machine before it is pushed (ADR-0364). The track adds
# its recipes here and nowhere else; the root justfile imports this file, and
# its settings and variables (`set shell`, `PATH`) apply here as there. An
# evidence recipe is `e14-<name>:`, writes `docs/evidence/E14/<name>.txt`, and
# is planned by the verification run like any other (scripts/ci_plan.py reads
# this file too).

# The recipes CI's verify would run for HEAD, run here in parallel lanes
# (`just preverify [BASE] [--lanes N] [--recipes "a b"] [--plan]`; BASE is a
# ref, origin/master where it is not given). A pre-check only: it writes no
# evidence into this tree and never pushes, and CI's verify stays the
# evidence of record (ADR-0281). `python3 scripts/preverify.py --help`.
preverify *args:
    @python3 scripts/preverify.py "$@"

# ADR-0364: a train is pre-verified locally. The tool's tests (a toy tree run
# in lanes: each lane its own ports, target and environment, the costliest
# recipe first, a failure reported as CI's summary reports it, the tree it was
# run in untouched, the lanes taken away, a signal stopping what they run),
# and the mutation controls (scripts/preverify_mutations.py).
e14-preverify:
    @mkdir -p docs/evidence/E14
    @{ echo "ADR-0364 - a train is pre-verified locally"; echo; \
       echo "produced by: just e14-preverify"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "python: $(python3 --version)"; echo "just: $(just --version)"; echo "git: $(git --version)"; echo; \
       echo "== the tool (scripts/tests/test_preverify.py)"; echo; \
       python3 -m unittest -v scripts/tests/test_preverify.py 2>&1 | grep -E ' \.\.\. |^Ran |^OK|^FAILED'; \
       echo; echo "== mutation controls (scripts/preverify_mutations.py)"; echo; \
       python3 scripts/preverify_mutations.py; \
     } > docs/evidence/E14/preverify.txt
    @grep -E "^Ran |^OK|^FAILED|mutants killed" docs/evidence/E14/preverify.txt
