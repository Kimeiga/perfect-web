# ADR-XXXX: a component is compiled once a process

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14 (infrastructure). Takes NEXT's
"the server's components compiled once a process" (W6's finding,
2026-10-09). Builds on ADR-0292 (a mutation script's processes, bounded in
memory) and the E10-I finding that a component is compiled once and run many
times (`Prepared`).

## Context

- **A server compiled each component of its build twice.**
  - `imports_of` made an engine and compiled the component to read its
    imports.
  - `Prepared::compile` made another engine and compiled it again to run it.
  - The development server loaded every build this way, and so did
    `pw-conformance`.
- **Every server a process loaded did it again.** A test process loads a
  server per test. W6 measured a kiokun test's server load at 7.7 s, 7.1 of
  them Wasmtime compiling the program's components (2026-10-09).
- **On 2026-10-12 the development server's suite stopped fitting CI's memory
  bound.**
  - Run whole, four tests at once, it held more than 3.9 GiB (ADR-0292's
    bound on CI's runner).
  - The whole-suite mutation scripts' baselines were stopped: ten in the
    narrowing track's verify (38240842933), and `e14-kiokun-word` on master
    after kiokun's merge (38240640053).
  - kiokun's program, grown by deinflection, Homophones and search, is
    compiled for each of its tests' servers.
- **Measured here**, a 12-core Mac with 16 GiB, the suite's binary whole,
  four tests at once (`scripts/suite_memory.py`'s measure, the bound's own):

  | | wall | peak footprint |
  |---|---|---|
  | master at `c1729bbd` (`/usr/bin/time -l`) | 1,034 s | 4.47 GiB, over the 4 GiB bound |
  | compiled once a process | 618 s | 1.40 GiB |

  CI's runner, on its four cores, is recorded by the recipe.

## Decision

1. **`Prepared::compile` compiles a component once a process for its
   bytes.**
   - The process keeps one cell for each component's bytes. The first
     caller compiles into it; a caller with the same bytes takes the same
     compiled component, and one that comes while it compiles waits for it.
   - A compile that fails is kept failing.
   - A compiled component is immutable, and the engine and the component are
     each shared by reference. Authority is still never cached: every call
     builds its linker from that call's grants (E10-I).
2. **`imports_of` reads the compiled component's imports** (`Prepared::
   imports`), and a server reads them from the component it runs.
3. **`Prepared::compiles()`** counts this process's compiles, which a test
   reads.

## Alternatives

- **Run the suite on fewer threads, or raise the bound.** Rejected. Fewer
  threads double a whole-suite run's time past its bound, and ADR-0292's
  bound exists for a runaway mutant.
- **Wasmtime's on-disk compilation cache.** It serializes compiled code to a
  directory, and a process still loads a copy for each engine. One process's
  servers would still each hold their own.
- **A cache keyed by a hash of the bytes.** Rejected for the bytes
  themselves: a collision would run another component's code. The bytes are
  a component's size, kept once.

## Acceptance

- `runtime/pw-host/tests/compiled_once.rs`:
  - one compile for one component's bytes, its imports read from it;
  - another for other bytes;
  - a failing compile tried once;
  - eight callers at once, one compile.
- The development server's suite whole, four tests at once, under the bound
  with margin (`scripts/suite_memory.py`): 1.40 GiB and 618 s here, from
  4.47 GiB and 1,034 s.
- `scripts/compiled_once_mutations.py`, 2 mutants, each killed.

## Not claimed

- **A process that loads many builds keeps each one's components.** A
  development server reloading builds would grow with each distinct build.
  None reloads yet. A live reload would bound the cells first.
- **Building a program** (`pw_core::build::build`) is not cached here. The
  kiokun tests cache their own builds by source (W6's `BUILDS`).
