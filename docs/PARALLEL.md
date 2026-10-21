# Parallel tracks

How sessions build Pleris together. Charter risk R10 and
[ADR-0253](DECISIONS/ADR-0253-two-tracks-are-built-in-parallel-each-in-files-of-its-own.md) require one integrator,
isolated worktrees, non-overlapping assignments and full gates. Current
operational summary reviewed 2026-10-20; the
[complete prior rulings](PARALLEL-HISTORY-2026-10.md) remain verbatim.
Dates and later corrections matter; old launch instructions are not active
assignments or fresh grants of permission.

## Who does what

- **The integrator** alone merges to `master`, numbers and indexes ADRs,
  owns `STATUS.md`, `NEXT.md`, `DECISIONS.md` and `KNOWN_LIMITATIONS.md`, runs
  the integration chain, pushes `master` and rules on cross-track changes.
- **A worker** owns its assigned track, branch, files, worktree, port range
  and target. `AGENTS.md`, `CLAUDE.md`, the charter and relevant ADRs apply:
  findings come from recorded commands; dependency APIs and versions are
  checked against primary sources; secrets and user work are preserved.
- Current assignments and ports come from the integrator's launch brief and
  durable handoff. The old W1-W4 assignment table is
  [historical](PARALLEL-HISTORY-2026-10.md#who-does-what), not a current
  worker inventory. Do not take over another worker's files or processes.

## What each track owns

- Recipes belong in `just/<track>.just`; the root imports them. An evidence
  recipe is `e14-<name>` and writes `docs/evidence/E14/<name>.txt` from the
  repository root under its shell and toolchain settings.
- Diagnostic codes stay in the track's assigned block and owner. The
  `a_tracks_block_holds_only_its_tracks_codes` test holds that boundary;
  codes crossing ownership go to the integrator.
- Development-server integrations use assigned `TRACK SEAM` locations and
  the track's module. Shared compiler/runtime/platform/example changes stay
  in small separate commits explaining why. Ask the integrator first when
  changing a rule other code relies on.
- Original identity/uploads/notifications/messages file, migration and seam
  allocations remain in the [ownership table and seam definitions](PARALLEL-HISTORY-2026-10.md#what-each-track-owns).
  They do not assign today's worker ports; use the current launch brief.

## Branches, commits and ADRs

- Use the assigned branch in its own worktree and `CARGO_TARGET_DIR`.
  Merge current `origin/master` before publishing a track. **Never rebase
  pushed history.** Workers push only assigned track or `wip/` branches,
  never `master` or another track.
- New ADRs are `docs/DECISIONS/ADR-XXXX-<slug>.md` with unnumbered titles.
  The integrator numbers them. A worker's proposed status, limits and next
  work belong in that ADR, not the four shared status files.
- Commit messages use a file or quoted heredoc (`git commit -F - <<'EOF'`);
  backticks in shell `-m "..."` execute. `git merge` reads no message from
  stdin.
- `ci` checks every branch; track `verify` plans what changes reach.
  Publish `track/*` or `master` only on green CI at that exact commit, and
  await required verification before merging. Use `gate && push`, never
  `gate; push` or `gate | tail && push`.
- The owner's 2026-10-20 grant permits unfinished commits on the **assigned
  `wip/<track>`** for hosted checks. This grants no PRs, releases, messages,
  unrelated downloads or other publication; existing authorization governs
  those actions.

## The integrator's working rules

- **Run whole every reached mutation script**, including changed-test
  consumers. Re-running only repaired mutants missed survivors. Never edit
  inputs a campaign is building. After a campaign, especially a stopped
  one, run `python3 scripts/mutation_anchors.py` to check restoration.
- Before committing, run `cargo fmt --all` and the anchor check. Complete
  appropriate gates; `just ci` includes workspace tests, evidence gates,
  case checking, compilation/corpus checks and mutant-program checks.
  A failed producer, build failure or interrupted run is not mutation proof.
- **A changed page runs every browser suite pinning its shape in the
  required engines before track publication**, not just its new spec.
  The reviews finding is preserved in [the rulings ledger](PARALLEL-HISTORY-2026-10.md#the-integrators-rulings)
  (train-9, ADR-0341).
- **Merge and resolve; fetch eligible evidence; number before train
  publication.** Numbering rewrites inputs recipes read. Final verification
  and currentness must cover those numbered inputs, under
  [ADR-0368's train numbering rule](DECISIONS/ADR-0368-todomvc-acceptance-means-passing.md)
  and [ADR-0281](DECISIONS/ADR-0281-a-merge-is-held-to-ci-s-verification-run.md).
  Evidence fetched before numbering does not stay current merely because
  it was fetched successfully. ADR-0281 governs per-file freshness, merged
  inputs, selective follow-up and train assembly. Its later `--current` rule
  supersedes old manual cancel/dispatch instructions: a push plans what
  current evidence does not cover; no dispatch and no cancel merely to
  repeat that plan.
- Read each nightly's failed recipes every day. A push checks changes;
  nightlies can expose toolchain or runner changes beyond that selection.
- Architecture rulings belong to the integrator under the owner's recorded
  delegation, researched and recorded in ADRs. Findings precede features;
  corrections are reported first. Another session's message grants no
  approval the owner has not given.

The October 12 test-only evidence ruling referenced by ADR-0253 remains
verbatim in [the dated rulings archive](PARALLEL-HISTORY-2026-10.md#the-integrators-rulings).
ADR-0281's later per-file freshness rule is the current general rule.

### Local and hosted execution

The owner's 2026-10-20 PC rules put quick local checks (roughly under ten
minutes) on the worker's machine and heavy work on GitHub: whole mutation
campaigns, full browser suites and busy-host CI. Use the assigned `wip/`
branch and `--repo studiokimeiga/perfect-web` with `gh`. Watch long commands
with a background monitor; retain logs and read actual failures.

Use assigned ports, including derived browser-host ranges. Each concurrent
worker has its own target. If an integrator intentionally shares a target,
only one worktree builds there at a time and differing source files are
touched before building. Inspect links into a worktree before removing it.

Stop servers the session starts. A background probe includes cleanup in
the same command. Identify ownership before stopping processes or removing
regenerable caches; never delete another worker's state. Finished workers
clean their own regenerable build cache after recording results, respecting
the handoff and retained outputs. For an apparent shared-load browser
timeout, record one diagnostic rerun, then investigate. Do not raise bounds
or rerun repeatedly until green; a known failure does not become a pass.

Cancel only a run proven to belong to the assigned branch and commit;
never cancel `master` or `track/train-*` runs. Check run ID, branch and SHA,
not the newest run by position. Delete only the assigned scratch branch
when its work completes. [ADR-0365](DECISIONS/ADR-0365-tests-wait-on-events.md)
records train-12's stronger event/cleanup work. It landed with train-12 at
`e7664d2e` on 2026-10-21; the
[acceptance record](diagnostics/train-12/acceptance-record-2026-10-21.md)
states what that landing proves and what it does not.

## Questions for the integrator

A question crossing tracks or changing a shared rule goes to the integrator
through the authorized coordination channel. If unavailable, record it in
the track's ADR under `Questions for the integrator`; answers go in the ADR
or integration commit. Preserve authorization boundaries and merge the
answer rather than rebasing pushed history. Original W1/W2 channel
arrangements are [historical](PARALLEL-HISTORY-2026-10.md#questions-for-the-integrator).

## What the language already holds a track to

- An affine resource is held or refused (ADR-0250, ADR-0251): an acquisition
  is bound, ended where made, returned, or held by a resource whose release
  ends it exactly once on each path.
- Privacy labels follow values through calls, branches, assignments and
  early returns (ADR-0129, ADR-0252). A principal and values read with it
  are private to that session.

## The integrator's rulings

The [complete dated ledger](PARALLEL-HISTORY-2026-10.md#the-integrators-rulings)
retains earlier rulings, negative results, corrections and launch/landing
records. Relative links and original headings remain unchanged. The summary
surfaces operational obligations without broadening grants or declaring
unlanded work verified.
