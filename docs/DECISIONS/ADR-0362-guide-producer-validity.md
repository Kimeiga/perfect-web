# ADR-0362: guide mutation kills require valid producer outcomes

Status: locally complete and validated; integrator owns landing/numbering (2026-10-16).
Parent: the reviewed evidence-integrity/check-is-build work. No diagnostic code,
package, feature, ceiling, accepted source or browser-selection allocation.

The immutable check/build checkpoint remains `track/check-is-build` at
`e82b0d6cd17270da78e3591d9bee0e5df76439f9`, with tested source `d74e29ff`.
This separate branch reuses only its owner's isolated worktree and target.
It does not amend that checkpoint or merge another track.

## Problem and evidence

The current guide harness discards producer exit status. A failed summary and
intended witness name can earn a kill after Cargo or Playwright exit -9,137,
or even contradictory exit0. Its named CHECKED branch also loses APP_BUILD
status, accepting partial exact diagnostic codes after -9/137. The integrator's
read-only probe is `/tmp/codex-guide-producer-exit-before.txt`, from immutable
D1 source SHA256 `57aa44cdd5f3ca8f774b73ce224be5454988cb331da71c173802c724943ccca5`.
It launches no child and edits no project file. Current owned source SHA256 is
`da1e1b61cfd50a6ed4b711245b84014abdaf4b65af7862d9d158609458e480f3`; the follow-up must retain its own causal RED/current GREEN
proof instead of treating another source's probe as whole-run evidence.

The prior complete guide74 result stays historical evidence for its existing
controls. It does not establish strict producer validity. This is a separate
assembled-train blocker; the completed check/build guarantees remain intact.

## Proposed authority and outcomes

Preserve all existing guide/browser command selections, retries, MUST witnesses,
CHECKED names and exact diagnostic sets. Validate each producer before summing:
Cargo allows exit0 with positive passes/no failures or exit101 with actual
failures; Python unittest allows exit0 with positive passes/no failures or
exit1 with actual failures. Missing/empty summaries, inconsistent statuses,
other exits and memory/time stops are invalid, not assertion failures. Retain
all producers' combined output so an earlier Cargo refusal is not overwritten
by the later Python producer.

For the real Playwright command, reuse the exact already adopted canonical
`scripts/run_suite.sh`; add no approximate browser parser or AST authority.
The guide may read counts/titles only to name witnesses after that command's
status/no-tests authority: exit0 requires positive passes/no failures, exit1
requires actual failed tests. Other statuses and contradictory counts are invalid.
The canonical helper's existing regression suite remains unchanged.

Keep the tuple-facing APIs narrow. An explicit typed compiler-refusal outcome
may be created only after the actual guestbook APP_BUILD exit1 with a nonempty
full set of real error diagnostic codes. The main's existing exactly named
CHECKED policy must match that complete set; arbitrary build failures, stops,
extra codes or a partial diagnostic after another exit cannot earn a kill.

Main must fail the campaign and earn no kill for invalid producer states,
including a bound watch reporting a stop even if partial output looks valid.
Keep valid intended assertion failures as kills, valid green mutants as survivors,
and existing wrong-witness handling. Gate final restored APP_BUILD completion.
Use separate killed/refused accounting where needed to prevent invalid results
from being subtracted from a survivor count and implicitly credited.

## Causal verification and scope

Add deterministic regressions against actual imported producer functions and
main with one private temporary source fixture, mocked producer outputs and no
compiler/browser children. Cover each invalid exit/count boundary and valid
assertion/green neighbours, exact named-refusal provenance and code sets,
canonical wrapper wiring, stop receipts, final-build failure and finally byte
restoration. Establish RED before correction and GREEN afterward.

Proposed independent campaign: a small dedicated guide-producer mutation script
with eight controls (Cargo status, Python status, canonical browser wiring,
browser status/count consistency, named-refusal exit, invalid main kill credit,
stop handling and final restored build gate), held by those regressions. This
keeps the existing guide74 control inventory unchanged. Run its WHOLE campaign,
the corrected WHOLE guide74 including real unchanged browser assertions, and
fresh full CI before source/harness commits. Keep all raw attempts, source-byte
copies, probe hash and natural completion receipts. No cancellation optimization.

Add a focused evidence recipe for the unit and independent producer controls;
record the separate real guide74 result with exact source identity and dirty
state if produced before a source commit. Do not repeat long campaigns merely
to hide a source-header mismatch. Preserve original probe bytes, isolated target,
both deployment variables unset after setup, and existing negative child tests.

The other 23 cleanup gaps remain parent-owned and are not repaired or credited
by this phase. No general cancellation correctness, universal suite propagation,
train landing, push, merge, authoritative status edit or ADR numbering is claimed.

## Integrator ruling and results

The integrator approved this separate phase and the canonical shared producer authorities before implementation. Results are recorded below; final gates remain required.

Own-source pre-correction unit-level probe: 28 unittest methods ran, with 46
failed subtests across 17 methods and no errors. Exit1 is the expected RED
observation, not a failed whole campaign. Actual imported producers/main were
used with only private temporary source fixtures and mocked command outputs;
the real main finally restored each fixture's exact bytes. The draft tests
remain outside the repository while review is pending. Raw
/tmp/guide-producer-unit-before.txt SHA256
6535b4d80a979aa00882fda186e452601027d8c934a24089ef939a17170488ae is also copied to Windows work.

## Reviewed scope and canonical Python dependency

The integrator approved Cargo/browser/checker-status hardening, with unchanged
inventory, selectors, MUST/CHECKED and final restoration gates. Implementation
may proceed for those paths on this separate branch. The earlier proposal to
validate Python using exit/count text alone is superseded: it does not prove
fixture or test-body provenance, even for native unittest exit1.

The parent supplied `/tmp/guide-python-fixture-before.txt`, generated by its
`work/guide-python-fixture-probe.py`: a real native unittest child exits1 after
setUp raises OSError; its required test body marker is absent, but the current
guide main credits the required method name as a kill. A setUp AssertionError
can print FAIL instead of ERROR, so filtering text or errors is insufficient.
This native probe is unit-level instrument evidence, not a browser or whole
mutation campaign. Its raw output is preserved separately.

Python must consume the single canonical structured producer/protocol being
coordinated with tables_d1. That worker owns the pure shared JSON/result-validator
extraction; process, cleanup, timeout and bound ownership remain separate.
The guide will add a narrow Python adapter to that protocol and actual native
fixture/deferred regressions. It will not introduce a second text heuristic or
claim all-Python validity before the dependency is integrated and verified.
Cargo/browser/checker work may progress while that reviewed dependency is prepared.

All source/harness commits still require the corrected whole guide74, own whole
producer controls, and applicable fresh CI. No source commit or completed guide
validity claim may precede those gates. Original probe and immutable check/build
branch remain protected. The other 23 scripts stay parent-owned.
Native fixture probe SHA256: 3b6cbe318561ca748b5f2b609a279b7a55485527bb80510a8188e90075eda56e.

## Frozen shared protocol adoption

The reviewed canonical implementation is copied without modification from the
instrument worker's immutable `parallel-instrument-review-v17/scripts` snapshot:
`unittest_producer.py` SHA256
`902dfef3cf62b16eaa264f0cba52a1bc30bc25d9c7e6bf0718ae6a2beea8b662`,
`unittest_protocol.py` SHA256
`91e941dc2157d53292aa5a709c4c3f0d10210f61a2db23d9edcaeb1df691ee55`.
The worker reports focused141/strict159/mock67 green, with its applicable whole
and peer review still pending. Those are upstream results, not local whole passes.
The v17 protocol adds only the approved malformed/deep-JSON RecursionError catch
to v16; the producer is unchanged. This consumer does not edit either owned file.

The pure authority validates complete JSON against a frozen sorted list of full
IDs and exposes passed, assertion_failed, body_errors and executed. Invalid
receipts expose no positive IDs/count. Inventory output is schema1 plus exact
IDs. The guide owns original native discovery comparison, process launch, fresh
report/log paths, timeout/cleanup and evidence retention. It maps bare MUST names
only after this shared validation and explicitly labels credited body assertions
versus body errors, as separately approved by the integrator.

Local launches use a fresh owned bytecode-cache prefix as well as -B, preventing
reuse of a previous mutant's import cache without deleting any cache. Native
selection identity is `test_guide_diagnostics`, with controlled own PYTHONPATH;
its full seven IDs were recorded before mutations in
/tmp/guide-producer-native-seven-before.json, without running test bodies.

## Shared Cargo result authority

The integrator supplies the single pure Cargo/libtest authority at
`/tmp/codex-cargo-result-api-v1.py`, adopted byte-identically as
`scripts/cargo_test_results.py` SHA256
`acb698a7921b762ea47b976780f9bc89c41bcf96e43a6e44e12ed7cee986bd33`.
Its parse(status, output) validates all binary summaries, exact 0/101 status,
nonempty consistent results, no build/stop diagnostics and retained failure
names. Guide, bounded and generic consumers share that implementation; the
guide retains command execution and witness policy. This replaces the
temporary local Cargo regex/status parser before any whole or commit claim.
Parent focused results are upstream evidence, not this branch's whole pass.

## Local causal results and first complete instrument campaign

The original 28 unit methods ran RED with 46 failed subtests across 17
methods, no errors. After implementation, 40 focused methods are GREEN.
Eight new native methods use actual canonical Python children and real guide
main/private source mutation: setUp/tearDown OSError and AssertionError are
refused, setup never enters its body; body assertion and body OSError retain
the approved distinct witness categories; skip, expected failure, async,
generator and returned coroutine are refused. The adapter's actual timeout
reaps its owned child and exposes no evidence. A same-timestamp/same-size
stale-bytecode regression executes the new source and leaves the old cache
bytes untouched. Exact native seven IDs and seven passing bodies are retained.
The whole focused suite also runs under the canonical structured producer.

The dedicated WHOLE eight controls completed naturally: 8/8 intended body
assertion kills, 0 refused, 0 survivors, no memory stop. Their frozen baseline
ran all 40 exact full IDs. Raw /tmp/guide-producer-whole8-v1.txt SHA256
d72587b6d9a93cada80e0cc45fea51e1f9ff83c3e68f96b1a38af48f1703e935
Wall 1:21.45, user61.20s, sys4.36s,80%CPU. All 2,723 frozen input bytes and
the original untracked probe matched after final restoration; global anchors
3090/269 matched before and after. This is instrument evidence, not a completed
whole guide74/CI claim. The unchanged guide inventory/MUST/CHECKED/commands were
compared by AST before implementation; no selector or retry changes.

## Final local validation and integration boundary

Tested source checkpoint: `0ff69eaaf309d56d59030bbfdb35479fcabbc6b4`. No source/harness commit preceded
the complete own8, corrected guide74 and fresh full CI. Those whole74/CI runs
used saved uncommitted v2 bytes at base e82, byte-equal to the source checkpoint;
their base HEAD line is not presented as a clean-source claim. The all-input
manifest SHA is 53f43939c583358a50427814ee110dc44df52b11455aa499dc33c00e1c2c1810.
All 2,723 input bytes, original probe, immutable check/build ref and zero own-range
listeners verified after each natural boundary. No cancellation was attempted.

Corrected WHOLE guide74: 74/74 killed,0refused/0survived/0memory stops. Exclusive
outcomes56 Rust only +7 Python assertion only +3 both +5 browser spec +3 named
checker =74. The named exact sets remain PW5048/PW5048/PW5107. There were no
Python body-error kills in actual74; the separate native body-error neighbour
proves that approved category without relabeling it as an assertion. Wall19:43.56,
user583.21s,sys42.14s,52%CPU; host load3.79/2.75/1.94 before and3.97/5.45/5.03 after.
Raw SHA52e4d664e7aa48297ae207e67c21059047f6c633a768faae9eff5989311a2e68.

Fresh full `just ci` completed naturally GREEN:Python RUN263,passed262,failed0,skipped1 (existing skip);
2975 reported Rust passes across344 summaries,0failures,25ignored (reported passes are
not a unique-mechanism census). Program controls248,11exact named compiler
refusals,0failing. Anchors3090/269. Wall10:15.87,user2263.93s,sys109.88s,385%CPU;
existing latency checks and ceilings unchanged. Full raw SHA0651f78a748af36d0aa8bb04af97a6dbff12a6d194b0a33738353f120f7e744d.
The original probe was temporarily guarded at its approved same-worktree backup
path for fmt/CI, then restored with exact SHA.

The actual standalone focused recipe then ran on clean tested source0ff69eaa:
40methods GREEN plus WHOLE8/8 intended structured body assertions,0invalid or
survivors/stops. This repeats the same eight controls for a truthful clean-source
recipe header; it is not sixteen mechanisms. Both runs and their costs are in
the validation index. The immutable check/build branch remains e82b0d6c.

Committed evidence: `guide-producer.txt`, `guide-producer-guide-whole.txt`,
`guide-producer-ci.txt`, `guide-producer-history.txt`, `guide-producer-native.json`
and `guide-producer-validation-index.json` under docs/evidence/E14. Historical
RED28 and the parent's actual native fixture RED remain distinct from GREEN40
and complete mutation runs. An intermediate test-helper missing-report error
and external receipt-helper newline SyntaxError are retained honestly; the latter
was fixed and mandatory restoration rerun before CI, without project edits.

Canonical upstream v17's own whole82 was reported naturally GREEN by its owner,
with unchanged producer902d/protocol91e9 hashes. That is upstream evidence, not
an additional local campaign. The local consumer uses those exact files and the
shared Cargo v1 authority; no competing result validator was introduced.

The root justfile import of just/guide-producer.just remains integrator-owned;
the current standalone command is recorded exactly in its report. Other23
cleanup gaps remain parent-owned; the descriptions correction is another track.
No all-framework validity, general cancellation correctness, universal browser
propagation, train landing, push/merge or larger-train latency result is claimed.
The assembler must measure its larger changed program and preserve the bound.

The final validation index enumerates all nine tested source files, every one
of the unchanged74 controls with its source/MUST/exact CHECKED set, all eight
producer controls with full witness IDs, original command selections and shared
authority hashes. AST equality to pre-correction source holds MUTANTS/MUST/CHECKED
and all original command arrays unchanged. Focused Python RUN40,passed40,failed0,
skipped0; full-CI Python RUN263,passed262,failed0,skipped1; Rust reported summed
passes2975,failures0,ignored25 across344 result groups. The source commit is
0ff69eaaf309d56d59030bbfdb35479fcabbc6b4; evidence metadata adds no runtime changes.

Metadata patch hygiene initially refused one trailing space in an embedded old
unittest log line. Only the embedded text normalizes trailing whitespace; the
original raw bytes and hashes remain retained. No runtime source changed.

## Merged into train-11 (2026-10-20)

Merged after check-is-build, as the integrator ruled, so only this branch's
own paths came in. One conflict: `scripts/cargo_test_results.py`, which this
branch added as the v1 named above (SHA-256 acb698a7...) and master already
held in its later form, ADR-0352's completion: a command's aggregate must
be nonempty, and a binary a filter leaves empty beside others is a result,
as every filtered `cargo test` prints (the bounded statements' and the
render merge's runs among them). Master's is kept; the guide's own command
runs one binary (`--test guide`), so its results are judged the same. The
test that held v1's rule became
`test_cargo_counts_an_empty_binary_beside_nonempty_ones_as_the_shared_parser_does`,
with the control that empty binaries alone are no result. `just/guide-producer.just`
is imported by the justfile, so `e14-guide-producer` is a recipe the verify
plans and records.

## Numbered on train-11 (2026-10-20)

- **Numbered ADR-0362** (the integrator's ruling of 2026-10-20: a train is
  numbered before it is pushed, after the train's first seven, in train
  order), merged from `6abc09f3` (`track/guide-producer-validity`).
  `ADR-XXXX` reads `ADR-0362` in nothing else (the placeholder was this
  file's name and title only), as in this file's name and title.
