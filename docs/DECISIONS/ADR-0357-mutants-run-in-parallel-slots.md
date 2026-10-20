# ADR-0357: a mutation script's mutants run in parallel slots

Status: proposed by track `parallel-mutants`, for the integrator's
numbering. Date: 2026-10-15. Milestone: E14; the verification run's harness
(ADR-0245, ADR-0281, ADR-0292).

## Context

- **Mutation controls are the project's core evidence, and its slowest
  part.** 261 `scripts/*_mutations.py` scripts (2882 mutants,
  `mutation_anchors.py` of 2026-10-14) each apply one mutant in place,
  rebuild, run its tests and restore, one mutant at a time.
  `store_accounts_mutations.py` took 37 minutes on the 32-thread PC (37 of
  37 killed); CI's verification shards take 4-5 hours and more, and reached
  GitHub's 345-minute limit a job (train-7's `e14-refusal` shard ran past
  4.5 h). Every track waits on them.
- **What the scripts share** (surveyed 2026-10-14, every script read by its
  syntax tree):
  - No runner. The scripts import the standard library alone (`sys`,
    `subprocess`, `re`, `pathlib` in each; `os` in 103, `signal` in 98,
    `shutil` 5, `random`, `glob`, `base64` one each). Each holds its loop.
  - **One module reaches every script**: each imports `mutation_baseline`
    first in its `main()` (ADR-0292; held by its tests). The import keeps
    what each command says and starts the memory bound, which wraps the
    script's output to read each verdict.
  - Each script's shape: `ROOT` its own tree (`Path(__file__).parent.parent`),
    a module-level list `MUTANTS` of `(what, [suite,] path, anchor,
    replacement)`, a baseline per suite (`baseline (<suite>): N passed, F
    failed`), then for each mutant in the list's order the anchor counted,
    applied, the suite run with `cwd=ROOT`, the source restored in a
    `finally`, and its verdict printed, then `<k> of <n> mutants killed`,
    then (at exit) the bound's line. 250 scripts loop `for ... in MUTANTS`;
    10 run their baselines for the suites their mutants name, in the order
    of the suites' names (`sorted({m[1] for m in MUTANTS})`);
    `row_reads_mutations.py` runs the mutants whose description holds one
    of its arguments.
  - Verdict lines: `{what}: {v}` (203 scripts), `{what} [{suite}]: {v}`
    (56), `{stack}: {what}: {v}` (bench_contract), `{gate}  {what}: {v}`
    (e9_value), `{what}: NOT RUN (..)` (messages, notifications, uploads),
    `DID NOT BUILD, so it shows nothing` (runtime_size).
  - Their readers: `evidence_survivors.py` (`: SURVIVED`, `ANCHOR NOT FOUND
    EXACTLY ONCE in`, `^(\d+) of (\d+) mutants killed`), `ci_summary.py`
    (`SURVIVED`, `ANCHOR NOT FOUND`, `memory bound: ` lines),
    `mutation_bound.Lines` (verdicts, `baseline` lines), and the recipes'
    `grep`s. The recipes keep the script's standard output alone as
    evidence (`{ ..; python3 scripts/x_mutations.py; } > docs/evidence/..`).
- **What two copies of one script running at once would share**, found in
  the tree:
  - **Ports.** The browser suites' hosts are `PORT + offset`, the largest
    offset +182 (`STORE_PAYMENTS_PORTS`, `playwright.config.mjs`). Two
    scripts start hosts on fixed ports instead:
    `messages_mutations.py` (`PW_MESSAGES_PORT`, 7141 where unset) and
    `notifications_mutations.py` (`PW_NOTIFICATIONS_PORT`, 6100). The
    server's tests bind `127.0.0.1:0`.
  - **Temporary directories of fixed names**: `one_comparison.rs`
    (`pw-one-comparison-control`, `-ok`, `-hash`, `-substring`),
    `name_keyed_maps.rs` (`pw-name-keyed-control`), and
    `tests/support/mod.rs`'s `temp_dir().join(name)`.
  - **git**: `marko_adapter.rs` asks `git ls-files '*.marko'` of the tree;
    with no git it would pass having read nothing.
  - **`target/`, by path**: the Playwright config starts
    `../../target/debug/pw-dev-server`, `run.sh` and `feed.sh` copy
    `$REPO_ROOT/target/wasm32-unknown-unknown/..`, and
    `runtime_size_mutations.py` writes `ROOT/target/..`. Nothing here sets
    `CARGO_TARGET_DIR`, nor does CI.
  - **PostgreSQL**: each test's schema is its process's
    (`pw_test_{pid}_{test}`, `pw_identity_{pid}_..`): shared safely.
  - **`node_modules`**: what is installed, read; the Next.js and SvelteKit
    builds (`bench_contract_mutations.py`) may write their tools' caches
    into it.

## Decision

1. **`PW_MUTANT_SLOTS=N`** asks for N slots. Unset or 1, nothing changes:
   every script runs as it did, in place. A number below 1 or not a number
   stops the run, said.
2. **Above 1, the tree is never mutated.** `mutation_baseline`'s import
   calls `mutation_slots.engage()` before it starts the memory bound; in
   the tree, that runs the slots (`scripts/mutation_slots.py`) and ends the
   process with the run's code. The script's own loop never runs in the
   tree, so a run stopped from outside leaves no mutant there.
3. **A slot is a copy of the tree**, `target/mutant-slots/<k>` (or under
   `PW_MUTANT_SLOT_DIR`), kept from run to run:
   - every file of the tree as it is, tracked, untracked and ignored, but
     its `.git` and `target/`; each `node_modules/` a link to the tree's;
   - brought to the tree at each run **by content**: a file that differs is
     written (its time now, so that cargo builds it again), one that does
     not is left (so that cargo's last build of it stands), what the tree
     does not hold is removed. What a stopped run left in a slot, a mutant
     or bytecode, is undone at the next;
   - its own git: the tree's commit, its index that commit's files, its
     objects read from the tree's repository (`objects/info/alternates`),
     registered nowhere in the tree's repository;
   - its own `CARGO_TARGET_DIR`, `<slot>/target`: where the repository's
     scripts look for what cargo built, and built from the slot's own
     paths. **A slot's target is not seeded from the tree's**: a copy of
     the tree's artifacts beside sources with the tree's times would look
     fresh to cargo, and a test binary built there reads its fixtures from
     the tree's `CARGO_MANIFEST_DIR`, not the slot's;
   - its own ports, `PORT + k x 200` (`PORT` 3141 where unset), and its own
     `TMPDIR`, emptied at each run;
   - and the bound of ADR-0292 in each slot's script, as in any script: the
     bound is a process's, and a mutant's verdict does not change with the
     number of slots.
4. **One run uses the slots at a time**: a run holds `<slots>/.lock`; a
   second waits for it, said on standard error.
5. **The mutants are dealt.** Slot 0 first takes the first mutant of each
   suite (so that its baseline, in the 10 scripts whose baselines follow
   their mutants' suites, is the script's whole); then each mutant goes to
   the slot holding fewest of its suite, then fewest in all. A slot runs
   the script's own copy in the slot, with `PW_MUTANT_PICK=<count>:<i>,..`;
   there `engage()` narrows `MUTANTS` in place to those (after checking
   the count they were dealt from), and takes `PW_MUTANT_PICK` and
   `PW_MUTANT_SLOT` out of the environment; `PW_MUTANT_SLOTS` is not given
   to a slot. Nothing a slot runs deals again.
6. **Each slot runs its own baseline**: a slot's copy is shown green before
   any mutant there counts. A slot whose baseline is red is said as the
   script says it, that slot's words alone, and the run stops.
7. **The output is the script's**, on standard output; each slot's
   standard error is passed on as it comes:
   - the baseline as slot 0 said it;
   - each mutant's lines (those since the slot's last verdict, and its
     verdict) in the script's order, each as soon as every mutant before it
     has been said. A line is a mutant's verdict where it begins with the
     mutant's first element, then `:`, ` [` or two spaces, holds its other
     elements, and says `KILLED`, `SURVIVED`, `ANCHOR NOT FOUND EXACTLY
     ONCE in`, `NOT RUN` or `DID NOT BUILD`; where it holds the first
     alone (`{what}: NOT RUN` names no suite), the next mutant's;
   - after the last, what any slot said there beside its count, once; the
     count summed, `<k> of <n> mutants killed`; and the memory bound's line
     as a `mutation_bound.Watch` says it over the lines said, its stops and
     verdicts in the order a run in order would have said them.
8. **In one slot**, in order, still out of the tree: a script given
   arguments (it chooses its own mutants, and slot 0's baseline would not
   be the run's whole), and those that cannot share the machine with
   themselves (`SEQUENTIAL`): `messages_mutations.py` and
   `notifications_mutations.py` (hosts on fixed ports, not `PORT`) and
   `bench_contract_mutations.py` (the baselines' builds may write caches
   into the shared `node_modules`).
9. **Stopped from outside** (SIGTERM, SIGINT), the run stops each slot's
   script by SIGTERM to its process group (each restores its slot's source),
   waits up to 30 s (then SIGKILL), and kills what each had started in
   sessions of its own (`bounded`'s commands) that is still the same
   process (its start time, read again).
10. **`python3 scripts/mutation_slots.py --remove`** removes the slots;
    `cargo clean` does too, with the tree's target.

## Acceptance

- `scripts/tests/test_mutation_slots.py`, 18 tests (8 s here): a toy tree
  holding a toy script of the scripts' shape (a suite-sorted baseline, one
  verdict a mutant, the count), the three modules beside it, its own git;
  its suite asks git what is tracked and records where it ran:
  - in order and in 3 slots, the same standard output byte for byte, and
    the same code; its control, a comment changed, `SURVIVED` in both;
  - the tree's file never written in slots (bytes and time); every suite
    run in a slot, with that slot's `PORT`, `TMPDIR` and `CARGO_TARGET_DIR`,
    and no `PW_MUTANT_*`; an uncommitted file carried; `node_modules`
    linked;
  - a red baseline in one slot said as the script says it, no verdict of
    another's; a run given arguments in one slot, said as in order;
  - a run stopped by SIGTERM while a slot holds its mutant: the tree as it
    was, in under 20 s, every process its suites started gone, the next run
    whole; a second run waiting for the slots the first holds, then saying
    what the script says;
  - each verdict shape above; a longer description not taken for a shorter
    one's; a slot's lines each the right mutant's, one it chose not to run
    skipped; dealing, narrowing, the copy by content, the bound's line, and
    the Playwright offsets within the 200 ports of a slot.
- `scripts/mutation_slots_mutations.py`, 31 mutants: FILL (own-4 run).
- The real scripts, in order and in 4 slots: FILL (target/proof).

## Not claimed

- **That every script's output is the same byte for byte in slots** is
  held for the toy, and FILL for the scripts run. A line a script prints
  between its baseline and its first mutant's verdict is said as that
  mutant's; a slot's memory-bound stop during its own baseline (other than
  slot 0's) is not said. The 261 scripts print neither.
- **That a slot's tests pass as the tree's do**: each slot's baseline is
  its own evidence; a slot that differs is red and stops the run.
- **CI**: no workflow sets `PW_MUTANT_SLOTS`; whether 2 slots help a
  4-vCPU runner is measured below, and the setting is the integrator's.

## Alternatives

- **Each script calls a shared runner.** 261 edits for one behaviour; the
  import that already reaches every script (ADR-0292's precedent) is one.
- **Detached `git worktree add` per slot.** Registers each slot in the
  tree's repository, where `git worktree list` shows it to every session;
  a slot's own git with the tree's objects as alternates is invisible.
- **A slot's target seeded from the tree's** (`cp -a`): cargo's
  freshness is by time, and a binary built in the tree reads the tree's
  fixtures (`CARGO_MANIFEST_DIR`); hard links share fingerprint files that
  cargo rewrites in place; ext4 has no reflinks. Kept slots are warm from
  their second run on.
- **Dealing mutants dynamically**, a slot taking the next as it finishes.
  The script's loop is its own: a slot learns its mutants once, at import.
- **One `bound` across slots.** A mutant's verdict would depend on what
  the other slots held.

## What was done, and what next

FILL: commits, measurements (in order against 4 slots, cold and warm; 2
slots on 4 CPUs), and the integrator's choices: whether CI sets
`PW_MUTANT_SLOTS=2`, the ports a worker's range must hold (`N x 200`).

## Questions for the integrator

FILL.

## Cancellation amendment, 2026-10-16

Accepted under the owner's delegation of 2026-10-02, before correction.
The integrator's mocked PID-reuse test failed on the preserved working
runner: it sent SIGSTOP to a descendant's reused numeric PID. The later
start-time check before SIGKILL did not protect the earlier stop. That
record used mocked signals only; it did not stop an unrelated process.

The original checkout and its working file remain unchanged. This isolated
checkout starts at 8e6388cb and copies exactly that file, SHA-256
449407ab9b04429a0e140fe0a53383fc0a836c87130ddd898c7b4644118717b0.

Cancellation must use Linux pidfds for every STOP and KILL, including the
slot root. Pin the unreaped direct child before starting its reaper thread.
Pin discovered descendants before signalling, recheck their observed start
identity and current descent from the live stopped root, and retain the
handles until cleanup completes. An exited pinned process is a no-op;
never signal a replacement PID or an entire numeric process group.
The sole startup exception is a direct child whose pin failed before any
reaper thread exists: it is still unreaped, so its PID cannot be reused.
Kill and reap that owned child through Popen, then fail the launch. Probe
kernel pidfd support before launching, not only the Python attributes.
Close every handle on normal completion, refused ownership or exceptions.
Without the pidfd APIs, refuse parallel launch before starting a child;
the unchanged serial path remains available. This is a Linux implementation,
not a portable process-tree ownership claim or a proof for already orphaned
daemons. A failure to establish ownership must be explicit.

The [Python os API](https://docs.python.org/3/library/os.html#os.pidfd_open),
[signal API](https://docs.python.org/3/library/signal.html#signal.pidfd_send_signal)
and Linux [pidfd_open](https://man7.org/linux/man-pages/man2/pidfd_open.2.html)
and [pidfd_send_signal](https://man7.org/linux/man-pages/man2/pidfd_send_signal.2.html)
manuals were read 2026-10-16. Local Python3.10.12 exposes both APIs.

Required proof: mocked reuse before and after pinning, absent identity,
unsupported capability, descriptor cleanup, and private real child/session
stop and kill. Existing toy cancellation and whole runner controls must
pass after their two stale cancellation anchors are repaired. No active
project campaign is cancelled to exercise this change, and no parallel
production campaign is enabled by this amendment.

Independent review found further cleanup obligations before whole validation:
probe signal0 as well as opening a handle; pre-register startup records and
defer repeated/launch-boundary cancellation; attempt every kill/close/reap
even after a failure; refuse persistent missing identities; and wait for
each pidfd's exit readiness before closing it or releasing the slot lock.
Require SIGCHLD's default disposition and reinstall that default to clear
SA_NOCLDWAIT; this command alone reaps its direct slot children. Embedding
with another waitpid(-1) reaper is unsupported. These are corrections to
the stated ownership rule, not additional cancellation authority.

## Result provenance amendment (integrator, 2026-10-16)

Accepted under the owner's delegation of 2026-10-02, before correction.
An actual private missing-module producer ran no test method, but the
42-control draft parser accepted its unittest loader error as a kill.
The retained raw record is /tmp/codex-parallel-import-producer-before.txt.
Independent review also found incomplete positive inventories accepted,
fixture errors miscounted as body failures, and subtest events subtracted
from method counts. This invalidates any general classification guarantee
from the draft text-summary parser; its observed real campaign is still
recorded truthfully rather than cancelled or rewritten.

Use one structured unittest result producer for this runner's own suite.
Freeze each full/mocked profile's exact method IDs before mutations; require
complete start/finish inventory and consistent exit/result. Only assertions
or unexpected exceptions from executed test bodies may detect a control.
Loader/import/setup/teardown/module/class fixture failures, skips, absent
or malformed reports, incomplete inventories and producer stops refuse the
control and earn no kill. Count failed methods separately from subtest
failure events; multiple subtests must not invent method counts.

The mocked profile must guard process creation as well as every signal/close
API. Cancellation-breaking controls must never run private real child tests.
The separate timeout path must use the retained reviewed pidfd implementation
and bounded reap/pipe closure, without numeric group kills or an unbounded
communicate. An already-orphaned descendant not owned by a retained live
root remains an explicit unsupported cleanup boundary; no all-daemon claim
or arbitrary PID signal is authorized by this amendment.

Prove actual private loader/setup/teardown/class/module failures, ordinary
body assertion and exception, multiple failed subtests, skips and partial
inventory, then add independent controls and run the whole actual suite.
The original PIDFD module c87d148c is frozen by this assignment. No source
commit, Cargo build, package installation or external process cancellation
is authorized in this worker phase; the integrator owns later integration.

### Structured instrument implementation and pre-whole evidence

The delegated worker uses a dedicated fresh JSON report file and a separate
fresh output log for every producer. The outer instrument freezes exact full
and mocked method IDs before mutations, retains the reviewed producer bytes
and c87d148c pidfd implementation outside mutation inputs, and validates report
schema, duplicate keys, body entry/completion, start/finish IDs, callback kinds,
body provenance, method partition and process exit independently. Publication
starts only after the runner and all result callbacks return. Partial writes,
missing output, skips, expected failures and fixture failures refuse a control.
The retained outer producer is essential when testing the live producer itself;
a producer mutant must not classify its own test failures.

The first real private regression run was19/23 green with4 assertion failures
(/tmp/parallel-instrument-private-v1.txt). Python3.10 queues subtest/error
callbacks until after tearDown. Capture provenance when executing each wrapped
body, public fixture and subTest context, then match the exact exception or
subtest object at its later callback. Exception occurrences preserve body versus
tearDown even when the same object is raised twice. Cleanup calls are wrapped as
fixtures. No terminal-text or traceback-shape heuristic establishes provenance.
The narrow unittest execution hooks used are the loaded method name, subtest
object and cleanup call hook. Custom execution hooks, asynchronous/generator
bodies or fixtures refuse explicitly; this instrument supports the actual
synchronous TestCase suite rather than claiming every unittest extension.

The corrected private23 methods passed. A later focused run had2 errors caused
by a missing false cancel_requested value on a Mock owner, retained separately
in /tmp/parallel-instrument-focused-v3.txt; correcting that fixture produced81
green. Subsequent portability/guard and failure-path controls brought the strict
pre-whole baseline to102/102 actual methods and the mocked profile to54 exact
IDs (/tmp/parallel-instrument-baseline-v7.txt). All58 mutation anchors are unique
and every replacement compiles. This is baseline evidence, not yet whole proof.

Mocked process cases guard Popen, fork/forkpty, spawn/exec/system/popen variants,
numeric signals, pidfd open/send and close unless a test explicitly mocks that
API. They simulate Linux platform/capability/handler state and create absent
pidfd attributes only inside the fixture. A private absent-API/non-Linux producer
passes the mocked profile while the real Linux children remain skipped. Guard
assertions precede any attempted API call, so the guard-erasure control cannot
launch a real child. Cancellation-breaking controls select only mocked methods.

Timeout execution uses file output, a pre-registered recoverable child record,
pin before the sole bounded wait, and the retained reviewed Run.stop/kill_unreaped
cleanup. No inherited output pipe, numeric process-group signal, unbounded wait
or communicate is used. An exit-probe failure still attempts canonical cleanup;
a root already exited at exceptional cleanup explicitly reports unproved
previously orphaned descendants. This does not claim ownership of arbitrary
orphaned daemons or introduce a subreaper. c87d148c production bytes stay frozen.
Whole validation and exact byte restoration remain required before handoff.

### Review corrections before the revised whole campaign

The first58-control campaign is historical negative evidence:42 original
controls detected actual body failures;16 new controls were equivalent no-ops
because the worker's anchor-normalization generator accidentally reused its
anchor as replacement. The worker checked uniqueness/compilation but omitted the
required existing global mutation_anchors.py gate. This was a verification
mistake. Parent review found it while the owner was already running, so no input
was edited and the owner1840153 ended naturally with exit1. All2597 input byte
hashes restored; memory bound reported no stops. Full raw stays outside the repo
at /tmp/parallel-instrument-whole58.txt, with immutable inputs and expected.json
at /tmp/parallel-instrument-whole58-input. Running the EXISTING full anchor gate
on that immutable copy produced actual exit1:16 of2940 controls equal anchors
(/tmp/parallel-instrument-historical-anchors-red.txt). These are equivalent
controls, not16 proven behavior defects. Historical raw bytes are not rewritten.

Under integrator review, correct those16 substitutions and add a preflight over
actual input bytes: anchor unequal replacement, exactly one match, altered bytes
and SHA different from original, and replacement compiles. Save all actual
before/after hashes in preflight.json before baseline/mutation writes. Add a
synthetic regression for the no-op generation failure; this supplements rather
than replaces the full global anchor gate.

Also preserve primary SystemExit/KeyboardInterrupt after attempting every stop,
close and signal-handler restoration, even when secondary cleanup fails. Log all
secondary failures without changing cancellation into an Exception caught by the
producer-refusal path. A mocked simultaneous stop/close/restore failure witness
and a campaign neighbour prove cancellation identity, source restoration and no
next control launch. A new mocked-only control suppresses that cancellation
branch, bringing the revised campaign to59 controls. c87 remains unchanged.

The Toy real-parallel class and ActualProducers class require real Linux pidfd
APIs; pure Verdicts/Pieces/parser and fully simulated process cases remain
portable. Ordinary own-module unittest discovery under Darwin with pidfd APIs
removed must run105 methods:68 passes and37 explicit Linux skips. This is a
Darwin/absent-API simulation, not native Windows support: the inherited frozen
Unix runner imports fcntl. No production portability workaround is introduced.
An interim focused run had one missing mocked EVIDENCE path error and one
namespace-discovery harness failure (Python3.10 requires different discover
arguments for namespace directories); raw /tmp/parallel-instrument-focused-v8.txt
is retained. Correcting only those fixtures produced87 focused methods green
(/tmp/parallel-instrument-focused-v9.txt). Full existing anchor gate and revised
whole remain pending at this entry; no commit/full-CI claim is made.

Revised pre-whole receipt: /tmp/parallel-instrument-prewhole59-receipt.txt.
Actual ordinary Darwin discovery105 methods=68 passes+37 explicit Linux skips;
focused87 green; strict full105 green with mocked57 IDs frozen. The existing
FULL global gate is green2941 controls/262 scripts, raw
/tmp/parallel-instrument-global-anchors-v9.txt. All59 actual before/altered source
SHA pairs and byte lengths are in /tmp/parallel-instrument-actual59-preflight.json;
each anchor is unique, unequal and compiles. Runner SHA256 is
a4131276729d5fa1b0f6e3e61da848afdc08af9bccd21be9172c39c820ba1f7b;
producer SHA256 is94df09358a0bff06cadadcce8f89a3c12b136c3863e7e6d23a3c834376f1bf26.
The revised whole59 campaign has not yet completed at this entry. Its immutable
inputs and all-input hashes are /tmp/parallel-instrument-whole59-input,
raw /tmp/parallel-instrument-whole59.txt and durable owner state
/tmp/parallel-instrument-whole59-state.json. No source edits until natural end.

### Final output-close cancellation boundary (approved before correction)

Whole59 ended naturally exit0:59/59 actual body-method failure kills, no invalid
producers or survivors, memory0 stops and all2597 byte hashes restored. Preserve
that exact source/evidence at /tmp/parallel-instrument-whole59-input and
/tmp/parallel-instrument-whole59.txt; it proves its59 controls, not the next edge.
Integrator review found the log context manager's __exit__ runs before the outer
except captures SystemExit/KeyboardInterrupt. A mocked-only probe against the
immutable59 input reproduced SystemExit17 replaced by OSError(output close
failure), while canonical stop/root close both ran once. Actual assertion RED is
/tmp/parallel-instrument-logclose-before.txt; no real process was created,
signalled or descriptor closed by that probe.

The integrator approved explicit log=open with log.close errors aggregated only
AFTER capturing primary cancellation. Preserve that primary in a finally even
if the diagnostic sink itself raises. Add an actual mocked close-failure witness
for both SystemExit and KeyboardInterrupt, and a mocked-only control which
replaces primary with the output-close error (whole60). Update the existing
simultaneous-cleanup case to cover a broken diagnostic sink. No c87 change, no
new cleanup/reaper authority. Hold after focused/global gates at a read-only
source review boundary until integrator acknowledgement before launching60.

### Cancellation first arriving during normal releases (approved before code)

Integrator's actual mocked probe on held runner1bc9882c reproduced the first
SIGINT during normal output.close being discarded because canonical c87
stopping=True deliberately defers repeated cleanup signals. Raw is
/tmp/parallel-instrument-cancel-normal-close-before-v2.txt. The expanded worker
matrix against that unchanged runner had25 methods:20 passed,2 assertion
failures and3 expected production exceptions, raw
/tmp/parallel-instrument-cancel-matrix-before.txt. It also proves a second control
could launch after the discarded callback; these are private fully mocked cases.

Install an instrument-local callback which records the first pending cancellation
before delegating unchanged c87 behavior. Recognize cancellation BaseExceptions
from already restored handlers in secondary collectors. After all releases,
existing primary SystemExit/KeyboardInterrupt keeps exact identity; otherwise
pending cancellation takes precedence over ordinary primary/secondary failures,
which remain in diagnostics. Never replace the first pending cancellation on
repeated callbacks. Tests deliver the actual captured signal.signal callback at
normal output close, identity close and both restoration boundaries and prove
source restoration/no next launch. c87 remains frozen. Source/whole review HOLD
remains until parent and peer acknowledge the final candidate.

### Deferred invocation results (approved architecture before producer code)

Independent reviewer actual11-case red receipts are
/tmp/instrument-independent-review-doa5w8zg/receipts.json (prior9 fixture cases:
/tmp/instrument-independent-review-xiy9ib74/receipts.json). Ordinary wrappers
returning generators bypass static function checks, and async generators can
report an unexecuted test as a pass. Producer94df0935 has not yet been corrected
at this entry. Use one synchronous-return checker over actual invoked bodies,
fixtures and cleanup callbacks, preserving stdlib executor/order. Python3.10
_callCleanup discards its callback result: pass a monitored callback to the
original hook. Wrap class/module cleanup callbacks at original list pop rather
than copy the executor. Require owned plain cleanup lists, reject replacement or
custom storage, restore exact descriptors/bindings and remaining consumed state
without replay. Pin setup registration, imported aliases, cleanup-registers-
cleanup and preexisting queues. Static/runtime async generators explicitly
refuse; no synchronous async cleanup claim. Version/platform scope is qualified.
No producer edits or whole launch until this bounded design and regressions are
reviewed; no c87 change or new reaper/semantic authority.

### Frozen synchronous unittest execution boundary (approved before correction)

Peer probes found a load_tests instance _callCleanup shadow and mutation of the
mutable TestCase base hook both bypass cleanup while accepting a body kill:
/tmp/instrument-independent-review-ov8dr9rb/receipts.json. Parent ruling freezes
pre-import canonical execution descriptors from the INSTALLED stdlib, including
instance/class cleanup registration and skip/expected-failure forwarding, and
compares effective bound __func__/__self__ plus class descriptors. Public body
and setup/teardown callables remain observed overrides. Reject custom suite
classes/effective execution; freeze actual TestSuite class/module dispatch hooks
so fixture-omitting suites cannot earn body kills. Freeze module cleanup
registration/executor aliases and recheck after restoring owned wrappers. Genuine
canonical bound aliases remain supported. This is a controlled synchronous
TestCase/stock TestSuite contract over the known Python3.10 call graph, not a
sandbox or arbitrary hostile runtime-monkeypatching proof. Installed optional
hooks are discovered explicitly; no invented version API is required.
Actual private custom-instance/base-hook, omitted-fixture suite, forwarding and
registration probes plus a stock-suite/genuine-alias control were added before
the correction; raw /tmp/parallel-instrument-hooks-before.txt is preserved.
Final source still requires parent and independent review before whole release.

### Installed helper identities and shared validator (approved before code)

Actual receipts /tmp/parallel-instrument-dispatch-probes-v2.json and
/tmp/parallel-instrument-outcome-probe.json show suite-global dispatch, runner
execution and Outcome exception-forwarding changes can omit a required fixture
while retaining an apparently legitimate body failure. Freeze the installed
suite _isnotsuite/_call_if_exists helpers, TextTestRunner __init__/run/_makeResult
and existing aliases, and _Outcome __init__/testPartExecutor/alias before imports
and verify after owned restoration. Refuse captured exception occurrences not
matched by result callbacks, including suppressed instance/subtest/scope throws.
Genuine bound aliases and ordinary stdlib cleanup/state behavior remain supported.
This is the controlled synchronous installed call graph, not hostile Python
sandbox security. No change to reviewed c87 pidfd cleanup.

The integrator approved ONE pure scripts/unittest_protocol.py validator shared
by parallel and guide. It owns schema, duplicate-key JSON rejection and exact
inventory/body/event/exit consistency; explicit validity/reason and separate
passed/assertion-failed/body-error IDs are returned. Discovery, report file
ownership/reading, process launch, timeout and cleanup remain consumer-local.
Parallel keeps python_result tuple compatibility. The producer imports SCHEMA;
its retained copy gets an exact adjacent protocol. Fresh child harnesses load the
live protocol bytes explicitly so sys.modules caching cannot erase a protocol
mutant, while the outer classifier retains its pre-mutation validator. Guide is
owned by E1 and will consume exact frozen bytes after focused validation, with
its original discovered seven-ID inventory checked against explicit selectors.
No discovery CLI or independent guide edits are added here. Applicable controls
must prove an actual validation-authority mutation is detected. Final source
review HOLD remains; no whole campaign until parent/peer acknowledgement.

### Focused extraction and dispatch evidence (review HOLD)

Private72 producer methods had nine actual assertion-failure events against the
pre-correction source, /tmp/parallel-instrument-dispatch-before.txt. All72 then
passed /tmp/parallel-instrument-dispatch-after-v2.txt. The first after log only
had a stale Darwin expected-message fixture, preserved separately. The native
setUp OSError and AssertionError controls both have entered=0 and no valid kill.
Captured instance fixture and subtest exceptions suppressed by forwarding now
refuse even with an otherwise legitimate body failure.

The pure canonical protocol distinguishes passed, assertion-failed and body-error
full IDs; failed is their distinct union. Invalid outcomes contain no positive
IDs/counts, and carry an explicit reason. Existing inventory/body/event/exit rules
were retained. Fresh-local-authority regression creates a private altered parser,
seeds a different cached protocol and proves the actual local validator executes.
No competing parser or process authority was introduced. One initial focused
fixture used tempfile's default-directory probe under a guarded os.close; corrected
to an explicit Unix temporary directory. Raw initial failure is preserved at
/tmp/parallel-instrument-focused-v15.txt, corrected141 green at
/tmp/parallel-instrument-focused-v15b.txt.

FULL existing anchor gate:2963 mutants/262 scripts all match once, raw
/tmp/parallel-instrument-global-anchors-v15.txt. All81 current own controls have
actual unequal before/altered bytes and SHA256 proof in
/tmp/parallel-instrument-actual81-preflight.json. Strict structured full159 and
mocked67 are green, exact inventories /tmp/parallel-instrument-baseline-v16-ids.json,
raw /tmp/parallel-instrument-baseline-v16.txt. Darwin ordinary discovery159 has78
portable passes and81 explicit Linux skips. Canonical c87 source remains exact.
Immutable reviewed files/hashes are /tmp/parallel-instrument-review-v16/expected.json.
No revised whole campaign or source commit occurred; parent/peer final source
review HOLD remains. Historical whole59 proves its own old59 inputs only.


### Deep malformed JSON boundary (approved before correction)

Read-only v16 probe /tmp/parallel-instrument-protocol-deep-probe.txt proves
2000 nested arrays raise RecursionError before schema validation. This cannot
earn a kill: the invocation fails instead of producing positive evidence. The
integrator approved catching RecursionError in the same pure malformed-report
boundary, with an actual portable deep-input regression and own control. No
process, discovery or schema authority changes. Reissue focused/global evidence
and exact hashes; final whole release remains held for independent review.


### Amended held candidate v17

Approved deep-JSON correction is focused141 green, raw
/tmp/parallel-instrument-focused-v17.txt. Full existing anchors2964/262 are green,
raw /tmp/parallel-instrument-global-anchors-v17.txt. All82 actual unequal source
substitutions and SHA pairs are in /tmp/parallel-instrument-actual82-preflight.json.
Strict full159/mock67 green, raw /tmp/parallel-instrument-baseline-v17.txt with
exact frozen IDs baseline-v17-ids.json. Producer bytes are unchanged from v16;
pure protocol now catches RecursionError and returns invalid/no positive evidence.
The portable regression converts any repeated escape into an actual body assertion
failure. Immutable candidate /tmp/parallel-instrument-review-v17/expected.json;
no whole launched and final parent/peer source-review HOLD remains.


### Whole82 and default CI acceptance receipt

The reviewed v17 implementation completed whole82 naturally, owner2187231 exit0:
82/82 body-failure controls,0invalid producers,0survivors,0processes stopped.
Actual unequal byte/SHA preflight ran before baselines/writes; full159/mock67
exact method IDs were frozen. All2598 paths and byte hashes restored, with no
new/missing source inputs. Raw /tmp/parallel-instrument-whole82.txt has SHA256
3c4bf4f1c959ea2d98e055cdc0db4df7c45a14de41b9488f5b99e006b35892cb.
The independent frozen-protocol replay of all83 reports is
/tmp/parallel-instrument-report-audit82.json:77 controls include body assertions,
12 include body errors,7 overlap,5 error-only. These are actual required-body
errors under the approved protocol; fixture/build/stopped producers remain
invalid. Do not describe these as82 assertion kills. The error-only controls are
capability-probe omission, repeated cleanup cancellation, output-close error
replacement, first-pending cancellation replacement and secondary cancellation
stringification.

Authorized default just ci completed naturally0/ci:OK, owner2219141, private target,
jobs4/threads4, Cargo offline and both Kiokun variables absent after setup.
Actual counts:315 Python tests run,314passed,1skipped;336 successful Rust target summaries with2882
passed,0failed,25ignored,0measured,0filtered. The Python skip is the master-only
evidence-presence gate, explicitly deferred on a branch until integrator merge.
Corpus377files already formatted,
corpus-check:OK;83/52/27/25-file example checks each had no diagnostics. Both
Clippy configurations, formatting, case check and global2964/262anchor gate
passed. Latency2/2 passed in15.96s. Full raw /tmp/parallel-instrument-ci.txt SHA256
c99f3e1c7ae1c99048ad0f7b156ad8c84815d01b99a1f8ecc88cc9a9ccc5e842.
Post-CI independent check confirmed the exact2598 pathset, every byte and all8
held-v17 hashes unchanged. Receipt /tmp/parallel-instrument-final-receipt.json.

The complete standard green report is docs/evidence/E14/parallel-mutants.txt;
it truthfully names the actual wrapper/runner invocation and appends original
whole82 output byte-identically. Historical58 no-op failures, older59 coverage,
all actual red witnesses and parent/peer reviews remain byte-exact outside the
repository with26 SHA/provenance entries in
/tmp/parallel-instrument-evidence-manifest.json. Historical58 omitted the REQUIRED
existing preflight; that error is acknowledged, and corrected actual preflight
now runs before baseline and every mutation. There is no whitespace exception.
Only this acceptance metadata and standard report were added after CI; production
and tests retain their reviewed, whole82/fullCI-validated v17 hashes. The final
metadata/format/anchor gates and integrator review precede any local commit.
This ADR remains track-local and unnumbered until integrator merge. D2 remains
frozen, no port allocator changes were included, and the controlled synchronous
installed-unittest scope and Unix/Darwin simulation limits remain explicit above.


### Allocator consumes the canonical browser registry (integrator ruling before code)

2026-10-16: the immutable c87 allocator still spaces slots by200 while the
accepted browser-host registry spaces them by256 and reserves role offsets
through212. The private native loopback probe
/tmp/allocator-registry-native-c87-proof.json reproduced a real collision:
slot0 reviews/chromium and slot1 base/shared both selected55573; the second
exclusive bind failed EADDRINUSE(98). Six offset pairs collide. No project host
was launched or signalled. Historical whole82/full CI above remains evidence of
its old inputs, not of this cross-track composition.

Adopt only the exact immutable registry leaf from browser-host-map native-v6,
SHA256 abcfc8d0df78aeee31456d1e8fb9f4fdd6983f9fc0e44371245a963779185f72.
It is the sole port metadata authority. Python reads its stride and reserved
role offsets lazily when allocating, never during import: retained copies of
this module are also cleanup-only authorities outside a repository. Missing,
malformed, duplicate, overlapping, out-of-stride or unrepresentable TCP metadata
must refuse allocation, with no hardcoded fallback. Validate the highest dealt
slot before any slot tree preparation or child launch, and retain each slot's
normal environment isolation. Private toy trees copy the same registry leaf.
No browser config/spec/runtime change, no pidfd/process cleanup policy change,
no new dependency, and no edit/hash bump to any already-active campaign's c87
copy is included. New allocation tests must prove actual reserved endpoint
uniqueness, registry-driven spacing and boundary refusals; synthetic no-launch
and lazy import cases are narrowly qualified. Record causal RED before fixing
production. Reanchor the existing port-deletion control to the same env_of
behavior and add genuine wrong-stride/preflight controls. Rerun the complete
own mutation campaign and fresh CI on the final locked inputs before commit.


Allocator causal RED is /tmp/parallel-ports-v1/causal-before.txt (native Python
status1,16 assertion failures across6 methods, no errors; SHA256
 dacaa516821987a3548b8b0919734ad59d1017831ec48c4597661656a7b21529).
The source helper remained exact c87 during that run. The first production
correction made all new allocation cases pass; the complete focused164 run had
one remaining failure only in the explicit Darwin simulated-discovery fixture:
its old exact159-method/78-pass inventory omitted the five added portable tests.
Raw /tmp/parallel-ports-v1/focused-after.txt SHA256
 e09c262f1932bd8b095904adafcac61eab7d1472e03a44c830cffed1a1bc100e.
Retain the81 exact Linux skips and update that exact fixture to164 methods and83
portable passes, without weakening any count or skip assertion. This is the
integrator ruling before the fixture edit. No completed campaign input changes.


Final focused164/164/0skip is GREEN, raw focused-final.txt SHA256
788e478941177f32c77245c9c7f3f1017467cb167a8983bf84d228965e0c8344.
The Darwin simulation remains83 portable passes/81 explicit Linux skips, not
164 Darwin behavioral passes. Global2967 anchors/262 scripts and diff-check
are green. Actual private native-after.json confirms the two formerly colliding
roles can hold distinct exclusive loopback sockets simultaneously (58487 and
58543 for base58287). The registry leaf remains byte-exact abcfc8d0; the new
allocator is8facb4097ca6d8bc363804b3de6e88d273ff10b77bd9ff0980da60722b92a0fa.
Only the existing port-environment control was reanchored; three additional
controls cover wrong spacing, TCP overflow and missing early preflight. Existing
pidfd operations, cleanup code, timeouts and cancellation policy are unchanged.
The upcoming whole85 uses an external passive wait observer to retain actual
native statuses and replays every report using the frozen pure protocol; it
changes no producer argument, native exit, timeout or cleanup behavior. Exact
all-source pathset+byte/index receipts gate fresh CI on the same locked snapshot.
No current campaign or any other worktree has adopted this helper hash.


Allocator whole85 naturally GREEN, 800.197s, all2600 frozen pathset/bytes
and index restored; raw docs/evidence/E14/parallel-ports-whole.txt SHA256
59b83e57dcd15363e7b03c001d4b20c23d156fbb7d06a9a40e3d702fb30bb434. Retained88 actual native commands comprise two frozen
inventories164/67, baseline164 and85 controls. Independent canonical protocol
replay validates every report and exact method roster.80 controls include
assertions,12 include body errors,7 overlap,5 error-only; do not call85 assertion
kills. The3 new allocation controls include genuine original assertions. No
invalid/build/stopped producer earned credit. Process/timeout policy unchanged.

The completed whole's external postcheck initially rejected its own broad regex
matching stopped inside the affirmative no-process-stopped sentence. Source and
native results remained unchanged. The retained observer ruling precedes its
exact-line correction. CI-only resume replayed all88 native records and the
same2600 inputs/index, then fresh default just ci naturally passed0 in
2052.135s, rawSHA256 b48e472b6d48c27046df091d8cfd43e8b04c598cff7734609c2f687fd14617af.
BothKiokun deployment variables were absent, Cargo offline/jobs2/threads2,
private target/PORT17201. The CI census and fullraw are committed evidence.

Only this acceptance metadata and evidence publication follow the locked gates.
Every functional source/test/registry byte retains its validated manifest hash.
Prior whole82/fullCI remain historical; active consumer copies of c87 were not
changed. This track remains unnumbered until integrator merge; final assembled
validation and remote gates remain required before any main landing.


The new same-source CI census is336 successful Rust target summaries with
2882passed/0failed/25ignored, plus320 Python tests run:319passed and one explicit
master-evidence-presence skip. The skip is the existing branch deferral, not
an allocator behavioral pass. Exact native summary lines are retained in
parallel-ports-ci-census.json and the byte-identical full CI output. Post-
publication cargo fmt --all -- --check and the unchanged global2967/262 anchors
passed; all functional inputs retained their frozen hashes after publication.

### Merged into train-11 (2026-10-19, the integrator's delegate)

Merged as track/parallel-instrument f6e8a14f, on train-10's head with the
render merge, two fixes and the browser host map, which the integrator had
merged first: both branches add `spikes/own-renderer/host-registry.json`,
the same bytes, which `mutation_slots.py` reads. Three resolutions:

- The root justfile imports `just/parallel-mutants.just` after the train's
  other imports. `scripts/mutation_baseline.py` merged by itself: train-10's
  bytecode rules (e8dc6d05) run first, then `mutation_slots.engage()`, then
  the memory bound.
- `host-registry.json`: one file, added by both.
- **Six harness transcripts are kept out of the train's committed
  evidence**: the train's gate `scripts/evidence_survivors.py` ("no committed
  evidence records a mutant that survived", train-6's landing) refuses any
  evidence line ending `: SURVIVED`, and these unittest transcripts of the
  slots' own tests print the toy controls' intended survivals
  (`a comment is changed [lint]: SURVIVED`, `first: SURVIVED`,
  `next: SURVIVED`). They stay in history at f6e8a14f, as the generic track
  kept its survivor record out of its evidence (6d049961, ADR-0351); their
  `report.json` and `expected.json` stay. By SHA-256:
  - `parallel-ports-native/0013/output.txt` c63f8047489e518be02d718d09c6138215fa33825718c40c50fb79ea27f95c70
  - `parallel-ports-native/0016/output.txt` bb42a305073cb6d7e0182136d8a227e5d0cc3f14348af5242841e03fba71c1aa
  - `parallel-ports-native/0022/output.txt` 65c3000a81c0fdfa55a1f2e94b5eaa5b5dfb639ba81aaa09c7cde7497877d044
  - `parallel-ports-native/0061/output.txt` 12cfefd5e00454f5c7872fb19542a6638c6ed8a5fd9b3c5796a18387c14dae60
  - `parallel-ports-native/0063/output.txt` 2bf4111c9081eddf10e70d24c4b6973a368c5138f24ed97e4fb5655764c6915b
  - `parallel-ports-native/0066/output.txt` 98a13b85acef5440202a6489009e20f000eac23423ea8f5b42c86a704d5d0b74

Not claimed by the merge: that a train's verification runs mutants in slots
(`PW_MUTANT_SLOTS` is set by no workflow and no recipe, and the train's own
whole runs ran each script in order).

## Numbered on train-11 (2026-10-20)

- **Numbered ADR-0357** (the integrator's ruling of 2026-10-20: a train is
  numbered before it is pushed, after master's ADR-0353, in train order),
  merged from `f6e8a14f` (`track/parallel-instrument`). `ADR-XXXX` reads
  `ADR-0357` in `just/parallel-mutants.just` (its recipe's header line too),
  `mutation_slots.py`, `mutation_slots_mutations.py`, and the header of its
  two evidence files, as in this file's name and title.
