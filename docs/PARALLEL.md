# Parallel tracks

How more than one session builds Pleris at once (ADR-0253; charter risk R10:
"One integrator; separate worktrees; non-overlapping assignments; mandatory
charter+ADR reading (`AGENTS.md`); small merges; full gate tests").

## Who does what

- **The integrator** (the session working on `master`) alone merges to
  `master`, numbers ADRs, edits the four status documents (`docs/STATUS.md`,
  `docs/NEXT.md`, `docs/DECISIONS.md`, `docs/KNOWN_LIMITATIONS.md`), runs the
  chain, pushes `master`, and rules on anything that crosses tracks. It also
  keeps a track of its own: the follows timeline (ADR-0195's ruling 10).
- **A worker** builds one track, in its own worktree, on its own branch, in
  the files its track owns. Everything in `CLAUDE.md` and `AGENTS.md` holds
  for a worker as for the integrator: evidence from a recorded command,
  dependency versions and APIs checked against primary sources, no secrets,
  nothing published.

| track | branch | worker |
|---|---|---|
| accounts and sign-in: a `requires` evaluator, sign-up, sign-in and sign-out, per-user sessions, an OIDC-style deployment interface with a local provider that is plainly not production | `track/identity` | W1, a local session; merged 2026-10-07 (ADR-0258) |
| image uploads on a post: a typed upload with size and content-type limits, a deployment's blob-storage capability, served safely | `track/uploads` | W2, a local agent of the session that runs W1 |

The follows timeline landed on 2026-10-07 (ADR-0257); notifications and
direct messages come next, the integrator's.

## What each track owns

| | identity | uploads |
|---|---|---|
| recipes | `just/identity.just` | `just/uploads.just` |
| diagnostic codes | `PW55xx`, `Owner::Identity` | `PW56xx`, `Owner::Uploads` |
| the development server | `spikes/own-renderer/server/src/identity.rs` | `spikes/own-renderer/server/src/uploads.rs` |
| new files | anything new under a directory or name the track's own: `identity`, `accounts`, `sign_in` | `uploads`, `blob` |
| browser hosts | `PORT+70..72`, `IDENTITY_PORTS` | `PORT+80..82` |
| PostgreSQL migrations | `0003` | the next free after `0004`, the follows timeline's |

- **Recipes**: the root `justfile` imports each track's file, whose recipes
  run in the repository's root under the root's settings and `PATH`. An
  evidence recipe is `e14-<name>:` and writes `docs/evidence/E14/<name>.txt`;
  the verification run plans it like any other (`scripts/ci_plan.py` reads
  every part of the justfile).
- **Codes**: a track registers codes only in its block, with its owner; a
  test holds each block to its owner (`codes.rs`,
  `a_tracks_block_holds_only_its_tracks_codes`). A code another track's
  change needs is the integrator's to register.
- **`main.rs`**: a track touches it only at the lines marked `TRACK SEAM`
  for that track, and where it must, in a few more lines marked the same
  way: `// TRACK SEAM (identity): ..`. The seams:
  - `Identity::answer(method, route, headers, session, fresh, body,
    stream) -> bool`, asked for every request after its body is read and
    before any other route: the identity track's own routes.
  - `Identity::requires(predicate, session) -> Result<bool, String>`, asked
    by `Server::run` for each predicate a command's `requires` names. Until
    the track lands, the development model: a session is `SignedIn`, and an
    unknown predicate is an error.
  - `Uploads::claims(method, route) -> bool`, asked for every request
    before its body is read, and `Uploads::answer(route, headers, session,
    fresh, reader, stream)` for one it claims: an upload reads its own body,
    within its own limits, where a command's is bounded to 64 KiB.
  - Each module's state is a field of `Server`, `identity` and `uploads`,
    built by `Default`; a track grows its own struct.
- **Shared files** (the compiler, the runtime, the platform packages, the
  examples): a track may change them where its work needs, in small,
  separate commits whose messages say why, and says so to the integrator
  first when the change is to a rule other code relies on.

## Branches, commits and ADRs

- Branch `track/identity` or `track/uploads` from `master`, in a worktree of
  its own, with its own `CARGO_TARGET_DIR`.
- **An ADR is unnumbered**: `docs/DECISIONS/ADR-XXXX-<slug>.md`, its title
  `# ADR-XXXX: ..`. The integrator numbers it when it merges, and indexes it.
- **A worker never edits the four status documents.** What it would write
  there goes in its ADR: what was done, what is claimed and not, and what to
  do next.
- **Rebase on `master` before pushing**, and push only the track's branch:
  never `master`, never another branch, no release, no PR unless the owner
  asks for one.
- **Every push is checked**: `ci` runs on every branch, and `verify` runs on
  a track's branch the recipes its commits changed since `master`
  (`scripts/ci_plan.py --changed <merge base> <head>`, ADR-0253). A track is
  ready to merge when both are green at its head.
- **A merge is small**: the integrator merges a track when a piece of it is
  done and green, rebasing it onto `master`, numbering its ADRs, writing the
  status documents, and running the chain.

## Questions for the integrator

A question that crosses tracks, or changes a rule other code relies on, is
the integrator's to answer:

- **W1** sends it by message (`SendMessage` to the integrator's session).
- **W2**, a cloud session that cannot message, writes it in its branch's ADR
  under a heading `## Questions for the integrator`, and pushes; the
  integrator answers in a commit to `master` the worker rebases onto, or in
  the ADR's own section, `## The integrator's answers`.

## What the language already holds a track to

- **An affine resource is held or refused** (ADR-0250, ADR-0251): an
  acquisition, an upload's handle as much as a transaction, is bound to a
  name, ended where it is made, returned to a caller, or held by a resource's
  `acquire` clause, whose `release` ends it exactly once on every path.
- **A value's privacy label follows it** (ADR-0129, ADR-0252): through
  calls, branches, assignments and early returns; a session's principal and
  anything read with it are private to it.

## The integrator's rulings

Each a decision for a track, with its date; a track's ADR records it too.

- **2026-10-07, identity (W1's plan).** Pleris owns the relying party:
  state, nonce, PKCE S256, the callback, the session it opens, its cookies,
  CSRF and the `requires` evaluator; a deployment owns the provider behind
  an `IdentityProvider` interface. A development provider refuses to start
  unless the deployment says development and its origin is loopback.
  - `Guest`, every session its own guest principal, stays the default and is
    named and development-only like the rest; a deployment opts the feed
    into accounts. Flipping the feed's default is the integrator's ruling
    once the track merges.
  - A `requires` refusal is answered as its own case, 403 with
    `{"committed":false,"refused":"<predicate>"}`, never as a declared
    error. The runtime is unchanged: a page hides what its viewer cannot
    do, and a stale tab gets the runtime's failure (to queue in NEXT).
  - No compiler change: the host resolves the principal from the session.
    A typed principal in the language is a language ADR, the integrator's.
  - `DataLayer::identified_by`, defaulted, is the one change to the trait.
  - Session cookies are `HttpOnly`, and `Secure` off loopback; a session
    id is 128 random bits from the OS.
  - CSRF, in `Identity::answer` for every unsafe request, in both
    providers: Go 1.25's `CrossOriginProtection` (`Sec-Fetch-Site`, then
    `Origin` against `Host`).
  - A new crate is the owner's to approve, in the track's own session; its
    version is checked against its primary source and `just ci`'s
    advisories gate stays green. A crate already in `Cargo.lock` is taken
    at the lock's newest version.
- **2026-10-07, uploads (W2's four questions).**
  - Limits are the program's: an `upload` declaration, parsed and checked
    like `source` and written by `pw build` to `uploads.json`, its limits
    literals, its `types` a closed set, its `route` no page's (PW56xx). A
    deployment may lower a limit, never raise one.
  - A staged upload is an affine handle, claimed and published or discarded
    in the command's body, so PW2005 holds every path; the development
    origin grants `resource.acquire<Upload>` and `resource.release<Upload>`.
  - A post's image is the post's own data: a field of `feed.rs`'s row and
    columns of `feed_pg.rs`'s posts (migration 0005), written in the post's
    transaction. No decorating layer; blobs are the uploads module's.
  - `image: Option<Image>` on `Post` and `Item`, with a required alt text;
    the timeline shows images.
- **2026-10-07, at identity's merge (ADR-0258).** A typed principal, the
  host's routes in the route table and a refusal shown by the runtime are
  queued (NEXT); the database job sets up browsers and the build for a
  recipe that needs them, and `e14-identity` runs there.
