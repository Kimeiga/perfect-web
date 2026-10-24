# Linux linker choice: default vs explicit LLD vs mold

This experiment controls the main ambiguity left by run 40119221093.
Rust 1.97's x64 GNU target uses LLD by default; ARM64 can use the
system linker. The earlier result (x64 mold 14.7% slower, ARM64 mold
42.6% faster) is for *end-to-end Rust test compilation*, not isolated
linking.

This branch does not modify any CI default. On the same x64/ARM runner,
it compares three successful clean builds each with the native default,
explicit GCC -fuse-ld=lld and GCC -fuse-ld=mold, counterbalanced in three
rounds. It probes the actual linker versions through separate Rust
executables and runs an adapter integration smoke test against each linker.
All observed timings, raw logs, exit codes and version probes are uploaded;
failed/incomplete results cannot win.

Run local script controls: python3 -m unittest discover -s tools/perf_linker
-p 'test_*.py' -v

The repository's Rust dependency, build profiles and compiler version
remain unchanged. Promoting an accelerator to ARM CI requires a separate
validated PR and compatibility with PR #46's sharding work.
