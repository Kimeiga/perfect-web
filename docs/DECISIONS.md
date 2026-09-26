# Decisions

Index over `docs/DECISIONS/ADR-*.md`. One ADR per consequential decision, written
**before** the change it authorizes (PROJECT_CHARTER.md §3.1 step 3).

Status values: `Proposed` · `Accepted` · `Superseded by ADR-NNNN` · `Rejected`

| ADR | Decision | Status | Strategy | Deletion / revisit condition |
|---|---|---|---|---|
| [0001](DECISIONS/ADR-0001-koka-as-temporary-semantic-compiler.md) | Koka 3.2.3 is a temporary semantic oracle, not the permanent compiler | Accepted | tape | Milestone 9 checker reaches corpus parity |
| [0002](DECISIONS/ADR-0002-marko-as-temporary-renderer.md) | Marko 6.3.32 is the temporary renderer behind an adapter | Accepted | tape | Milestone 7 renderer passes the golden suite |
| [0003](DECISIONS/ADR-0003-rust-as-permanent-implementation-language.md) | Rust 1.97.1 is the permanent compiler/host language | Accepted | build | Dependency MSRV exceeds the pin |
| [0004](DECISIONS/ADR-0004-preserve-html-css-http.md) | Preserve standard HTML, CSS, URLs and HTTP | Accepted | reuse | Never wholesale; narrow additions at M13 |
| [0005](DECISIONS/ADR-0005-sqlite-first-data-layer.md) | SQLite first, PostgreSQL when nodes separate | Accepted | reuse | Milestone 11 |
| [0006](DECISIONS/ADR-0006-wasmtime-capability-host.md) | Wasmtime 47.0.3 capability host; effects become WIT imports | Accepted | reuse | A capability cannot be expressed in WIT |
| [0007](DECISIONS/ADR-0007-explicit-invalidation-before-inference.md) | Explicit typed invalidation events before any inference | Accepted | build | After Milestone 6, as an auditable optimization only |
| [0008](DECISIONS/ADR-0008-wasi-0.2-not-0.3.md) | Target WASI 0.2; 0.3 not reachable through the stable toolchain | Accepted | reuse | Rust ships stable `wasm32-wasip3` |
| [0009](DECISIONS/ADR-0009-parser-and-diagnostics-stack.md) | Hand-written parser + `annotate-snippets` 0.12.16 diagnostics | Accepted | build | Milestone 2 lossless-tree design |
| [0010](DECISIONS/ADR-0010-provisional-pw-extension.md) | `.pw` is the provisional source extension | **Proposed** | build | Milestone 2 task 1 owns the real decision |
| [0011](DECISIONS/ADR-0011-koka-is-an-effects-only-oracle.md) | Koka is an effects-only oracle; `pw` owns value semantics | Accepted | tape | E9 checker reaches parity |
| [0012](DECISIONS/ADR-0012-adopt-rowan-before-body-parsing.md) | Adopt `rowan` 0.17.0 before body parsing; analyses consume HIR, not syntax nodes | Accepted | reuse | rowan cannot express a needed property |
| [0013](DECISIONS/ADR-0013-formatter-design.md) | Canonical formatting rules; implementation deferred until ADR-0012 lands | Accepted (design) | build | canonical examples drafted |
| [0014](DECISIONS/ADR-0014-hir-representation.md) | HIR is id-indexed arenas per body, with a span on every node; only `lower.rs` sees syntax | Accepted | build | name resolution needs to interleave with lowering |
| [0015](DECISIONS/ADR-0015-koka-backend-scope.md) | The Koka backend lowers a pure subset only, and states what that does not prove | Accepted | tape | the `pw` checker reaches corpus parity (ADR-0001) |
| [0016](DECISIONS/ADR-0016-e2a-r-runtime-shape.md) | E2A-R is a thread-scoped runtime on `std::thread::scope`; its results are behaviour, not guarantees | Accepted | build | E8 selects the host execution model |
| [0017](DECISIONS/ADR-0017-marko-adapter-boundary.md) | The Marko adapter is a one-way lowering from HIR; generated files are build output, never authored | Accepted | tape | E7-R passes the golden suite (ADR-0002) |
| [0018](DECISIONS/ADR-0018-manifest-is-the-compiler-runtime-boundary.md) | The resource manifest is a data artifact; neither compiler nor runtime depends on the other | Accepted | build | E8 selects the host execution model |
| [0019](DECISIONS/ADR-0019-materializer-store-and-outbox.md) | The materializer's state and its outbox share one SQLite database, so a command writes both in one transaction | Accepted | build | E8 selects the host execution model |
| [0020](DECISIONS/ADR-0020-component-contract-is-the-compiler-host-boundary.md) | The compiler hands the host a six-field `ComponentContract`, one per declaration, and actual Wasm imports must be a SUBSET of what it allows | Accepted | build | E9 lowers capabilities and effects into it |
| [0021](DECISIONS/ADR-0021-the-language-is-called-pleris.md) | The language is **Pleris**; Perfect Web stays the project, `.pw` the source format, `pw-*` the internal crates | Accepted | prose only | a public launch needs a formal trademark/organization clearance pass |
| [0022](DECISIONS/ADR-0022-causal-analysis-validation.md) | **Coincidental correctness** has a name and a countermeasure: important conclusions must explain which resolved facts caused them, and fixtures grow from three checks to five | Accepted | E8 slice 2 is the first consumer | a provenance DAG that RECOMPUTES rather than RECORDS would be a second answer to the same question |
| [0026](DECISIONS/ADR-0026-a-capability-authorizes-an-operation-it-does-not-identify-one.md) | A host dependency is a **callable import** — `ImportId` + ABI + a SET of capabilities + who defined it — and a capability is never the callable identity. A host implementation is explicit declaration metadata, never inferred from a `todo` body or an effect row. The audit is three separate layers, and operation authority ⊆ component authority | Accepted | E10 | the Wasm encoder could not encode `add_to_cart`: `Carts.add(s, item, qty)` and `Carts.clear(s)` share `database.write<Carts>` and a core import has one signature, so a capability-keyed import had no honest arity |
| [0025](DECISIONS/ADR-0025-an-optimistic-clause-targets-a-resource-entry.md) | An optimistic clause names the resource **entry** it updates, binds its current value, and applies a **pure** transition. There is no written `rollback`: the platform restores the value it held | Accepted | E10 | two entries can share a type, so a transition naming only the type does not say what it changed; and a hand-written inverse is generally false — `Apple × 3` optimistically `+2` is not restored by `remove(apple)` |
| [0024](DECISIONS/ADR-0024-optimistic-and-rollback-bind-with-a-lambda.md) | *(rejected)* `optimistic` / `rollback` bind their subject with a bare lambda | Rejected, superseded by 0025 | E10 | the lambda said what transformation to perform and not which entry it applied to; and it kept a written inverse, which describes an operation the runtime will not use |
| [0023](DECISIONS/ADR-0023-milestone-gates-prove-one-layer.md) | A milestone gate proves **one layer's** contract; a cross-layer end-to-end proof belongs at the milestone where both sides exist, as a recorded deferred obligation | Accepted | build | a SECOND gate item deferred for the same reason — one mistake is a patch, two is a structural inversion in the milestone order |

| [0028](DECISIONS/ADR-0028-recursive-written-types-before-signature-cutover.md) | Preserve recursive written types and complete return annotations before the resolved-signature cutover; resolve only valid type identities | Accepted | reopened E9 | no nested type reparsing or partial resolution; ordinary call/return compatibility remains a separate gate |

## Decisions the charter asked for and where they landed

Charter §14 Milestone 0 task 5 lists eight required ADRs. All eight exist:

| charter item | ADR |
|---|---|
| Koka as temporary semantic compiler | 0001 |
| Marko as temporary renderer | 0002 |
| Rust as permanent compiler/host language | 0003 |
| standard HTML/CSS/HTTP preservation | 0004 |
| SQLite-first data layer | 0005 |
| Wasmtime capability host | 0006 |
| no browser fork before profiling | see below |
| explicit invalidation before automatic dependency tracking | 0007 |

ADRs 0008–0010 were added because Milestone 0's spikes forced decisions the
charter left open (the WASI version) or that the corpus needed immediately
(the diagnostics stack, the file extension).

**"No browser fork before profiling"** is deliberately *not* an ADR. It is a
standing constraint in the charter itself (§11.4, §14 M13) and there is no
decision to record until Milestone 13 produces profiling data. Recording an ADR
that says "we did not do the thing we were told not to do" would add
ceremony without content. It is tracked in `docs/vision/non-goals.md`.

## Decision log

Short entries for choices that shaped the repository but are too small for an ADR.

**2026-08-05 — `docs/`-scoped STATUS/DECISIONS/ASSUMPTIONS.**
The charter mandates `docs/STATUS.md` and `docs/DECISIONS/ADR-*.md`. The three
requested filenames live under `docs/` rather than at the root so there is one
source of truth. Recorded as assumption A-006.

**2026-08-05 — The original prompt file became `PROJECT_CHARTER.md` and was removed.**
Byte-identical copy verified by SHA-256 (`d37dfa2d…a90e4c`) before deleting the
duplicate, so there is exactly one constitution in the repository.

**2026-08-05 — `wasm32-wasip2` spike crates are excluded from the Cargo workspace.**
They target wasm and must not be pulled into a host-target
`cargo test --workspace`. `spikes/wasmtime-component/run.sh` builds them.

**2026-08-05 — Node 22.21.1 rather than Node 24.**
Both `vite@8.2.0` (`^20.19.0 || >=22.12.0`) and `marko@6.3.32` (`>=22`) are
satisfied. Pinning what was actually tested, per charter §3.6. Assumption A-002.

**2026-08-05 — one diagnostic code per invariant, not per detector.**
`PW2004` ("a resource cannot outlive the scope that owns it") is emitted by both
the declaration rule and the scope graph, distinguished by a `reason` and
`detector` field. `PW0326` is a deprecated alias resolving to it. Two permanent
codes for one invariant would be wrong; aliasing during migration is fine.

**2026-08-05 — semantic rules may not live in the CLI.**
`pw-syntax` owns syntax, `pw-core` owns meaning and emits one `Diagnostic` type,
`pw-cli` renders. This is what lets a language server, test harness, build
system, playground, AI loop and PR analyser share one checker.

**2026-08-05 — E7 is subdivided.**
E7-R (resumption/DOM), E7-P (patch semantics), E7-L (lazy loading). Marko is the
accepted oracle for E7-R only; it **fails** E7-L. The undifferentiated sentence
"Marko is the E7 oracle" is forbidden.

**2026-08-05 — four small syntax decisions forced by the body grammar.**
None is large enough for an ADR; all four are load-bearing for the tree shape.
*Compound comparisons* (`==` `!=` `<=` `>=`) lex as one token, while `<` and
`>` stay separate because they also delimit type arguments — the joined forms
are unambiguous since a type argument list is never followed directly by `=`.
*An infix operator that can also begin an expression* (`<`, `-`, `!`) may not
begin a line; without the rule, `let s = f(id)` followed by `<main>` parses as
one comparison. *Dotted paths are not absorbed* in expression position: the
parser cannot distinguish `Stores.get` from `store.name` and must not pretend
to, so every `.ident` is a field access and name resolution folds the segments
that turn out to be a module path. *Triple-quoted strings* carry wrapped
`because "..."` justifications, leaving the single-quoted form ending at the
newline, which is the error-recovery property worth keeping.

**2026-08-05 — silent parser recovery is a defect, not a convenience.**
Three closers were consumed with a bare `eat` whose `false` was discarded.
Turning them into diagnostics moved coverage from 56/68 corpus files to 68/68,
because the silence was hiding four real defects — each of which reported its
error one line *after* the cause. Recovery must continue parsing; it must not
continue quietly.

**2026-08-05 — `just ci` runs `clippy -D warnings`.**
Dead code is treated as a signal, not noise: the first `-D warnings` failure
surfaced genuinely unused model surface, which was resolved by *using* it
(`--explain`, a warning-level rule) rather than by silencing the lint.

## 2026-09-16: resolved signature authority

[ADR-0030](DECISIONS/ADR-0030-resolved-signature-authority.md): replace written
signature types atomically with resolved identities, share semantic keys and
stable projections, and keep missing/blocked/known slots distinct. Boundary
privacy and WIT decisions consume the same signature. Does not close ordinary
call/return typing or the component execution gate.

## 2026-09-24: the value relations

[ADR-0031](DECISIONS/ADR-0031-value-relations.md): one module decides every
place a value meets a declared type. That covers arity, arguments, fields,
results (including `?`), annotated bindings and unresolved written types, with
diagnostics projected from a queryable, three-valued analysis. Adds:
- callable type parameters instantiated per call;
- `type` parameters kept;
- function types;
- `?` in the HIR;
- declared-constructor arity.

Decides without a ruling, for reversal:
- privacy qualifiers are not value-transparent;
- function types exist;
- snapshots are read explicitly.

Closes E9-V1..V6. Does not close E10-I.

## 2026-09-24: compiled components (E10-I)

[ADR-0032](DECISIONS/ADR-0032-compiled-components.md): Canonical ABI adapters
from the world, with every number from `wit-parser`, one encoder, and upstream
`wit-component` wrapping. Component identity is checked twice: during encoding
and by decoding the artifact. The host runs a component with the deployment's
operations, compiled once. The contract locates each export (ruling needed). The
dev server's command closures are deleted. Closes E10-I.

## 2026-09-25: compiled resumable handlers (E10)

[ADR-0033](DECISIONS/ADR-0033-compiled-handlers.md): a resumable handler's body
compiles to an ES module (`pw emit-handlers`). Each fact the backend needs is
read from the stage that owns it: identity, name, callee, component id and
capture paths. The component id now has one derivation instead of three. An
element carries exactly the capture paths its handler reads (ruling needed).
The browser's arguments are typed by the command component's own parameters.
The dev server has one command path, which fixes `clear_cart`'s uncommitted
state. A string literal has a value only where no escape rule is involved
(ruling needed). Supersedes ADR-0032 §8.

## 2026-09-25: `pw build` (E10 gate item 1)

[ADR-0034](DECISIONS/ADR-0034-pw-build.md): one command builds every artifact
of a checked program: the template IR, the handler modules, every component
(audited), and the contracts and WIT. It composes the stages that own each
artifact, and is all or nothing. Each contract gets one outcome: component, no
body, placeholder, or refused. A `todo` placeholder fails the build only when
something depends on it (ruling needed). The gate's evidence runs the build
with neither Koka nor Node on the PATH.

## 2026-09-25: bounded subscribers (E10 gate item 3)

[ADR-0035](DECISIONS/ADR-0035-bounded-subscribers.md): a consumed outbox event
is deleted. A subscriber holds at most 256 frames, and past that gets one
`Recovery::Reload`. One that has not asked for 120 s is forgotten, with its
cached cart fragment, and its next poll is told to reload. A served document's
cursor is never zero, and the runtime reloads on `Reload`. Measured before:
3,000 retained events, 600,000 frames, 1,001 entries.

## 2026-09-25: matches, fields and variants in the backend (E10)

[ADR-0036](DECISIONS/ADR-0036-matches-fields-variants.md): `match` over
`Option` and `Result` is a structured IR instruction whose arms are regions.
Lowering is bidirectional, so `None` takes the type its use fixes. The encoder
types a constructed variant from its uses and checks every move with
`same_type`, and every layout number comes from `wit-parser`. The store's
components are byte-identical afterwards.

## 2026-09-25: the kiokun slice (E10)

[ADR-0037](DECISIONS/ADR-0037-kiokun-slice.md): a second application, entry
lookup and search over one real shard of kiokun.com, compiled by `pw build`.
Its data is kiokun's own files, copied with attribution (owner decision needed
on share-alike data). The data layer is the deployment's, as kiokun's search is
its database's. Building it found the platform package depending on the store
example, and interpolated attribute strings rendering literally; both fixed.

## 2026-09-25: every match is analysed (E10)

[ADR-0038](DECISIONS/ADR-0038-every-match-typed.md): the exhaustiveness
analysis reads every match. Scrutinees are typed by the value relations,
`Option` and `Result` are sum types to it, and each pattern is read against its
own type. A constructor its type lacks is the new PW0608. `return` is a
statement in an arm too, and an arm with no `=>` is an error. Five gaps, each
of which reported a match as exhaustive when it was not; the kiokun slice
found the first. Two decisions need a ruling: a bare name is a constructor
whenever some type has one of that name, and no `return` expression is built.

## 2026-09-25: pure computation in the component backend (E10)

[ADR-0039](DECISIONS/ADR-0039-pure-computation.md): arithmetic, comparisons,
`&`, `|`, `!`, `if`, string literals, interpolation, records, and calls to
other declarations (inlined). `Int` traps where its exact result does not fit,
and `/` and `%` are Euclidean, as Koka's are; a zero divisor traps where Koka
answers 0 (ruling needed). Types the world never names are defined in a
private copy of its `Resolve`. Building it found a query body the parser
dropped, a backend that compiled programs that did not parse, a Koka backend
that emitted invalid negation, and an import missing when called only in an
arm; all fixed.

## 2026-09-25: the standard library's lists and strings, compiled (E10)

[ADR-0040](DECISIONS/ADR-0040-standard-library.md): `List` and a new `String`
module are declarations the compiler supplies, each marked `intrinsic`, read
as `host` is. `map`, `filter`, `fold`, `any`, `all`, `find` and a stable
`sort_by` compile their function argument where they run it; `length`, `get`,
`take`, `concat`, and the string operations are routines in the component.
`to_lower_ascii` maps `A`–`Z` only (ruling needed). Building it found that
`(a, b) => e` had never parsed.

[ADR-0041](DECISIONS/ADR-0041-kiokun-in-pleris.md): kiokun's shard rule and
its search ranking are Pleris, compiled to `shards.Place`, `shards.Places` and
`kiokun.page.Search`; the host keeps the index and the files. The Rust rule and
ranking stay as the references every compiled answer is compared with, on
every word kiokun has. Building it found that the E9 value relations left a
generic call's `let` binding uninstantiated, that a statement keyword could
name a value, and that kiokun's file names are not its words.

[ADR-0042](DECISIONS/ADR-0042-template-branches-and-attributes.md): a
template block's markers are kept and checked (PW5019), `{:else}` and
`{:else if}` are branches, `{#match}` takes an `Option` or a `Result` apart,
and an attribute interpolates, each value escaped for its context (a URI
component in a URL). It fixes a silent miscompile: `{:else}` was dropped, and
both branches rendered together.

[ADR-0043](DECISIONS/ADR-0043-operands-are-typed.md): an operator's operands,
and an `if`'s condition, are related to the types they take (PW0609), so
`1 == "a"` no longer checks. `Float.from_int` converts a count. Building it
found two accepted fixtures dividing a `Float` by an `Int`, which ADR-0039 §2
refuses.

[ADR-0044](DECISIONS/ADR-0044-pure-computation-in-javascript.md): a query
that reaches no host also compiles to an ES module, from the same backend IR
as its component. Every place JavaScript's semantics differ from Pleris's
(BigInt, Euclidean division, code point order, White_Space) is encoded
explicitly. The two backends agree on 6,800 generated calls under Node,
kiokun's shard rule among them.

[ADR-0045](DECISIONS/ADR-0045-affine-exactly-once.md): PW2005's invariant,
"exactly once, in the scope that acquired it", is checked on every path:
the end of a scope and a failing `?` are exits, a second release is
counted, and a release in a loop is refused. A declaration whose row
promises to release a parameter must. No syntax is added.

[ADR-0046](DECISIONS/ADR-0046-memory-strategy-measured.md): E10 task 4.
Every compiled kiokun query was measured per call on the whole shard:
instructions, peak linear memory, and instantiation beside the call.
Invocation regions stay the strategy; the other six the charter names are
judged against the numbers. Per-call instantiation, not memory, dominates.

[ADR-0047](DECISIONS/ADR-0047-every-name-resolves.md): every name resolves,
in lexical scope, not only a call's. A `let` binds after itself, a pattern
for its own body, and a template's blocks for their children. Clauses
written as statements are read by position from the policy table. The walk
found `derived` parsed as a bare name, so A-016 and A-018 computed nothing,
and a statement's named argument read as an assignment.

[ADR-0048](DECISIONS/ADR-0048-members-exist.md): a read or a call through a
value names a member its type has (PW0610): a field, or a declaration whose
first parameter takes the type. An opaque type's `.value` is its
representation in its own module only. It found eight reads of members no
type has, among them the store page's `{cart.line_count}`.

[ADR-0049](DECISIONS/ADR-0049-string-escapes.md): a string's escapes are the
language's, settling A-023. One decoder in `pw_syntax::strings` reads every
string token; an undefined escape is PW0014; `"""` strings are raw; each
backend encodes the value in its own syntax.

[ADR-0050](DECISIONS/ADR-0050-recursion-and-generic-callees.md): recursion
and generic callees compile. A call is inlined until it recurses; the
recursion is compiled beside the export, once per instance, and called, in
the component and the JavaScript module. A generic callee is instantiated
from its arguments. Existing artifacts are byte-identical.

[ADR-0051](DECISIONS/ADR-0051-early-return-and-loops.md): an early `return`,
`?`, and `for` loops with `let mut` bindings compile, in the component and
the module. A callee that returns early is compiled beside its export. The
checker refuses an assignment to a binding that is not `let mut` (PW0611),
and relates the assigned value to the binding's type (PW0607).

[ADR-0052](DECISIONS/ADR-0052-function-values.md): a function is a value. A
lambda or a declaration's name compiles to a closure of its captures, is
stored, returned, passed and called through: in the component an
environment in the region naming its code's slot in a `funcref` table,
called with `call_indirect`; in the module a JavaScript function.

[ADR-0053](DECISIONS/ADR-0053-callback-parameters.md): a lambda's parameters
take the types its use declares. `infer.rs` reads them from the callee's
declared function type, where it gave the first parameter the element type
of any list beside it; `values.rs` keeps the types a solved call, an
annotation or a declared result gives them. `fold`'s accumulator was typed
as the element and refused.

[ADR-0054](DECISIONS/ADR-0054-opaque-values.md): an opaque value is built and
read inside a component, as its representation retyped (`Instr::Retype`),
in the component and the module. An opaque type's representation is a type
tree, where it was a spelling that never resolved with type arguments.

[ADR-0055](DECISIONS/ADR-0055-slices-and-placeholders.md): `List.drop`,
`List.slice`, `List.reverse` and `String.slice`, each bound clamped, a
slice a view. `sum` and `maximum` are Pleris in `list.pw`, where their
placeholders answered 0.0; `maximum` is an `Option`. `enumerate`, which
answered `[]`, is removed: the language has no tuple type.

[ADR-0056](DECISIONS/ADR-0056-unicode-case-mapping.md): `String.to_lower`
and `String.to_upper` map each code point as Unicode 17.0 does, from
tables generated from the compiler's own `char` mapping. The component
holds them in its data segment and the module as constants, so neither
reads a platform's mapping. A final sigma lowers to `σ`.

[ADR-0057](DECISIONS/ADR-0057-maps-and-sets.md): `Map<K, V>` and `Set<T>`,
keyed by an `Int` or a `String`, in ascending key order: in the component
`list<tuple<K, V>>` and `list<T>`, searched in binary and changed by
copying; in the module sorted arrays. A map or set from outside is checked
on arrival. `List.sort_by`'s merge sort takes a comparison now.

[ADR-0058](DECISIONS/ADR-0058-handlers-that-compute.md): a resumable
handler's body is lowered as a query's is and written by the
pure-computation emitter, its commands awaited in order: bindings,
arithmetic, branches, loops and several commands. A captured `Int` is read
into a `BigInt`, and one sent past ±2^53 traps before it is sent.

## 2026-09-26: declared sum types, typed, built and matched (E10)

[ADR-0059](DECISIONS/ADR-0059-declared-sum-types.md): a sum type's case is
typed where it is written through its type, `Shape.Circle(3)`: its fields,
its arity, and a case the type lacks (PW0608). A pattern may name its type
too, and an arm's fields are typed in its body. The backend builds and
matches declared cases, in the component as WIT variants and in the module
as `{ $case, value }`, with `_`, name and `A | B` arms. Writing it found
four ways a wrong program passed `pw check`: a case's fields were
unchecked, an undeclared case passed, `Shape.Empty` in a pattern bound a
name and matched everything, and an arm's bindings were unknown in its
body.

## 2026-09-26: nested and literal patterns (E10)

[ADR-0060](DECISIONS/ADR-0060-nested-and-literal-patterns.md): the
exhaustiveness analysis types each match's subject, an ADT per instance, so
a pattern nested under `Some` or a declared case is read against its own
type, and `true`, `false` and each literal are constructors. The backend
compiles any match that is not one level deep to a decision tree of nested
matches and tests. It fixes a silent miscompile: `Some(Empty)` bound every
payload to a name `Empty`.

## 2026-09-26: a declared sum type in a template's match (E10)

[ADR-0061](DECISIONS/ADR-0061-template-matches-over-sum-types.md): a
template's `{#match}` takes a declared sum type apart, checked as any match
is. An arm binds each field of its case, `{:Rect(w, h)}`, and may name its
type. The template IR names a declared case by its WIT name, as the value a
component gives does, and a case of several fields is a list the renderer
binds field by field.

## 2026-09-26: generic types in the backend (E10)

[ADR-0062](DECISIONS/ADR-0062-generic-types-in-the-backend.md): a nominal
type carries its arguments, so a generic record, sum type or opaque type is
laid out per instance inside a component and a module. An instance's
arguments come from its fields, as a generic callee's do, or from its use; a
parameter nothing fixes is refused by name. A generic type still does not
cross the boundary. Writing it found a type and a query of one name failing
the whole WIT package.

## 2026-09-26: every name means one binding (E10)

[ADR-0063](DECISIONS/ADR-0063-every-name-means-one-binding.md): each local
name is resolved once, to the binding in scope where it is written, by the
scopes the backend lowers with. The value relations, the declared-type
environment and the privacy labels keep their facts by binding, where each
kept them by name. It fixes type errors that passed through any name bound
twice, a handler capture typed by another binding of its name, and secrets
logged or rendered through a `for` loop's, a lambda's or an `{#each}` block's
name.

## 2026-09-26: a label carried through a call (E10)

[ADR-0064](DECISIONS/ADR-0064-a-label-through-a-call.md): a declared call's
result joins its declaration's label with the label of each argument whose
type mentions a type parameter the result mentions, so a secret passed
through `List.get`, `List.map`, `List.fold` or a program's own generic
function stays secret. A lambda is labelled by what it computes, and a call
no declaration answers by all that goes into it, its receiver included.

## 2026-09-26: a value that holds at every type (E10)

[ADR-0065](DECISIONS/ADR-0065-a-value-of-any-type.md): `None`, `[]`, `Ok` and
`Err`, `todo`, an early return, and a generic value whose parameter no field
mentions hold at every type their hole could be, and the value relations
decide them. A variable such a value meets is not fixed by it, a function's
link between its parameter and its result is kept, and a `let mut` holds one
type, completed by an assignment.

## 2026-09-26: a nested declaration sees the bindings around it (E10)

[ADR-0066](DECISIONS/ADR-0066-a-nested-declaration-sees-around-it.md): a
declaration nested in another sees the enclosing parameters and the bindings
in scope where it is written, typed and labelled as the enclosing declaration
has them, and is in its module. A stream's parts and a release clause's name
are resolved; a call's callee is the binding in scope or a declaration;
`(x) =>` binds. It fixes a secret logged publicly through a nested function,
and a nested declaration's annotations, which never resolved.

## 2026-09-26: a record is built with each of its fields, once (E10)

[ADR-0067](DECISIONS/ADR-0067-a-record-is-built-with-its-fields.md): a record
built by its fields' names is related to the fields its type declares, each
once and no other (PW0612), where each field was related alone and a field
left out, one its type lacks, or one given twice passed. An `if` without
`else` is the unit value, so a body declaring another result cannot end in
one.

## 2026-09-26: what each construct takes, checked (E10)

[ADR-0068](DECISIONS/ADR-0068-what-each-construct-takes.md): a call through a
function value is checked against its type (arity, arguments, result), and a
value called that is not a function is PW0614; `for`'s list and `?`'s operand
are related; the branches of an `if` or a `match` whose value is used produce
one type (PW0613). It fixes a silent miscompile: every `elif` and `else if`
chain lowered to its first branch and, for its `else`, the next condition.

## 2026-09-26: a list's items share one type, and an Int literal fits an Int (E10)

[ADR-0069](DECISIONS/ADR-0069-list-items-and-int-literals.md): a list's items
are related to each other (PW0615), where two of different types made the
element a hole and `[1, "a"]` passed as a `List<Int>`; an `Int` literal is
related to the 64-bit range (PW0616), which the backend was the first to
enforce.

## 2026-09-26: an assignment to a field has the field's type (E10)

[ADR-0070](DECISIONS/ADR-0070-an-assignment-to-a-field.md): `b.value = e`
relates `e` to the field's type (PW0607), where an assignment to a field
related nothing.

## 2026-09-26: what a template's blocks and events take (E10)

[ADR-0071](DECISIONS/ADR-0071-what-a-template-takes.md): an `{#each}`'s
collection is a list, an `{#if}` or `{:else if}` condition is not a declared
sum type (PW0609), and an event attribute is given a function (PW0614). Each
of the three passed `pw check`, and the first two failed only when rendered.

## 2026-09-26: an element named with a capital letter is a view (E10)

[ADR-0072](DECISIONS/ADR-0072-an-element-named-with-a-capital-is-a-view.md):
`<Money value={p} />`, a view used in another view as the charter writes it
(§8.1), built as an unknown HTML element, its markup never rendered and its
props checked by nothing. It is refused (PW5020) until views compose, and so
is a tag that names no view. How a view composes needs a ruling.

## 2026-09-26: a template reads each value by path (E10)

[ADR-0073](DECISIONS/ADR-0073-a-template-reads-each-value-by-path.md): a
computed hole checks and does not build, where it built with an empty path
and failed every render; `pw build` refuses a part the renderer refuses; a
`style:` directive no longer builds as an attribute a browser ignores; and a
loop's key is read from its element (PW5021) along its whole path, where
`(k.r.id)` keyed on `k.id`.

## 2026-09-26: what a template writes has a text form (E10)

[ADR-0074](DECISIONS/ADR-0074-what-a-template-writes-has-a-text-form.md): a
value a template writes as text has a text form (PW0609); one that may be
absent is taken apart (PW0600), in `{:else if}` too, which ADR-0071 left to
nothing; a boolean attribute has a truth; and a loop's list and key are read
through fields their values have (PW0610). Each passed `pw check` and failed
when rendered.

## 2026-09-26: a stream and a mounted resource do not build (E10)

[ADR-0075](DECISIONS/ADR-0075-a-stream-and-a-mounted-resource-do-not-build.md):
the template IR refuses a `<stream>` and an element that mounts a resource
by name, where each lowered as a literal element: a stream was refused for
its `query` attribute, and A-007's mounted resource built a page that fails
when rendered.

## 2026-09-26: an arm no value reaches is refused (E10)

[ADR-0076](DECISIONS/ADR-0076-an-arm-no-value-reaches.md): the
exhaustiveness analysis always found unreachable arms, and nothing reported
one. `_ => 0` before `Circle(r) => r` checked, and a literal matched twice
built with a dead arm. PW0333 refuses each.

## 2026-09-26: a call through a field holding a function is checked (E10)

[ADR-0077](DECISIONS/ADR-0077-a-call-through-a-field.md): `r.f(x)`, where
`f` is a field holding a function, resolved to nothing, so its arguments,
its arity and its result passed unchecked. ADR-0068's relations check it
now.

## 2026-09-26: an effect is performed where its function is named (E10)

[ADR-0078](DECISIONS/ADR-0078-an-effect-is-performed-where-its-function-is-named.md):
a view declared `!{}` read the clock through `List.map(xs, stamp)`, a
local, a helper's parameter or a record field, since only a call counted;
R-037's invariant held only for a lambda. A declaration named as a value
performs its effects where it is named, and a helper declaring no row
carries its members' and its values' effects too.

## 2026-09-26: a function value carries the label of what it makes (E10)

[ADR-0079](DECISIONS/ADR-0079-a-function-value-carries-its-label.md): a
`Secret<Payments>` logged publicly passed through `let f =
secrets.payments` then `f()`, or a record field holding it: a name meaning
a declaration was public, and a call through a value left out its callee.
Both carry the label of what the function makes now.

## 2026-09-26: the affine rule follows bindings, not names (E10)

[ADR-0080](DECISIONS/ADR-0080-the-affine-rule-follows-bindings.md): PW2005
found a transaction's uses by its name, so an outer `tx` never ended passed
where each branch ended an inner `tx`, correct programs were refused, and
an end through `let end = Database.rollback` counted nothing. It follows the
binding a name means now; a local bound to a declaration is that
declaration, and a value given to any other function value is refused.

## 2026-09-26: a named argument is given to the parameter of its name (E10)

[ADR-0081](DECISIONS/ADR-0081-a-named-argument-is-its-parameters.md): the
backend passed arguments in written order, so `g(b = 1, a = 10)` computed
`g(1, 10)`, a silent miscompile; the checker related no argument of a call
that named one; and a generic result's label was carried from the argument
in the wrong place. Signatures carry parameter names now, and one
arrangement serves the checker, the labels and the backend (PW0617).

## 2026-09-26: a `derived` value performs no effect (E10)

[ADR-0082](DECISIONS/ADR-0082-a-derived-value-performs-no-effect.md): the
charter's `derived` is a pure value computed from other values, and nothing
held it to that: `let t = derived clock.now()` checked. PW0334 refuses an
effect inside one, whether called, read through a member or named.

## 2026-09-26: a call's arguments open on the callee's line (E10)

[ADR-0083](DECISIONS/ADR-0083-a-call-opens-on-its-callees-line.md): a `(`
at the start of a line continued the expression before it, so `g(n)` then
`()` parsed as `g(n)()` and a correct program was refused for "`` does not
resolve". It begins a new statement now, as `<`, `-` and `!` do.

## 2026-09-26: what a string interpolates has a text form (E10)

[ADR-0084](DECISIONS/ADR-0084-what-a-string-interpolates-has-a-text-form.md):
`"{xs}"` over a list, a record or an `Option` checked, and the backend was
the first to refuse it. A string's holes are related to a text form now, as
a template's are (ADR-0074); three fixtures interpolated a value with none
and are corrected.

## 2026-09-26: a call carries what it is given (E10)

[ADR-0085](DECISIONS/ADR-0085-a-call-carries-what-it-is-given.md): a
secret logged publicly passed once it went through `String.trim`, or any
declared function over plain values: ADR-0064 carried a label only where the
result mentions a type parameter. A declared call carries every argument
given to a parameter that states no label now, and a parameter declared
`Secret<C>` keeps its contract. Corrects ADR-0064 §1.

## 2026-09-26: a function crosses no boundary (E10)

[ADR-0086](DECISIONS/ADR-0086-a-function-crosses-no-boundary.md): a
resumable handler that captured a function checked, and the build refused it
for a name that "names no declaration". A value that is or holds a function
crosses no boundary now: PW5008 for a capture, untransferable for a call to
another placement.

## 2026-09-26: a call names a term (E10)

[ADR-0087](DECISIONS/ADR-0087-a-call-names-a-term.md): a call to a view, a
page, an event or an effect checked, answered for as a call to nothing, and
`fn f() -> Int { Badge(1) }` returned a view where an `Int` is declared. A
call names a function, a data operation, or a type it builds (PW0027).

## 2026-09-26: a clause names a declaration of its kind, and gives it its key (E10)

[ADR-0088](DECISIONS/ADR-0088-a-clause-names-a-declaration-and-gives-it-its-key.md):
`depends_on`, `invalidates` and `emits` were text. The graph drew an edge to
whatever a name found, and nothing resolved, counted or typed a key, so
`invalidates Cart(item)` and `emits Cart(..)` checked. A clause names a
declaration of its kind (PW5103), and its key's arguments are terms related
to what it names. The store's own `emits` gave its event the wrong type.

## 2026-09-26: a policy's value is one its domain has (E10)

[ADR-0089](DECISIONS/ADR-0089-a-policy-value-is-one-its-domain-has.md):
nothing held a policy's value to the table that says what it is. `cache
Shared` checked on R-004's page and escaped the rule that keeps a session's
data out of a shared cache; `placement originn`, `retry nope(..)` and `key
nope` checked; and three readers were bypassed by a prefix or a substring.
A value is one its domain has now (PW0335), and each reader reads it exactly.

## 2026-09-26: `pw build` checks what `pw check` checks (E10)

[ADR-0090](DECISIONS/ADR-0090-the-build-checks-what-pw-check-checks.md):
the declaration rules ran only in the `pw check` command, so `pw build`
compiled R-015's `retry forever` query into a component and built R-014.
One checker runs them now, and their diagnostics meet the checker's
standard: the registry's invariant, a boundary span, an explanation.

## 2026-09-26: a listener binds its entry's key (E10)

[ADR-0091](DECISIONS/ADR-0091-a-listener-binds-its-entrys-key.md): the
materializer read an event's values as a set, so `InventoryChanged(47, item
3)` never reached store 47's menu, and nothing checked what a listener
wrote. Each argument of `invalidates_on` is a parameter the entry's key
binds, or `_` (PW5104), related to the event's values, and the materializer
compares them position by position.

## 2026-09-26: a dependency-graph clause belongs to a declaration that can mean it (E10)

[ADR-0092](DECISIONS/ADR-0092-a-graph-clause-belongs-to-a-declaration-that-can-mean-it.md):
a query that `emits`, a command that listens, a `fn` that emits and a page
that invalidates each checked, and nothing read what they said. A command
emits and invalidates, a resource or a materialization listens, and a
materialization depends (PW5105).

## 2026-09-26: an element handles an event the platform declares (E10)

[ADR-0093](DECISIONS/ADR-0093-an-element-handles-an-event-the-platform-declares.md):
`on:clik={go}` checked, skipped by the rule that reads the platform's
`events`, and ran by accident, since the runtime listens for a click
whatever the name. An `on:` attribute names a declared event now (PW5022).

## 2026-09-26: a template writes no code (E10)

[ADR-0094](DECISIONS/ADR-0094-a-template-writes-no-code.md):
`<script>{msg}</script>`, `onclick={msg}`, `srcdoc={msg}` and
`<style>{msg}</style>` checked and built, escaped as text or an attribute,
which makes none of them inert: `msg` ran; so did `href="javascript:go({id})"`.
A template writes no script, no inline handler, no script URL, no `srcdoc`
and no value into a stylesheet (PW5023).

## 2026-09-26: an attribute's context is read as HTML reads its name (E10)

[ADR-0095](DECISIONS/ADR-0095-an-attributes-context-is-read-as-html-reads-its-name.md):
the template IR chose a value's escaping from the attribute's name as
written, so `<a HREF={msg}>` was escaped as an ordinary attribute and
`msg = "javascript:alert(1)"` ran. The name is read as HTML reads it now,
lowercased.

## 2026-09-26: a template moves no URL (E10)

[ADR-0096](DECISIONS/ADR-0096-a-template-moves-no-url.md): `<base
href={msg}>` chose where the platform's runtime loaded from, since the page
writes the view before the runtime's script, and an SVG `<animate>` set a
link's `href` to any URL. A template writes no `<base>` and animates no link
or handler now (PW5024).

## 2026-09-26: data embedded in a page cannot end its script element (E10)

[ADR-0097](DECISIONS/ADR-0097-embedded-data-cannot-end-its-script.md): the
parts manifest's JSON broke only a lowercase `</script`, and `</SCRIPT>` or
`<!--<script>` in it would end or swallow the element. It holds no `<` now:
`escape::json_in_script` writes `\u003c`.

## 2026-09-26: a name is written once where it is declared (E10)

[ADR-0098](DECISIONS/ADR-0098-a-name-is-written-once-where-it-is-declared.md):
a parameter taken twice, a field or a case declared twice, a policy written
twice and an attribute given twice each checked, and one of the two was
dropped: `cache private` then `cache shared` decided by its order whether
a session's data was refused a shared cache. Each is written once now
(PW0028), an effect's `impact` excepted.

## 2026-09-26: a failure is handled (E10)

[ADR-0099](DECISIONS/ADR-0099-a-failure-is-handled.md): a statement's
`Result` was dropped without a word, and nine corpus fixtures dropped one, a
rollback's or a clear's, one of them `@expect: clean`. A `Result` whose value
nothing uses is refused (PW0618): `?` returns it, `match` handles it, and a
binding that says so discards it by name.

## 2026-09-26: a query reads (E10)

[ADR-0100](DECISIONS/ADR-0100-a-query-reads.md): a query whose body cleared
a cart checked, and the platform caches, deduplicates and retries a query as
a read. A query or a subscription that writes or opens a transaction is
refused (PW0401); a mutation is a command's.
