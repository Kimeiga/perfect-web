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

**C1 is frozen.** A change to any file under `examples/accepted` or
`examples/rejected` opens C2 and requires a row in the table below plus a
recorded reason. The rule exists because the path to 44/44 changed ten
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

## Opening C2

C2 opens when the *specification* changes — a new charter category, a corrected
`@expect-error`, a fixture that was wrong about the language. It does not open
because a checker got better; that is what `examples/generality/` is for.

To open it:

1. Add the row to the C1→C2 table with the four fields above.
2. Copy the pre-change text into `examples/history/C1/` if the change is to a
   fixture C1 measured.
3. Re-run all three commands and record the new figures here.
4. Say in the commit message which measurement changed and why.
