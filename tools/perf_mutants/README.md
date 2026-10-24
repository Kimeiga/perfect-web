# Pleris full mutation-program worker-count experiment

The existing strict gate `scripts/mutant_programs.py` executes 344 real
program-source mutants in 13 source programs using six Python threads, with
each thread spawning a `pw check` subprocess. On the latest x64 green CI,
its gate alone took approximately 10m26s. Six concurrent compiler processes
may exceed the actual GitHub runner CPU capacity.

This experiment runs the **same unmodified mutation checker** with 2, 4 and
6 worker threads on separate x64 CI runners; all its real checks and
non-vacuity, cross-program, named-refusal and source-provenance rules remain
active. A comparator fails if a result is missing, red, or disagrees on
mutants, programs, checks, refusal counts or revision.

All changes are isolated under tools/perf_mutants and an experiment-only
workflow; no CI, compiler, or application runtime defaults change. Trials
record wall time, worker counts, exact checksum identity and the complete
mutation gate's own output. Runner variability limits conclusions from one
trial per worker count.

Benchmark controls: python3 -m unittest discover -s tools/perf_mutants -p
'test_*.py' -v

Original CI reference:
https://github.com/studiokimeiga/perfect-web/actions/runs/40127610536
