# ADR-0253: two tracks are built in parallel, each in files of its own

Status: accepted under the owner's delegation of 2026-10-02, on the owner's
approval of parallel workers (relayed 2026-10-07): charter risk R10's model.
Date: 2026-10-07. Milestone: E14, the owner's Twitter list, items 4 and 7.

## Context

- **The owner approved parallel workers**, R10's model: "One integrator;
  separate worktrees; non-overlapping assignments; mandatory charter+ADR
  reading (`AGENTS.md`); small merges; full gate tests." Two tracks start:
  accounts and sign-in (W1, a local session) and image uploads (W2, a cloud
  session). The integrator keeps the follows timeline; notifications and
  direct messages wait for it.
- **Every track would edit the same few files.** The justfile holds every
  recipe, about five thousand lines; `codes.rs` numbers every diagnostic; and
  the development server's `main.rs`, 13,469 lines, is where a request's
  session is read, its body bounded, its route chosen, and a command's
  `requires` decided, in one inline closure.

## Research

- **`just` 1.58.0** (`tools/versions.lock`) has `import`: "One `justfile`
  can include the contents of another using `import` statements", relative
  to the importing file, and `import?` for an optional one
  ([manual](https://just.systems/man/en/imports.html)). Checked here with
  the pinned binary: an imported recipe is in `--summary` and `--list`, runs
  in the root justfile's directory wherever `just` is invoked, and runs under
  the root's `set shell` and exported `PATH`.
- **`scripts/ci_plan.py` read only the root justfile** (`bodies`, and the
  recipes whose own lines changed), and `test_evidence_gates.py` copied only
  it: an imported recipe would be planned by no push, and the gate tests
  would fail on a missing import.

## Decision

1. **Each track's recipes are in a file of its own**, `just/identity.just`
   and `just/uploads.just`, which the root justfile imports; the integrator
   alone edits the root. The root's recipes stay where they are: one session
   edits them, so they conflict with nothing, and moving five thousand lines
   would read to the verification plan as every recipe changed.
2. **The plan reads every part of the justfile** (`ci_plan.justfile_parts`):
   a recipe in an imported file is planned when its own lines or what it
   runs change, as one in the root is. The gate tests copy the imported
   files with the justfile.
3. **Each track's codes are a block of its own**: `PW55xx`, owner
   `Identity`, and `PW56xx`, owner `Uploads`. A test holds each block to its
   owner, so no other code can take a number there.
4. **The development server has a module per track**, `identity.rs` and
   `uploads.rs`, reached from `main.rs` only at lines marked `TRACK SEAM`:
   - `Identity::answer`, for every request after its body is read and
     before any other route; `Identity::requires`, which `Server::run` asks
     for each predicate `requires` names. It starts as the closure it
     replaces: a session is `SignedIn`, an unknown predicate is an error.
   - `Uploads::claims`, for every request before its body is read, and
     `Uploads::answer` for one it claims: an upload reads its own body,
     within its own limits, where a command's is bounded to 64 KiB.
   - Each module's state is a field of `Server`, built by `Default`.
5. **The protocol is `docs/PARALLEL.md`**: the branches `track/identity`
   and `track/uploads`; unnumbered `ADR-XXXX` files, numbered at merge; the
   files and codes each track owns; that a worker never edits the four
   status documents; rebase on `master` before pushing; and questions to the
   integrator, W1's by message and W2's in its ADR's `## Questions for the
   integrator`.
6. **`verify` runs on a track's push**, for what the track changed since it
   left `master` (`--changed <merge base> <head>`), in at most 8 shards;
   `ci` already ran on every branch.

## Acceptance

- **`scripts/tests/test_ci_scripts.py`**: a recipe in an imported file is
  read, and its changed line is its recipe's. **`test_evidence_gates.py`**
  green with the imports.
- **`codes.rs`**: `a_tracks_block_holds_only_its_tracks_codes`, and every
  code still in its owner's range.
- **The development server's tests**, `requires` among them through the new
  seam, and `identity.rs`'s own: `requires_starts_as_the_development_model`.
- **`just --summary`** lists the root's recipes, the imports adding none
  yet; `just ci` green.

## Not claimed

- **A track's recipes in CI's nightly run** are planned like any other once
  they exist; none does yet.
- **The root justfile split by area.** One session edits it; split when a
  third worker needs part of it.
