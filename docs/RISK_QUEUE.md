# Risk-retirement queue

Experiments that test a load-bearing assumption **early**, using temporary
dependencies if that is what it takes.

> Passing an `RQ` retires a risk. It does **not** close the corresponding
> engineering milestone. `docs/MILESTONES.md` holds that distinction.

Each entry records its **decision rule before it runs**. That is not ceremony:
three separate measurements in this project produced plausible, favourable
numbers that turned out to be measuring nothing (see "the recurring bug" below).

Order was fixed by the project architect after reviewing E0's evidence.

| id | question | status | ruling |
|---|---|---|---|
| **RQ-1** | Is Marko's resumption real, in Chrome **and** Safari? | **done** | 11/12 in both engines → oracle for **E7-R** (resumption, DOM preservation) and partially **E7-P**. **Not** the oracle for **E7-L**: check 4 failed. |
| **RQ-2** | Does Koka propagate effects through higher-order abstraction? | **done** | Outcome 1, clean pass → Koka remains the effects oracle; `pw` effect checker not pulled forward |
| **RQ-3** | `pw`-owned exhaustiveness and canonical typed ABI decoding | **partial** | exhaustiveness + type-directed ABI land in `compiler/pw-core`; 30 tests. **Exhaustiveness now runs on `.pw` source** and rejects R-007 with named witnesses. Remaining: the ABI half has no source path |
| **RQ-4** | Minimal static task-scope checker (E2A-S) + runtime structured concurrency (E2A-R) | **done** | Both halves. E2A-S: `compiler/pw-core/src/scope.rs`, PW2001-PW2004, fed from real `.pw` bodies — R-013 and R-039 are compile failures. E2A-R: `runtime/pw-tasks` (ADR-0016), 12 behaviour tests, 0 failures in 25 consecutive runs |
| **RQ-5** | Final-artifact declared-vs-actual component import verification | **partial** | rule + `PW4007` in `compiler/pw-core/src/capability.rs`, tested against E0's real 15-vs-1 import lists. Remaining: call it from the build, not just from tests |
| **RQ-6** | Backfill direct / helper-hidden / generic-callback rejection cases for **every** effect family | **partial** | layout/DOM families done: 10 accepted + 13 rejected, including both hidden cases. Remaining: backfill the other families |
| **RQ-7** | Generate and adopt the E→P claim/evidence table before publishing P0 | **done** (first draft) | `docs/EVIDENCE_LEDGER.md` |

---

## RQ-1 — Marko resumption (done)

**Pre-registered rule.** All core properties pass in both engines → behavioural
oracle for E7. Passes Chrome, fails Safari → architectural donor only. Component
replay, eager handler, DOM replacement, or lost browser state → stop calling
Marko resumable for our purposes. Only payload scaling passes → report payload
scaling and nothing more.

**Result.** Core properties (no replay, first click works, node identity, focus,
second interaction) passed in **Chrome 150 and Safari 26.5.2**.

**Check 4 failed in both engines** — the interaction artifact loads eagerly at
40–55 ms. That was a *pre-registered property*, so it is not a footnote: it
falsifies charter §1.11's "interaction-lazy code" for this configuration, and
E7 is subdivided accordingly (`docs/MILESTONES.md`).

> Marko is the behavioural oracle for resumption, DOM preservation and
> patch-placement semantics. **It is not the oracle for interaction-lazy code
> delivery.**

Safari's autorun path is acceptable for what it can observe and **must not** be
described as equivalent to the Chrome stream-timing harness.

Evidence: `docs/evidence/E0/spike-browser-resumption.txt` ·
`spikes/browser-resumption/README.md`

## RQ-2 — Koka higher-order effects (done)

**Pre-registered rule.** Koka may overstate an effect but must never silently
understate or erase one. Four enumerated outcomes with a fixed decision table.

**Result.** Outcome 1 — automatic inference of effect polymorphism on
*unannotated* generic helpers, propagation through two layers, correct selective
discharge, and no capability hiding. E1 clean pass on effects.

Evidence: `docs/evidence/E0/spike-koka-row-polymorphism.txt` ·
`spikes/koka-row-polymorphism/README.md`

## RQ-3 — `pw` exhaustiveness and typed ABI (partial)

Delivers the core of E1A. Pre-registered acceptance, with what is done marked:

- **done** — an incomplete match is rejected **regardless of the function's
  effect row**, the case Koka provably does not catch
  (`tests/differential_vs_koka.rs`);
- **done** — `Option<T>` and `List<T>` cannot be confused at the boundary, even
  though both collapse to `null` in Koka's JS output;
- **done** — a raw primitive cannot enter a nominal position undetected: debug
  mode catches a `Money<EUR>` payload arriving in a `Money<USD>` slot;
- **done** — decoding is type-directed: `decode(path, ExpectedType, payload)`,
  and there is deliberately no `decode_unknown(payload)` entry point;
- **done** — rejections carry a stable `PW1004` code, a primary span and an
  origin span.

**Open:** there is no parser, so the checker runs on a constructed pattern
matrix rather than on `.pw` files. The original acceptance line — *"fails if any
accepted-corpus program is rejected, or any rejected-corpus program compiles"* —
cannot be evaluated until **E2** puts a front end in front of it. RQ-3 is
therefore `partial`, not `done`.

## RQ-4 — structured concurrency

Split, because a runtime task tree cannot make a compile-fail fixture fail at
compile time:

- **E2A-R (runtime):** owner scopes, cancellation propagation, cleanup, leak
  detection, results not committing into dead scopes. These are *behaviour*
  tests and must never be presented as static guarantees.
- **E2A-S (static):** `task.spawn` owned by the lexical scope; handles that
  cannot be returned, stored, or captured into a longer-lived scope; no
  detachment without a durable capability; effects propagating through helpers.

**E2A closes only when both halves pass.** Koka representing `task.spawn` in an
effect row would be useful but insufficient — it does not prove scope
non-escape, and `pw` owns that property.

**E2A-S status: implemented** in `compiler/pw-core/src/scope.rs`. A scope tree
plus handle-lifetime rules, with four diagnostics:

| code | rule |
|---|---|
| `PW2001` | a handle escapes into a scope that outlives its owner |
| `PW2002` | an ordinary task is detached without the `durable_job.enqueue` capability |
| `PW2003` | a handle is used outside its owning scope — the late-result-into-dead-component case |
| `PW2004` | a subscription declares a scope longer than the component that created it |

Each carries an **origin span** as well as a violation span, per charter §16.3.
Escaping into a *descendant* scope is allowed, because a descendant dies no
later than the handle does. `durable.spawn` is the legal escape hatch and is
exempt by capability, not by convention.

**E2A-R status: done** — `runtime/pw-tasks`, ADR-0016. A thread-scoped runtime
built on `std::thread::scope`, adding cancellation propagation, ordered cleanup,
and the dead-scope commit rule. Twelve behaviour tests, each mapped to one of
RQ-4's five properties in `docs/evidence/E2A/e2a-r-runtime.txt`, with **0
failures across 25 consecutive runs** — a concurrency suite that passes once has
measured almost nothing.

The separation still holds in both directions: these are **behaviour** results
and do not make any misuse a compile error, and E2A-S's static rules do not
cover runtime cancellation. Two cases genuinely need both — a task that finishes
*after* its scope was torn down has no escaping handle for the static checker to
see, and `Scope::commit` refusing it is the only thing that catches it.

Deliberately not proven: anything async. E8 chooses the host execution model and
these semantics must be re-proved there.

## RQ-5 — artifact import verification (partial)

Every build emits declared capabilities, actual component imports, and the
difference. Any undeclared import fails the build (`PW4007`).

**Implemented** in `compiler/pw-core/src/capability.rs`, deliberately free of any
Wasm dependency so the rule is unit-testable without a runtime. Its tests use the
**real import lists E0 measured** — 1 for the `no_std` guest, 15 for the `std`
guest — rather than invented fixtures.

Two runtime profiles, because charter §14 M8 task 4 requires denying ambient
authority by default while still allowing a richer runtime *explicitly*:

| profile | allowance |
|---|---|
| `minimal` | declared interfaces only; `no_std`-equivalent |
| `wasi-cli` | additionally tolerates `wasi:cli/`, `clocks/`, `io/`, `random/`, `filesystem/` |

Choosing `wasi-cli` does not hide the surface: the extra imports are still
listed under `allowed_by_profile`. And an import outside the allowance —
`wasi:sockets/tcp` — still fails.

The `PW4007` text carries the architect's terminology correction, because it is
the difference between a true and a false claim: these imports are a *requested
authority surface*, not authority already held. The host still decides what to
link.

**Open:** the audit is called from tests, not from a build step. Wiring it into
`spikes/wasmtime-component/host` (which already computes the actual list) and
then into the E8 build is the remaining work.

## Milestone ordering, and one place it was wrong — RULED, 2026-08-08

Not a defect in the code. A defect in the plan, and the architect's ruling turns
it into a charter correction rather than a workaround.

**E8's fifth gate item — "run the store's `add_to_cart` as a component" — needed
a Pleris→Wasm-component backend that only E10 knows how to build.**

```text
E8 host
    supposedly must execute Pleris-generated Wasm
                    ↑
E10 Pleris→Wasm backend
```

> E8 cannot honestly require an artifact that only E10 knows how to create.
> Building a temporary backend inside E8 would be exactly the sort of duplicated
> mechanism this project has repeatedly had to delete.

**The gate was amended, not deleted.** E8 now claims what one layer can prove on
its own, and the end-to-end proof became **E10-I** in `docs/EVIDENCE_LEDGER.md`
and in E10's own gate — recorded twice so neither can close without it.

> Changing a gate because its dependency belongs to a later milestone is better
> evidence discipline than building fake machinery just to make the old wording
> green.

### The charter correction

> **Milestones should prove one layer's contract independently. Cross-layer
> end-to-end proofs belong at the milestone where both sides of the boundary
> actually exist.**

The `do not start the next milestone until the gate passes` rule stands
unchanged. What moved is the gate, and only because it was asking one milestone
to demonstrate two layers at once.


## A bare call to nothing checks clean

Found by the E10-A backend on its first run against the store demo, 2026-08-09.

`examples/store/app.pw` calls `current_session()` and **never imported it.**
`pw check` reported nothing, `just ci` was green, and the corpus had been in
that state since E4.

```text
unresolved_uses    checks a DOTTED path whose head looks like a module
a bare name        not checked at all
```

Assumption A-009 says Term names are not ambient — only the Effect namespace is,
through the platform's `prelude Effect`. So `current_session()` resolved to
nothing, and nothing said so.

**Why the backend found it and the checker did not:** every analysis upstream
can produce an answer for a call it cannot resolve — inference contributes no
effects, privacy contributes no label, placement contributes no constraint — and
each of those looks exactly like "this call is harmless". A backend cannot emit
a call to nothing, so it is the first consumer for which the absence is fatal
rather than merely quiet.

**The import is NOT yet added, and the reason is the finding's real size.**
Adding `import context.{ current_session }` makes the call resolve — and then
`StorePage` requires `session.read` **to render**, because it calls
`current_session()` while building its query key. That invalidates a documented
E8 claim:

```text
docs/evidence/E8/component-contracts.txt
  "Note StorePage: rendering needs no authority and runs anywhere."

runtime/pw-host/tests/contract_mirror.rs
  the_store_page_renders_without_authority_and_its_command_does_not
```

Both were TRUE only because the call resolved to nothing. The page does read the
session while rendering; nothing was measuring it. The knock-on set is at least:
the committed contracts, the generated WIT and its host fixture, the dev
server's topology, three `contract_mirror` tests, and the E8 evidence text.

So this is a semantic movement in what the compiler tells the host, not a
one-line repair, and it is left applied nowhere rather than applied halfway.
The backend lowering that found it is committed and refuses `add_to_cart` with
`current_session` unresolved — which is the honest state.

### Step 1 was written, validated, and reverted — its SCOPE is the work

`PW0024` (a bare call to an unresolved name is an error) was implemented and
run. It found `current_session` and nothing else in the store demo, which is
exactly right.

Against the **accepted corpus** it reported 16 errors over 12 names, and almost
all of them are the rule being too broad rather than defects:

```text
release(handle) { .. }        a resource declaration's release CLAUSE
translate(x, y) / scale(1.0)  CSS transform functions in a style value
repeat(..)                    CSS grid
for                           a keyword
```

These are call-SHAPED syntax, not term calls. It is the same failure the rule
itself catches, one level up: the right question asked in the wrong place.

**Scoped from the grammar's own table, the count goes 16 → 11.** Excluding
`pw_syntax::grammar::POLICY_KEYWORDS` removes `release`, `acquire` and `draw` —
read from the grammar rather than from a list written in `check.rs`, which is
`one_parser.rs`'s discipline applied to a checker.

The remaining eleven split into two clean classes, and only one is a false
positive:

```text
POLICY VALUES — the last scoping class, not defects
  conflict merge_by_field(..)      A-011
  identity content_address(..)     A-014
  transform: scale(1.0) -> ..      A-020
  translate(x, y) in a style value A-015
  repeat(..)                       CSS grid

GENUINELY UNRESOLVED — the same defect as `current_session`
  current_consumer()               A-005
  add_to_cart(..)                  a handler body
  include_markdown(..)             a build-time page
```

A policy's KEYWORD is excluded and its call-shaped VALUE is not: `conflict` is in
the table, `merge_by_field` is what follows it. Both lower into the body
expression tree, so the checker cannot currently tell a declarative policy
expression from a term call.

**That is the remaining work, and it is one class rather than four.** Once a
policy value is distinguishable, the rule reports only real defects — and there
are at least three more of them in the accepted corpus besides the store's.

Two scoping fixes were already needed and found the same way. A bare call must
be checked against **both** namespaces, because `Session("")` is a call-shaped
constructor of an opaque type; checking only `Term` reported every constructor
in the platform. And `todo`, `resumable`, `query`, `Some`/`None`/`Ok`/`Err` are
language-supplied and need an explicit list.

So the rule is right and its implementation is not finished. Reverted rather
than landed over-broad or weakened to fit — weakening it is what the ruling
forbids, and landing it over-broad is how a real rule gets reverted by somebody
else later.

Three questions, and they are the architect's:

```text
1  is a bare call to an unresolved name an error? (widening `unresolved_uses`
   may reject corpus files, which is a corpus-version decision)
2  is "StorePage renders without authority" a claim to correct, or does
   reading a session key at render time belong somewhere else in the page?
3  does E8's evidence get regenerated under the corrected program, or does
   the correction wait for a corpus version?
```

The shape is the one this file exists for. It is not a wrong answer from a
wrong mechanism — it is **no question asked at all**, which is the failure mode
that survives longest, because every test that could have caught it was passing
for the reason it was designed to.

## Corpus harness rules

Architect ruling: rejection is insufficient; a program must be rejected **for
the specified invariant**.

```text
accepted program
  - parses
  - produces no error diagnostics

rejected program
  - parses
  - produces its DECLARED diagnostic code (aliases resolved to canonical)
  - produces it at the declared or a compatible span
  - is not counted as passing merely because another rule rejected it
```

**Accepted programs are checked before rejected ones**, so a false positive
stops the suite immediately. A rule that fires on a program the corpus calls
correct is worse than one that misses a violation.

Multi-defect fixtures must list every expected code explicitly. Otherwise each
rejected fixture should isolate one primary defect.

## RQ-6 — effect-family rejection coverage

For **every** effect family, three cases are required before the family counts as
implemented:

1. one accepted use;
2. one **direct** rejected use;
3. one rejected use **hidden behind a helper or generic callback**.

The third is the one that matters. An effect checker that catches
`element.offsetWidth` in a component body and misses it two calls deep inside
`measure_tooltip` is worth approximately nothing, and it is exactly the failure
that survives a green test suite. RQ-2 showed Koka handles this for its own
effects; the `pw` checker must be shown to do the same.

---

## Coincidental correctness

**Named 2026-08-07**, after five instances in one session:

> The compiler emits the right verdict, but the semantic information that is
> supposed to justify that verdict never actually flowed through the program.

`R-037` is the clearest: the diagnostic was correct, the code was correct, the
invariant was correct, and the mechanism had no connection to the claim. The
fixture would have passed identically had it written `fn(anything_at_all)`.

**None of the five was found by a test going red.** Each surfaced when a change
removed the mechanism propping it up — deleting a name-based fallback, moving
placement to inferred effects, requiring a capability argument to resolve. Which
ones were found depended on what happened to be changed next.

The admissibility rule below does not catch this, and neither does the
broken-path control: R-037's chain was intact in the sense that every link
existed. **The link that mattered was carrying no information.**

The countermeasure is ADR-0022 — semantic provenance, a five-check fixture rule,
and metamorphic perturbation testing. For Tier 1 analyses the admissibility
requirements gain a fifth entry:

| | |
|---|---|
| **provenance** | the conclusion can name the resolved facts that caused it, and a test asserts the required edges and the forbidden evidence sources |

## The recurring bug

Thirty-nine measurements in this project produced plausible, favourable results
while measuring nothing. The count is kept accurate deliberately: it is the
argument for the admissibility rule above.

| where | what it reported | what was actually happening |
|---|---|---|
| layout spike F-2 | 79.7 ms → 0.3 ms, looking like a fix | the write stopped invalidating layout, so no work was done |
| RQ-1 F-1 | 72 "component replacements" | the browser's own initial HTML parse |
| RQ-1 F-4 | "silent failure, no error signal" | resource-load errors do not bubble; only a capture-phase listener sees them |
| RQ-2 F-2 | a generic helper reported as **pure** | an effect-polymorphic row variable fell through to the "total" branch |
| E2 | `pw check` green on 68 files | 13 of them were malformed and no tool had ever read their bodies |
| E2 grammar | 56/68 files parsed; the other 12 were called "exotic forms" | a missing closer was consumed by a bare `eat` whose failure was discarded, so five files' lambdas never parsed at all. Making it an error found four real defects underneath |
| E2 grammar | `a >= b` parsed without complaint | it lexed as `>` then `=` and parsed as `a > (= b)` |
| E2 grammar | an error on `</main>` | the real fault was two lines earlier: `expr` crossed a newline into `<` and parsed markup as a comparison |
| E2 grammar | a statement parsed, the next one failed | a modifier loop crossed the newline and ate the next statement's first token |
| E3 adapter | the generated template looked correct | **Marko strips whitespace between elements**, so `<span>a</span> <span>b</span>` rendered as `ab`. The template-level test asserted the template, and the template was fine — only a browser saw the DOM |
| E5 placement | corpus coverage rose from 7 to 9 files | an effect family absent from the capability table was granted by **no** world, so two files were reported as unplaceable for a reason that had nothing to do with their actual defect. Right file, wrong rule, and the number went up while nothing was detected |
| E5 markup | corpus coverage read 20 files | `<form>` was missing from the list of interactive elements, so `<form on:submit={..}>` — the normal way to submit a form — was reported as an accessibility defect. It made R-022, a handler *type* mismatch, look caught. The real number was 19 |
| E2D | `R-012` was caught, and its code matched | it was caught for a **second** defect — its `setup` declared neither `dom.mutate` nor `layout.measure` — which masked the affine-resource leak the fixture was written for. A fixture that fails for the wrong reason is not enforced, so the fixture was repaired rather than the count banked |
| E2D | corpus coverage rose from 25 to 27 | giving an effect's type argument its place in the effect's *identity* was correct, but `covered()` split the family on `.` before stripping arguments — and `secret<Payments>` has no dot. Two fixtures were reported for an effect their own row declared. The 27 was worse than the 25 |
| registry | `PW0323` looked owned and consistent | it meant three things at once: the registry called it "a resource must declare how it is released" (nothing enforced that), `rules.rs` emitted it for a placement failure, and the corpus fixture declaring it is about placement. Had the rule and the fixture ever met, the ratchet would have counted a correct catch under a description of a different rule — and `declared_code == errored` **cannot see that**, because the code genuinely matches |
| registry | four gap entries read as open work | `PW3001`, `PW3002`, `PW3004` and `PW3008` were aliased onto `PW0401`/`PW0402` when the layout family landed, so nothing could ever resolve to the gap entries. They described work that was already done as unowned |
| E9C fuzzer | 69 covered regions for the whole compiler | the profile parser looked for `Function name: `, which `llvm-profdata` does not emit, so it matched nothing in the dependencies and measured only the harness. The real figure is 1,200-2,300 per target. Caught by the number being implausible for a parser plus fourteen analyses — not by a test |
| E7-P integration | the store demo's cart updated, and its resource declared the event that drives it | it did not. `add_to_cart` declared `emits CartChanged(..)` and `invalidates Cart(..)`; `Cart` declared no `invalidates_on`. Those are different statements — the command says which cache entry it drops, the resource says which fact about the world it depends on — and a materializer consumes EVENTS, so the event was emitted, committed, drained, and matched no listener. The Node stand-in had reimplemented the mapping in JavaScript, so the demo worked and the program did not say why. Found by wiring the real materializer to the browser |
| E7-P protocol | a `resource_changed` notice followed by the patch realizing it: both delivered, both handled | the notice advanced the held version, so the patch carrying the same version advanced nothing and was refused as stale. The page stayed at 0 while the log recorded a resource change and a patch. `held` is what the DOCUMENT reflects; hearing that newer state exists is a different fact and does not make the document reflect it |
| E7-R runtime | the store page's Add buttons were attached, and one of them worked | `TemplateSchemaId + LocalPartId` uniquely identifies a template part **definition**, not a document part **instance** — and a position inside `{#each}` has one instance per item, so all three Add buttons carry `data-pw="0"`. The runtime kept a `Map` from id to element and silently held the LAST one: the third button worked, the first two were inert, and clicking the first read as a page still loading. Found by a test that clicks `.first()`. The distinction is not about loops — the same gap appears with a component used twice, a conditional region recreated after toggling, and a streamed instance — so the fix is a generic `InstancePath`, not an each-specific one |
| E7 renderer | `aria-label="Menu"` rendered correctly, as the template said | the HIR keeps an attribute value AS WRITTEN, delimiters included, because it is a faithful record of the source. The template IR passed it through, so the document carried `aria-label="&quot;Menu&quot;"` — a list whose accessible name a screen reader announces with the quote marks. The bytes looked like an escaped value doing its job. Caught by a DOM assertion reading `getAttribute`, which no amount of reading the output would have produced |
| E7 golden suite | Marko PASSED "no handler code loaded before interaction" | RQ-1 had already falsified that property for Marko. The new check filtered request URLs by `/handler|interaction/` and Marko's bundle is not named either — so the detector read a NAMING CONVENTION and matched nothing, which is indistinguishable from finding nothing. Caught by the result contradicting a measurement the project had already made; a suite without that prior would have recorded a pass |
| E6F witnesses | `task_detached/neighbour.pw` was valid evidence: "parses without recovery" | the validity pipeline asked the LEGACY parser whether the file parsed, and every analysis ran on the tree parser. It wrote `task.spawn(scope component)` — a policy-clause spelling inside an argument list, which is not the language — so the real grammar never parsed it. The valid neighbour proving the rule does not ban all spawns reported nothing because nothing had read it, and the pipeline built to catch invalid witnesses declared it valid |
| E6 corpus | A-009 passed as the "edge materialized menu" fixture | a `materialize` block writes its policies INSIDE its braces, the grammar parsed that block as an expression body, and `decl.policies` was empty for every materialization in the corpus. All four of A-009's graph edges pointed at nothing. The fixture demonstrated the SYNTAX of a dependency graph and none of its semantics, and it passed because no analysis had ever read those clauses — the C0→C1 repair arriving in a second place, unnoticed for the same reason |
| E6 graph | every page's dependency on its own query resolved | it resolved to the TYPE of the same name. `store.page` declares `query Store` and imports `domain.Store`, and the general `resolve` tries the type namespace first, so every page edge pointed at a record definition. The graph was full, the paths were plausible, and no edge named a thing that could be invalidated |
| E7/E9 inference | every checker resolved sibling declarations, because the API offered a way to say so | the module was a **builder step** — `Types::of_body(..).in_module(m)` — and four of the seven callers never called it. Those four silently inferred with no module, so a bare name never resolved to a sibling and rules that depend on a type went quiet rather than wrong. Found only because a new rule needed to resolve `query Menu(..)` **during** construction, where no builder step had run yet. The fix is not a seventh call site: the module is now a parameter of `of_body`, so a caller cannot forget it and there is no order to get wrong |
| E2 grammar | an audited `unsafe` was reported as unjustified | the newline rule that ends a statement also ended `unsafe capability … because "…"` before its `because` clause, so the justification became a separate statement and never reached the declaration. **`pw fmt` then baked the misparse into the source**, which is the part worth remembering: a formatter faithfully renders a wrong parse |
| E7-P materializer | every E6 test passed, and the browser suite failed "sometimes, under load" | `Materializer::drain` consumed **every** pending outbox event and invalidated only those matching the instances the CALLER supplied. A caller's instance set is what that caller happens to know about — one session's entry — never everything that exists, so a drain issued for session A consumed session B's event and left a `Consumed` trace claiming the work was done. Invisible to every single-instance test, because with one instance the supplied set IS the whole set. It was written off as flakiness for one commit, which is the part worth remembering: **"it fails occasionally under parallelism" is a description of a race, not of a flaky harness** |
| E7-P transport | the long poll delivered frames, and a reloaded page silently stopped receiving them | frames were REMOVED from the queue when read. A page that reloaded left an in-flight poll behind; that request's thread woke, took the frames the new page had not yet asked for, wrote them to a socket nobody was reading, and returned. Reading is not delivery. Fixed by making the client's next request its acknowledgement of the last sequence it APPLIED, so writing to a dead socket loses nothing |
| E7-P transport | the cursor protocol worked for every subscriber that had ever received a frame | sequences started at zero and the initial cursor was zero, so the very first frame a subscriber ever received was numbered zero, `since=0` read it as already acknowledged, and it was never delivered. Every test that sent two things passed. An off-by-one at the exact boundary where nothing has happened yet is invisible to any fixture that warms up first |
| E7-L resume | `resumable() => clear_cart()` rendered a button that could never work | a resume manifest was only generated when the handler CAPTURED something, so "not resumable" and "resumable with nothing captured" were one test. A handler that needs nothing from the document got no identity, could not be authorised, and attached nothing — and the page looked complete. The compiler said nothing because, as far as it was concerned, there was no resumable handler there to say anything about |
| E7-L resume | the second handler was refused with code 9, malformed | the runtime built a capture schema as `of_fields(&[(name, "T")])`, and for a handler with no captures that is a schema of ONE field named `""` rather than the schema of no fields. `decide` correctly refuses a manifest carrying no capture bytes under a non-empty schema, so a handler that captures nothing was malformed by construction. Fail-closed worked and the only symptom was a button that never attached |
| E7-L host | `add_to_cart` incremented the cart correctly in every test | the read sat OUTSIDE the lock: `let next = cart_value(session) + 1;` then commit, then write. Two presses landing in the same instant both read 0, both computed 1, and one was lost. Every test that clicks once and waits passes; the test that found it clicks TWICE CONCURRENTLY, and it was written to prove something else entirely — that a handler module is fetched once rather than twice |
| E8-0 contract *(fixed)* | `Resources.Cart` required `database.read`, which read like exactly the right answer for a cart query | its body is `todo` — it performs nothing. `Inference::known` is keyed by the BARE declaration name, so `Resources.Cart` and `store.page.Cart` share one entry, and the contract handed one component the authority of a different component that happens to have the same name. Every capability in the emitted set was plausible for the component it was attached to, which is why reading the output did not find it; deriving from the BODY did |
| E8 resolution | R-037 was caught, with the right code, for the right invariant: `layout.measure` smuggled through `List.map`'s callback | its callback parameter was never bound. `fn(el) ..` writes the parameter as a declaration-shaped `Param` holding a `Name`, and `param_pattern` handled only the `x => e` spelling — so `el` lowered to `Pattern::Error` and had no type. The effect was found by matching the spelling `getBoundingClientRect` against every declaration in the program, which would have worked equally well if the fixture had written `fn(anything_at_all)`. The fixture tests that effects propagate through a generic callback, and nothing was propagating through anything |
| E8 corpus history | `the_pre_change_text_of_every_repaired_fixture_still_fails` passed for R-001 and R-024 | both C0 texts call a function they never import, so the call names nothing the program declares. They became catchable only through the same by-spelling matching — a test asserting "this historical defect is still detected" was itself relying on ambient resolution E2B had removed. Deleting the fallback made both go silent, correctly, and they are now recorded as `FixtureDidNotExpressIt` with the reason |
| E8 capability | `style.mutate<LayoutAffect>` distinguished a layout-affecting write from an ordinary one, in five corpus files, and `style.pw`'s own comment called the type argument "the point" | `LayoutAffect` was declared NOWHERE. The distinction rested on a spelling no checker could resolve, so `style.mutate<LayoutAffect>` and `style.mutate<Anything>` were the same effect to every analysis that read the row. Two fixtures, an accepted example and two generality witnesses all wrote it. Found by `PW5200` on its first run against the real corpus — the rule was written for a typo and found a five-file convention naming nothing |
| E8 audit | the privacy checker read a page's label from every declaration in the program sharing the query's spelling, and had never produced a wrong answer | it JOINS rather than picks, so an unrelated `Cart` could only make a page look MORE private — the safe direction, which is exactly why nobody looked. It was still RISK_QUEUE 34's shape, and the fixtures that appeared to prove cross-module labels worked (R-004's C0 text, `cross_file.rs`) were passing through it. Repairing it exposed a second defect underneath: `query Cart(s)` resolved to the TYPE `Cart` rather than the query, because the general resolver tries the type namespace first — the E6 defect, in a third place |
| E8 placement | `generality/no_feasible_placement/caught.pw` proved the analysis reaches "two capabilities no single world grants" — an INDIRECT, general shape rather than a declaration the checker could read off | its body called only `Stores.get`, so `device.location` existed in the annotation and nowhere else. The witness proved that a checker reading a DECLARED row reports a declared row. It went silent the moment placement started reading what the body does, which is the only reason anyone found out. Repaired by making the body perform both effects |
| E8 placement *(fixed)* | `secret<Payments>` — the corpus's most-used secret effect, twelve uses — was constrained to the origin by the placement solver, as charter §1.7 requires | it was not. `World::grants` did its OWN `split('.')` without stripping the type argument, and `secret<Payments>` has no dot at all — so the family was the whole string, `worlds_for` returned `None`, and every world including the browser and build time granted it. `effects::family_of` strips first and carries a comment saying exactly why; this was a second implementation that did not. It never showed because `forbidden_in` and `secret_to_browser` catch a secret reaching the browser through rules that DO strip: defence in depth hid a hole in one of the layers, and every corpus verdict stayed correct while one of the three checks was answering `true` for everything. Found by `tests/contract_matrix.rs` on its first run — an instrument written to freeze today's behaviour BEFORE changing it, which is the only reason it was seen rather than silently repaired by the change it was built for |
| E8 frame phase *(fixed)* | `R-042` proved that measuring inside `post_paint` is caught — the fixture writes `post_paint { let h = measure { self.height() } }` and the diagnostic named exactly that | `self` has no resolved type, so `self.height()` named no declaration and performed nothing. The `layout.measure` came from the `measure` KEYWORD, which synthesized it at the keyword's own span — and `phase_at` then attributed that span to `post_paint`, which is the only reason the outer phase was consulted at all. Right verdict, right code, from a made-up effect recorded in a convenient place. Found by deleting the synthesis on the architect's ruling that a phase says WHEN work runs rather than what it does: the fixture went silent, and the repair needed both a real geometry read AND a phase rule that reads the whole enclosing chain instead of the innermost phase |
| E8 resolution *(fixed)* | `measure(el)` inferred `layout.measure`, which is `style.measure`'s declared row | it never resolved to `style.measure`. `measure` is a frame-phase keyword, so the parser makes `measure(el)` a keyword expression rather than a call, and `intrinsic_effect` supplied the effect anyway. `set_width(s, ..)` beside it — same import, not a keyword — resolved normally. No corpus file calls a phase-keyword-named function bare, so nothing depended on it; found when a matrix row written to exercise the platform's own `measure` came back empty |
| E8 frame phase | `forbidden_in_phase("animate", "layout")` says a compositor animation may not force layout, and its comment describes animating a layout-affecting property | it keys on an effect's FAMILY, and `style.mutate<LayoutAffect>` has family `style` — so the case the comment describes is the one case the rule cannot see. `layout.rs` matches the written form and can, which means the distinction is expressible and this rule works one level too coarse to use it. No corpus fixture, so nothing was measuring it |
| E10 policy values | a page's declared authority and allowed placements are derived from what its body does | they are derived from what its body TEXT contains. `key helper(a)` is a policy value, not executable code, and it contributes `database.read<Carts>` to the page's `required_capabilities`, narrows `allowed_placements` to `origin`, and makes `PW0401` fire — identically to the same call rendered in the markup. Naming a `session query` in a policy value is enough to trip `PW5001`. Four of six consumers cannot distinguish the two positions, and the one that decides what a host grants is among them. Frozen in `tests/policy_consumers.rs` before the `PolicyExpr` split |
| E10 placement | `allowed_placements` is the placement solver's answer for a component, so a component pinned to one world ships a contract naming it | `contract.rs` builds its `Demand` with `label: Label::public()` and `declared: None` — four lines under a comment saying *the solver also weighs privacy labels, and a second derivation here would agree until a label mattered*. `check.rs` builds the same `Demand` with the real label and the author's pinned world. So `pw check` refuses a bad placement correctly while the artifact a host grants placement from ignores both inputs: A-013 pins `placement build` and its contract permits the browser, the edge and the origin; A-015, A-020 and A-023 pin `placement browser` and their contracts permit build time. One fact derived twice, agreeing until an input mattered — the `secret<Payments>` shape, in the artifact rather than in a rule. Found by the evidence-reachability audit asking which observer could witness A-013's build-time claim |
| E10 evidence | the accepted corpus is 24 fixtures the compiler accepts, each evidence for its stated subject | 9 of the 24 cannot witness their own claim. Two are `Unqueryable` — `exhaust::check_match` returns early on `Proven` and its HIR→pattern lowering is private, so an audit can only observe the absence of a diagnostic, which is what an unexercised checker also produces. Seven are `Missing`: A-011's conflict policy is recorded by nothing at all; A-013's build-time claim has no observer; A-015/A-018/A-020/A-021/A-023 claim positive scheduling facts and produce an empty effect set, indistinguishable from A-001, whose claim IS emptiness. Frozen in `tests/evidence_reachability.rs` with the A-014-shaped negative control the architect asked for |
| E10 grammar *(fixed)* | `for (i, v) in xs.enumerate() { .. }` is a loop | it is `Expr::Call` with the callee `Name("for")`. Every analysis that walks calls sees a call to something that does not exist, and — worse — the loop variable is bound by nothing, so `for badge in badges { badge.style.set_width(u) }` makes `badge` look like an undeclared name. `resolve::local_bindings` handles `{#each xs as x}` correctly, which is what says the gap is this form and `draw(ctx) { .. }`, not the function. `Expr::Keyword`'s exact twin: there a call is demoted to syntax by its spelling and contributes nothing; here syntax is promoted to a call by its position and contributes a callee. Found by the semantic-ownership gate, which could not classify `ctx.rect` or `badge.style.set_width`. Repaired 2026-08-10 on the architect's ruling that syntax must remain syntax: `K::ForExpr` → `Expr::For { pat, iterable, body }`, and the pattern binds. `draw(ctx) { .. }` followed on 2026-08-11: a block policy's header is a `ParamList` and its parameters are the binders of a `TermRoot`, so `a_loop_and_a_policy_block_both_bind` is now an affirmative proof rather than a failing-forward one |
| E10 placement *(fixed)* | a `page` or `component` that writes `placement browser` has pinned a world, and `PW5005` refuses an effect that world cannot grant | **`PW5005` could not fire for any UI declaration.** `declared_world` reads `decl.policy("placement")` first and falls back to a body scan for exactly this case — and the fallback did not work, because a UI declaration's `placement browser` lowered as two unrelated bare `Name` statements. So a component pinned to the browser and reading the database was reported by `PW5002` *nowhere can run this* rather than by the rule that names the pin. Found on 2026-08-11 when UI declarations began parsing their policies inside their braces and `R-002`'s code changed under a test asserting it. Two more consequences surfaced with it: `routes::table` read `route "/…"` only from the body, so `R-023` lost its catch entirely; and `generality/forbidden_effect/build-page-clock.pw` began firing a second, correct diagnostic it had never been able to fire |
| E10 backend | `Instr::HostCall { capability, args }` names a call across the capability boundary, so the encoder can emit one core import per capability | **a capability is not a function.** `add_to_cart` and `clear_cart` both call `database.write<Carts>` — through `Carts.add(s, item, qty)` and `Carts.clear(s)`, two Pleris functions requiring one authority — so the IR carries the same capability with three arguments at one site and one at the other. A core import has ONE signature. Encoding against the first arity found produced a module `wasmparser` rejected with *"expected i32 but nothing on stack"*, and the second attempt, which dropped the ambiguous import, produced *"exported function index out of bounds"* because the function index was computed from the DECLARED import count rather than the emitted one. `HostCall` conflates **which function is called** (a `DefId` with a signature) with **what authority the call needs** (a `CapabilityId`); the encoder needs the first and the IR carries only the second. Found on the first run of the first encoding slice, exactly as the architect predicted the encoder would find it. The encoder refused rather than mis-encoding, and the model was repaired on the architect's ruling of 2026-08-20: **a capability authorizes an operation and does not identify one.** `Instr::ImportCall` names an `ImportId`; a `CallableImport` carries the ABI and a SET of required capabilities; and a host implementation is now explicit declaration metadata — `host "store:data/carts#add"` — never inferred from a `todo` body or from an effect row. (It was `pw:host/carts#add` for one commit; the ownership ruling later that day moved the application's own operations out of the platform package.) `an_effectful_function_without_a_host_binding_is_an_ordinary_call` is the control that proves the conflation was removed rather than a field added |
| E10 ABI | a privacy label is a type like any other, so a signature returning one crosses a component boundary the same way | **a privacy label has no ABI, and a hand-written fixture chose erasure.** `pw:host/session#read` returns `Session<SessionId>`, declared `opaque type Session<S> = String`, and the WIT generator refuses it: `Unmappable { ty: "Session<SessionId>" }`. Its own comment says what it is for — *the scoping labels, as declared types […] what lets the privacy checker read a label instead of inferring one from a function's name*. The deployment's stand-in publishes `read: func() -> string` for the same operation, which is not a mapping but an **erasure**, and nothing decided it: a fixture picked a type and no check compared it to anything. It never surfaced before because no EXPORT returns a label, so the generator never met one — it appears the moment host operations are rendered, which makes it the ABI layer's finding rather than the checker's. Three answers are possible and none is taken: the label has a representation that crosses; the erasure is declared; or an operation returning a label may not cross a component boundary at all. Refusing is the only one available today and is at least not silent. Pinned by `an_operation_returning_a_privacy_label_has_no_wit_form` |
| E10 ABI *(fixed)* | `backend::host_binding` reads the `host` policy, and one reader means one answer | **an `effect` declaration carries the same clause and is not a callable.** `effect session.read { host "pw:host/session#read" }` and `fn current_session() -> Session<SessionId>` both claim `pw:host/session#read`, so a reader that walks all declarations answers with whichever it met last — and `wit::host_signatures` did exactly that, rendering the effect's empty `read: func();` over the function's real signature. A wrong answer shaped like a right one, in code written the same hour to expose a different instance of the same disease. Two repairs: `host_binding` refuses a `DeclKind::Effect`, because a capability authorizes an operation and does not identify one — the effect's clause is the residue of the deleted `interface_for(capability, ontology)`, and the ontology still reads it into a field **nothing consumes**; and `host_signatures` collects claimants first and returns `WitError::Claimed` for a second one rather than resolving by order. Removing the clause from the effect vocabulary is not done and is the honest end state |
| E10 ABI | a host operation has one ABI, and the WIT resolve proves the deployment agrees with the contract about it | **it is derived twice and nothing compares the two.** The contract fixes `store:data/carts#add : (SessionId, MenuItemId, PositiveInt) -> Result<Cart, CartError>`, read off the Pleris `fn` that carries the `host` binding. The deployment publishes `add: func(session: string, item: string, quantity: s64) -> string`. All **six** of the store's host operations disagree on their result type, and every gate is green: `wit_parser` resolves the worlds because the resolve checks that the interface exists and that every type a world names is declared — the contract is not an input to it — the artifact audit is satisfied because the NAMES match, and `contract::consistent` is satisfied because the capabilities are untouched. It is precisely what `contract::abi` refuses one layer in (*same `interface#operation`, wrong ABI*), with the second signature being somebody else's artifact rather than a mutation of ours. **Not a regression**: the contract gained `signature` on 2026-08-20, so before that there was nothing to compare — this is a check that became possible and does not exist. **And the Canonical ABI does not catch it either — it hides it.** The two flatten to the SAME core signature: `(SessionId) -> Result<Cart, CartError>` and `(string) -> string` both give `[Pointer, Length, Pointer]` with a return pointer and no core result, because `SessionId` is a `string` alias and a `result<record, variant>` and a `string` alike exceed the flat limit and return through a pointer. All six coincide this way. So **no core-level check can find this**: not the validator, not the import types, not a signature comparison in the encoder, because at the core level there is nothing to find. The disagreement lives entirely in the COMPONENT types, where the host lifts three pointers as a `string` while the guest meant a `result<cart, cart-error>` — the same bytes read as different shapes, with no trap and no diagnostic. I predicted the flattenings would differ and asserted it; they do not, and the correction is the substance: the check must compare component types and cannot be deferred to the validator. Same shape as the `secret<Payments>` and placement-`Demand` rows — **one fact derived twice, agreeing until an input mattered** — except here the input already mattered. Which derivation is authoritative is a model question and turns on `Import::owner`: for `store:data/*` the application declared the operation, so the compiler arguably should EMIT that WIT; for `pw:host/*` the platform publishes it and the Pleris declaration is a claim to be CHECKED. That the answer differs by ownership is why that distinction had to land first. Pinned by `tests/canonical_abi.rs`, which asserts the disagreement so the repair turns it red |
| E10 methodology | a consumer that handles the declaration kinds it knows about is complete | **"consumer enumerates known declaration kinds" is a recurring failure pattern**, demonstrated three times in one slice. `routes::table` read `route "/…"` only from a page's body, so `R-023` lost its catch entirely and every internal link was checked against an empty table. The semantic-ownership gate walked only from `body.root`, so when block policies became roots it stopped examining `draw`/`acquire`/`release` — and every owner class still looked non-empty. `pw explain` had an arm per `DeclKind`, so `replicated` and `paint` policies became invisible the moment they became real, breaking charter §14 M4's gate. The shape: **a reader that lists what it knows about is a reader that silently drops what it does not**, and in all three cases the corpus still checked clean. Architect ruling, 2026-08-11: do not hunt for every instance before encoding — if the encoder exposes one concretely, stop and classify it then. **A fourth instance, 2026-08-20, and the first found by a rename rather than by a gate.** `a_world_never_imports_a_capability_its_contract_does_not_require` skipped every world import not starting with `pw:host/`, which was every host import there was — until the store's data operations became `store:data/carts#…` under the ownership ruling. The test then examined nothing and stayed green, so a world importing an operation its contract never named would have passed the check written to refuse exactly that. Same shape at a fourth site and a fifth spelling: **a reader that lists the prefixes it knows about is a reader that silently drops the ones it does not**, and the renaming that created the gap is the sort of change nobody re-reads a test for. Repaired by subtraction rather than by extending the list — a world import is a component dependency (the generator's own `-api` names, which it can recognise exactly) or it is a host operation — plus a non-vacuity floor, because the failure mode here was a loop that ran zero times |
| E10 resolution *(fixed)* | the `measure(el)` keyword-expression row above is marked *(fixed)* | the wrong answer was removed; the hole was not. An identifier-led call whose spelling is in `STMT_KEYWORDS` still lowers to `Expr::Keyword` instead of `Expr::Call`, and inference contributes **nothing** for a `Keyword`. So `key helper(a)` charges a page for a database read and `key measure(a)` — same syntax, same position, same declared row on the callee — charges it for nothing. The earlier fix replaced a fabricated effect with silence, which is quieter and no more correct. Found while building the policy-consumer audit: the first draft named its helper `measure`, measured silence, and would have frozen *policy values are invisible to inference* as the answer. Repaired 2026-08-11 on the architect's ruling that **a keyword spelling may select a syntactic production only when the tokens actually match that production**: a statement keyword followed by an argument list and no block is an ordinary call, so `measure(el)` resolves normally and `measure { .. }` stays a phase. Treated as blocking before codegen, because the Backend IR would otherwise faithfully encode a program whose effects the front end never established |

The two E6 rows and the inference row share a shape with the by-name member
fallback deleted in E2C, arriving through three different doors. The E6 graph
row is the sharpest: a wrong answer that is *shaped like* a right one — a real
path, to a real declaration, of the wrong kind — is harder to see than no answer
at all, because every summary count looks correct.

The inference row: **whether a correctness analysis ran at
all depended on something other than the program.** There it was a global
accident of spelling; here it was whether a caller remembered a second method
call. The countermeasure is the same one, and it is not a test — it is removing
the way to express the mistake. A test asserting "every caller calls
`in_module`" would be a lint that a new call site can be added without.

Four of the grammar rows share one shape: **the diagnostic pointed at the line
after the defect.** A parser that recovers silently moves the blame downstream,
which is why the errors read as "exotic corpus syntax" rather than as parser
bugs. The countermeasure is in the E2 tests — every construct asserts its tree
*shape*, so a wrong parse fails where it happens instead of somewhere plausible.

Five share a different and more dangerous shape — the two `E5` rows, `R-012`,
the 25-to-27 rise, and `PW0323`: **the number went up.** Coverage rising is the
signal everyone is watching for, and in each case it rose, or would have risen,
while detection got worse or stayed the same. Two countermeasures now exist:

- **three numbers, not one** — produces an error / emits the declared code /
  fully enforces the declared invariant — with `declared_code == errored`
  asserted as an *equality* rather than a floor, so a catch that is merely red
  is a regression even when the count rises;
- **registry tests that fail closed** —
  `every_registered_code_is_one_a_checker_can_emit` rejects a code the registry
  claims and no checker emits, and the known-gap test now rejects an entry the
  alias table has made unreachable.

Neither countermeasure would have caught the `PW0323` case, which is worth
stating: the code matched, the fixture was uncaught, and only reading the
registry against the corpus by hand found it. That is the residual risk.

Almost all were caught by looking at a raw number and asking whether it was
plausible — never by a test going red.

**Rule adopted**, promoted by the project architect into a general
admissibility standard:

> **A measurement or checker result is not admissible evidence until its
> instrument has a negative control proving it can detect the corresponding
> failure.**

Every gate needs four things:

| | |
|---|---|
| **positive control** | the expected-good case passes |
| **negative control** | an intentionally broken case makes *that exact detector* go red |
| **plausibility check** | raw values fall in a physically or semantically credible range |
| **raw evidence** | the unprocessed trace or output is archived |

For parsers and metadata readers, add **mutation tests**: remove an effect row,
change a constructor tag, omit a source span, misclassify an open row as total,
change an expected import. The reader must fail closed or make the test red.

This applies to benchmarking, compiler gates, browser instrumentation,
capability auditing, and the E→P evidence ledger — not only to unit tests.

**And a second standard, adopted 2026-08-11:**

> **Subsystem green is not project evidence green.**

Every crate's tests passed individually while `pw explain` — an externally
observable semantic gate the charter names — was broken for nine policies.
`just ci` is the unit of evidence, not `cargo test -p <crate>`, and a claim that
something passes must name which of the two produced it.

The five instances so far are why. Two flattered the system, one damned it, and
one (`.kki` reporting polymorphic rows as total) would have let a *gate* pass
while hiding the case the architect had flagged as most important.
