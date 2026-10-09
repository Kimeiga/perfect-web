# ADR-XXXX: a mutation script's processes are bounded in memory

Status: accepted under the owner's delegation of 2026-10-02, on a finding of
the integrator's of 2026-10-09 (CI's runner taken down, NEXT). Date:
2026-10-09. Milestone: E14; the verification run's harness (ADR-0245,
ADR-0281).

## Context

- **A mutant took CI's runner down.** `e14-graphs-on-the-wire` killed the
  runner of every verification run that planned it: navigate's (ADR-0280),
  three times, and the nightly of 2026-10-08. Each job ended with exit 143
  and nothing said of why.
- **The heartbeat named it** (ADR-0281, amended). A run of that recipe alone,
  37902799186, logged every thirty seconds:

  ```
  [e14-graphs-on-the-wire, 210s] 11.4 GiB available; graphs-34050469 2.9 GiB, ...; ... the renderer takes an earlier node: KILLED (1 of 25 tests fail)
  [e14-graphs-on-the-wire, 240s] 0.3 GiB available; graphs-34050469 14.5 GiB, ...
  ```

  The mutant after "the renderer takes an earlier node" is "the renderer
  takes a node twice" (ADR-0205): `take` made `clone`. Under it the
  100,000-node chain of `a_value_as_deep_as_its_data_is_held_without_recursion`
  copies each node's whole rest and keeps it: five billion values. The test
  that kills the mutant, "held twice", fails at once; the binary ends only
  when every test has.
- **Nothing bounded a mutant's memory.** Each of the 223 scripts runs its
  test commands as they are. 62 bound them in time (a `bounded` helper,
  first in ADR-0161's script), none in memory. A mutant that loops without
  allocating costs a job its time; one that allocates without end takes the
  machine.
- **This Mac lost the same binary** without a result each time it ran the
  script (the record of 2026-10-07: "KILLED (2 of 20 tests fail)", against a
  baseline of 25). No crash or jetsam report says how.
- **Every script imports `mutation_baseline` first in its `main()`**
  (2026-10-08, held by its tests), so one module reaches all of them.

## Decision

1. **Every process a mutation script starts, and every process those
   start, holds at most `BOUND`**: 4 GiB, or a quarter of the machine's
   memory where that is less. A thread started when `mutation_baseline` is
   imported looks twice a second (`scripts/mutation_bound.py`).
2. **A process past the bound is killed, and the script's output says so**
   where it happened, before the mutant's verdict:

   ```
     memory bound: stopped `graphs`, which held more than 4 GiB
   ```

   Its command fails as a test that cannot finish fails, and the mutant
   counts as killed. PIT counts a mutant's memory error as detected
   (`DetectionStatus.MEMORY_ERROR(true)`), and Stryker its timeout ("counted
   as detected").
3. **A kill by the bound is a kind of its own** (asked by the peer session,
   2026-10-09). cargo-mutants reports a timeout apart from a test's kill,
   and a suite whose kills were the bound's would look stronger than it is:
   W4 and W6 state every mutant killed by a failing test. So:
   - the script's output is read as it is written, and each mutant's
     verdict line after a stop names that mutant;
   - the script's last line is the bound's, in every run:

     ```
     memory bound: no process was stopped
     memory bound: a process was stopped in 1 of 19 mutants' runs, each perhaps killed by the bound alone: the renderer takes a node twice
     ```

   - `ci_summary.py` lists the second kind apart in the run's summary,
     failing nothing.

   "Perhaps": the bound cannot see whether a test also failed. Under that
   mutant, `graphs.rs`'s "held twice" fails at once, and its binary is
   stopped before it reports.
4. **What a process holds**: on Linux, `VmRSS` and `VmSwap`; on macOS, the
   physical footprint (`proc_pid_rusage`'s `ri_phys_footprint`), which
   counts what the kernel compressed (XNU's `task.c`), where the resident
   size does not.
5. **Only the script's own processes are read**, and one is stopped only
   where two readings agree it is the script's: the list of processes
   walked down from the script, and the process's own line of parents, read
   again and walked up. A process another started is never stopped, though
   its number were given again between the two.

## Acceptance

- **`scripts/tests/test_mutation_bound.py`, 15 tests**, one new in
  `test_mutation_baseline.py` and one in `test_ci_scripts.py`. Among them:
  - a process past the bound, started by a shell the test started, is
    stopped (exit 137) and said once;
  - a stop names the verdict after it, and a script's last line is the
    bound's, in a script run as each is; the run's summary lists it apart;
  - one within it runs to its end;
  - only the script's own processes are read;
  - one whose parents, read again, are another's is left alone;
  - what a process holds counts what it wrote, on the platform it runs on;
  - the structures read on macOS are the SDK's (`<sys/proc_info.h>`,
    `<sys/resource.h>`);
  - importing `mutation_baseline` starts the bound.
- **`scripts/mutation_bound_mutations.py`, 16 mutants**, 16 killed here.
  The tests that watch real processes stop only processes they marked, so a
  mutant that watches every process stops none of the machine's.
- **The case itself.** On this Mac, "the renderer takes a node twice" under
  the bound: `graphs` stopped six seconds in, cargo reporting signal 9, the
  source restored. On CI: `e14-graphs-on-the-wire` and `e14-mutation-bound`
  in one verification run, named in the merge.

## Not claimed

- **That macOS's footprint, and Linux's swap, are what is read** is held by
  the code, not by a test. Neither differs from the resident size until the
  machine is short of memory, which no test makes it.
- **A process another process of the script started and left** (its parent
  gone, adopted by `launchd` or `init`) is not the script's any more, and
  not watched. No command these scripts run leaves one.
- **The bound is not a measurement.** A mutant stopped at 4 GiB might have
  ended at 5; it counts as killed all the same, and its line says why.
- **Time is not bounded here.** The scripts that run a browser bound their
  commands in time already (`bounded`, since ADR-0161); a mutant that loops
  costs a job its time and is named by the heartbeat.

## Alternatives

- **An address-space limit (`RLIMIT_AS`)** on each command. Wasmtime and V8
  reserve far more address space than they use, so a limit low enough to
  matter breaks the tests that run them.
- **A cgroup (`systemd-run -p MemoryMax=`)** on CI. Linux's alone, and it
  needs `sudo`; this Mac would still be unbounded.
- **A smaller chain under that mutant.** The test's 100,000 nodes are what
  it is for: a value as deep as its data, held without recursion. Any
  mutant elsewhere could do the same.
- **Killing whatever holds the most when the machine runs short**, as an
  out-of-memory killer does. Whether a mutant counts as killed would then
  depend on what else the machine was doing.
