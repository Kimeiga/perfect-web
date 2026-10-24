Benchmark only: pw-core test-binary sharding, not default CI.

Source of truth: Cargo metadata targets of kind test, cross-checked against
all direct compiler/pw-core/tests/*.rs files. New tests join exactly one
shard. Each shard invokes cargo test --locked -p pw-core --no-fail-fast with
unfiltered --test targets; independent job runs lib and doc tests. Any failure
fails the experiment. Shards are weighted using observed per-binary times
from the passing x64 pw-core CI job 120972413421 on master dafe726a.
Reference: https://github.com/studiokimeiga/perfect-web/actions/runs/40127610536

Runtime, memory, total GitHub runner-minutes and inventory must be compared
before adopting any version of this orchestration. No production defaults
or required checks are changed by this experiment.
