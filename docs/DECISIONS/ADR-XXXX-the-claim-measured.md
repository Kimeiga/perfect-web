# ADR-XXXX: the claim, measured: the bug museum and the head-to-head

Status: proposed (track `proof`, a plan; the integrator numbers it).
Date: 2026-10-14. Milestone: E14 (charter §14 M14; the proof roadmap §A).

## Context

The owner's claim is that Pleris makes complex web apps more correctly than
today's frameworks, having learned from their failures. The reference apps
prove breadth. Two things prove the claim (the owner, relayed 2026-10-08,
`docs/NEXT.md` "then the evidence that answers 'is Pleris perfect?'"):

1. **the bug museum, extended**: real incidents, each with a primary source,
   reproduced in Next.js or SvelteKit, refused in Pleris, with honest misses;
2. **the head-to-head comparison**: Pleris against Next.js and SvelteKit,
   extending ADR-0120's three-stack contract: lines of code, defects, bytes,
   Core Web Vitals, accessibility; fair, idiomatic, and losses reported.

Neither had started. What exists to build on:

- `examples/rejected/`: 69 `R-*` programs, each refused by a named
  diagnostic (`// @rule: PWxxxx`, `// @expect-error:`). They are the Pleris
  half of an exhibit, without the other half: nothing shows the bug happening
  in a familiar stack.
- `research/failures/`: the census of 224 invariant-level failures in 24
  families, stable leaf IDs (`E01`, `G01`, `J08`, ..), and 76 external
  sources (`SOURCES.md`). Several are standards or documentation, not
  incidents; the census says so itself.
- ADR-0120's baselines: the store in Next.js 16.3.8 / React 19.3.0 and
  SvelteKit 2.70.3 / Svelte 5.57.1, pinned, in the pnpm workspace with their
  dependencies installed, and `benchmarks/harness/` (the contract's fixtures
  and its grading config). `scripts/bench_contract_mutations.py` already
  builds a patched baseline in place and restores it; the museum does the
  same, so it downloads nothing.
- Tasks T01-T12 already carry an `unsafe` patch per stack: plausible wrong
  code a grader must catch. Some are museum-shaped (T03 private cart lines,
  T08 duplicate submissions, T12 an unsafe cache), but a task's unsafe patch
  is graded as a wrong *answer*, not presented as a *failure with a source*.

## Decision

### 1. The museum's method

**An exhibit qualifies** when it has all of:

- **a census leaf**: its stable ID (charter §3.3: a leaf names an invariant,
  not a framework's syntax). A new failure is added to the census first
  (validated by `research/failures/tools/validate.py`), then exhibited;
- **a primary source with its date**, one of, strongest first:
  1. an incident report or postmortem by the party that had it;
  2. a CVE or the vendor's own security advisory;
  3. an issue or pull request in the framework's own tracker;
  4. the framework's own documentation describing the failure (for example
     SvelteKit's "state management", which states its server-global example
     leaks one user's data to another). A living document is dated by the
     day it was read, and its URL is recorded with the revision when the site
     gives one.
  A third party's blog or a forum answer may supplement a source; it never
  qualifies an exhibit alone. The exhibit records which kind its source is,
  and the evidence prints it, so a documented failure mode is never presented
  as an incident;
- **a class Pleris claims to refuse**: the charter section or ADR making the
  claim. A class Pleris does not claim is still exhibited when users hit it,
  and is then a miss by design (below).

**The reproduction** lives in `benchmarks/museum/<leaf>-<slug>/`:

- **minimal**: the smallest patch to ADR-0120's Next.js or SvelteKit store
  that shows the class (`next-react.patch` or `sveltekit.patch`), on top of a
  task's setup where the class needs one (F01 renders T09's notice), or none
  at all where the baseline already has the bug (G01). Applied in place to
  the worktree's baseline, on the dependencies the workspace installed for
  it, and taken back after; a baseline with uncommitted changes is refused.
  Checked by the stack's own checker (`tsc`, `svelte-check`): the bug is one
  its checker accepts. Built artifacts only (`next start`, `node build`), as
  the contract runs them;
- **idiomatic**: each construct the patch uses follows the framework's own
  documentation, cited in the patch's comments. The bug must be one a
  competent developer following those docs can write, never a strawman;
- **in the pinned versions**: when the source's bug was fixed upstream
  before the pinned version (a CVE), the exhibit reproduces the *pattern*
  where the pattern still fails, says "pattern, not version", and records
  the vulnerable version it would need as a download question;
- **shown to happen**: `bug.spec.mjs`, a Playwright test asserting the
  outcome a user sees (user B reads user A's name; one press makes two
  orders), which **passes** on the reproduction.

**Refused in Pleris** means one of, recorded per exhibit:

- **refused by a diagnostic**: `pw check` on the faithful program reports
  the named code (`PWxxxx`). The faithful program is `pleris.patch`, the same
  change made to the benchmark's Pleris store (`benchmarks/baselines/pleris`,
  ADR-0156), checked with the platform packages as `bench.mjs` checks it.
  *Faithful*: the same intent written as directly as Pleris allows, not a
  program contrived to trip the checker; where Pleris cannot write the
  framework's construct at all (a module-level mutable variable, J08), the
  exhibit says which construct stands in for it and why;
- **refused at runtime**: a test runs the same scenario against the Pleris
  development server and passes (the bug does not happen), naming the
  mechanism (an ADR) that prevents it;
- **refused by construction**: Pleris has no way to write the buggy
  program. Weaker evidence, labelled so: the exhibit shows what one writes
  instead and a test of its safe behaviour.

**Every exhibit carries controls**, so that neither half is vacuous (the
census's U04, "a false-green rejection uses the wrong reason"):

- **the reproduction's control**: `fixed.<stack>.patch`, the idiomatic
  repair per the framework's docs, applied on the bug. With it,
  `bug.spec.mjs` must **fail**: the test detects the bug, not something
  else;
- **the Pleris control**: the accepted neighbour, which must check clean:
  the program without the change, or with the repair the code asks for
  (`pleris-fixed.patch`, when an exhibit has one). The diagnostic is for
  this bug, not an unrelated error;
- **mutation controls**: `scripts/proof_mutations.py` (the pattern of the
  existing mutation scripts and `scripts/mutation_anchors.py`) disables each
  refusing rule or mechanism in turn; the museum's run must then report that
  exhibit as not refused (the mutant is killed). Each `.pw` mutant stays a
  well-typed program.

**Misses are reported as prominently as wins.** An exhibit Pleris does not
refuse is a **miss** with its reason, one of: no rule exists; a rule exists
and the faithful program evades it; the checker accepts a declaration the
runtime does not perform (ADR-0120's finding about `optimistic` and
`idempotent_by` is this kind). The evidence lists misses first, then
partial refusals, then refusals. NEXT.md's order holds: what the museum
finds Pleris does not catch is fixed before more breadth is built, so each
miss names the ADR or task that would close it.

**Statuses**: `refused (diagnostic PWxxxx)`, `refused (runtime, ADR-n)`,
`refused (by construction)`, `partial (what is and is not refused)`,
`miss (why)`. An exhibit whose either half has not run is `not run`, never
a pass.

### 2. The first five exhibits

Chosen from the census for classes Pleris claims to refuse, spread over
privacy, commands, injection, caching and data access, with one expected
miss. "Expected" statuses are this plan's; the evidence file is what
counts.

| exhibit | census leaf | source (kind, date) | reproduced in | Pleris's claim | expected |
|---|---|---|---|---|---|
| `J08-shared-server-state` | J08, E01 | S54: SvelteKit docs, "state management", its server-global example (framework documentation; read 2026-10-14) | SvelteKit: a module-level variable a `load` sets for one request and another request reads | `R-004`'s rule, `PW0100`: a session-labelled value in a shared materialization; charter §7.8 | refused (diagnostic) |
| `G01-retried-command-twice` | G01, M04 | S13: AWS Builders' Library, "making retries safe with idempotent APIs"; S14: Stripe, "idempotent requests" (engineering article, provider contract) | Next.js: a Server Action that places an order, its POST delivered twice (a retry), two orders | `R-014`'s rule, `PW0312`; `idempotent_by` (ADR-0024/0025); T08's refusal (ADR-0154) | partial: to establish what the checker refuses and what the runtime performs since ADR-0120's finding |
| `F01-markup-from-text` | F01 | S21: OWASP XSS prevention cheat sheet; S27: W3C Trusted Types (guidance, specification) | SvelteKit `{@html}` and Next.js `dangerouslySetInnerHTML` on a store description holding `<img onerror>` | `R-024`'s rule, `PW0322`: `raw_html` needs the `unsafe.raw_html` capability and a justification; charter §8.2 | refused (diagnostic), and a runtime test that text is escaped |
| `J01-stale-page-after-save` | J01 (B06) | ADR-0280: the owner's own Next.js case; Next.js docs, `staleTimes` and the Router Cache (framework documentation, read 2026-10-08) | Next.js: a save, then `router.push` to a page the Router Cache serves as read before the save | ADR-0280: a handler navigates after its command commits | refused (runtime) |
| `H07-n-plus-one` | H07 | the census has a model/spec only; a primary source is to be found (an incident report naming N+1, or the framework's docs on request waterfalls) | Next.js: a Server Component that reads one row per list item | none found yet: the census marks H07 "specified but unproven" | **miss**, unless a rule is found |

The sixth, queued: a race between two tabs (H01, lost update, S16
PostgreSQL's isolation docs), expected a miss.

### 3. The recipe

`just e14-museum` (in `just/proof.just`) writes `docs/evidence/E14/museum.txt`:
the versions; for each exhibit, the reproduction checked, built and served
in place, its `bug.spec.mjs` run on it (must pass) and on its repair (must
fail); `pw check` on the faithful program (must report the code) and on its
neighbour (must check clean), and the runtime test where there is one; one
table, misses first, then whatever is not as its exhibit expects, then the
refusals, each with its source's kind and date; and the mutation controls.
The runner is `benchmarks/museum/museum.mjs`; it reuses the contract's
fixtures and grading config (`benchmarks/harness/`). An exhibit is a
directory, `benchmarks/museum/<leaf>-<slug>/`: `exhibit.json` (its leaf,
source, claim, patches and what it expects), its patches, and
`bug.spec.mjs`. Playwright on the brief's port range (`PORT`, 8941-9140).
The run fails when any exhibit comes out otherwise than it expects, so a
refusal that stops being made, or a miss that starts being refused, fails
the recipe until the exhibit is updated.

### 4. The head-to-head's protocol

**Apps, in order:**

1. **TodoMVC**: the Pleris TodoMVC being built on `track/todomvc`, against
   TodoMVC's own React and Svelte implementations, never rewritten here
   (NEXT.md: "taken from TodoMVC's own repository and never written here").
   Fetching them is a download, listed below for the owner;
2. **the store slice** ADR-0120 already has: the three stores and their one
   contract, extended by the tasks' reference patches where each stack has
   one.

**Metrics and their instruments**, each from a recorded command:

| metric | instrument | rule |
|---|---|---|
| lines of code | a counter in `scripts/` (no new tool) | non-blank, non-comment lines of the app's own source; configuration, lockfiles and generated files excluded and listed; the rule printed with the count |
| defects | one hidden suite run against every stack | for TodoMVC, the behaviours of TodoMVC's own specification; for the store, ADR-0120's contract plus the tasks' hidden tests; count of failing tests per stack |
| museum entries permitted | `e14-museum` | per stack: which exhibits' bugs it lets a developer write and ship |
| bytes | Playwright on Chromium, the DevTools protocol's `Network.loadingFinished` | JavaScript before the first interaction, and the bytes the first interaction downloads, transferred and decoded, production builds |
| Core Web Vitals | `PerformanceObserver` in the page (`largest-contentful-paint`, `layout-shift`, `event`), lab only | fixed CPU and network throttling, N runs in randomized order, median and spread; lab numbers are labelled lab, never field |
| accessibility | Playwright's accessibility snapshot and role assertions now; axe-core when the owner approves it | rules violated per stack |

**Fairness rules:**

- idiomatic in each stack, reviewed against that framework's own docs; each
  file of a store written here cites the doc pages its patterns follow;
- each stack's standard libraries allowed; the same markup contract and the
  same data model (in memory, one process), as ADR-0120 fixed;
- production builds only, the same machine, browser and throttling;
  versions pinned in `tools/versions.lock` and verified against primary
  sources; any package a stack adds waits for the owner's approval;
- who wrote each stack's code is stated beside its numbers.

**Losses**: every result file has a section "where Pleris loses", in the same
table and the same type as its wins; no metric is dropped for losing; a lab
measurement Pleris loses is not explained away beside it, only explained in
the ADR that reports it.

## Downloads the owner is asked about

Nothing was downloaded. These would be:

- **TodoMVC's own React and Svelte implementations**, from the `tastejs/todomvc`
  repository: name, version (a commit), licence (expected MIT, to verify)
  and size to be stated when asked, before fetching;
- **axe-core** for the accessibility audit (licence MPL-2.0 expected, to
  verify; version and size to state);
- any framework version a CVE exhibit needs (for example Next.js before
  15.2.3 for CVE-2025-29927, S10), only if a pattern reproduction is judged
  not enough.

## Consequences

- Each exhibit is evidence both ways: the bug shown happening, and the
  refusal shown to be for it. A miss is a result, not a failure of the run.
- The museum and the comparison read the same exhibits: "museum entries each
  stack permits" is one of the comparison's metrics.
- Not claimed: that five exhibits sample the census fairly; that a lab Core
  Web Vitals number predicts the field; that a refused exhibit means Pleris
  refuses every program of its class.

## Results

`node benchmarks/museum/museum.mjs`, 2026-10-14, the five exhibits run here;
`just e14-museum` records them with the mutation controls in
`docs/evidence/E14/museum.txt`. Misses first:

| exhibit | the bug, shown | its repair | Pleris |
|---|---|---|---|
| `H07-n-plus-one` | Next.js: one view of a three-item menu, three round trips | one batched read: the spec fails | **miss**: the same per-item read, `List.map` over a host read in a query, checks clean |
| `F01-markup-from-text` | SvelteKit: a notice rendered with `{@html}` runs its `onerror` in a customer's page | the notice as text: the spec fails | refused, `PW5010`; neighbour clean |
| `G01-retried-command-twice` | Next.js, ADR-0120's store unchanged: one press, its POST delivered twice, two adds | T08's reference, an interaction per press: the spec fails | refused, `PW0338`; neighbour clean; and the neighbour's runtime adds once when delivered twice (T08's hidden test, score 1) |
| `J01-stale-page-after-save` | Next.js: after a save, `router.push` shows the cart page prefetched before it ("0"); a reload of the same address shows "1" | `router.refresh()` before the push: the spec fails | refused at runtime (ADR-0280): `navigate.spec.mjs`'s order placed and read after the commit |
| `J08-shared-server-state` | SvelteKit: Bob, who added nothing, is shown Alice's count, from a module-level `let` | the session's cart read per request: the spec fails | refused, `PW5001`; neighbour clean |

Each bug is one its stack's own checker accepts (`tsc`, `svelte-check`),
built and served as the contract serves it.

## Findings

Corrections first.

1. **The `R-*` headers' codes are stale.** R-004 says `@rule: PW0100` and
   `pw check` reports `PW5001`; R-024 says `PW0322` and it reports
   `PW5010`. The `@invariant` names still match. The brief's "59 `R-*`
   programs" are 69 now.
2. **`bench.mjs`'s Next.js and SvelteKit sandboxes cannot install on the PC
   this ran on**, two ways: corepack's last known good pnpm there is 12.11.2,
   whose files are missing, so any `pnpm` outside the repository fails, and
   the sandboxes live under `/tmp`; and inside it, the harness's pinned
   sandbox lockfile needs `nanoid@3.3.19`, which the store does not hold
   (the workspace's lockfile has 3.3.18), so an offline install stops at
   `ERR_PNPM_NO_OFFLINE_TARBALL`. The E14 harness's runs and the tasks'
   controls on those two stacks need a download on this host (shown by the
   same install run by hand, not by running those recipes). The museum
   builds in place instead.
3. **G01's bug is in ADR-0120's own store.** An idiomatic Server Action in a
   `<form action>` adds once per delivery of its request; no change was
   needed to show it.
4. **J08's faithful program is a stand-in, said so.** Pleris has no
   module-level mutable state, so the framework's construct cannot be
   written; the one place a Pleris program keeps a value across requests is
   a resource's cache, and the same intent there is refused. Not shown: data
   a user types, kept in a public resource, carries no private label, and
   whether Pleris would refuse that variant is untested; it is the next
   exhibit's question, and may be a partial.
5. **H07: Pleris accepts a read per row.** The generality corpus holds the
   same shape as an accepted neighbour
   (`examples/generality/undeclared_effect/complete-row.pw`); nothing
   batches it. The census's "specified but unproven" is accurate. Not yet
   measured: the round trips the Pleris runtime makes for it. H07 also has
   no dated primary source yet, so by this ADR's own rule it does not yet
   qualify; it is kept, labelled, because a miss is better shown than
   omitted.
6. **J01 confirms ADR-0280's reading in the pinned version**: next@16.3.8's
   own defaults are `staleTimes { dynamic: 0, static: 300 }`, and a fully
   prefetched page is shown after a save until `router.refresh()`.
7. **A readiness probe that reads the page is the first read of what it
   reads.** The runner's probe asked for the store page, T09's notice was
   kept five minutes from it, and F01's spec never saw its markup. The probe
   asks for a path no route serves.

## What is claimed, and what is not

- Claimed, for these five: the bug happens in the pinned framework version
  and its idiomatic repair removes it, and Pleris's answer is as the table
  says, each by a recorded command, with mutation controls for the three
  refusing codes.
- Not claimed: that five exhibits sample the census; that a refused class
  is refused for every program of it (finding 4); that the census's
  documentation and guidance sources are incidents (each exhibit prints its
  source's kind); H07's runtime cost in Pleris; any head-to-head number,
  none of which is measured yet.

## Next

1. Dated primary sources where the census has guidance or a model only:
   H07 (an incident naming N+1, or a framework's documentation of it), F01
   (a CVE of this pattern in a Next.js or SvelteKit app), J01 (a Next.js
   issue of the owner's case).
2. The sixth exhibit, two tabs (H01, a lost update, S16), and J08's variant
   (finding 4).
3. F01's second layer: every Pleris page carries a CSP from its build
   (ADR-0195's ruling 13), which would refuse an inline handler that
   reached the page; not yet shown by a test.
4. The census's "missing" and "partially covered" leaves as exhibits that
   say "Pleris does not catch this yet" (NEXT).
5. The head-to-head, TodoMVC first, once `track/todomvc` lands and the
   owner decides the downloads above; then the store slice.

## Questions for the integrator

1. The downloads above (TodoMVC's React and Svelte implementations,
   axe-core, a vulnerable Next.js only if a CVE exhibit needs one), and, for
   finding 2, whether this host's pnpm store should get `nanoid@3.3.19` or
   the harness's sandbox lockfiles should follow the workspace's.
2. May a worker read upstream web pages (framework docs, issues) to date a
   source? Meanwhile every source here is one the census or an ADR already
   read, or the installed package's own code.
3. Should an exhibit be linked from its census leaf (editing
   `research/failures/taxonomy/*.md`), or only the exhibit to its leaf, as
   now?
