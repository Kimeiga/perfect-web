# Corpus versions

The charter §16 corpus is the executable specification. Once a conformance
figure is published against it, it stops being a floating collection of files
and becomes a **versioned artifact** — otherwise a fixture can be edited into
compliance and a previously discovered failure quietly disappears.

Architect ruling, 2026-08-06:

> Keep two distinct measurements. Current-corpus conformance tells you whether
> today's specification passes. Historical-corpus compatibility prevents a
> fixture from being edited into compliance and silently erasing a previously
> discovered failure.

---

## C1 — frozen 2026-08-06

```text
Corpus version:            C1
Accepted programs:         24
Rejected programs:         44
Charter categories:        24/24 accepted, 44/44 rejected
Corpus commit:             324148b
Compiler commit:           324148b
Result:                    24/24 accepted clean, 44/44 rejected enforced
Wrong-reason catches:      0
Modified since C0:         10 fixtures (below)
Generality at freeze:      7/29 invariants generality-tested
```

Reproduce: `just ci` for the gate, `just evidence-corpus` for the per-fixture
table, `just generality` for the second score.

**C1 is frozen. C2 and C3 both opened on 2026-08-06; C4 opened 2026-08-07** — see below. A change to
any file under `examples/accepted` or `examples/rejected` opens the next version
and requires a row in its table plus a recorded reason. The rule exists because the path to 44/44 changed ten
fixtures, and a reader who does not know that will read the number as stronger
than it is.

### C0 → C1: every changed fixture

`C0` is, **per fixture**, the text immediately before the change that let the
compiler enforce it — not a single global commit. That distinction was learned
the hard way and is recorded under *Historical compatibility* below. For each
change: what it was, why, whether it repaired **missing program context** or
**altered the invariant**, and the compiler's result before and after.

| fixture | change | kind | before | after |
|---|---|---|---|---|
| R-001 | `import Stores.{ fetch_store }`, and `fetch_store` declared in `examples/lib/Stores.pw` with `!{ network.fetch }` | missing context | silent — the call resolved to nothing, so no effect was inferred | `PW0401` |
| R-004 | `import store.queries`, `import cart.queries` | missing context | caught *through* an ambient module union that E2B removed | `PW5001` |
| R-010 | `import domain.{ DatabaseConnection }`, `import Database`; `DatabaseConnection` declared with `Database.connect` acquiring it | missing context | silent | `PW5008` |
| R-012 | `import Maps`; effect row corrected to `!{ resource.acquire<MapHandle>, dom.mutate, layout.measure }` | **second defect removed** | caught, but for undeclared effects — not for the affine leak it specifies | `PW2005` |
| R-022 | `import events.{ PressEvent }` | missing context | silent | `PW0602` |
| R-023 | a `page StoreOverview` declaring `route "/stores/{id}"` added to the file | **declaration added** | silent — with no declared route, "matches no declared route" is not a statement about anything | `PW5009` |
| R-024 | `import html.{ raw_html }`, and `raw_html` declared with `!{ unsafe.raw_html }` | missing context | silent | `PW5010` |
| R-025 | `import device`, and `device.current_location` declared with `!{ device.location }` | missing context | silent — the placement rule existed and was registered, and had nothing to compare | `PW5005` |
| R-030 | `import domain.{ Cart }`, `import cart.queries` | missing context | silent | `PW5007` |
| R-041 | `import document`, and `document.pw` declared | missing context | silent | `PW0401` |

Nine of the ten are the same repair: **the fixture called something that
existed nowhere**, so the call resolved to nothing, no effect was inferred, and
a rule that in several cases already existed could not fire. The fixture was
silent because the *program* was incomplete, not because the checker was.

Two are not that, and are called out separately because they are the ones a
sceptical reader should look at hardest:

- **R-012** had a second, real defect that masked the one it was written for.
  Removing it made the fixture test its declared invariant instead of an
  unrelated one. This *narrowed* what the fixture proves.
- **R-023** gained a declaration rather than an import. Its invariant is
  relational — a link is dead relative to a route table — and the file had no
  route table.

**No change weakened an invariant.** Every `@expect-error` line in C1 is the
one C0 had; the diffs above add imports and declarations and remove one
unrelated defect. `a_caught_file_conveys_every_fact_its_expect_error_lines_declare`
holds the fixtures to that text, so a future edit that softened an expectation
would have to soften the assertion too, visibly.

### Historical compatibility

`compiler/pw-core/tests/corpus_history.rs` runs the **C0 text** of every changed
fixture against today's compiler and asserts it is still caught **for its
declared invariant** — not merely that it is still red. That distinction is not
theoretical: the first version of this test used a single global baseline
commit that predated the corpus being well-formed, so R-025's old text failed
to *parse*, and the test counted a syntax error as historical compatibility.
The baseline is now per-fixture — the text immediately before the change that
enabled enforcement — and the assertion is on the invariant symbol.

Result: **nine of the ten repairs did not remove the defect.** The old text is
still caught, for the same invariant, without the import or declaration that
was added. That is the evidence the repairs were obstruction-removal.

| fixture | its C0 text today |
|---|---|
| R-001 | `forbidden_effect` |
| R-004 | `private_in_shared_cache` |
| R-010 | `unserializable_capture` |
| R-012 | `affine_not_consumed_once` |
| R-022 | `handler_signature_mismatch` |
| R-023 | **clean** — classified `FixtureDidNotExpressIt` |
| R-024 | `unsafe_audit_incomplete` |
| R-025 | `declared_placement_cannot_grant` |
| R-030 | `private_in_resume_manifest` |
| R-041 | `forbidden_effect` |

The one miss is **classified**, not rounded away. `EXPECTED_TO_PASS` carries
one of four judgements per entry, and they mean different things:

| classification | meaning |
|---|---|
| `FixtureDidNotExpressIt` | the old text did not contain the violation it declared. Benign — the corpus got more precise. |
| `SpecificationChanged` | the language or charter changed. Needs a charter reference. |
| `CompilerRegressed` | it used to be caught and is not. A **defect**. |
| `KnownCheckerGap` | the checker cannot see it. A **defect**, and must also exist as a `slips-through.pw`. |

The last two are rejected by the test rather than accepted as explanations: a
regression must be fixed, and a checker gap must be tracked where gaps are
tracked. Only the two benign classifications may sit in the list quietly.

**R-023 is `FixtureDidNotExpressIt`.** `dead_internal_link` is a *relation*
between a link and a route table, and the old file contained one half of it.
Its silence is correct.

The history suite is kept because it can **disagree** with the current corpus.
Forcing it to 10/10 would remove the only thing it is for.

The C0 texts live in `examples/history/C0/`.

---

## Three separate test bodies

They answer different questions and must not merge. Folding the new programs
into C1 would make the specification drift every time an implementation is
tested for generality — which is precisely the failure mode versioning the
corpus was meant to prevent.

| suite | where | question |
|---|---|---|
| **C1** — executable specification | `examples/{accepted,rejected}` | does the compiler conform to the declared corpus? |
| **G1** — generality challenges | `examples/generality/` | do the same guarantees survive programs the fixtures did not anticipate? |
| **Robustness** | `compiler/pw-core/tests/robustness.rs`, `examples/robustness/` | can an unanticipated program shape crash the compiler or suppress its output? |
| **History** | `examples/history/` | does the pre-change text of a modified fixture still fail? |

C1 is frozen. G1 grows freely — it is not a specification, it is a record of
what has been challenged. Robustness grows by regression: every panic found
keeps its original reproducer, a minimized one, the phase that crashed, the
fix, and a negative control showing the old implementation fails.

---

## C2 — opened 2026-08-06 (E6)

```text
Corpus version:            C2
Accepted programs:         24
Rejected programs:         44
Charter categories:        24/24 accepted, 44/44 rejected
Result:                    24/24 accepted clean, 44/44 rejected enforced
Wrong-reason catches:      0
Modified since C1:         7 fixtures (below)
Generality at open:        29/29 invariants generality-tested
```

Reproduce: `just ci` for the gate, `just evidence-corpus` for the per-fixture
table, `just generality` for the second score.

### Why it opened

**E6 resolved policy clauses for the first time.** `depends_on`,
`invalidates_on`, `emits` and `invalidates` name declarations, and until this
milestone nothing checked that the declarations existed. Seven fixtures named
resources and events that no file in their program declared.

That is the same shape as the C0 → C1 repair — *the fixture referred to
something that existed nowhere* — arriving in a second place, and it went
unnoticed for the same reason: a clause nobody resolved cannot be reported as
unresolved. Nine of ten C1 changes were calls that resolved to nothing. These
are edges that resolved to nothing.

It matters more here than it looks. An edge to nothing is not an error at run
time, it is **silence**: the fragment never regenerates, the page it produces
stays valid, well-formed and permanently out of date, and no test of the page
can tell that apart from a fragment whose inputs never changed.

### C1 → C2: every changed fixture

| fixture | change | kind | before | after |
|---|---|---|---|---|
| A-003 | `import Events.{ StoreChanged }` | missing context | `invalidates_on StoreChanged(id)` named nothing | resolves; `PW5100` clean |
| A-005 | `import Resources.{ Cart }` | missing context | `invalidates Cart(..)` named nothing | resolves |
| A-008 | `import Events.{ MenuChanged }` | missing context | `invalidates_on MenuChanged(id)` named nothing | resolves |
| A-009 | `import Resources.{ Store, Menu }`, `import Events.{ MenuChanged, InventoryChanged }` | missing context | **all four** graph edges named nothing — the fixture for edge materialization described a graph with no edges | resolves |
| A-010 | `import Resources.{ Order }` | missing context | `invalidates Order(order)` named nothing | resolves |
| R-017 | the same imports, plus `code_version included_in_key` | missing context + **second defect removed** | caught for `PW0401`, and would now also emit `PW5102` | `PW0401` alone |
| R-029 | `import Resources.{ Cart }` | missing context | `invalidates Cart(..)` named nothing | `PW0327` alone |

Six of the seven are imports. The two that are not:

- **R-017** gained `code_version included_in_key`, which removes a *second*
  defect the fixture did not declare. Same repair as R-012 in C1, and it
  narrows what the fixture proves: it is about the wall clock, so it must be
  caught for the wall clock.
- **A-009** is the one worth looking at hardest. Its category is "edge
  materialized menu" and every edge it declared pointed at nothing, so the
  fixture demonstrated the SYNTAX of a dependency graph and none of its
  semantics. It passed C1 because no analysis had ever read those clauses.

**No change weakened an invariant.** Every `@expect-error` line in C2 is the one
C1 had.

### New: `examples/lib/Resources.pw`

The library gained the four resources the corpus's policy clauses name —
`Store`, `Menu`, `Cart`, `Order`. Library files are not corpus fixtures and do
not change the 24/44 counts; they are the world the fixtures refer to, the same
role `Stores.pw` and `Carts.pw` have had since C1.

`A-003` and `A-004` keep their own `Store` and `Cart`. They are the fixtures
that demonstrate how such a query is *written*, and a fixture referring to
itself would prove nothing about resolution across files.

---

## C3 — opened 2026-08-06 (E6, on the architect's ruling)

```text
Corpus version:            C3
Accepted programs:         24
Rejected programs:         46
Charter categories:        24/24 accepted, 46/46 rejected
Result:                    24/24 accepted clean, 46/46 rejected enforced
Wrong-reason catches:      0
Added since C2:            2 fixtures (below)
Generality at open:        30/31 invariants generality-tested, 1 known narrow
```

### Why it opened

Architect ruling, 2026-08-06:

> A new user-facing language invariant introduced by E6 should have at least
> one canonical accepted/rejected specification pair. Otherwise your dashboard
> eventually says `generality-tested 29 / 29` while the compiler actually
> contains 31 or 32 semantic invariants. That denominator would have stopped
> meaning "all invariants".

E6 added two invariants that no §16 category named. They had rule fixtures and
generality witnesses but no place in the executable specification, so the
denominator did not move and the score was measuring a smaller compiler than
the one that exists.

### C2 → C3: the two fixtures added

| fixture | category | invariant | why it is canonical |
|---|---|---|---|
| R-045 | private dependency of a shared materialization | `private_in_shared_materialization` | the defect is on the **edge between two individually valid declarations** — `Cart` correctly asks for a private cache, the fragment is correctly entitled to a shared entry, and neither is wrong on its own. No single-declaration rule can see it, and `PW5001` does not |
| R-046 | dependency graph edge to an undeclared target | `graph_edge_unresolved` | invalidation is driven by the graph, so an edge to nothing is not an error at run time — it is silence, and silence is indistinguishable from a fragment whose inputs never changed |

No accepted fixture was added. A-009 is the accepted neighbour for both: it is a
shared materialization whose edges all resolve, and since C2 they do.

### The two lines this changed in the test suite

Both were floors written as constants — `errored >= 44`, `checked == 44` — and
a corpus that grows past a constant floor leaves its new fixtures unenforced
while the assertion still passes. They now count the directory.

`generality-tested` is **30 / 31 with one known narrow invariant**, not 31/31.
`private_in_shared_materialization` has two executable known gaps
(`slips-through-helper.pw`, `slips-through-branch.pw`), and an invariant with a
known gap is not generality-tested. The published triple is asserted, and a
`NARROW` witness is only admissible where `DIMENSIONS.md` has a `## Known gaps`
section — otherwise `NARROW` becomes the escape hatch that turns any failing
witness into an accepted limitation.

---

## C4 — opened 2026-08-07 (E8, on the architect's Effect-prelude ruling)

```text
Corpus version:            C4
Accepted programs:         24
Rejected programs:         46
Charter categories:        24/24 accepted, 46/46 rejected
Result:                    24/24 accepted clean, 46/46 rejected enforced
Wrong-reason catches:      0
Changed since C3:          3 fixtures (below), imports only
Generality at open:        30/31 invariants generality-tested, 1 known narrow
```

### Why it opened

Architect ruling, 2026-08-07:

> Effect names resolve through that prelude; their arguments resolve through
> ordinary lexical/module visibility. […] If `Payments` isn't imported or
> otherwise in scope, that's a source error.

and:

> If `secret<Payments>` appears in `branch-join.pw` and `Payments` isn't visible
> there, the witness was underspecified. Fix the witness. Do **not** weaken
> Pleris's visibility model to preserve it.

**This is a specification change, not a checker improvement.** Before it, an
effect's type argument was judged against every declaration in the program;
after it, against what the file imports. 31 rows across 20 files named an
argument they had not imported — including `packages/pw-platform-web/log.pw`,
which declared `log<Public>` while importing neither `capability` nor `Public`.
The platform library was doing it too.

Three of the 20 files are rejected fixtures, so the version opens.

### C3 → C4: the three fixtures changed

| fixture | change | what it was missing | before | after |
|---|---|---|---|---|
| R-006 | `+ import capability.{ Payments, Public }` | `secret<Payments>` and `log<Public>` named types it never imported | `PW5006` | `PW5006` |
| R-012 | `+ import browser.{ MapHandle }` | `resource.acquire<MapHandle>` named a type it never imported | `PW2005` | `PW2005` |
| R-026 | `+ import capability.{ Payments }` | `secret<Payments>` named a type it never imported | `PW5003` | `PW5003` |

**No `@expect-error` line changed and no verdict moved.** Each fixture is
caught for the same code, by the same rule, for the same reason. What changed
is that the fixture is now a valid Pleris program apart from the defect it
exists to demonstrate — which is what a rejected fixture is supposed to be.

The other 17 files are generality witnesses, rule fixtures and the platform
library, none of which is versioned.

### What this did NOT do

It did not make the corpus stricter about the defects it tests. Every one of
the three was already caught, and would have been caught with or without the
import. The change is that they were relying on a visibility rule the language
no longer has — the same shape as the C0 texts recorded as
`FixtureDidNotExpressIt` in `corpus_history.rs`, caught before it had a chance
to become one.

---

## Opening a corpus version

A new version opens when the *specification* changes — a new charter category, a corrected
`@expect-error`, a fixture that was wrong about the language. It does not open
because a checker got better; that is what `examples/generality/` is for.

To open it:

1. Add the row to the C1→C2 table with the four fields above.
2. Copy the pre-change text into `examples/history/C1/` if the change is to a
   fixture C1 measured.
3. Re-run all three commands and record the new figures here.
4. Say in the commit message which measurement changed and why.
