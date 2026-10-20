# A superseded keyed request never starts another flight to catch newer frames.
e14-keyed-retry:
    @mkdir -p docs/evidence/E14
    @report="$(mktemp docs/evidence/E14/.keyed-retry.XXXXXX)"; \
     trap 'status=$?; if [ "$status" -ne 0 ]; then cat "$report" >&2; fi; rm -f "$report"; exit "$status"' EXIT; \
     { echo "Keyed reads retry only while their page still waits for them"; echo; \
       echo "produced by: just e14-keyed-retry"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' || echo ' + uncommitted changes')"; \
       python3 scripts/keyed_retry_mutations.py; \
     } > "$report" 2>&1; \
     mv "$report" docs/evidence/E14/keyed-retry.txt
    @cat docs/evidence/E14/keyed-retry.txt
