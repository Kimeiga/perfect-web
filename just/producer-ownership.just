# The producer owns and reaps its child before cancellation restores source.
e14-producer-ownership:
    @mkdir -p docs/evidence/E14
    @{ set -eu; echo 'ADR-0376 - producer process ownership'; echo 'produced by: just e14-producer-ownership'; echo "commit: $(git rev-parse HEAD)"; python3 -m unittest discover -s scripts/tests -p test_producer_ownership.py -v; python3 scripts/producer_ownership_mutations.py; } > docs/evidence/E14/producer-ownership.txt 2>&1
    @tail -n 5 docs/evidence/E14/producer-ownership.txt
