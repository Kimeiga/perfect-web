# ADR-0371: real producer exits and complete audit reports, before allowances.
e14-node-audit:
    @mkdir -p docs/evidence/E14
    @report="$(mktemp docs/evidence/E14/.node-audit.XXXXXX)"; \
     trap 'status=$?; if [ "$status" -ne 0 ]; then cat "$report" >&2; fi; rm -f "$report"; exit "$status"' EXIT; \
     { echo "ADR-0371 - a Node audit passes only with a completed scan"; echo; \
       echo "produced by: just e14-node-audit"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       echo "python: $(python3 --version)"; echo; \
       python3 -m unittest -v scripts/tests/test_node_audit.py; \
       python3 scripts/node_audit_mutations.py; \
     } > "$report" 2>&1; \
     mv "$report" docs/evidence/E14/node-audit.txt
    @grep -E "^Ran |^OK|^FAILED|mutants killed|memory bound" docs/evidence/E14/node-audit.txt
