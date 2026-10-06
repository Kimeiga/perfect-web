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

## 2026-09-26: a command invalidates what it writes (E10)

[ADR-0101](DECISIONS/ADR-0101-a-command-invalidates-what-it-writes.md): a
command that wrote the cart and declared neither `invalidates` nor `emits`
checked, and nothing told the queries reading the cart that it had changed.
A-005 told the library's stub and not A-004's cart. A command reaches each
reader of what it writes that has no staleness window: by name, or by an event
the reader listens for (PW5106).

## 2026-09-26: an event reaches what reads what it invalidates (E10)

[ADR-0102](DECISIONS/ADR-0102-an-event-reaches-what-reads-what-it-invalidates.md):
the materializer invalidated an event's direct listeners and nothing built
from them, although the compiler's graph follows reads. A-009's fragment
depends on a store that listens for `StoreChanged`, and kept the old store
after one. An event now reaches each entry that reads what it reaches, at
the key the read supplies, and a key the read does not supply matches any
value.

## 2026-09-26: a write reaches the fragments built on it (E10)

[ADR-0103](DECISIONS/ADR-0103-a-write-reaches-the-fragments-built-on-it.md):
a fragment is rebuilt only when an event reaches it, and one built from a
query with a staleness window kept a renamed store for good, since the
command emitted nothing and PW5106 left the query to expire. A command now
emits an event that reaches each fragment built on what it writes (PW5106);
`invalidates` does not reach a fragment.

## 2026-09-26: the dev server commits the events a command declares (E10)

[ADR-0104](DECISIONS/ADR-0104-the-dev-server-commits-what-a-command-declares.md):
the dev server committed `CartChanged` after any cart write, whatever the
command declared, which hid ADR-0101's finding at run time. It now reads the
command's `emits` edges from the compiler's graph, commits those events, and
refuses a key it cannot compute before the command runs.

## 2026-09-26: a command invalidates the entry it speculates on (E10)

[ADR-0105](DECISIONS/ADR-0105-a-command-invalidates-the-entry-it-speculates-on.md):
a command speculating on `Cart` with neither `invalidates` nor an event
`Cart` hears checked, and nothing replaced the speculation with what the
command committed (ADR-0025's reconciliation). A command now reaches each
entry it speculates on (PW5107).

## 2026-09-26: `pw check` reports each file by its place (E10)

[ADR-0106](DECISIONS/ADR-0106-each-file-is-reported-by-its-place.md):
`pw check` kept each file's diagnostics under its file name, so of two files
named `app.pw` the second's replaced the first's, and `pw check a/app.pw
b/app.pw` passed with an error in `a/app.pw`. Every `just ci` check named two
files `effects.pw`. Diagnostics are paired with their files by position now,
and a file whose name is shared is named by its path.

## 2026-09-26: a cache key names each parameter its entry depends on (E10)

[ADR-0107](DECISIONS/ADR-0107-a-cache-key-names-what-its-entry-depends-on.md):
`query Other(id, other)` with `key id` and a body reading `other` checked, and
two calls differing in `other` shared one cache entry. A `key` or `dedupe_by`
names each parameter the body reads now (PW0336).

## 2026-09-26: a query names a resource that exists (E10)

[ADR-0108](DECISIONS/ADR-0108-a-query-names-a-resource-that-exists.md): the
name check bound a keyword statement's first word, so `let menu = query
Nonexistent(id)` in a page checked, and the graph kept the read as a dangling
edge no rule reports for a page. The resource a `query` or `subscription`
statement names is resolved now (PW0021).

## 2026-09-26: a timeout is a budget above zero (E10)

[ADR-0109](DECISIONS/ADR-0109-a-timeout-is-a-budget-above-zero.md):
`timeout 0.seconds` checked, being a duration, and the resource runtime ends a
flight once its budget is spent, so the query could never answer. A
timeout's domain is a duration above zero now (PW0335); a freshness of zero
is still a promise.

## 2026-09-26: a resumable handler reads what it captures (E10)

[ADR-0110](DECISIONS/ADR-0110-a-resumable-handler-reads-what-it-captures.md):
`resumable() => add_to_cart(item.id, ..)` inside an `{#each}` checked, and
`pw emit-handlers` refused it, "`item` is not bound here": `pw check` passed
a program its build refuses. A handler reads what it captures and what it
binds itself now (PW5025).

## 2026-09-26: a shorthand field reads its capture (E10)

[ADR-0111](DECISIONS/ADR-0111-a-shorthand-field-reads-its-capture.md): a
handler capturing `item` and building `Pick { n: 1, item }` was refused by
PW5017, a program with nothing wrong in it. The capture paths, the handler
artifact and the backend each read a capture through a name or a field path
only. A shorthand field reads its capture whole in all three now.

## 2026-09-26: a call's privacy is the declaration it resolves to (E10)

[ADR-0112](DECISIONS/ADR-0112-a-calls-privacy-is-the-declaration-it-resolves-to.md):
the privacy rules read a callee by its fully qualified spelling, so a bare
imported name carried no label: R-003's secret in markup and R-006's secret in
a public log passed `pw check` written with `import secrets.{ payments }` and
`import log.{ public }`. A callee is resolved as the unit sees it now, and a
value PW5001 refuses a shared cache is not reported again by PW5004.

## 2026-09-26: a resumable handler runs in the browser (E10)

[ADR-0113](DECISIONS/ADR-0113-a-resumable-handler-runs-in-the-browser.md): a
handler calling `Carts.add` itself passed `pw check`, and `pw emit-handlers`
refused it. The page's contract leaves its handlers out, and a handler has
none, so nothing asked where its effects run. What a handler performs itself,
beside the commands it calls, is the browser's to grant now (PW5005). R-010
and a witness flushed a connection from the browser, a second defect, and
are corrected.

## 2026-09-26: what is built before any request reads no request's value (E10)

[ADR-0114](DECISIONS/ADR-0114-what-is-built-before-any-request-reads-no-requests-value.md):
a `placement build` page rendering its `id` passed `pw check`, and the file
it is built into exists before any request supplies one. A build-placed
declaration reads none of its parameters now (PW5026). Whether a build may
enumerate a parameter's values needs a ruling.

## 2026-10-01: a command's `requires` is held before its body runs (E10)

[ADR-0115](DECISIONS/ADR-0115-a-commands-requires-is-held-before-its-body-runs.md):
`requires SignedIn` was parsed and then ignored: no contract or host read it,
so the store command ran for any session. Authorization is now an invocation
precondition on the compiled export. The deployment evaluates every predicate
over the typed command arguments before execution; unknown, denied, malformed
or unevaluated requirements fail closed. Static capabilities remain separate
from per-request authorization.

## 2026-10-01: Wasmtime 48.0.3 replaces the vulnerable 47.x pin (E8/E10)

[ADR-0116](DECISIONS/ADR-0116-wasmtime-48-0-3-replaces-the-vulnerable-47-x-pin.md):
the supply-chain gate found RUSTSEC-2026-0315 and RUSTSEC-2026-0316 in Wasmtime
47.0.4. The host, conformance engine, standalone spike and bootstrapped CLI now
pin 48.0.3. The advisories are not suppressed, and historical measurements keep
their original engine labels.
 
## 2026-10-01: patch brace expansion below Marko's glob layer

[ADR-0117](DECISIONS/ADR-0117-patched-brace-expansion-stays-below-markos-glob-layer.md):
the Node advisory gate found three denial-of-service advisories in
`brace-expansion 5.0.9` through Marko's glob dependency. The 5.x transitive
dependency is pinned to 5.0.12 and the lockfile was regenerated; no advisory
exception was added.

## 2026-10-02: what a declaration reads, it reads through what it calls (E10)

[ADR-0118](DECISIONS/ADR-0118-what-a-declaration-reads-it-reads-through-what-it-calls.md):
the privacy rules read a declaration one call deep, so a public query reading
the session through a helper or another query passed PW5101, PW5004 and
PW5001, and its page's contract allowed `build`. Labels are joined through
what each declaration reads, to a fixed point, and the checker and the
contract share that derivation. A command's reads stay its own.

## 2026-10-02: E10 closes, and the AI benchmark comes next (E10, E14)

[ADR-0119](DECISIONS/ADR-0119-e10-closes-and-the-ai-benchmark-comes-next.md):
E10's five gate items were re-recorded at `bff437c` and pass. Re-recording
found world-level exports refused by ADR-0115's authorization lookup (fixed),
and E7 gate 8 unstable on this machine at HEAD and at the recorded tree alike
(open, E7-G8). Tasks that are not gate items are carried to E15. E14 runs
before E11-E13, harness and tasks first, agents after the owner chooses
models and budget.

## 2026-10-02: three stores, one contract (E14)

[ADR-0120](DECISIONS/ADR-0120-three-stores-one-contract.md): the canonical
store in Next.js 16.3.8 and SvelteKit 2.70.3 (not the day-old 3.0.0) beside
the Pleris store, held to one behavioural contract with mutant stores as
negative controls. Found that the Pleris store's `optimistic` and
`idempotent_by` are checked and not executed; the contract excludes both.

## 2026-10-02: an idempotent command runs once per interaction (E14)

[ADR-0121](DECISIONS/ADR-0121-an-idempotent-command-runs-once-per-interaction.md):
`idempotent_by InteractionId` was checked and read by nothing, so a retried
request added twice. The contract carries it, the runtime sends one
interaction per press, and the host runs an idempotent command once per
interaction through `pw-resource`'s reservation, keeping a bounded number per
session. A request without an interaction, or one reusing an interaction with
other arguments, is refused.

## 2026-10-02: an optimistic transition runs in the browser (E14)

[ADR-0122](DECISIONS/ADR-0122-an-optimistic-transition-runs-in-the-browser.md):
`optimistic` was checked and executed by nothing, its store transition was a
stub, and the backend could not compile the page's count. The compiler emits a
speculation module per page, the browser holds the speculated entry's value
from `entry_value` frames, and a speculation is reconciled by version or
restored exactly. An unpriced optimistic line and the new frame are offered
for reversal.

## 2026-10-02: the development server runs what `pw build` built (E14)

[ADR-0123](DECISIONS/ADR-0123-the-development-server-runs-what-pw-build-built.md):
the server read its commands and contracts from `docs/evidence/` and compiled
the graph in, so a rebuilt program kept running the repository's commands.
`pw build` writes the graph and speculations too, and the server loads one
build directory. Query values remain the server's (E14-Q).

## 2026-10-02: the benchmark harness, and the controls a score needs (E14)

[ADR-0124](DECISIONS/ADR-0124-the-benchmark-harness-and-its-controls.md):
a sandbox per run, isolation checked, grading by checker, build, the shared
contract and hidden tests against the sandbox's own server, and four controls
per task and stack, each requiring tests that ran. T08 holds all four on
three stacks; Pleris's unsafe patch is refused by `pw check`.

## 2026-10-02: scoped signals (proposed)

[ADR-0126](DECISIONS/ADR-0126-scoped-signals.md), **proposed; accepted with changes by ADR-0130**:
UI state as `signal`s declared in a scope, read by name and resolved by the
compiler, written only by handlers, updating only what reads them, dropped
with their scope, typed and resumable. Four rulings are needed before it is
built.

## 2026-10-02: a page shows what its queries return (E14)

[ADR-0125](DECISIONS/ADR-0125-a-page-shows-what-its-queries-return.md): the
development server computed the store page's values itself and ran none of
its queries. The compiler now plans each page (its bindings' queries, each
part's field and member reads), compiles each member function a plan names
as a component, and the server follows the plan. Query policies are the next
slice.

## 2026-10-02: queries run by their declared policies (E14)

[ADR-0127](DECISIONS/ADR-0127-queries-run-by-their-declared-policies.md): the
development server ran every query on every render. Each binding's policy now
travels in the page plan and `pw-resource` applies it: freshness, cache
partition, key (every argument when no `key` is written), retries, timeout,
one flight per key; a commit drops exactly the entries it invalidates. Found
on the way: a page served after a menu change showed the old menu, and two
presses at once rolled both back.

## 2026-10-02: a shared cache holds no one reader's value (E14)

[ADR-0128](DECISIONS/ADR-0128-a-shared-cache-holds-no-one-readers-value.md):
charter §7.8's first must-fail example, `SharedCache<Cart@Session>`, passed
when the cart query was declared `public` and given the session as a
parameter (T12's wrong fix), and when any reader's value was keyed by its
reader. A declaration now observes what it is given, every cache rule reads
that whole label, and no key makes a session's, user's or device's value
shareable; a tenant's, keyed by its organization, is the one a shared cache
may hold. Closes generality's last known gap (31 / 31).

## 2026-10-02: a value's label follows it (E14-L)

[ADR-0129](DECISIONS/ADR-0129-a-values-label-follows-it.md): the sinks read a
value's label, and it was lost in seven ways: through a parameter that states
a label (ADR-0085's contract), through a helper's body, through a branch's
condition, at a sink a branch decides, into an assigned binding, at a public
log taking a session's value, and at PW5003 reading one restriction. An
argument now comes out of a call less a secret its parameter states (a key
the call uses), a call carries what its callee's body makes, a branch carries
its condition to its value and to every sink it decides, an assignment
labels its binding, and a public log takes only a public value. Settles
ADR-0085's open ruling: inference over bodies, keys only for stated secrets,
and declassification only at a host binding until a program needs more.

## 2026-10-02: UI state is a signal; a handler's event is plain data (rulings)

[ADR-0130](DECISIONS/ADR-0130-ui-state-is-a-signal-provided-where-it-lives.md)
accepts ADR-0126 with its four rulings: the keyword `signal`; views composed
at compile time and instantiated at run time (ADR-0072's question, Marko 6's
model); a signal ephemeral or in the URL; and a handler may not assign a
body's `let mut`. It changes two rules: a shared signal is a declaration a
scope `provide`s and views read by its own name, and an inner `provide`
shadows an outer one explicitly.
[ADR-0131](DECISIONS/ADR-0131-a-handler-is-given-its-event-as-plain-data.md)
settles ADR-0058's two rulings: a handler takes its event as a closed record
of plain data read synchronously by the runtime, synchronous behaviour is a
static modifier (`on:submit|prevent`), `bind:value` is shorthand with a codec,
a form submits one typed record, and a handler may read its command's answer.
Neither is built yet.

## 2026-10-02: the resume decision knows the build; a page holds its own UI state

[ADR-0132](DECISIONS/ADR-0132-the-resume-decision-knows-what-the-build-compiled.md):
the browser's resume decision knew the store page's two handlers by name, so
every other handler built and was refused in the browser. It now knows the
build's handlers by identity, from the build the runtime was served with.
[ADR-0133](DECISIONS/ADR-0133-a-page-holds-its-own-ui-state.md): `signal x: T =
v` in a page, changed by handlers that need call no command, read in text and
blocks, held by the browser and rendered again there by the server's renderer
built for the browser. Three rules (PW5300-PW5302) keep it written by
handlers and read where the browser reads it again. ADR-0130's first step.

## 2026-10-02: every handler is resumable

[ADR-0134](DECISIONS/ADR-0134-every-handler-is-resumable.md): a handler not
written `resumable(captures = { .. })` built into a button with no code
behind it, silently. Every `on:` lambda is a handler now, and one derivation
says what it captures: what it lists, or else what it reads that the body
around it binds, never a signal or its own binding. `() =>` takes no
parameter. A handler that is not a lambda is refused at build until the
event is passed (ADR-0131).

## 2026-10-02: a handler's identity is its own file's

[ADR-0135](DECISIONS/ADR-0135-a-handlers-identity-is-its-own-files.md): the
build's map from a handler's place to its identity was keyed by indices
counted within one file, so two pages of one shape in two files shared a key
and one page's button ran the other's handler. Keyed by the file as well.

## 2026-10-02: a view is written where it is used

[ADR-0136](DECISIONS/ADR-0136-a-view-is-written-where-it-is-used.md): a view
used in another was refused (ADR-0072), so no page could be built from views.
A view now composes. Its markup is lowered in place, in the page's one
numbering, each parameter read as the path its prop gives, and a name it
binds renamed where it would hide one. A view's handler keeps its own module,
and the template says where the page holds what it captures. Props are
checked as arguments (PW0619). What a view's handler captures is checked
where the page gives it, and a signal it would capture is refused.
ADR-0130's second step.

## 2026-10-02: what the browser renders again, it can (a correction)

[ADR-0137](DECISIONS/ADR-0137-what-the-browser-renders-again-it-can.md):
ADR-0133's limits said a signal read inside a query's `{#each}`, and a
query's `{#each}` inside a block a signal decides, were refused by the plan.
Each built: the first would have shown its first value forever, and the
browser could not render the second again. The plan now refuses every part a
signal decides that the browser does not render again, and every part of a
block it renders that reads what it does not hold: three more of the same
kind, which ADR-0133 did not list, with them.

## 2026-10-02: a handler is given its event

[ADR-0138](DECISIONS/ADR-0138-a-handler-is-given-its-event.md): ADR-0131's
first slice. A handler takes nothing, or its event's record, `(e:
InputEvent) => ..` or `(e) => ..`, read in the browser's listener before any
code loads. `on:submit|prevent` stops the form's submission there. Seven
defects found building it, each checked or built clean:
- every handler listened for a click;
- a lambda's written parameter type was dropped, in any lambda;
- `(e: T) =>` did not parse, and a handler with a parameter was refused;
- a modifier parsed as two more attributes;
- two handlers on one element wrote two capture attributes, and the second
  was never bound.

## 2026-10-02: a frame is forgotten when the page says it applied it (a correction)

[ADR-0139](DECISIONS/ADR-0139-a-frame-is-forgotten-when-the-page-says-it-applied-it.md):
the stream adapter dropped each frame 25 ms after writing it, though its own
comment said it waited for the page's next request. A stream held for a page
that had been reloaded wrote the new page's frames into a socket nobody read
and dropped them, so a change made within two seconds of a reload never
arrived. Found as the keyed-list suite's intermittent failure. A frame is
dropped now when a page says it applied it.

## 2026-10-02: a page that reads queries holds signals too

[ADR-0140](DECISIONS/ADR-0140-a-page-that-reads-queries-holds-signals-too.md):
the store's route rendered its page from its queries and ignored its signals,
so a store page with one did not render, and T11's dialog could not be
written. It renders each signal's first value now, and its document carries
the signals' manifest, as a page of signals alone does.

## 2026-10-02: a dialog a signal shows is the browser's modal dialog

[ADR-0141](DECISIONS/ADR-0141-a-dialog-a-signal-shows-is-the-browsers-modal-dialog.md):
a `<dialog>` without `open`, in a block a signal decides, is shown with
`showModal()` and closed with `close()`. The browser then focuses it, makes
the page behind it inert and closes it on Escape. The dialog handles
`close` (PW5303), so the signal that shows it hears of every closing,
Escape's included. A dialog nothing shows is refused. Focus goes back to
what invoked it in every engine, WebKit included.

## 2026-10-02: an input bound to a signal

[ADR-0142](DECISIONS/ADR-0142-an-input-bound-to-a-signal.md): `bind:value={s}`
is lowered to `value={s}` and an `on:input` handler setting `s`, and binds a
signal of `String` by its name on a field (PW5304). The browser sets an
attribute a signal decides in place, never from the field's own typing. A
block a signal decides is rendered again only for what it cannot set in
place, so a field bound inside one keeps its focus. Until then a block was
rendered again for every signal read anywhere in it.

[ADR-0143](DECISIONS/ADR-0143-what-names-a-form-control.md): a correction to
PW5014. A form control is named only by something in the same declaration
that reaches it, as the HTML standard and accname 1.2 define:
- a `<label for>` with text, naming the first element with that `id`;
- a `<label>` with text wrapping it;
- an `aria-labelledby` reaching text;
- a non-blank `aria-label`.

Until then any `id` counted as a name, unchecked, so T06's unsafe store and
`tricky.pw`'s field passed with no label, and a wrapping label was refused.

[ADR-0144](DECISIONS/ADR-0144-a-view-holds-signals-and-a-page-provides-them.md):
ADR-0130's third step.
- A view holds its own signals, and each use of it holds its own instance.
- A signal a module declares, `signal drawer: Bool`, has no value: a page
  or view gives it one with `provide`, for everything that body contains.
- A view names it as it names anything it imports, so sibling views share
  UI state with nothing passed by hand.
- A page that needs a signal nothing provides is refused (PW5305). So is a
  second `provide` of one signal in a body, or a `provide` of anything but a
  module's signal (PW5306), and a view holding a signal in a loop's row
  (PW5307).
- One compiled handler serves every instance: the element says which
  instance each signal it names is.
- An instance in a block starts again when the block shows another arm.

[ADR-0145](DECISIONS/ADR-0145-a-page-keeps-what-its-queries-decide-current.md):
E14-Q's third slice. The server renders every list a session's queries fill,
and keeps a page current by difference.
- It records what each document shows, and after a command derives a text
  patch for each part that changed, and keyed list operations: remove,
  insert after the one before, move, and set in place inside an instance.
- One change's patches travel as one `patch_set` frame, held once whole.
- A set the document cannot apply reloads it.

Until then the store patched its cart count and its menu by name, and a page
listing a private query's values did not render. T03 is gradable on Pleris
since.

[ADR-0146](DECISIONS/ADR-0146-a-block-a-query-decides-is-rendered-and-kept-current.md):
E14-Q's fourth slice.
- The page plan names each block a query's value decides at the top of the
  page.
- The server gives the renderer each binding's whole value, and a
  component's case as a case.
- After a command, the server sends `ReplaceRange` for each block whose
  rendering changed, and none for one that renders the same.
- What a loop's row reads of another query is refused, since a row is
  rendered again for its own item alone.

This replaces ADR-0145's refusal of such a block.

[ADR-0147](DECISIONS/ADR-0147-a-query-binding-is-the-query-s-value.md): what
T04 needed, and two corrections it found.
- A page's query binding is the query's value, the `Ok` value of its
  declared result. A `{#match}` over it takes that value apart and covers
  every case. Taking its `Result` apart waits for T10.
- Loops and arms are typed together until nothing more is learned.
- A page whose queries fail is answered 503, and a served one is told to
  reload, where the server had panicked holding its subscriber table and
  failed every request after.
- The development server holds a session's order, which the kitchen's
  benchmark hook sets.

[ADR-0148](DECISIONS/ADR-0148-a-stream-region-shows-its-query-s-state.md):
what T05 and T10 needed.
- A `<stream>` is compiled where ADR-0075 refused it. Its query's delivery
  says when its region is filled:
  - `delivery streamed`: in the same response, after the document, as the
    WHATWG's `<template for>` patch. Chrome 150+ applies it itself; the
    runtime applies it elsewhere.
  - otherwise: with the document, which waits for it.
- PW5400: a streamed query is read only by a `<stream>`.
- PW5401: a stream shows each state its query can be in, and no other.
- PW5402: a streamed query declares a `timeout`.
- The failed arm is given `Option` of the declared error; `None` is the
  host's failure.
- A declared error is given to the readers of its flight and not kept.
- The runtime starts from an inline `import()`, since a deferred module waits
  for the whole response.

Found on the way: the server's handler table missed a handler inside a
stream, and WebKit paints nothing until a page holds about 200 characters of
text.

[ADR-0149](DECISIONS/ADR-0149-pw-diff-what-a-change-means.md): E14-D, gate
item 1.
- `pw diff OLD NEW` checks and builds two programs, and reports what the
  change means in charter §19.2's sections: domain, effects, capabilities,
  privacy, placement, cache, invalidation, pages, client bytes, server
  components, obligations, unsafe and diagnostics.
- A side that does not check is refused, naming its errors.
- A policy no section names is reported, never dropped.

Its first use found that a page streaming a query was charged the query's
capability. The stream's query is now excluded from the page's authority, as
a handler's body is.

[ADR-0150](DECISIONS/ADR-0150-one-change-reaches-a-page-whole.md): two
development-server corrections.
- One change's frames reach a page in one hold of the subscriber table, the
  entry's value from the snapshot the patches came from. They had gone in
  two holds, which an intermittent browser failure exposed.
- A materialized fragment is kept while it shows its query's value, not
  until a command invalidates it.

[ADR-0151](DECISIONS/ADR-0151-a-page-s-values-are-read-outside-the-subscriber-table.md):
a development-server correction, found designing T02.
- A document's values are read, and it is rendered, outside the subscriber
  table. Pages are read at once, and pages asking while a query's flight is
  under way share it, as `concurrency one_per_key` says. They had been read
  one after another, server-wide.
- A change that reaches a session while its page is read is not lost. The
  page is read again, and the third attempt is read inside the table.
- The cart's count is read from the cart's binding alone, where a command
  asked every query on the page for it.

[ADR-0152](DECISIONS/ADR-0152-a-key-a-page-changes.md): what T07 and the
charter's §15.6 tests 6-8 needed. Settles ADR-0089's open ruling.
- A page's query may be given a page signal. The browser reads the binding
  again, for the new key, when the signal changes. The server applies only
  the latest read for a page that is still the session's, and sends what it
  shows as one patch set in the session's frames.
- PW5308: a signal that keys a query is a `String`, an `Int` or a `Bool`.
- PW5309: a query a signal keys declares `on_key_change`.
- `on_key_change`:
  - `cancel` stops the old key's read: the browser aborts it, and the
    server lets go of its flight, which `pw-resource` stops when nobody else
    holds it;
  - `supersede` lets the old read finish, and drops its answer;
  - `keep` reads the new key once the old read has finished, dropping keys
    passed over meanwhile.

  In none is an old key's answer shown for a new key.

Found on the way: the browser ran presses in the order their code arrived.
Each handler now starts after the press before it.

[ADR-0153](DECISIONS/ADR-0153-a-template-tests-a-case-with-match.md): the
first form gate item 5 found open. In a template a case is tested with
`{#match}`, which PW0305 holds to every case: an `{#if}` comparing a value
with one of its cases is refused (PW0337). A template's catch-all arm was
refused already (PW5019).

[ADR-0154](DECISIONS/ADR-0154-a-command-a-page-sends-declares-idempotent-by.md):
the second. A command a page's handler calls declares `idempotent_by`
(PW0338): its request can be delivered twice whatever the program does, and
each press carries an interaction. RFC 9110 lets a client retry only what it
knows to be idempotent.

[ADR-0155](DECISIONS/ADR-0155-the-page-keeps-its-subscription-and-recovers-a-refused-handler.md):
three runtime defects an audit of the store against charter §15 found.
- A failed subscription request ended the subscription: it is asked again,
  with a growing pause, from the cursor the page holds.
- A press on a handler from another build did nothing: it reads the page
  again, once, and never replays the press (§15.6 test 16).
- The recovery codes were read one place off.

[ADR-0156](DECISIONS/ADR-0156-the-benchmark-s-pleris-store-is-its-own-copy.md):
the benchmark's Pleris store is `benchmarks/baselines/pleris`, frozen as the
other two stacks' are. The canonical `examples/store` can then grow toward
charter §15 without moving the 108 patches the tasks are anchored on.

[ADR-0157](DECISIONS/ADR-0157-a-handler-is-answered-what-its-command-did.md):
the first gap the charter §15 audit found, item availability, end to end.
- A handler's call to a command is typed `Result<(), E>`. It learns whether
  the command committed, and the error it declares if not, never the value,
  which reaches the page from the query the command invalidates.
- The server answers `Ok` without its value, or the declared `Err` whole.
- PW0339: a command is called only by a page's handler.
- PW0620: a handler that binds a command's value is refused where it is
  written.
- The canonical store revalidates availability inside `add_to_cart`, and its
  page says when an item sold out (§15.6 test 10).

Found on the way:
- a compiled handler dropped a command whose answer decodes as `()`;
- a kept answer lost `null`;
- a handler matching on its command's answer had no name;
- ADR-0154 named its rule PW0339 for PW0338;
- E14-A's contract ran against the canonical store;
- the value relations behind PW0605 typed a command's call by its declared
  result.

[ADR-0158](DECISIONS/ADR-0158-a-test-leaves-nothing-behind.md): the
repository's tests had left 4,809 directories, 673 MB, in the temporary
directory, and made about 500 more each `just ci`. A Rust test's directory is
a `tempfile::TempDir` now, removed when the test is done with it. The
development server's test store keeps its directory as long as its server.
The harness removes its sandboxes and copied specs however a step ends.

[ADR-0159](DECISIONS/ADR-0159-a-handler-handles-what-its-command-answers.md):
what ADR-0157 left open. A handler's value goes to the runtime, which drops
it. So PW0618 refuses a `Result` a handler gives the runtime: its last value,
a value it returns, or the failure a `?` returns. A handler matches its
command's answer, or discards it by name. A block ending in a binding
compiles, so that discard can end a handler. The canonical store shows every
refusal. The benchmark's store discards each answer by name, its behaviour
unchanged, with T11's patches re-based.

[ADR-0160](DECISIONS/ADR-0160-a-page-s-route.md): the compiler's half of
the audit's route gap.
- The page plan carries a page's route, read by one reader with the link
  check.
- PW0340: a route is `/` and segments, a word or a `{parameter}` each, and
  names each of its page's parameters once.
- PW0621: a route's parameter is text.
- PW0341: one route is one page's.
- The canonical store declares `/stores/{id}`.

[ADR-0161](DECISIONS/ADR-0161-each-document-is-its-own-subscriber.md): the
second step of the route gap.
- The development server kept one subscriber per session, and serving a
  document cleared it, so a change waiting for one tab was lost when
  another was served (a server test failed before).
- Each document is now numbered and has its own subscriber, record of what it
  shows, and keyed reads.
- A change reaches every document of its session, and the browser names its
  document in its subscription.
- This revises ADR-0152's "a page that is still the session's".

Found on the way:
- the browser suite served a build made with an older runtime;
- a stopped mutation run could leave its mutant unseen;
- two earlier mutants survived and are dealt with.

[ADR-0162](DECISIONS/ADR-0162-each-store-at-its-route.md): the third step of
the route gap.
- The development server routes a path by the plans' routes, so the store is
  served at `/stores/{id}`.
- Each document reads its own parameters.
- A second store, 48, has its own menu fragment.
- A change to a store's menu reaches that store's pages only: §15.6 test 11,
  end to end.

[ADR-0163](DECISIONS/ADR-0163-a-page-says-when-it-is-absent.md): the last step
of the route gap.
- A page declares the error that means its address names nothing:
  `not_found_on StoreError.NotFound`.
- PW0342 holds the clause to a case that a query the page reads can fail with.
- The plan carries the case, and the development server answers it 404, any
  other failure still 503.

Found on the way: every response said `OK`, whatever its status.

[ADR-0164](DECISIONS/ADR-0164-a-menu-change-drops-what-declares-it.md): a
correction to ADR-0162's test 11.
- A change to store 47's menu dropped every store's kept menu, by the query's
  name, in the server's code.
- The store's `Menu` now declares `invalidates_on MenuChanged(id)`, and the
  change is the event `MenuChanged(47)`, which drops store 47's entry only.

[ADR-0165](DECISIONS/ADR-0165-the-store-s-estimate-and-recommendations.md):
the audit's third gap.
- The canonical store streams its delivery estimate and its recommendations,
  each in a named region, after its own content (§15.6 tests 3 and 17).
- The recommendations are A-008's: shared, kept ten minutes, dropped by
  `MenuChanged(id)`, which now reaches a stream's kept answer.

Found on the way:
- WebKit paints the store only when its slots are filled, since it holds
  under 200 characters of text: tests 3 and 17 hold in Chromium and Firefox.
  §15.1's descriptions and prices are next.
- A `//` line in markup is text.

[ADR-0166](DECISIONS/ADR-0166-the-store-and-its-items-say-what-they-are.md):
part of the audit's eighth gap, and a correction to ADR-0165.
- `Store` and `MenuItem` declare a `description` (§15.1), and the store's
  page shows them: WebKit now paints it before its slots are filled.
- A host's answer is read through the type the importing component
  declares: fields it does not name are not passed in, and one it names
  that is missing is refused by name. One data layer serves the canonical
  store and the benchmark's frozen copy.
- Correction: ADR-0165's browser suite ran on a stale build. The suite now
  refuses a build whose sources have changed since.

[ADR-0167](DECISIONS/ADR-0167-markup-text-is-text.md): markup text is text,
and a comment in markup is `<!-- -->`.
- The lexer read `//` in markup as a comment. A `//` line between elements
  became page text. `<p>http://example.com</p>` did not parse. An unquoted
  `href=http://x.y` built `href="http"` and broke the next element. An HTML
  comment built a nameless element. None of these was reported.
- The parser now reads markup text, `<!-- -->` and unquoted attribute
  values from the source, as HTML reads them.
- PW5028 refuses a line of markup text that begins with `//` or `/*`.
- Seven rejected fixtures had shown their `// ERROR:` notes as page text;
  they are `<!-- -->` now.

[ADR-0168](DECISIONS/ADR-0168-a-change-reaches-every-part-that-reads-it.md): a
change reaches every part that reads it.
- The store names each Add by its item and says its count in a live region.
  Naming the buttons showed a rename left an attribute stale.
- The renderer now says what changed in an instance, text and attributes
  alike, and a host sets them in place with `SetAttribute`, which the
  runtime now applies.

Found on the way: E7-P moved an instance already in place, which scrambled
the list's anchors; a single patch that addressed nothing hid it.

[ADR-0169](DECISIONS/ADR-0169-a-member-read-is-computed-or-refused.md): what a
template reads through a member function, a host computes or the build
refuses.
- A menu row's `{item.price.display}` was neither planned nor refused. The
  store built, then failed at its first render. Values outside text
  (attributes, block subjects, loop lists) were not looked at by the plan at
  all.
- A loop's row now reads members of its item. A host computes each one for
  each row of a query's list. Every other member read no host computes is a
  build refusal, wherever it is.
- The store shows each price, `$3.50`, written as en-US writes dollars, and
  exact for every amount.

Found on the way: a transport test assumed the cart's frame came before the
menu's, and read one entry under load.

[ADR-0170](DECISIONS/ADR-0170-a-list-inside-a-querys-value.md): a loop over a
list inside a query's value, and a part a speculation would not reach.
- `{#each cart.lines as line}` built, and the development server failed at
  the cart's first change: the plan recorded `cart` as the list. A row's
  member read over it was refused, and one of a number, `quantity.count`,
  had nowhere to go.
- A list is recorded by its path. Its rows read members of their item, and
  a computed value is set in the row whole, by its path.
- The speculation module refuses an attribute, a block's subject or a loop's
  list that reads a speculated value. The page would have shown two values
  of one thing at once.

[ADR-0171](DECISIONS/ADR-0171-an-attribute-that-reads-a-query-is-set-again.md):
an attribute at the top of the page that reads a query's value is set again
when it changes.
- `hidden={cart.lines}` kept its first value for the document's life: a host
  set a text part, a list's rows and a block again, and no attribute at the
  top of a page.
- The plan names each one, and a host sets it with `SetAttribute`, or
  removes a boolean one that goes, as the page's render writes it.

[ADR-0172](DECISIONS/ADR-0172-the-cart-lists-its-lines.md):
the cart lists its lines, and a speculation reaches every part that reads it.
- Each line shows its name, − and + around its quantity, its total and
  Remove, and the cart its subtotal (§15.3). `increase_in_cart`,
  `decrease_in_cart` and `remove_from_cart` are optimistic and idempotent,
  and `add_to_cart` takes the item the page showed, so a new line has its
  name and price before the server answers.
- The browser renders again each attribute, block and loop that reads a
  speculated value, with the server's renderer, rows set where they are and
  focus kept. A region reading what the browser does not hold is refused.
- Corrections: a speculation whose value came before its answer was shown
  twice, and two changes of one session could be sent at once.

[ADR-0173](DECISIONS/ADR-0173-a-command-is-sent-again-where-no-answer-came.md):
a command is sent again where no answer came, and a command that is retried
is idempotent.
- The compiled handler passes the command's `retry` clause where it sends
  it, and the runtime sends the request again, with the same interaction,
  as many times as the clause allows. An answer of any kind is never sent
  again. The store's commands declare `retry transport_only(max = 2,
  jitter = true)` (§15.4).
- Correction: PW0312 let `transport_only` retry a command that is not
  idempotent, though a browser cannot tell a request never sent from one
  whose answer was lost.

[ADR-0174](DECISIONS/ADR-0174-a-store-delay-a-cart-delay-and-a-one-shot-database-error.md):
a store delay, a cart delay, and a one-shot database error.
- `/bench/store?delay=`, one for every reader; `/bench/cart?delay=`, one
  session's; `/bench/fail?next=write|read`, the session's next cart write
  or read failing once as a database that is down does (charter §15.5).
- Correction: the one test of a rolled-back command posted an add of the
  server's own from another session, so it could not fail. It is the page's
  own press now.

[ADR-0175](DECISIONS/ADR-0175-a-network-error-and-a-forced-reconnect-the-server-makes.md):
a network error and a forced reconnect, made by the server.
- `/bench/drop?next=command&at=before|after` closes the session's next
  command connection with no answer, before the command runs or after it
  commits; `/bench/reconnect?for=ms` ends its subscriptions now and refuses
  new ones for a while (charter §15.5).
- A press survives either drop as one mutation, and a page cut off hears,
  once back, what changed meanwhile, in three engines.

[ADR-0176](DECISIONS/ADR-0176-a-regeneration-that-fails-sends-nothing-and-is-tried-again.md):
a regeneration that fails sends nothing, and is tried again.
- `/bench/materializer?fail=next` fails the session's next regeneration
  (charter §15.5). A failed one sends nothing, its stale entry is tried
  again at the session's next drain, which a page's subscription request
  makes, and the commit's answer names a version a later one passes.
- Correction: a failed regeneration's frames went out at a version that had
  not moved, and the page ended a line short, with no error.

[ADR-0177](DECISIONS/ADR-0177-a-public-read-whose-origin-fails-is-answered-with-the-last-value-kept.md):
a public read whose origin fails is answered with the last value kept.
- Charter §15.6 test 18. PW0343 keeps `fallback last_known_good` to public
  data; the query runtime answers a public read whose origin failed with
  the last value kept, checking the manifest's privacy and the kept
  value's; the store's `Store` and `Menu` declare it.
- `/bench/store?fail=next` fails the store's origin once; the page is shown
  with the last store kept, and a session's cart never is.

[ADR-0178](DECISIONS/ADR-0178-whether-an-item-can-be-ordered-is-shown-before-the-press.md):
whether an item can be ordered is shown before the press, and a change to it
reaches every page open.
- Charter §15.1 and §15.2. `MenuItem.available`; a sold-out row says so and
  has no Add; a stock change is `InventoryChanged`, which the `Menu` query
  hears; `/bench/stock?tell=true` tells.
- Every new version of the shared menu fragment reaches the pages that show
  it, as the whole difference, amending ADR-0150: a change at the source
  that a document read had left the pages open a version behind.
- The renderer looks into a block that decides as it did; the value
  analysis types a listener's `_` as any value.

[ADR-0179](DECISIONS/ADR-0179-an-opaque-type-states-its-invariant.md):
an opaque type states its invariant, and every construction and every
boundary holds it.
- `opaque type PositiveInt = Int where value >= 1`: bounds on an `Int`'s
  `value`, joined by `&`; PW0623 refuses any other predicate.
- PW0622: a construction the build cannot show holds it is refused; the
  value analysis bounds the `Int` it is given, and a test narrows it.
- The contract states each boundary's checks, as paths into the value; the
  host holds a command's arguments to them as it decodes them, and a data
  layer's answer before the component reads it.
- Measured before: a forged quantity of 0 committed a line of nothing, and
  −3 a line of minus three.

[ADR-0180](DECISIONS/ADR-0180-a-delivery-estimate-is-a-range.md):
a delivery estimate is a range, and says when it was made.
- Charter §15.1: `DeliveryEstimate { min_minutes: PositiveInt, max_minutes:
  PositiveInt, generated_at: Instant }`, and the platform's `clock` declares
  `Instant`. An estimate of 0 minutes is a failed read (ADR-0179).
- The slot says "Delivery in 25 to 35 min", in words: an en dash in a range
  is read unreliably by screen readers.

[ADR-0181](DECISIONS/ADR-0181-a-menu-is-grouped-by-its-category.md):
a menu is grouped by its category, and a list inside a row is changed where it
is.
- Charter §15.1: `MenuItem.store_id` and `category`; the store's `Menu` answers
  `MenuSection`s, and the page gives each category a heading and its items.
- The plan reads a loop inside a loop (`menu.*.items`); the renderer derives a
  keyed list's change, a list inside a row diffed where it is; every menu
  change is derived from the menu's values, E7-P's operations included.
- A menu's version and a cart's identity stay the runtime's, not fields.

[ADR-0182](DECISIONS/ADR-0182-keyboard-and-screen-reader-semantics-remain-valid.md):
keyboard and screen-reader semantics remain valid.
- Charter §15.6 test 14: the store read by axe's and WCAG 2.2 AA's rules in
  Chromium, Firefox and WebKit, as served and after each kind of change; the
  keyboard's order and presses, the accessibility tree, live regions, reflow,
  a phone's width and reduced motion.
- The runtime writes a part only when what it shows changes: one Add said
  "Items in cart: 1" up to four times. Every page has a viewport.
- The store's title, "Store" for every store, is the page's to declare
  (ADR-0183); axe-core itself is the owner's to install.

[ADR-0183](DECISIONS/ADR-0183-a-page-states-its-title.md):
a page states its title.
- `<title>{store.name}</title>` at the top of a page's view: the host writes it
  into the document's head, and a change to what it reads is set as text,
  as `document.title`. Store 47's page is "Blue Bottle", not "Store" (WCAG
  2.4.2, F25).
- PW5029: a page served at a route states its title. PW5030: a title is the
  page's, once, at the top of its view, written as text and values. A title
  that reads a signal or a speculated value is refused at build.
- Corpus C9: A-025, R-047 and R-048; four routed fixtures given a title;
  generality 33 / 33.

[ADR-0184](DECISIONS/ADR-0184-what-a-cache-may-keep-holds-nothing-of-a-session-s.md):
what a cache may keep holds nothing of a session's.
- Charter §15.6 tests 2 and 13 against the running store. Found: the
  store's page went out with no `Cache-Control`, and a fresh session's
  cookie went on a build's files. A cache could have handed one person's
  cart, or session, to the next.
- A session's response says `private, no-store`; a build's file names no
  session. What the query runtime keeps for every reader, and the
  materializer's public fragments, are the same whether or not anyone
  pressed.

[ADR-0185](DECISIONS/ADR-0185-ids-and-the-aria-that-names-them-checked-at-build.md):
ids, and the ARIA that names them, checked at build.
- Charter §8.2's "duplicate IDs" and "invalid ARIA relationships". PW5031: an
  id names one element of its page, none in a loop. PW5032: a reference
  names an element its page shows whenever the referrer is shown. PW5033:
  ARIA attributes, values and roles are WAI-ARIA's; `aria-labeledby` is
  refused, with `aria-labelledby` offered.
- One mistake, one report: a field left unnamed by such a reference or id is
  not reported again by PW5014. Corpus C10: R-049 to R-051, A-026;
  generality 36 / 36.

[ADR-0186](DECISIONS/ADR-0186-a-page-states-its-description.md): a page
states its description.
- Found by Lighthouse on the store's page: SEO 75, with no meta
  description. A page writes `<meta name="description" content={…} />` or
  Open Graph's `<meta property="og:title" …>` at the top of its view, and
  its host writes it into the head as the page is served. Optional: a
  description is not an accessibility requirement.
- PW5034 follows HTML: one description, `color-scheme`, `application-name`
  and `theme-color`, compared ignoring case; no `media` or `lang`, which the
  head would drop; the host's charset and viewport refused. A
  `<meta itemprop>` is microdata, written in the body where it is, as React
  19 reads it.
- A correction to ADR-0183: a static page that stated its title shipped the
  browser runtime. Corpus C11: R-052, A-027; generality 37 / 37.

[ADR-0187](DECISIONS/ADR-0187-nothing-contained-is-on-screen-when-the-page-is-first-laid-out.md):
nothing the store contains is on screen when its page is first laid out.
- Found by Lighthouse on a phone: CLS 0.136. Every menu item was contained
  with a 42 px placeholder, and items are 125 px, so the cart moved 249 px
  when the browser rendered the three on screen. That was before the first
  paint, and the Layout Instability API, which field data reads, reported it
  all the same.
- An item is contained when 28 items precede it in its list, or 16 lists
  precede its list: at least 2,400 px down, below any first screen up to a
  4K display's. At most 448 items are laid out uncontained, whatever the
  menu. The placeholder is 7.75em, within 5% of an item in three engines,
  and the style is in the head.

[ADR-0188](DECISIONS/ADR-0188-a-page-s-runtime-is-bounded-as-it-is-sent-in-every-run.md):
a page's runtime is bounded as it is sent, in every run.
- A correction to E7's record: gate item 7b's bound, 128 KiB of what the
  store's page downloads to run, had been passed by ADR-0172 at the latest
  (141,339 bytes on 2026-10-04), unseen, as its test ran only in `just e7-performance`, last run on
  2026-08-07.
- Each file is bounded compressed with Brotli at quality 11, as a static
  file is sent: 64 KiB for activation (44,129 bytes), 128 KiB for the
  renderer's WebAssembly (82,888). The byte counts run in every browser
  suite; controls grow each file past its bound.
- For the owner: minifying the runtime, which needs a minifier this session
  did not download.

[ADR-0189](DECISIONS/ADR-0189-a-link-is-written-where-html-allows-it.md): a
`<link>` is written where HTML allows it.
- `<link rel="canonical">` and `rel="icon"` in markup checked, built, and
  were written into the body, where HTML does not allow them and nothing
  reads them. PW5035: a `<link>` in markup has only body-ok relations
  (`stylesheet`, `preload`, `preconnect`...) or is an item's property.
- Corpus C12: R-053, A-028; generality 38 / 38.

[ADR-0190](DECISIONS/ADR-0190-every-page-that-binds-a-query-is-served-at-its-route.md):
every page that binds a query is served at its route.
- E14-Q's next slice. Only the store's page could bind a query: the server
  read the store's plan in 21 places. Now each document records its page,
  and is read, rendered and kept current by its own plan and template.
- The menu's fragment and the speculation stay the store's page's. The store
  gains a second page, its cart at `/cart`: a change made on either page
  reaches the other while both are open.

[ADR-0191](DECISIONS/ADR-0191-every-page-speculates-from-its-own-module.md):
every page speculates from its own module.
- `pw build` wrote every page a speculation module; the server read the
  store's alone, so the cart's page showed a press only when the server
  answered. Now each page carries its own, and each document whose page
  speculates on a value is sent it.

[ADR-0192](DECISIONS/ADR-0192-the-stores-as-the-home-page.md): the stores,
as the home page.
- A delivery site starts with its stores. `Stores.list()`, a host operation
  of the store's data layer, a public `StoreList()` query over it, and a
  `HomePage` at `/`: each store linked to its page, and the session's cart
  counted beside them. `/` was store 47's page, an alias from E7.

[ADR-0193](DECISIONS/ADR-0193-an-order-is-placed-and-its-page-follows-it.md):
an order is placed, and its page follows it.
- The cart's page places its cart as an order: one commit writes the order
  and the emptied cart, and an empty cart places nothing. The order's page at
  `/order` shows its status, kept current as the store moves it along.
- A change the store makes, not a command, reaches the session's open pages
  (`session_changed`), against the order's own entry at an advancing version.

[ADR-0194](DECISIONS/ADR-0194-a-type-that-contains-itself-crosses-as-its-nodes.md):
a type that contains itself compiles, and crosses a boundary as its nodes.
- PW0624 refuses a type no finite value has, `type Loop = Loop { again: Loop }`.
- Through a list, such a type is laid out by its type inside a component. At
  a boundary it is `list<node>` in level order, each list of itself a
  `list<u32>` of node indices: canonical, checked in one pass, encoded and
  decoded with no recursion, and decoded in place. Malformed nodes trap.
- A host holds the value nested, no deeper than 128, and an invariant inside
  a tree holds at every node. Held in place, through another declaration,
  or on the browser's wire, it is refused by name.

[ADR-0195](DECISIONS/ADR-0195-the-owners-rulings-on-fifteen-open-questions.md):
the owner's rulings on fifteen open questions.
- Relayed 2026-10-05: record shorthand by Rust's rule; a case uppercase and
  a lowercase pattern name always a binding; only statement-starting words
  reserved; "type annotation needed" where nothing fixes a result; a bare
  case with a payload resolved like a payload-free one; Float `%` as Rust's
  `rem_euclid` and a Float's text as ECMAScript's `Number::toString`;
  `return` stays a statement.
- Cache keys by information flow; labels on clause keys; `Cart(_)` written
  out, materializations that read others; events returned by the command
  and written in its transaction; a named function as a handler; `<link>`
  origins declared and the CSP generated from them; a speculation never
  invents a value; E7's gate 8 counting attributed long frames.
- Each checked against a primary source and confirmed, `%` refined by
  `rem_euclid`'s own contract, and ordered behind the app layer except
  where it fixes a wrong value.

[ADR-0196](DECISIONS/ADR-0196-only-a-word-that-begins-a-statement-or-an-expression-is-reserved.md):
only a word that begins a statement or an expression is reserved (ADR-0195,
ruling 3).
- PW0013, revision 2: no such word names a binding (a `let`, a parameter, a
  loop's or a pattern's, a signal), and a declaration is not named by a word
  that begins an expression. `let return = n` had checked, its use read as a
  `return`.
- Every other keyword is contextual. The platform's `query`, `measure` and
  `mutate` stand, since a call by a statement word reads as a call.
- The repair suggests a name: `match_`, PEP 8's trailing underscore.

[ADR-0197](DECISIONS/ADR-0197-a-pattern-tells-a-case-from-a-binding-by-its-capital.md):
a pattern tells a case from a binding by its capital (ADR-0195, ruling 2).
- The parser decides once, `pw_syntax::pattern_kind`; six analyses had each
  guessed, and a misspelt case, `Circel`, bound a name that matched every
  value (rustc's E0170).
- A capitalized name its type lacks is PW0608 (revision 3); a case named in
  lowercase is PW0625; `{:Some(Draft)}` is no arm. `check::Env` is gone.
- Corpus C14, generality 41 / 41.

[ADR-0198](DECISIONS/ADR-0198-a-bare-case-with-a-payload-is-the-one-type-that-has-it.md):
a bare case with a payload is the case of the one type that has it
(ADR-0195, ruling 5, its first half).
- `Circle(3)` alone resolves as `Empty` alone does: the name check accepts
  it, the typer relates its payload, the backend builds it, and PW0022 names
  every type where several have it. It was PW0021.
- Resolution from the expected type, for both forms, is the ruling's other
  half, not built yet.
[ADR-0199](DECISIONS/ADR-0199-a-function-or-command-named-as-a-handler-is-the-lambda-that-calls-it.md):
a function or a command named as a handler is the lambda that calls it
(ADR-0195, ruling 12, its remainder).
- `on:submit={save}` is `(e) => save(e)`: compiled as that lambda, and held
  to its event, its answer, idempotency and the browser, as it is.
- A local's value, a page, a type or a case named as a handler is refused
  when checked (PW0614), and a handler named through a module is checked
  against its event (PW0602). Each checked before.
[ADR-0200](DECISIONS/ADR-0200-unit-is-a-value-and-nothing-unread-checks.md):
`()` is the unit value, and nothing the compiler cannot read checks (found
building ADR-0199).
- `()` was an expression that did not parse, typed as anything: `fn f() ->
  Int !{} { () }` checked. It is typed `Unit` now, and built.
- `check_sources` reports a file's syntax errors and keeps the file out, as
  `pw check` does; four tests had passed on programs that do not parse.
- An error node in a file that parses is refused (PW0015), and every `.pw`
  file in the repository is held to that.
- A `{#..}` block left open ends with its element, so PW5019 names it where
  `pw check` had reported the end of the file, twice.
[ADR-0201](DECISIONS/ADR-0201-a-case-written-alone-is-the-expected-types.md):
a case written alone is the case of the type expected where it is written
(ADR-0195, ruling 5, its second half).
- Where several types have `Empty`, the type expected where it is written
  chooses: a result, an annotation, an argument, a field, an assignment, a
  comparison's other side, a list's element, and through branches and arms.
- The typer decides it once and owns PW0022 for it; the backend asks the
  same function, so it builds the case the checker typed.
[ADR-0202](DECISIONS/ADR-0202-a-type-that-holds-itself-in-place-is-boxed.md):
a type that holds itself in place is boxed, and crosses as its nodes.
- `next: Option<Node>` and `Add(Expr, Expr)` compile: each value of such a
  type is the address of its cell, a `u32` of its own in the private
  layout, built, read and matched through the cell. The IR is unchanged.
- At a boundary a node's slot is a `u32` for a box and an `option<u32>` for
  an option of one, beside ADR-0194's `list<u32>`, in the same level order,
  encoded and decoded in place without recursion.
[ADR-0203](DECISIONS/ADR-0203-a-view-that-contains-itself-is-an-instance-made-at-run-time.md):
a view that contains itself is an instance of its own template, made at run
time (ADR-0130, ruling 2).
- A reply thread's view shows each reply as itself, as deep as the data:
  each use is an `instance` part, rendered in a frame of its own, its
  template compiled once. One with no block on the way back to it, or
  holding a signal, is refused (PW5020).
- The renderer writes each instance after the markup around it, not on its
  stack, and refuses a page nesting more than 500 elements, under what
  Blink and WebKit's parsers nest (512).
- The browser reads each instance in its own template: its parts, its
  range, and its handlers, bound in every instance.
[ADR-0204](DECISIONS/ADR-0204-an-element-holds-the-children-html-permits-as-the-page-holds-them.md):
an element holds only the children HTML permits, as the page holds them
(found building ADR-0203).
- PW5012 read only the elements written inside: `<ul><Thread /></ul>` was
  refused though `Thread` renders an `<li>`, and a `<div>` row of an
  `{#each}` in a `<ul>` passed. A block's rows and what a view renders are
  read now.
[ADR-0205](DECISIONS/ADR-0205-a-value-that-contains-itself-crosses-the-browsers-wire-as-its-nodes.md):
a value of a type that contains itself crosses the browser's wire as its
nodes (ADR-0194's next step).
- `{ "$graph": [node, ...] }`, each value of the type inside a node
  `{ "$node": k }`, in ADR-0194's level order: a signal's first value, what a
  handler reads and sets, and a command's argument, which the host passes to
  the component as its nodes.
- The renderer's values are dropped, cloned and compared without recursion.
[ADR-0206](DECISIONS/ADR-0206-a-case-has-one-name-where-values-are-rendered.md):
a case has one name where values are rendered, its WIT case's (found
building ADR-0205).
- The template and a host's values named the language's four `Some`,
  `None`, `Ok`, `Err`, and the browser's wire `some`, `none`, `ok`, `err`, so
  a `{#match}` on an `Option` a signal holds found no arm. One name now, the
  wire's.
[ADR-0207](DECISIONS/ADR-0207-a-data-source-states-what-it-guarantees.md):
a data source states what it guarantees, and nothing asks it for more (the
owner's priority 21; ADR-0195's ruling 11).
- `source X  holds A, B  transactions …  reads …  changes …`, stated by the
  program: what a database gives depends on how it is deployed.
- A query's consistency, a command's isolation, events and idempotency, and
  its writes in one source are held to it (PW0344-PW0349). A resource no
  source holds is the host's database's.
[ADR-0208](DECISIONS/ADR-0208-a-command-computes-its-events.md): a command
computes its events, and the outbox commits them with its writes
(ADR-0195's ruling 11's other half).
- Each event is a function of the platform's outbox,
  `pw:host/outbox#cart-changed`, which the command calls before its body
  with the values it computed. Emitting is `outbox.write`, which a node
  grants where it keeps an outbox.
- The server stages them and commits them with the writes; it evaluates no
  key's text for an event. Found: an `Int` key missed its entry, and an
  empty value reached every entry.
[ADR-0209](DECISIONS/ADR-0209-a-command-computes-the-entries-it-invalidates.md):
a command computes the entries it invalidates (ADR-0195's ruling 11,
completed).
- Each query a command invalidates is a function of the platform's
  invalidations, `pw:host/invalidations#store-page-cart`, called before its
  body with the values it computed, under `outbox.write`.
- The server drops the entry by that key once the writes commit, and reads
  no key's text: until then anything but `current_session()` dropped every
  entry of the query.
[ADR-0210](DECISIONS/ADR-0210-the-owners-rulings-on-the-pre-delegation-marks.md):
the owner's rulings on the pre-delegation marks, ADR-0031 to ADR-0122,
relayed as ADR-0195's were.
- 85 marks: 26 overruled, 30 confirmed, 29 already settled. Ten soundness
  defects the sweep found come first.
- Each is built in its own ADR, which may overrule it with research.
[ADR-0211](DECISIONS/ADR-0211-a-path-that-leaves-a-loop-owes-its-releases.md):
a path that leaves a loop's body leaves the function, and owes its releases
(ruling 0045-a; ADR-0210's urgent defect 1).
- A `return` or failing `?` in a `for` body left a transaction open and
  checked; the correct roll-back-then-return was refused. Both are fixed.
[ADR-0212](DECISIONS/ADR-0212-a-query-reads-a-query.md): a `query` reads a
query or a resource, and a `subscription` a subscription (ruling 0108-a;
urgent defect 6).
- `let v = query helper(n)` over a `fn` checked. PW5108 refuses the name
  where it is read, by its kind.
[ADR-0213](DECISIONS/ADR-0213-the-maximum-of-zeros-is-plus-zero.md):
`List.maximum` gives +0 over −0 (urgent defect 7).
- It kept the first, and its test compared by value, under which the two
  are equal. It compares bits now, as IEEE's `maximum` orders them.
[ADR-0214](DECISIONS/ADR-0214-an-opaque-types-value-is-its-representation.md):
an opaque type's own module reads its representation as `.value`, and
declares no member of that name (urgent defect 8).
- `LayoutSnapshot`'s accessor was `value`, which shadowed the representation
  in `browser`. It is `measured`, and PW0626 refuses the shape.
[ADR-0215](DECISIONS/ADR-0215-a-querys-retry-reaches-its-runtime.md): a
query's `retry` reaches its runtime as declared, and `fixed` is no strategy
(ruling 0089-b; urgent defect 4).
- The plan carried a query's attempts alone, so `jitter = false` ran with
  jitter, and `fixed` ran as exponential. The plan carries its jitter, and
  `fixed` left the language.
[ADR-0216](DECISIONS/ADR-0216-every-clause-belongs-to-a-declaration-that-reads-it.md):
every clause belongs to a declaration that reads it, and a code body admits
none (rulings 0092-b and 0047-a's interim; urgent defect 5).
- PW5105 placed four heads and let 61 check anywhere: `freshness` on a
  command and `retry` on a function were read by nothing. Every head has a
  place now, and the table fails closed.
- A policy word in a function's body was a clause whose words were never
  resolved. It is a name now.
[ADR-0217](DECISIONS/ADR-0217-a-handlers-captures-are-set-again.md): what
a handler at the top of the page captures is set again when it changes
(urgent defect 2).
- A row's captures were patched with the row. A top-level handler's were in
  no plan, so a button capturing the cart sent the cart the page was first
  rendered with. The plan, the server and a speculation set them again now.
[ADR-0218](DECISIONS/ADR-0218-a-host-serves-any-program-its-data-is-the-deployments.md):
a host serves any program, and its data is the deployment's (the feed
reference app's first step).
- The development server served the store alone: its machinery and the
  store's data were one. The split is made in place, in steps, each keeping
  every test green. First the store's state, reads, command staging and
  grants move into `StoreData`, and a build importing what no layer supplies
  is refused at start.
[ADR-0219](DECISIONS/ADR-0219-what-a-commit-drops-reaches-every-session-that-reads-it.md):
what a commit drops reaches every session that reads it (the feed reference
app's second step).
- A commit told the session that made it. Another reader's open timeline
  saw a post when the page was next loaded. Each other session whose open
  page reads a query the commit dropped, whole or by a shared key, is read
  again and sent the change, after the author is answered.
[ADR-0220](DECISIONS/ADR-0220-the-feed-is-served-in-browsers.md): the feed
is served in browsers by the host that serves the store (the feed's third
step).
- A browser opened its stream at once, and the drain regenerated the store's
  cart for every program: the feed's page was told to reload, for ever. A
  layer with no session entry drains nothing, a page's style is its layer's,
  and a guest is named for its session. The feed runs in three engines on
  hosts of its own.
[ADR-0221](DECISIONS/ADR-0221-a-form-controls-value-is-written-where-html-reads-it.md):
a form control's value is written where HTML reads it (found by the feed).
- `bind:value` on a `<textarea>` was written as an attribute it does not
  have: a first value showed nothing until a script ran. It is written as
  the textarea's text now. PW5036 refuses a `<select>`'s value, which is the
  option it marks `selected`, and what no change would set again.
[ADR-0222](DECISIONS/ADR-0222-a-post-is-shown-before-the-server-answers.md):
a post is shown before the server answers (the feed's optimistic posting;
ruling 0105-a's unnamed key).
- Speculation was the store's cart's: the server sent no other value. A
  target may leave a key unnamed, `_`, and the server sends any value a page
  speculates on, read with what the page shows, when it changes. The feed's
  post shows first, by "You", and is the server's when its value arrives.
[ADR-0223](DECISIONS/ADR-0223-a-streamed-region-is-filled-when-its-whole-arm-has-arrived.md):
a streamed region is filled when its whole arm has arrived (found by an
intermittent `slots.spec.mjs`).
- The runtime applied a `<template for>` as soon as it saw one, and a
  response that arrived in parts gave a region the part of it parsed: one
  recommendation of two. It waits for the comment the renderer writes after
  each template now. The slots' tests hold the recommender instead of
  outwaiting a delay half a second short of the query's timeout.
[ADR-0224](DECISIONS/ADR-0224-a-longer-read-is-not-applied-over-a-commit-it-did-not-see.md):
a longer read is not applied over a commit it did not see (found by
ADR-0222).
- A keyed read read outside the session's hold and applied its value after,
  so a commit between the two was sent first and then undone by the older
  value. It applies in the hold now, and reads again when a change reached
  its document while it read.
[ADR-0225](DECISIONS/ADR-0225-a-strings-length-is-an-invariant.md): a
`String`'s length is an invariant (the feed's typed length limit; extends
ADR-0179).
- `opaque type PostText = String where String.length(value) >= 1 &
  String.length(value) <= 280`: bounds on a length in code points, as
  `String.length` counts it. Every construction is shown to hold them at
  build, and a host holds a browser's post to them. The feed's post is one.
[ADR-0226](DECISIONS/ADR-0226-a-value-the-template-computes-compiles.md): a
value the template computes compiles (ruling 0073-a, the host's part; the
feed's counts).
- A text hole or an attribute's whole value may compute from one query's
  value at the top of a page. The compiler lifts it into a function, a
  component of its own, which the host runs when the page renders and again
  when the value changes. It performs nothing (PW0334, revision 2). Found:
  a member an imported module lacks resolved, and checked clean.
[ADR-0227](DECISIONS/ADR-0227-a-value-computed-from-a-signal-is-the-browsers.md):
a value computed from a signal is the browser's (ruling 0073-a, the browser's
part; the feed's draft).
- A text hole or an attribute's whole value may compute from one signal. The
  host renders its first value with the component ADR-0226 lifts; the build
  compiles the same function into the page's module, which the browser loads
  when the signal first changes. The feed's draft says what is left, and its
  post button is disabled while there is no post to send.
[ADR-0228](DECISIONS/ADR-0228-a-value-computed-in-a-row-is-the-rows.md): a
value computed in a row is the row's (ruling 0073-a in a loop's row and from
a speculated value; the feed's likes).
- A value computed from a row's item is named by a path from the item, and a
  host computes it for each row as a member read; the speculation module
  computes it for each row it renders. The feed's rows say their likes in
  words, and a like is shown before the server answers. Found: a field of a
  value was charged a same-named function's effects, and an opaque value did
  not compare.
[ADR-0229](DECISIONS/ADR-0229-a-computed-condition-decides-its-block.md): a
computed condition decides its block (ruling 0073-a for a block's subject and
inside a block; 0071-a's repair).
- `{#if}`, `{:else if}` and `{#match}` read a computed subject by a path the
  compiler names: a host's from a query's value, its block rendered again
  when it changes, and the browser's from a signal's, its block rendered
  again as the signal changes. A value computed inside a block a host renders
  is the host's. The feed's draft too long says so; a thread with no reply
  says that.
