# ADR-0352: statement counts wait for the completed payment transaction

Status: accepted by the integrator; local source gates passed, and its
exact head's remote gates too: verify 38909882781 and ci 38909883157 at
`e2a4acd9`, green; landed with train-9 (master `32757966`, 2026-10-17).
Date: 2026-10-16.
Track `train9-bounded-fix`, exact base `289068a65badb431c14548c3f9fa0db86c3fa19f`.
Amends ADR-0336's test synchronization and ADR-0334's scheduled-state write
ordering. Preserves exact count comparisons; the successful sweep settlement
pin is corrected from12 to14 by the measured causal evidence below. Builds
on ADR-0332's real provider/webhook boundary.

## Measured context

GitHub verify38863242953 at that exact head failed its summary:84 recipes,
one failed (`e14-bounded-statements`,113.3s);83 recipes and all three full
browser jobs passed. The PostgreSQL shard job itself intentionally exits0
after recording every recipe; `ci_summary.py` correctly makes the verdict.
The recipe's first real statement suite passed4/4. The mutation harness's
unmutated rerun passed3/4 and refused to plant any of its7 mutants; no
process was stopped. `placing_and_settling_where_payments_are_taken_is_bounded`
failed its exact seed==more assertion at line742. Numeric left/right were
discarded by the current first-line panic extractor. They are not recovered
or guessed here. The failed run remains failed historical evidence.

The complete downloaded summary/job/artifact bytes are retained outside the
repository at `/tmp/train9-38863242953-review/manifest.json`, manifestSHA256
`7830f3723e83ef05d0fa441ed28b86f0ee28831ede830686522d0f1ea0a93975`.

`quiet()` calls two equal tally snapshots300ms apart complete. This does
not establish either that a delayed provider event has arrived or that a
started transaction finished: `Tally::begin` first appends0, then each actual
operation and the commit add statements. A suspended webhook can leave a
stable incomplete snapshot for arbitrary scheduler time.

Actual private reproduction on installed PostgreSQL18.6, with production
code unchanged: hold the development provider's real signed cancel event;
run the compiled cancel_order; call unchanged quiet(); release that event
through the real HTTP webhook and compiled settle_payment. The snapshot
before release is[25], and after completion[25,14]. The exact25/14 count
pins pass; equality then fails0/1 specifically because quiet returned before
the real settlement. Raw command/output: [red probe](../evidence/E14/train9-bounded-completion-before.txt).
This proves the helper's completion defect; it does not claim which numeric
partial snapshot appeared in the earlier CI run, whose details were lost.
The initial experimental fixture used two cart lines against the one-line
pin and failed at that pin instead; that superseded raw result stays in task
work/train9-bounded-held-before.log and is not counted as the intended red.

## Initial test-only proposal (superseded by the production ruling below)

1. Replace test-only quiet inference with `after_payments(s, completed)`.
   Wait, under a finite failure deadline, for the existing Payments.settled
   record of exactly the expected number of completed outcomes. `Payments::heard`
   publishes that marker only AFTER settle() and the real command answer
   return. Drop the marker lock before copying the tally. An event that never
   completes fails with its recorded progress and counts; a timeout is never
   successful completion.
2. Count immutable Vec snapshots only at that boundary. The existing paying
   scenario expects1 complete outcome before cancel,2 after cancel/before the
   sweep, and3 after the sweep. Preserve seed==more and every original exact
   AWAITING/PAGES/PINNED value, including25/14 for the one-line cancel.
3. The regression holds the real signed event while a scoped counting thread
   waits. It must not return during the hold, then must return[25,14] after
   release. Release/drain precedes assertion failure so no held HTTP child or
   test thread is abandoned. This is a correctness condition on completion,
   not an additional timing allowance for the workload.
4. Add a control restoring the original equal-snapshots implementation and
   require an actual assertion kill. Run the complete affected mutation
   script, all bounded statement tests on real PostgreSQL, relevant payment/
   scheduled neighbours, fmt/anchors and final CI before any source commit.
   Integrator owns push/landing and requires exact-head remote CI+verify green.

## Initial test-only scope (historical)

No production payment, SQL, scheduling, transaction counting or provider
protocol change. No relaxed count/ceiling, skipped test, or rerun-only remedy.
No new dependency or install. All data are private synthetic repository
fixtures in task-owned schemas. Every launch sources setup then unsets the
unrelated deployment dictionary variables; no external dataset access.

Primary stdlib documentation checked2026-10-16:
[scoped threads](https://doc.rust-lang.org/stable/std/thread/fn.scope.html),
[bounded channel receive](https://doc.rust-lang.org/stable/std/sync/mpsc/struct.Receiver.html#method.recv_timeout).
No new upstream API or production guarantee is inferred from a sleep.

## Initial pending gates (historical)

Integrator architecture ruling, actual correction, delayed-regression green,
whole controls and final gates. No source commit or merge-ready claim.

## Integrator ruling, 2026-10-16

The actual[25] versus[25,14] at unchanged pins establishes the completion
defect. Approved before correction: narrow test-only repair uses existing
post-settle Payments.settled authority, bounded wait and exact completed-event
counts, then an immutable tally snapshot. No300ms stability inference and no
count changes. Keep the real delayed-provider regression and full[25,14];
run focused real PostgreSQL/memory tests, all existing7 controls plus the
new restoring-old-completion control, fmt/anchors and fresh full CI before
source/hash/result review. No source commit/push authorized.

## Further measured refusal: first repair is insufficient, 2026-10-16

The first corrected focused PostgreSQL suite is RED4/5. The delayed-provider
regression is green with no early return and[25,14]. The original sweep
scenario records2 successful completions, not3, although its transaction
tally includes26+12. The wait correctly refuses to invent a third success.
[Full failed focused output](../evidence/E14/train9-bounded-marker-focused-failed.txt).

Temporary cfg(test) exact outcome logging, restored byte-for-byte in finally,
shows the sweep's actual command Answered{committed:false,result:None,
why:"PostgreSQL40001: could not serialize access due to concurrent delete"};
the actual webhook respondsHTTP500. Payments.settled correctly records only
successful handling, so it stays2. [Actual refused response](../evidence/E14/train9-bounded-refused-settlement.txt).
This is not a reason to increase the timeout or lower the expected successes.

Two actual controlled operation orders now establish the causal boundary:

- Hold the real signed webhook until run_due and its cleanup finish, then
  release: both seed and+50 giveHTTP200, committed:true, and sweep settlement14.
  All other counts remain37/17/25/14/26. Exact seed==more passes; the old pin12
  fails. [Ordered successful path](../evidence/E14/train9-bounded-ordered-settlement.txt).
- Pause the real settlement at existing cfg(test) Server.before_commit after
  its serializable snapshot, run actual Runs::ran(id), then resume: cleanup
  returns across the active payment transaction; the payment is refused40001,
  HTTP500, no success marker, transactions[26,12]. The regression is RED0/1
  at the cleanup ordering assertion. It releases/drains every operation before
  assertions. Its clock command and actual cleanup are the same calls and
  ordering run_scheduled uses, separated only to control this boundary.
  [Forced overlap](../evidence/E14/train9-bounded-overlap-settlement.txt).

The original pin12 describes this refused attempt, not the intended successful
settlement. The remote run's lost numeric snapshot remains unknown; the new
private reproduction establishes both branches without rewriting its history.

## Production proposal before the integrator ruling (historical)

StorePg command staging owns writes from before BEGIN through commit/drop.
Its scheduled cleanup Runs::ran takes a pool connection and deletes the row
without that guard, racing settlement's serializable unschedule of the same
row. FeedPg.ran has the same bypass. Each layer's Runs::retry also writes the
same scheduling table outside its guard. Read-only due_runs/next_run are not
mutations and do not need the write guard.

Route ran/retry scheduled-state mutations through each layer's existing
write guard, acquired BEFORE obtaining the operation's connection. This
orders same-instance cleanup either before a command's snapshot or after
its complete transaction; no new lock, database-wide serialization, automatic
payment retry, or cross-host guarantee. All real call sites perform these
mutations after the preceding command's staging dropped, so the existing
non-reentrant guard is not nested by that sequence. Add actual transaction/
barrier regressions for each covered layer/transition and a guard-removal
control; retain different-host serializable refusals as possible.

Only after successful semantic path is measured after correction, update the
single successful sweep settlement pin12→14, with exact equality and all
other pins unchanged. Require actual successful completion and canceled
poststate, not merely a stable tally or refusedHTTP response. A test authority
for refused cases must count finished HTTP responses separately from success.
Do not make Payments.settled include failures just to satisfy a test.

PostgreSQL18 primary documentation checked2026-10-16:
[Serializable isolation](https://www.postgresql.org/docs/18/transaction-iso.html#XACT-SERIALIZABLE),
[locking](https://www.postgresql.org/docs/18/explicit-locking.html),
[lock observations](https://www.postgresql.org/docs/18/view-pg-locks.html).
Serializable conflicts are legitimate refusals; the existing host contract
also intends not to conflict with its own protected writes.

The first marker-only correction remains an unvalidated intermediate. No
production change, count correction, source commit, push, or green claim.

## Integrator production ruling, 2026-10-16

After reviewing both actual outcomes, approve the consistent four-method
correction: StorePg and FeedPg ran/retry acquire their existing writes guard
BEFORE pool acquisition, matching begin's lock order. Call sites return from
command_answer before scheduled removal/retry; command_held explicitly drops
Staging before returning, including unwinding on failures. No self-reentrant
acquisition is introduced. Scope is one layer instance, not distributed
serialization or guaranteed absence of legitimate retries.

Keep forced real overlap now requiring cleanup to wait and settlement to
complete successfully with correct poststate. Cover all four guards with real
PostgreSQL staged-commit blocking tests and independent removal controls;
whole roster includes them and the original7. The success pin12→14 is approved
only as the correction of aborted-path evidence established above. Success
scenarios may explicitly wait on the success-only post-settle marker; refusal
controls must finish/drain actual HTTP handling rather than invent a success.
No source commit/push before full gates and source/hash/result review.


## Final implemented decision and validation boundary

StorePg and FeedPg `Runs::ran`/`retry` now acquire the existing instance's
write guard before a pool connection. The guard covers each whole scheduler
write. Command staging retains the same lock through transaction completion;
the real callers drop staging before scheduler cleanup/retry. No new lock or
dependency is introduced, and cross-instance serializable refusals remain
possible.

Required-success statement scenarios wait for the exact post-settlement
success-marker count, then return an immutable copied tally. A held real
signed webhook cannot complete that wait. Forced real PostgreSQL overlap now
keeps cleanup behind the payment transaction and requires a successful
settlement, the canceled order and no pending sweep. Each of the four guard
boundaries also has a direct real PostgreSQL staged-commit regression that
preserves a moved run's new identity, due time and attempt state.

Focused PostgreSQL tests passed10/10, including the actual delayed25/14 and
overlap26/14 transactions, and both fixture sizes retain exact successful
pins37/17/25/14/26/14. The memory-config native summary also says10/0, but five
tests explicitly return when the store layer is not PostgreSQL; those five
are skips, not memory behavior evidence. The other five execute against the
configured real PostgreSQL database. See [focused raw output](../evidence/E14/train9-bounded-focused.txt).

Historical whole campaigns reported test-failure kills: bounded-statements12,
reviews30, uploads33 and scheduled37 (112 total). Their natural aggregate exits,
reported counts, raw hashes and limited2222-path restoration receipts remain
in the historical validation record. Their per-command native statuses and full
raw outputs were not retained; they are not strict native-producer proof. The
old bounded runner also accepted invalid/build results, as established below.
The current strict12 campaign and fresh CI replace that runner's readiness
claim; the three unchanged neighbor campaigns remain qualified history.
See [historical receipt](../evidence/E14/train9-bounded-historical-validation.json)
and [current strict receipt](../evidence/E14/train9-bounded-strict-validation.json).

The earlier failed remote/intermediate evidence remains failed historical
evidence. The original train-9 worktree is untouched. No source commit or
push is authorized before parent review of the final source/hash/results.


## Pre-commit producer finding and integrator correction ruling

The first fresh full CI completed naturally exit0, rawSHA
72f836b5fe6de3ef898ab9aa42bca2eda961df1bb93212e718458602ad6d0220;
all2222 frozen inputs restored. Before a source commit, parent review found
the inherited bounded-statements runner maps timeout(None) to failed1 and
ignores native exit when a summary exists. Synthetic exact status/text probes
demonstrated acceptance of None/-9/137/1 and compiler-error101. These are
mocked interfaces, not real process-stop or compilation experiments. Six
native Python unit methods report14 failed subcases at actual Python exit1
before the correction; raw /tmp/train9-bounded-producer-unit-before-exact.txt.

The112 earlier whole outputs retain their reported test-failure summaries,
aggregate statuses and source restoration. Their per-command statuses/full
raw were not exported from the child memory deque and are not recoverable.
Own12 elapsed314.56s rules out its1200s per-command timeout in that attempt,
and the memory wrapper reported no stop. Those observations do not establish
strict native-exit classification. No invented exit status or panic text is
attached to historical evidence. It is not current strict-producer proof.

Ruling before code change: adopt the exact shared Cargo pure APIa177209c and
its8-test authority, require matching native0/101 plus nonempty consistent
test evidence, and refuse a build/unknown/stopped/contradictory outcome with
no kill and no next mutation. Retain the existing process/timeout/cleanup
mechanism, observe its memory-stop flag, and preserve source finally. Existing
12 controls and all production/test bytes stay unchanged. Add narrow synthetic
consumer/restoration tests, rerun whole12 with external native status/fullraw
retention, then fresh default fullCI. Other three unchanged whole scripts and
their112 historical aggregate findings remain separately qualified; the new
API is consumed only by bounded-statements on this branch. The earlier green
CI is historical and cannot certify the amended harness. No source commit or
push until these new gates naturally pass and exact source restoration holds.

The fresh freeze additionally caught an old inventory-scope omission before any
new child launched: the old newline Git listing omitted55 quoted Unicode
repository fixture paths. Its2222 receipt is retained as its actual scope,
not an all-path claim. New NUL-delimited inventory pins all2280 current
inputs, including those55 accepted sample fixtures (each independently equal
to base289068a6) and the3 new pure-API/consumer-test source files. No external
dataset was read. No new whole/CI result is claimed by this correction.


## Current strict producer acceptance

The corrected bounded runner completed all12 unchanged controls naturally,
with an actual0 baseline and12 actual101 commands, each containing named native
test failures. All13 full raw outputs, native statuses, frozen control identity
and hashes are retained in
[the command receipt](../evidence/E14/train9-bounded-strict-commands/receipt.json).
No build/unknown/stopped/contradictory result earned a kill or launched another
control. Whole raw SHA256 is43273b78fd1a8eeb68fd18f718be8be90045dbf357fc1afdd9de8cd7812a9892.

Fresh default `just ci` then completed naturally0 on the SAME locked source
snapshot, with `ci: OK`, 338 successful Rust target summaries,
2954 reported Rust passes and25
explicit ignored tests. This is separate from the older green CI whose raw
bytes are preserved as train9-bounded-pre-strict-ci.txt. The current CI raw
SHA256 isfd4b0388d70272ab485648694b064df32cc3423f6475217f31318ca917f2c3ff.

All2280 NUL-inventoried source/config input paths and bytes match before and
after both gates; this includes55 Unicode repository sample fixtures omitted
by the old quoted-line listing. Evidence files, the generated store IR and
Git-ignored build/dependency files are explicit scope exclusions. The current
receipt retains the exact input manifest, preflight, actual counts and hashes.
Only this acceptance metadata and exact raw evidence copies were added after
the final CI; production, tests, result API and control tuples are unchanged.
Final formatting/anchor/diff review and integrator commit/remote gates remain
required. No failed historical run is relabelled green.


### Exact evidence bytes in Git

Final staging caught a metadata-only preservation issue: Git's inherited text
normalization changed four historical CRLF raw logs, while the first native
baseline output ends in a blank line and failed diff-check at that exact EOF.
The original working bytes and before-normalized blob hashes were independently
compared in git-evidence-normalization-before.json outside the repository.
Ruling before the attribute edit: mark ONLY those four explicit raw paths
`-text`, and disable ONLY blank-at-eof for strict-commands/0000.txt. Do not
rewrite, trim or normalize any raw output, and do not relax whitespace checks
on source or other files. Installed Git's gitattributes manual documents these
per-path attributes. Restage and prove every evidence blob equals its recorded
working bytes, then require clean final diff-check and anchors. These five
path-scoped evidence attributes are the only post-CI config adjustment; no
production, test, producer, mutation tuple or result parser byte changes.

Preserving those CRLF blobs exposed Git treating the retained CR at each line
end as trailing whitespace. Before the follow-up edit, allow `cr-at-eol` ONLY
on the same four exact raw paths while retaining all ordinary whitespace
checks. This is line-ending classification, not trimming or permission for
trailing spaces in source. The native baseline's separate blank-at-eof
exception remains confined to0000.txt.


## Verified and landed (2026-10-17)

The exact head `e2a4acd9` passed GitHub's verify 38909882781 (45 jobs, 85
recipes on 40 shards and the three engines, 1,391 of 1,391 mutants killed;
`e14-bounded-statements` among them, its 12 controls killed) and its ci
38909883157. Train-9 landed on it: master `32757966`, its evidence fetched
at `e2a4acd9`. The pending gates named above are met; the earlier failed
runs stay failed. Train-10, merging master, carries the completion barrier
into its scheduled orders' four transactions (`after_payments`, 3 and 4
outcomes) with every pin as measured.

## A flake of its guard controls, fixed on train-10 (2026-10-17)

`scheduled_transition_waits` gave an unguarded scheduler write 100 ms to
return early, the only test that kills two of the guard controls: on
train-10, "feed_pg.rs ran bypasses the command write guard" survived one
whole run of two on the PC, and "store_pg.rs retry bypasses ..." survived
the train's verify 39054436657 (12 of 13). A slow runner's unguarded write
took longer than the window. Guarded, the write cannot end before the
command does, however long the test waits: the window is two seconds now,
the test no weaker, each of its four cases two seconds longer.
