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
  SvelteKit 2.70.3 / Svelte 5.57.1, pinned, in the pnpm workspace, and
  `benchmarks/harness/bench.mjs`, whose `sandbox()` copies a baseline,
  installs its pinned lockfile **offline** from the local store, and applies a
  patch. The museum reuses that path, so it downloads nothing.
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
  that shows the class (`next-react.patch` or `sveltekit.patch`), applied in a
  sandbox made by `bench.mjs`'s `sandbox()`. Built artifacts only
  (`next start`, `node build`), as the contract runs them;
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

- **refused by a diagnostic**: `pw check` on `pleris/` (the faithful
  program) reports the named code (`PWxxxx`) with the expected message, as
  the `R-*` corpus asserts them. *Faithful*: the same intent written as
  directly as Pleris allows, not a program contrived to trip the checker;
- **refused at runtime**: a test runs the same scenario against the Pleris
  development server and passes (the bug does not happen), naming the
  mechanism (an ADR) that prevents it;
- **refused by construction**: Pleris has no way to write the buggy
  program. Weaker evidence, labelled so: the exhibit shows what one writes
  instead and a test of its safe behaviour.

**Every exhibit carries controls**, so that neither half is vacuous (the
census's U04, "a false-green rejection uses the wrong reason"):

- **the reproduction's control**: `fixed.patch`, the idiomatic repair per
  the framework's docs. With it, `bug.spec.mjs` must **fail**: the test
  detects the bug, not something else;
- **the Pleris control**: `pleris-neighbor/`, the accepted neighbour (the
  same program with the repair Pleris asks for), which must check clean: the
  diagnostic is for this bug, not an unrelated error;
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
for each exhibit, the reproduction built and served from its sandbox, its
`bug.spec.mjs` run on it (must pass) and on `fixed.patch` (must fail); `pw
check` on `pleris/` (must report the code) and on `pleris-neighbor/` (must
pass), or the runtime test; then one table, misses first, with each source's
kind and date. The runner is `benchmarks/museum/museum.mjs`, beside
`bench.mjs` and reusing its `sandbox()`. Playwright on the brief's port range
(`PORT` 8941-9140).

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

## What was done, and what is next

Written as the track proceeds.

- 2026-10-14: this plan; the five exhibits chosen, sources recorded.
- Next: the runner and the first exhibit end to end (`J08`), then the other
  four, then `just e14-museum` and its evidence, then the mutation script.
