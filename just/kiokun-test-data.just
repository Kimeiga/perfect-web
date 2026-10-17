# ADR-0286: unit-test answers use committed data, independent of deployment paths.
e14-kiokun-test-data:
    @mkdir -p docs/evidence/E14
    @{ echo "Kiokun unit-test dataset isolation (ADR-0286)"; echo; \
       echo "produced by: just e14-kiokun-test-data"; \
       echo "commit: $(git rev-parse HEAD)$(git diff --quiet HEAD -- . ':(exclude)docs/evidence' ':(exclude)spikes/own-renderer/store-ir.json' || echo ' + uncommitted changes')"; \
       echo "rust: $(rustc --version)"; echo; \
       python3 scripts/kiokun_test_data_mutations.py; \
     } > docs/evidence/E14/kiokun-test-data.txt 2>&1
    @grep -E 'baseline:|mutants killed|memory bound' docs/evidence/E14/kiokun-test-data.txt
