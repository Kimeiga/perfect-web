# Pleris code-quality gates

The project favors evidence from executable invariants over a high count of
scanners. This page distinguishes checks that block a merge from pilots that
only collect baseline evidence.

## Required foundation

- `ci` runs a cheap Rust formatting, Python syntax and change-scope gate
  before installing the heavy toolchain. The full `just ci` still performs
  Clippy, test-unit, compiler conformance and evidence checks.
- `verify` covers PRs against master, not just train branches. The existing
  dependency planner selects reached recipes; application/compiler/runtime
  changes also require the complete Chromium, Firefox and WebKit suites.
- PRs cannot cite a cancelled job or an earlier commit as verification.
  A GitHub ruleset is needed to *enforce* this policy at merge time; CI
  workflow files alone cannot prevent administrators bypassing it.

## Advisory scanners

CodeQL Rust/JavaScript/Python/Actions, Ruff, actionlint and zizmor are
introduced in separate workflows. First runs establish the baseline and
tool compatibility. Do not make new scanners required until findings are
triaged and coverage is demonstrated; a successful scanner invocation
does not imply there were no security issues.

## Where next

1. Replay dated pre-merge defects from representative PRs and train
   integrations. Measure extra true findings versus false positives and cost.
2. Pilot `cargo-mutants --in-diff` plus `cargo-llvm-cov` on one changed
   crate. The repo already runs hundreds of tailored mutation campaigns;
   generic mutants must add independent fault detection before promotion.
3. Model a small concurrency/command state machine with Loom or bounded
   schedule tests; not a blanket concurrency guarantee.
4. Require current/unique job names in the GitHub branch ruleset; configuration
   is an administrator control not writable through this integration.

Preflight locally with `just preflight`, or rely on hosted results when the
pinned Rust toolchain is unavailable. Hosted CI is not background work by an
agent: the branch's exact SHA must be checked before any merge.
