# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

**Reviewed:** 2026-09-25, against master `1e2d0de` plus the exhaustiveness
change below.
**Charter:** v2, `PROJECT_CHARTER.md`.
**Numbering:** engineering E0-E15, public proofs P0-P9, risk experiments RQ-*.

**Current milestone:** E10. **All five gate items have recorded evidence as
of 2026-09-25** (see below). Closing E10 is left to the architect's review, with
this charter task not done: task 2's Wasm for compute-heavy modules in the
browser (its JavaScript modules for pure computation are ADR-0044's). Task
4's evaluation of memory strategies is ADR-0046's.

Task 7's affine annotations are the effect rows `resource.acquire<T>` and
`resource.release<T>`, and since ADR-0045 the invariant they express is
checked on every path.

 **E10-I closed 2026-09-24**
([evidence](evidence/E10/e10-i-2026-09-24.md), [ADR-0032](DECISIONS/ADR-0032-compiled-components.md)):
`add_to_cart` compiles to a Wasm component, runs through the E8 host with the
host's own operations, and the dev server's Rust closure path is deleted.
**Resumable handler bodies compile to JavaScript modules since 2026-09-25**
([evidence](evidence/E10/handlers-2026-09-25.md), [ADR-0033](DECISIONS/ADR-0033-compiled-handlers.md)),
so the store's behaviour is compiled from `.pw`: the page's handlers and the
commands they reach. E9 closed on 2026-09-24 (E9-V1..V6,
[evidence](evidence/E9/value-relations-2026-09-24.md),
[ADR-0031](DECISIONS/ADR-0031-value-relations.md)). Next: the rest of E10, and
the kiokun proof slice. See [NEXT](NEXT.md).

**Correction, 2026-09-25:** GitHub CI failed on the pushed E10-I merge
`26feb6b`, although `just ci` had passed locally. The dev server's `engine`
feature turned on E8's guest tests in every workspace build, and a clean
checkout has no guests. Fixed in `83af93c`, and CI is green on it. Recorded in
[the E10-I evidence](evidence/E10/e10-i-2026-09-24.md).

**Correction, 2026-09-25: ADR-0011's exhaustiveness rule was not enforced for
most matches, and the analysis proved some incomplete ones exhaustive**
([ADR-0038](DECISIONS/ADR-0038-every-match-typed.md)). ADR-0011 requires `pw` to
reject an incomplete match whatever the effect row.
- The checker analysed only a scrutinee that was a parameter annotated with a
  sum type the program declares. A match over a call, a field, a local, or any
  `Option` or `Result` was never analysed, so `match get(w) { Some(x) => .. }`
  passed `pw check`. The kiokun slice found it: the backend refused such a
  match while compiling `Lookup`.
- Fixing it found four ways the analysis reported an incomplete match as
  exhaustive, all present at `1e2d0de`:
  - nested patterns were read against the scrutinee's type, so
    `Circle(Draft)` covered `Circle(Sent)`;
  - a constructor its type lacks read as a wildcard;
  - a shadowed parameter was read as the parameter;
  - `=> return Ok(())` parsed as an arm ending at `return` plus a phantom arm
    whose pattern was `Ok(())`, with no error, and the phantom arm read as a
    wildcard.
- All five are fixed, with tests in
  `compiler/pw-core/tests/match_exhaustiveness.rs`. A constructor its type
  lacks is the new PW0608. What the analysis still cannot read (a literal, a
  pattern nested under `Some`) blocks it, with the reason, and is never proven
  (KNOWN_LIMITATIONS).
- Mutation controls: 14 of 14 killed. Each of the 14 pieces of the fix,
  undone in turn, fails the tests ([match.txt](evidence/E10/match.txt),
  `just e10-match`). The E9
  evidence and the E10 oracle were re-recorded at the same commit, `f08672b`:
  E9's counts and its 10 of 10 mutants are unchanged, and the oracle's missing
  case is now the checker's refusal.

**Correction, 2026-09-25: "kiokun's ranking" is the slice's port**
(ADR-0041, corrected). Every agreement ADR-0041 reports is with
`data::Index::search`, ADR-0037's Rust port. kiokun.com's current
`/api/search` takes hangul and CJK punctuation as CJK and astral Han as Latin,
and searches script variants (地図 ⇄ 地圖); the port does neither.

**Correction, 2026-09-25: PW2005 enforced less than its invariant**
([ADR-0045](DECISIONS/ADR-0045-affine-exactly-once.md)). "An affine value
must be consumed exactly once" was checked as "released before each
`return`". A transaction never ended, one live across a failing `?`, and one
ended twice all passed `pw check`, and a declaration promising to end a
transaction parameter was never held to it. Every path is counted now.

**2026-09-25: recursion and generic callees compile**
([ADR-0050](DECISIONS/ADR-0050-recursion-and-generic-callees.md)). A call is
still inlined, and one that recurses is compiled beside its export, once per
instance, and called: in the Wasm component and in the JavaScript module
alike. A generic callee is instantiated from its arguments. Programs with no
recursion compile as they did: the store's and kiokun's 19 artifacts are
byte-identical to `fd95b59`'s. Evidence: [recursion.txt](evidence/E10/recursion.txt)
(`just e10-recursion`), 7 of 7 mutants killed.

**2026-09-25: a string's escapes are the language's**
([ADR-0049](DECISIONS/ADR-0049-string-escapes.md), settling A-023). One
decoder reads a string token: `\n` `\t` `\r` `\\` `\"` `\{` `\}` and
`\u{..}`, holes, and raw `"""` strings. An escape it does not define is
PW0014. The backends refused such strings, or passed the token to their
targets' rules. Each now encodes the value in its own syntax, Koka's checked
against Koka 3.2.3. The Marko adapter rendered an interpolated string as its
token, braces and all; it joins the pieces now. Evidence:
[strings.txt](evidence/E10/strings.txt) (`just e10-strings`), 16 of 16
mutants killed.

**Correction, 2026-09-25: a member no type has was never refused**
([ADR-0048](DECISIONS/ADR-0048-members-exist.md)). A read or call through a
value is related to its type's members now (PW0610). Eight reads in code that
checked clean named members nothing declares:
- the store's page rendered `{cart.line_count}`, and `Cart` has only `lines`;
  the dev server filled the path from its own state. `domain` now declares
  `line_count`, the count it was computing.
- A-015 read `box.x` and `box.bottom`, called `anchor.bounds()`, and assigned
  `self.style.transform`. It now uses the platform's declared geometry and
  setter, and its claim is witnessed by the effect analysis for the first
  time.
- A-001 read an opaque `PositiveInt`'s `.value` from another module, and
  A-016 and A-018 read members their types lack.

An opaque type's `.value` is its representation in its own module, and
refused elsewhere. Evidence: [members.txt](evidence/E10/members.txt)
(`just e10-members`), 10 of 10 mutants killed; the audit decides 28 member
reads in the accepted corpus and leaves 26 undecided. E9's audit is
re-recorded at the same commit ([value-relations.txt](evidence/E9/value-relations.txt)).

**Correction, 2026-09-25: a name used as a value was never resolved**
([ADR-0047](DECISIONS/ADR-0047-every-name-resolves.md)). `PW0021` examined
calls and qualified paths only, so `let x = nothing` and `{nothing.here}` in a
template passed `pw check`. Every name is now resolved in lexical scope. The
walk found two parse defects in accepted code:
- `derived`, the charter's computed value (§7.5), was a bare name. So
  `let total = derived widths |> List.sum()` parsed as `let total = derived`
  and a discarded statement: A-016 and A-018 computed nothing into `total`,
  `max` and `columns`. It is one expression now.
- `observe intersection(self, threshold = 0.1)` read its named argument as an
  assignment to an undeclared name.

It also found six fixtures reading a session that nothing declared, and one
form submitting to an undeclared handler. Each is corrected, and each is
caught or clean exactly as before. Evidence:
[names.txt](evidence/E10/names.txt) (`just e10-names`), 20 of 20 mutants
killed.

**Correction, 2026-09-25: `pw check` did not type an operator's operands**
([ADR-0043](DECISIONS/ADR-0043-operands-are-typed.md)). `1 == "a"` checked,
and so did two accepted fixtures that divide a `Float` by an `Int`: A-017 and
A-021. ADR-0039 §2 refuses both. PW0609 relates each operand to the type its
operator takes. The fixtures now convert with the new `Float.from_int`, and
their claims are unchanged.

**Correction, 2026-09-25: `{:else}` in a template was a silent miscompile**
([ADR-0042](DECISIONS/ADR-0042-template-branches-and-attributes.md)).
`{#if a}A{:else}B{/if}` passed `pw check` and `pw build`. It rendered A and B
together when `a` held, and neither when it did not. The HIR lowering dropped
every `{:..}` marker. Nothing checked that a block closed with its own name,
and an unknown directive passed `pw check`. All are fixed:
- markers are kept as branches;
- a malformed block is PW5019;
- a template `{#match}` is exhaustive (PW0305).

**Correction, 2026-09-25: a secret in an attribute string checked clean.**
`<a href="/pay/{key}">` with `key: Secret<Payments>` passed `pw check`: the
hole was static text, which no privacy rule reads. Neither renderer
interpolated attribute strings, so nothing leaked. An attribute's holes are
expressions now (ADR-0042), and the page is PW5003.

**Correction, 2026-09-25: E9's value relations left a generic call's `let`
binding uninstantiated** ([ADR-0041](DECISIONS/ADR-0041-kiokun-in-pleris.md)).
E9 claims generic callables are instantiated per call.
- `infer.rs` typed `let ys = List.filter(xs, ..)` with `filter`'s declared
  `List<T>`, and the value relations skip names already bound. So `ys` stayed
  `List<type parameter 0>`.
- A later call over `ys` was then refused (PW0605) for a mismatch the typer had
  made.
- Such a binding is now solved like any call, with a regression test in
  `compiler/pw-core/tests/value_relations.rs`. The E9 evidence is re-recorded.

**Correction, 2026-09-25: a statement keyword could name a value.**
`let query = ..` parsed. Then every later `query` in an expression read as a
`query ..` statement, and the program checked while meaning something else.
The parser now refuses such a name (PW0013, ADR-0041, ruling needed).

Decisions awaiting a ruling:
- ADR-0050: a callee is inlined until it recurses, and only the recursion is
  a call, rather than every declaration being a function of its component.
- ADR-0048: an opaque type's representation is read as `.value`, only in the
  module that declares it, rather than by a constructor pattern.
- ADR-0047: clauses written as statements (`scope component`,
  `release(h) { .. }`, `view { .. }`) are recognised by position, from the
  policy table, rather than re-parsed as policies; `derived` is reserved and
  cannot name a binding (PW0013).
- ADR-0046: whether the next performance work is instance reuse, which the
  measurements favour, rather than a memory strategy.
- ADR-0045: a release in a loop is refused even when the loop runs once; the
  body's value moves a resource to the caller; a declaration whose row
  releases `T` owes its `T` parameter one release on every path.
- ADR-0044: a module's values: `BigInt` for `Int`, objects keyed by Pleris
  field names, and `{ $case, value }` for `Option` and `Result`.
- ADR-0043: `Float.from_int` names the `Int` to `Float` conversion; there is
  no implicit one.
- ADR-0042: `{#match e}{:Some(x)} .. {:None} .. {/match}` is the template
  syntax for taking an `Option` or a `Result` apart.
- ADR-0042: a value interpolated into a URL attribute is one URI component,
  and the URL must begin with the author's text.
- ADR-0041: a binding or parameter cannot be named with a statement keyword
  (PW0013); contextual keywords are the alternative.
- ADR-0040: `String.to_lower_ascii` maps `A`–`Z` only; Unicode case mapping is
  not decided.
- ADR-0039 §1: `Int` traps where its exact result does not fit; `/` and `%`
  are Euclidean, as Koka's are; a zero divisor traps, where Koka answers 0.
- ADR-0038: a bare pattern name is a constructor whenever some type has one of
  that name, so a binding cannot share a constructor's name.
- ADR-0038: `return` stays a statement, whose value is the next statement,
  rather than becoming an expression.
- ADR-0032: the contract locates each export in its component.
- ADR-0033 / A-022: captured values carried on the element are resume
  metadata, not identity markup.
- ADR-0049: the escape set (`\n` `\t` `\r` `\\` `\"` `\{` `\}` `\u{..}`), and
  `"""` strings are raw.
- (settled by ADR-0049) ADR-0033 / A-023: a string literal has a value only where no escape rule is
  involved, until the language defines escapes.
- ADR-0034 §3: a placeholder is not a refusal until something depends on it.
- ADR-0037 §3: kiokun's share-alike data (CC-CEDICT, JMdict, Tatoeba) lives in
  this repository, attributed; the alternative is to keep only the script.

ADR-0032's other ruling-needed item, the handler sending the pressed loop
instance, is superseded by ADR-0033.

Three ADR-0031 decisions were made without an architect ruling and are offered
for reversal:
- privacy qualifiers are not value-transparent (§7);
- function types exist (§4);
- snapshots are read through `.value` (A-020).

The former status file mixed chronological notes with obsolete headlines such as
"E9 is complete" and "no benchmarks yet". Its complete bytes are preserved in
[the historical ledger](STATUS-history-2026-09-15.md). That ledger records earlier
observations, not the current completion state. No old raw evidence is rewritten.

## last passing commit

Baseline master `c200ae3e39934285858064fa56a09dabe882657d` includes the
recursive-type prerequisite (PR #4) and concurrent resource repair (PR #5). The new change's final-head CI must pass
before integration; the baseline pass is not a substitute.

## completed gate items

- **2026-09-25: E10 task 4, memory strategies evaluated (ADR-0046).** The host
  reports each call's instructions, peak linear memory and instantiation
  time. Every compiled kiokun query was measured on the whole shard. 98.9%
  of Search calls stay in their first 64 KiB page; the heaviest, `T`, grows
  its region to 3.2 MB. A fresh instance (7–14 µs) costs more than a `Place`
  or `Lookup` call. Invocation regions stay; the revisit conditions are
  written down. Evidence: `docs/evidence/E10/memory.txt` (`just e10-memory`).

- **2026-09-25: E10 task 2, pure computation as JavaScript modules
  (ADR-0044).** A query that reaches no host compiles to an ES module from
  the component's own IR. The module and the component agree on 6,800
  generated calls under Node, kiokun's shard rule among them, and ten mutants
  that each restore one of JavaScript's own semantics are caught. `pw build`
  writes the modules. A handler that computes, and Wasm in the browser, are
  not done. Evidence: `docs/evidence/E10/javascript.txt`
  (`just e10-javascript`).

- **2026-09-25: operands are typed (ADR-0043).** Not a gate item; the checker
  gap KNOWN_LIMITATIONS named. PW0609 relates operands and conditions to the
  types they take, and `Float.from_int` converts. Evidence:
  `docs/evidence/E10/operands.txt` (`just e10-operands`).

- **2026-09-25: the template gaps (ADR-0042).** Not a gate item; `docs/NEXT.md`
  named them after ADR-0041.
  - `{:else}` and `{:else if}` are branches.
  - `{#match}` takes an `Option` or a `Result` apart, exhaustively, with its
    payload bound and typed.
  - An attribute interpolates, and each value is escaped for its context. In
    a URL, each value is one URI component.
  - kiokun's two word pages are one, `WordPage`, and its search links are
    `href="/{hit.target}"`. An entry now shows its Korean words, Japanese
    names and character, with nested `{#match}` over `Option<Int>` fields
    (ADR-0037, amended). All 17,597 entries of the whole shard render.
  - Found and fixed: `{:else}` was a silent miscompile, and a secret in an
    attribute string checked clean (corrections above).
  - Evidence: `docs/evidence/E10/templates.txt` (`just e10-templates`).

- **2026-09-25: kiokun's shard rule and ranking, in Pleris (ADR-0041).** Not a
  gate item; the test of E10 task 2's computation that `docs/NEXT.md` named.
  - `examples/kiokun/Shards.pw` and `app.pw` state kiokun's rule and ranking.
    They compile to `shards.Place`, `shards.Places` and a 12 KB
    `kiokun.page.Search`, and the host keeps only the index and the files.
  - On a kiokun-data checkout, the compiled rule agrees with kiokun's on all
    1,485,890 words.
  - The compiled ranking agrees with kiokun's Rust ranking on 28,951 queries
    and 77,225 hits over the whole shard. p50 is 0.25 ms a query.
  - A lookup now reaches every shard, so all 17,597 of the shard's entries
    render, stubs included.
  - Found and fixed:
    - the E9 `let` defect and the keyword names (corrections above);
    - `List.group_by` was missing;
    - kiokun's file names are not its words: its builder writes nine characters
      as `_`, and 33 files are escaped;
    - a component call per word made the load three times slower. Batched,
      the load took 8.6 s to 10.8 s across four runs, against 9.5 s with
      kiokun's Rust rule.
  - Evidence: `docs/evidence/E10/kiokun.txt` (`just e10-kiokun`, with
    `KIOKUN_DATA`) and `docs/evidence/E10/kiokun-mutants.txt`
    (`just e10-kiokun-mutants`).

- **2026-09-25: E10 task 2, the standard library compiled (ADR-0040).**
  - `List`'s operations and a new `String` module are `intrinsic`
    declarations the component backend compiles; the placeholder bodies are
    gone.
  - Each operation agrees with Rust's `Vec`, `str` and `char` over generated
    inputs, trapping where they have no value.
  - Found and fixed: `(a, b) => e` had never parsed.
  - Evidence: `docs/evidence/E10/stdlib.txt` (`just e10-stdlib`).

- **2026-09-25: E10 task 2, pure computation compiled to Wasm (ADR-0039).**
  Not a gate item; the backend breadth `docs/NEXT.md` puts first.
  - The component backend compiles `Int` and `Float` arithmetic, comparisons,
    `&`, `|`, `!`, `if`, string literals, interpolation, records, and calls to
    other declarations, inlined.
  - Every operator agrees with an exact `i128` reference over generated
    inputs, trapping exactly where the reference has no 64-bit value, and with
    Koka 3.2.3 wherever Pleris produces a value.
  - Found and fixed on the way: a query body the parser dropped with an error
    only the CLI saw; a backend that compiled programs that did not parse; a
    Koka backend that emitted invalid negation; an import missing when it was
    called only inside a `match` arm.
  - Evidence: `docs/evidence/E10/pure.txt` (`just e10-pure`).

- **2026-09-25: E10 gate item 2, and the kiokun slice.**
  - The backend compiles `match` over `Option` and `Result`, field reads, and
    `Some`/`None`/`Ok`/`Err` (ADR-0036). The store's components are
    byte-identical afterwards.
  - The kiokun slice (ADR-0037): entry lookup and search over one real shard
    of kiokun.com.
    - On the whole shard, the compiled `Lookup` and `EntryPage` looked up and
      rendered 16,921 entries and followed all 103 in-shard redirects, with 0
      failures.
    - 15/15 in three browser engines, with JavaScript on and off.
  - The differential oracle: all seven compiled declarations agree with
    independent Rust references over 300 generated cases each, and three wrong
    references are caught. Evidence:
    [oracle-2026-09-25.md](evidence/E10/oracle-2026-09-25.md) and
    [kiokun-2026-09-25.md](evidence/E10/kiokun-2026-09-25.md).
  - Found:
    - the platform package depended on the store example;
    - interpolated attribute strings rendered literally;
    - the checker did not check exhaustiveness over `Option` and `Result`
      (corrected the same day, with four related false proofs; see above).

- **2026-09-25: E10 gate item 4, size and performance against baselines.**
  - The store's components are 3,037 to 4,221 bytes, against 5,276 for a
    hand-written Rust `no_std` guest and 43,837 with `std`.
  - Through the same host, `add_to_cart` costs 21.1 µs a call and the Rust
    guest 22.1 µs. The native operation costs 0.30 µs, so per-call
    instantiation dominates; that is named, not reduced.
  - Browser activation shows no measurable change against the pre-E10 runtime
    (15.70 against 15.80 ms median, 21 interleaved samples each, twice).
  - Two corrections made while recording:
    - a one-sample comparison with E7's record showed a fivefold gain that is
      not real;
    - a blocked comparison showed a regression of half again (10.3 against
      15.7 ms), which was machine drift between the blocks.
  - Evidence: [bench-2026-09-25.md](evidence/E10/bench-2026-09-25.md).

- **2026-09-25: E10 gate item 5 and task 10.**
  - Only effect-row producers make a value affine: `DatabaseConnection`,
    `DatabaseTransaction` and `MapHandle`, in the store and in the accepted
    corpus.
  - The store's 10 declarations have 20 typed value positions, with 0 affine
    and 0 annotated.
  - Borrow, lifetime, move and box syntax are not Pleris.
  - Carried captures serialize deterministically. Resume versioning is E7V's.
  - Evidence: [ownership-2026-09-25.md](evidence/E10/ownership-2026-09-25.md).

- **2026-09-25: E10 gate item 3, sustained load (ADR-0035).**
  - Measured first, with no bound in place: 3,000 commands left 3,000 consumed
    events in the outbox. 1,000 departed visitors and 300 menu changes left
    600,000 queued frames and 1,001 materialized entries.
  - Now: 0 events, one `Recovery::Reload` per departed visitor, and nothing
    held once they have been idle 120 s.
  - The runtime acts on `Reload`; it had only logged it.
  - 20,000 compiled `add_to_cart` calls through the host left resident memory
    flat (24 µs per call in release).
  - Evidence: [load-2026-09-25.md](evidence/E10/load-2026-09-25.md).

- **2026-09-25: E10 gate item 1, the store builds from source (ADR-0034).**
  - `pw build` writes every artifact of a checked program:
    - the template IR;
    - both handler modules;
    - all five commands and queries as audited components;
    - the contracts and the WIT.
  - It ran with neither Koka nor Node on the PATH, and its outputs are
    byte-identical to the artifacts recorded separately
    ([build.txt](evidence/E10/build.txt)).
  - Found: lowering refusals did not say which declaration they belonged to,
    and `pw emit-template` re-read its sources with `unwrap_or_default()`.
  - Not claimed: the dev server runs the queries as components.

- **2026-09-25: compiled resumable handlers (E10, ADR-0033).**
  - `backend/js.rs` compiles each handler's body to an ES module;
    `pw emit-handlers` writes `<identity>.mjs`. The store's `add_to_cart`
    handler is `context.command("store.page.add_to_cart",
    [context.captures["item"]["id"], 1])`.
  - Every fact is read from the stage that owns it. `values::named` is
    extracted from the typer, and `contract::component_id` replaces three
    derivations.
  - The element carries exactly the capture paths the handler reads (`item.id`).
  - The host types the browser's arguments by the artifact's own parameters and
    refuses the rest with 400.
  - The dev server has one command path. The server-written modules, the
    address resolution and the per-command glue are deleted.
  - Found: `clear_cart` never committed its state; the Wasm IR's string
    constants were source tokens. Both are fixed.
  - `just ci` passes. Browser suite: three engines, 273/273 in three
    consecutive recorded runs (`f8adc03`). The first recording failed one
    WebKit test. The cause was harness interference: `public-fragment` renamed
    the shared menu on the shared host. It was reproduced on demand and fixed
    by isolating that suite; the failed record is kept.

- **2026-09-24: E10-I, a Pleris-compiled command through the E8 host.**
  - `backend/wasm.rs` now emits Canonical-ABI core modules, with every
    signature, flattening, layout and mangled name from `wit-parser` 0.257.1.
  - `backend/component.rs` wraps them with `wit-component` 0.257.1 and audits
    each artifact against its world through `wit-component`'s decoder.
  - The store's `add_to_cart` is a 4,221-byte component importing exactly
    `pw:host/session#read` and `store:data/carts#add`. All five store commands
    and queries compile and audit.
  - `pw_host::engine::call_within` ran it: the component read the host's
    session, called `carts#add("session-7", "cortado", 2)`, and returned the
    data layer's result.
  - Controls: admission and engine refusals, an unimplemented grant, fuel, and
    a core-identical but component-different import.
  - The dev server's commands run the compiled components, and its closures
    are deleted.
  - Browser suite: three engines, 258/258 in three consecutive runs,
    recorded (`docs/evidence/E10/browser-suite.txt`).
  - Found on the way: `imports_of` reported type exports as imports, which
    corrected E8's audit counts. Per-call compilation also amplified a
    pre-existing Firefox flake; components are now compiled once.


- **2026-09-24: E9-V1..V6, the ordinary value relations.**
  - `compiler/pw-core/src/values.rs` checks, three-valued, with diagnostics
    projected from a queryable analysis:
    - arity and argument types for path, member, piped, query, construction
      and policy-term calls;
    - declared results through branches, arms, `return` and `?`;
    - annotated bindings;
    - written types that resolve to nothing.
  - Language additions: callable type parameters instantiated per call,
    `type` parameters kept, function types typed bidirectionally, `?` as a HIR
    node, and declared-constructor arity.
  - Tests: 38 gate tests; 10 of 10 mutation controls killed.
  - Coverage: every relation in the store program is decided and agrees;
    the accepted corpus has 144 agreeing value relations and 343 resolving
    annotations; `pw audit-values` counts the undecided remainder.
  - Defects found and repaired (corpus C8): in the milestone demo, the
    accepted corpus and the libraries; see the evidence.
  - The pre-C8 text of every repaired rejected fixture is still caught for its
    declared invariant (`corpus_history.rs`).
  - `just ci` passes.
  - Not claimed: member existence, sum-type variant constructors,
    named-argument calls, non-phantom generic layouts at the boundary.


- **2026-09-16 resource repair:** shared requests now return the real terminal
  outcome; cancellation/invalidation fence late publication; command admission
  is atomic within this runtime; unwinding commands retain an explicit unknown
  outcome; logical deadlines and privacy mismatches are enforced. The full local
  workspace passed **937 tests, 0 failures, 1 existing ignored test**, including
  **24 new tests**. All nine initial regressions failed on the old runtime first.
  Formatter, workspace Clippy, corpus checks, recipe gates, and census/tooling
  suites passed. See [ADR-0029](DECISIONS/ADR-0029-owned-resource-flights-and-command-outcomes.md)
  and [scope, reproduction, and census mapping](evidence/E4/resource-repair-2026-09-16.md).
  This is a synchronous process-local repair, not durable exactly-once, a
  production cancellation adapter, or completion of E9/E10-I. Its own PR checks
  must establish the engine-feature build and current dependency audit.


- **2026-09-16: atomic resolved-signature cutover.** Parameter and return slots
  now contain recursive `TypeResolution`; the old written fields and independent
  `Interface::of` derivation are removed. Inference, members, privacy, captures,
  boundary decisions, WIT and backend callable signatures consume resolved
  identity. Contract identities are stable structured projections.
  Eleven new behavioral regressions failed on the old compiler and pass here;
  six authority tests and two historical-boundary controls also pass. Local
  workspace validation: **956 passed, one existing ignored documentation test**,
  including 178 core unit tests and 418 core integration tests. The accepted
  and store programs, formatting, workspace Clippy and census tests pass.
  [Evidence and exact scope](evidence/E9/signature-authority-2026-09-16.md).
  **This is not comprehensive ordinary-call typing or E10-I completion.**

- **2026-09-15 compiler follow-up:** recursive written types and one complete
  return annotation now survive lowering. Nested arguments resolve recursively;
  built-in arity and the qualified type namespace are checked; unit spelling is
  recognized. Direct generic resource results retain their arguments in manifests.
  The local compiler run passed **178 unit tests and 399 integration tests**
  across all 51 integration targets, including 11 new regressions. Formatter,
  compiler Clippy, and the accepted/store corpus checks passed. See
  [the bounded E9 evidence](evidence/E9/recursive-written-types-2026-09-15.md).
  That prior change did not complete ordinary argument/return checking or the
  signature migration. The latter is completed by the September 16 change above.

- The Web Failure Census and layout-attribution repair were merged in
  `c10b825de40528a591e101653218ddff52e9ec6e`. The inventory contains 224 failure
  records and 72 source/build plus 72 runtime obligations. These are research
  records and proposed acceptance contracts, not 224 eliminated compiler bugs.
- The supply-chain repair was merged in
  `4a9bcddc095b3b4d0eac0e0c2ddb8a0143de03d9`. The September 15 baseline CI run
  [35034596466](https://github.com/Kimeiga/perfect-web/actions/runs/35034596466)
  passed both Ubuntu architectures and the license/advisory job. Its head,
  `3bd5dbdb02b84d52a49e033a12bb71cff6a62b31`, was that baseline plus a temporary
  read-only source-snapshot workflow. That helper is removed by this repair.
- The evidence-gate repair adds `errexit` and `pipefail` to the recipe shell,
  drains summary output instead of closing the pipe early, and puts regression
  tests in `just ci`. Locally, all 45 success/failure scenarios over 17 real
  recipes passed. The original shell masked 24 of 28 injected failures.
  Grouped-command, renderer, large-output, and deliberately weakened-shell
  controls also passed. See [the bounded evidence report](evidence/tooling/evidence-gates-2026-09-15.md).
- The existing census Python suite and layout-attribution Node suite were rerun
  locally: 20 tests passed in each. No browser timing measurement was rerun.

The baseline CI result is not a result for this patch. Use the patch's own PR
checks for its Rust builds, audit, and complete gate-regression suite. Earlier
E7/E8 observations remain in the historical ledger and milestone documents;
these compiler changes do not independently re-establish those milestones.

## failing gate items

- **Member existence is not a value relation yet.** `box.x` on a `Rect`
  without `x` is unknown rather than refused. This is the first follow-up after
  E10-I, recorded in KNOWN_LIMITATIONS; it was not one of E9-V1..V6.
- **The rest of E10 is open.** The component backend is narrow. It lacks:
  - records and variants written into the region;
  - branches;
  - calls between compiled declarations;
  - a compiled data layer;
  - an automatic memory strategy.

  Each missing construct is refused by name. The handler backend is narrow
  too: one command call per handler, with no local computation, branches or
  event parameter. Captures are not a patched part.
- **CI cost of the engine, measured:** since E10-I every workspace build
  compiles Wasmtime. On a warm cache, CI on `ebd4696` took 2m54s, against 2m28s
  to 2m47s before. The two cold-cache runs took 5m06s and 5m29s.
- **Broader proposals remain proposals.** Temporal authorization, compatibility
  across live versions, commitment/unknown outcomes, composed budgets, and
  browser-owned editing behavior are recorded in the census. Their presence in
  prose does not establish complete generated-runtime enforcement.

## exact commands to reproduce

```sh
cargo test --locked -p pw-resource
just evidence-gates
just ci
just audit
python3 research/failures/tools/validate.py
python3 -m unittest discover -s research/failures/tools -p 'test_*.py' -v
node --test spikes/layout-phase-scheduler/test/loaf.test.mjs
just e10-component
just e10-i
just e10-handlers
just e10-build
just e10-load
just e10-ownership
just e10-bench
just e10-oracle
KIOKUN_DATA=/path/to/kiokun-data/output_dictionary just e10-kiokun
just e10-browser chromium
just e10-browser "chromium firefox webkit" docs/evidence/E10/handlers-browser-suite.txt
just e9-values
cargo test --locked -p pw-core --test value_relations --test corpus_history
python3 scripts/e9_value_mutations.py
cargo test -p pw-core --test signature_behavior --test signature_authority --test corpus_history
cargo test -p pw-core --test recursive_declared_types --test resolved_types --test call_arity --test canonical_abi --test one_comparison --test evidence_is_current
```

`just evidence-gates` requires Python 3 and `just`, but not Rust or browsers. It
runs actual recipe bodies in temporary trees with fake producers. The other
commands retain their real toolchain requirements. `just doctor` is read-only;
`just bootstrap` installs the pinned project dependencies.

## known environmental issues

**2026-09-24: the host's disk filled during E10-I.** 460 GB, with between 0
and 3 GB free, almost all of it used outside this repository. Shell commands
failed until the repository's incremental build cache (3.6 GB, regenerable) was
removed. Builds since use `CARGO_INCREMENTAL=0`. macOS purged
`~/Library/Caches`, which included every Playwright browser build. The browsers
were reinstalled once about 25 GB was released, and the three-engine run was
recorded then.

**2026-09-25: the Playwright builds were purged again.** Free space fell to
6.9 GB while the workspace built Wasmtime into several test binaries (`target/`
is 8.8 GB). macOS then emptied `~/Library/Caches` again, and the Cargo
registry's unpacked sources with it: 44 crates remain unpacked, and builds run
from compiled artifacts. The three browsers were reinstalled (Playwright 1.58.0:
Chromium 145.0.7632.6, Firefox 146.0.1, WebKit 26.0). Wasmtime 47.0.4's API was
checked against docs.rs, because its source was no longer on disk.

The September 16 resource repair used the pinned Rust 1.97.1 and locked registry
snapshot locally on Linux x86_64. Its full workspace run completed with a captured
zero exit code. These real runtime/compiler results are distinct from the earlier
mocked recipe tests. The evidence report records scope and environment.


The September 15 local review environment was Linux x86_64 with Python 3.13.5,
Node 22.16.0 and just 1.58.0. Rust was not available locally; actual Rust builds
and dependency auditing are checked through GitHub Actions, separately from the
injected-producer tests. No compiler or browser behavior is inferred from mocks.

The compiler follow-up used a verified, isolated copy of the pinned Rust 1.97.1
and its locked registry dependencies. Its real local compiler tests are distinct
from the earlier shell failure-injection tests. Long full-suite commands exceeded
the review runner's execution window, so all compiler integration targets were
run to completion in explicit batches; the incomplete attempts are not passes.

Changing the parent recipe shell does not repair every nested shell, conditional,
or independently invoked script. Redirected evidence files may still be partial
after failure. A file's existence is not a successful test result.

## last benchmark summary

No new compiler, browser, memory, or application performance benchmark was run in
this review. Existing numbers retain their original revisions, machines, and
scope. In particular, earlier frame-level reads of forced-layout attribution
cannot establish browser non-support; ADR-0027 corrects that interpretation.

## next three concrete tasks

1. **The kiokun proof slice**, as the next forcing application: dictionary
   entry lookup and search over one shard. It will exercise records, lists,
   branches and field reads, which the component backend refuses today.
2. **Widen the component backend by what that slice needs:**
   - constructing records and variants in the region;
   - field projection;
   - branches;
   - string constants via a data segment;
   - calls between compiled declarations.

   Each lands with a mutation-controlled test, in the style of E10-I.
3. **Compile resumable handler bodies**, so the page's
   `add_to_cart(item.id, PositiveInt(1))` is Pleris end to end. Then replace
   `store:data/carts` with compiled Pleris over narrower primitives (step 10).
