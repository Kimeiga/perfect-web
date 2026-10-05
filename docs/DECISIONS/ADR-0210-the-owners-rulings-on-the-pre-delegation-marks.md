# ADR-0210: the owner's rulings on the pre-delegation marks, ADR-0031 to ADR-0122

Status: accepted, 2026-10-05: the owner's rulings, relayed as ADR-0195's
were, and recorded under the owner's delegation of 2026-10-02. Each is built
in its own ADR, and the recording session may overrule one its research
shows wrong, saying why there. Milestone: E14.

## How this is recorded

- **The rulings below are the relayed text**, kept whole: each mark, its new
  rule, and why.
- **Verified so far:**
  - Urgent 1 is reproduced with `pw check`. A `return` in a `for` body and a
    failing `?` there both checked with a transaction open, and the correct
    program, rolling back then returning, was refused. ADR-0211 builds ruling
    0045-a parts 1 and 2.
  - **Urgent 3 is not reproduced.** A page's handler, and a view's, sending a
    command whose body is `todo` checks, and `pw build` refuses it: "`P`
    depends on `bump`, whose body is a placeholder (`todo`)"; nothing is
    written. A function cannot send a command (PW0339), so no other path
    reaches one. The rule stands as ruled (0034-a), and holds.
  - Urgent 6 is reproduced: `let v = query helper(n)`, `helper` a `fn`,
    checks.
- **Overruled by this session:** none yet. Each ADR that builds a ruling
  says whether its research confirms it.
- **Order** (the owner, relayed 2026-10-05): urgent defects 1 to 8 first;
  then the feed reference app, pulling in the rulings it needs (0073-a,
  0071-a, 0122-d, 0057-a, 0099-a, a `String`'s length as an invariant,
  ADR-0195's ruling 10); then the remaining rulings of ADR-0195 and this
  one, NEXT 21's host comparison, and the other two reference apps.


Ruled 2026-10-05 by the session "Web Pleris capabilities and limitations", under the owner's request "rule on the 109 older ones too", and relayed to this one to record. Five read-only research agents prepared the material: each mark was checked against later ADRs and the code, probed with the existing `pw check` binary where possible, and compared with primary sources. The final call on each was the relaying session's.

The "109" count included each ADR's boilerplate status line. There are **85 real marks**:
- **26 overruled**
- **30 confirmed**
- **29 already settled** (12 by ADR-0195's rulings, 17 by later ADRs)

The recording session may overrule any of these when its own research shows the ruling wrong, saying why in the ADR (as with ADR-0195).

## URGENT: soundness defects found by the sweep

1. **A transaction leaks out of a `for` loop, and `pw check` passes it.** `affine.rs` `Paths::expr` gives a `for` loop `exits: Vec::new()`, so a `return` or failing `?` in the body is not an exit that owes a release. Probes that pass with no diagnostics:
   - `let tx = Database.begin(); for s in sessions { if s == "" { return Ok(()) } }; tx.commit()`
   - `for s in sessions { Carts.clear(current_session())? }` before `tx.commit()`

   The correct program, roll back then `return`, is refused. See ruling 0045-a.
2. **A captured value can go stale.** A patch that changes a captured field which is not the loop key leaves the old value on the element, and the handler sends it. Rule: every captured path is either re-rendered by each patch that can change it, or refused at check.
3. **A handler may call a `todo` command and the build still succeeds** (unverified). `build.rs` `refusals()` follows component imports only, and ADR-0113's page contract leaves handlers out. Verify; if real, a handler's command calls count as dependents of a placeholder.
4. **A declared retry policy is silently replaced.** `page_values.rs:116` passes only `retry`'s `max`. `fixed` and `jitter = false` run as exponential backoff with jitter. See 0089-b.
5. **Misplaced policy heads are silently ignored.** `freshness`, `on_key_change` and `cache` on a command, `retry` on a `fn`, and clause words in a plain `fn` body (`cache nothing_y`) all check clean. See 0092-b and 0047-a.
6. **`let v = query helper(id)` checks**, where `helper` is a `fn`. See 0108-a.
7. **`List.maximum` gets −0 wrong** (`packages/pw-std/list.pw:95`). It keeps the first of ±0, but IEEE `maximum` and ECMA-262 `Math.max` treat +0 as larger.
8. **`.value` means two things in an opaque type's own module.** A declared member named `value` shadows the representation (`values.rs::member_type`). It is live in `browser.pw` (`LayoutSnapshot`), and `fn value(p: PositiveInt) -> Int { p.value }` would recurse forever. Rule: an opaque type may not declare a member named `value` in its own module; rename `LayoutSnapshot`'s accessor (A-015, A-016).
9. **Inlining has no size budget** (ADR-0050). A fan-out call graph could grow the output exponentially. Rule: inline within a budget, otherwise compile a call.
10. **The kiokun `NOTICE.md` is incomplete.** It points to `LICENSE`, but the repo has `LICENSE-MIT` and `LICENSE-APACHE`. It names only CC-CEDICT, JMdict and Tatoeba, but the sample also holds JMnedict, Korean data, `chinese_char`, mnemonics and Wiktionary/Kaikki examples (CC BY-SA).

## Overruled (26)

| id | new rule | why | effort |
|---|---|---|---|
| 0045-a | (1) A `return` or failing `?` inside a `for` body is an exit owing one release of each live affine value. (2) A release inside a `for` body is accepted when every path from it reaches a `return` or failing `?` before the body ends; otherwise PW2005 stays. (3) Inside a lambda, unchanged. | Rust's borrow checker is flow-sensitive; part 1 closes a soundness hole (urgent 1). | medium |
| 0046-a | Keep a fresh instance per call. Speed comes from wasmtime's pooling allocator, copy-on-write memory images and an `InstancePre` cached per (component, grant set). A live instance is never reused; a revoked grant gets a different pre-link. | Wasmtime's fast-instantiation guide; reusing a reset instance risks leaking state between calls. | medium |
| 0047-a | A clause is syntax only in a block that admits it (the policy table names the block kinds for each head). The parser makes a clause node there and checks its words against the closed set. Elsewhere the head is an ordinary name, resolved or refused (PW0021). **Interim, now:** `names::is_clause` consults the enclosing declaration kind and refuses clause words where they are not read. | Silent no-ops today (urgent 5); Rust's weak keywords are parsed in context. | large (interim small) |
| 0049-b | `"""` stays raw (no escapes, no holes) and adopts C# 11's layout rule. Multi-line: the opening `"""` ends its line, the closing `"""` begins its line, and the closing line's indentation is removed from every line; a line that lacks it is PW0014. The newline after the opening and before the closing are not content. Rewrite A-024. | C# 11, Swift SE-0168 and Java text blocks all strip incidental indentation. | small |
| 0051-a | Add `while cond { body }`: `cond` is a `Bool` checked before each pass, the body's value is discarded, `return` leaves the function, no `break`/`continue` (like `for`). An endless loop is stopped by the host's fuel. | The premise was wrong: Pleris has no tail-call elimination, so condition loops written as recursion trap on depth. | medium |
| 0052-a | A lambda compiled as a function value may read a `let mut` binding only if no assignment to it can run after the lambda is made (a later assignment in scope, or any assignment in a loop around the lambda). Otherwise refuse by name and suggest `let snapshot = x`. | Today the closure silently sees a stale copy; Java and Rust refuse the ambiguous case. The flow rule over Java's blanket rule for less ceremony. | small |
| 0055-b | Add `List.map_indexed(items, f: fn(Int, T) -> U) -> List<U>`, positions from 0. `enumerate` stays out until tuples. | The suggested counter workaround cannot work inside a list operation's lambda (ADR-0051); Elm `indexedMap`, OCaml `mapi`. | small |
| 0056-a | `String.to_lower` is exactly Rust's `str::to_lowercase`, including Final_Sigma (Unicode §3.13). Cased/Case_Ignorable tables generated by probing Rust; keep the Unicode 17.0 pin. | "ΟΔΟΣ" → "οδοσ" is a wrong answer that looks right. | medium |
| 0057-a | A map key may be an `Int`, `String`, `Bool` (`false < true`), or an opaque type over one, ordered as its representation. Others are refused at **check**, not only at build. | `Map<ProductId, V>` must be writable; the Component Model's `map` keys allow these. | small–medium |
| 0057-c | A map or set arriving from outside is sorted on arrival (String by code point). A repeated key still stops the invocation, by name. Record the deliberate divergence from WIT `map` (last wins), and why `Map.from_lists` (program's choice, last wins) differs from the boundary rule. | Out-of-order is not "two things"; refusing it ties every host to one collation, against database independence. | small |
| 0060-a | An arm reached on several paths is lowered once, as a shared labelled block (Wasm `block`/`br`, JS labelled block), not copied. The 1,024-leaf refusal goes. **After the app layer.** | A correct program must not be refused for size; Elm, Rust MIR and OCaml share join points. (Agent recommended confirm; overruled for "perfect".) | medium |
| 0061-a | In a `{#match}` whose subject's type is known, an arm's case is resolved in that type (as ADR-0197 does for patterns): `{:None}` over a `Shape` is `Shape`'s case, rendered by its WIT name (ADR-0206). PW5019 stays only where the subject's type is unknown. | Expression matches already accept it; templates refusing it refuse a correct program. | small |
| 0071-a | An `{#if}`/`{:else if}` condition or boolean attribute is a `Bool`, or a `List`/`String` tested non-empty. `Int`, a record or anything else is PW0609, with the repair `n > 0`. **Land with or after 0073-a**, so the repair builds. | A record is always true; `Int` truthiness is JSX's `{count && …}` bug. | small |
| 0073-a | A computed hole compiles. The compiler lifts each into a pure derived function of its inputs, computed by the host per render and by the browser's pure emitter (ADR-0044) when the inputs are signals or speculated values. The renderer reads it by a compiler-named path. The build refusal stays until then. | Charter §8.1 writes `disabled={!item.available}`; every template system allows it. **App-layer priority.** | large |
| 0078-a | A function type carries an effect row. A written `fn(A) -> B` with no row is row-polymorphic (a fresh row variable, as in Koka), and a helper's row includes its callback's variable, filled at each call. `!{}` states purity. **After the app layer.** | Charter §7.2 says function types "must" carry effects; today's rule is sound but over-counts. | large |
| 0089-b | `fixed` leaves the retry domain (PW0335), and the fixtures write `bounded_exponential`. A retry's strategy and `jitter` reach the runtime manifest, or the build refuses what it would not honour. | Urgent 4. | small |
| 0092-b | Every policy head has an entry in `declared_by`. A head on a declaration kind it is not listed for is PW5105, failing closed. | The architect's 2026-08-20 ruling; urgent 5. | medium |
| 0099-a | `let _ = e` is an explicit discard and satisfies PW0618 for a `Result`. An affine resource bound to `_` is refused as never consumed. | Rust, Swift, Go; avoids Rust's `let _ = mutex.lock()` trap. | small |
| 0100-a | The effect ontology marks each effect read-only or not. A query or subscription may perform only read-only effects plus observability (log, trace); everything else is refused (an allowlist, PW0401). `network.fetch` carries its method, and only safe methods (GET, HEAD) count as read-only. | RFC 9110 §9.2.1; a denylist passes every effect added later. | medium |
| 0101-a | A freshness window excuses the writer only when the reader's consistency is `eventual` or `snapshot`. A reader declaring `strong` or `read_your_writes` is held to PW5106 regardless of its window. | RFC 9111 §4.4; `strong` rules out the staleness a window allows. | small |
| 0101-b | A write naming no domain (`!{ database }`) meets every reader of any `database.read<_>`. `database.write<T>` narrows it. | Matching nothing reports an undecided case as agreement. | small |
| 0101-c | A write (`database.write`, `database.transaction`, `outbox.write`) belongs to a command alone, and the `fn`s it calls; in any other declaration kind it is refused (PW0401). | Charter §7.5: the command is the explicit mutation. | small |
| 0101-d | Until keys are compared, each PW5106 reach found by name is recorded **undecided** in the audit, not agreeing. The comparison is then built on ruling 8's label flow (each command parameter labelled, carried to the write's and the event's arguments). | Undecided is never agreement; a wrong-key event leaves a stale entry that looks fresh. | small, then medium |
| 0105-a | A command's `invalidates` key, or the listener key its `emits` binds, must be its optimistic target's key: the same parameter, the same invocation-context call, or `_`. A provable mismatch is PW5107; an incomparable pair is recorded undecided. | A mismatch leaves a speculation on the page as if committed. | small |
| 0108-a | In `query R(..)`, `R` names a query or a resource; in `subscription R(..)`, a subscription. Anything else is refused at the name. | Today a `fn` silently gets resource semantics (urgent 6). | small |
| 0122-d | A speculation target's key also matches when the handler passes, unchanged, the page parameter (or invocation-context call) that the binding's key reads to the command parameter the target names. Any other flow stays refused by name. | `/post/:id` speculation is the common case for the app layer. | medium |

## Confirmed (30)

Each default stands; the agents' reasons are noted.

| id | decision | why |
|---|---|---|
| 0031-b | `Session<SessionId>` is not `SessionId`, in either direction. | Nominal qualifier, like a newtype; prevents cross-partition mix-ups. |
| 0031-c | `LayoutSnapshot<T>` is read explicitly through `.value`. | Explicit over phase-dependent coercion (charter §7.5A). (`.value` shadowing: urgent 8.) |
| 0032-a | `Export` carries `component: {interface, function}` from one place. | ADR-0020's single answer. **Also: make it required**; no older contracts exist outside the repo. |
| 0033-a | `data-pw-captures` carries the handler's declared read paths. | This is what resumability is (Qwik, LiveView). (Staleness: urgent 2.) |
| 0034-a | A placeholder is a refusal only once something depends on it. | Like Rust's `todo!()`, but stricter. (Handler gap: urgent 3.) |
| 0037-a | The share-alike kiokun sample stays in the repo, with a repaired NOTICE (urgent 10). | Collection, not adaptation (CC FAQ). **Flagged to the owner** as a licensing judgement, revisited if the repo is ever relicensed. |
| 0039-a | `Int` is 64-bit and never wraps; `/` and `%` are Euclidean; ÷0 and `MIN / -1` trap. | Swift traps, Rust's `div_euclid`; matches WIT `s64` and SQL `bigint`. |
| 0042-a | `{#match e}{:Some(x)}..{/match}` template syntax. | Svelte's block form; one form for every sum type. |
| 0042-b | A URL hole needs leading static text; values are percent-encoded except RFC 3986 unreserved characters. | RFC 3986 §2.3; keeps a value to one segment. |
| 0043-a | `Float.from_int`. | The std's `Target.from_x` convention; OCaml `Float.of_int`. |
| 0045-b | Returning the bound affine value moves it to the caller. | Rust's move-on-return. |
| 0045-c | A `resource.release<T>` in the row obliges the body to release once per path. | Like Rust's `fn commit(self)`. |
| 0048-a | An opaque type's representation is read as `.value`, only in its module. | Elm, OCaml, Gleam opacity; consistent with ADR-0179. |
| 0049-a | The escape set `\n \t \r \\ \" \{ \} \u{…}`. | One spelling per character. **Fix the ADR's wording:** omitting `\0` and `\'` is a choice, not "the common core" (Rust, Swift and JS all have them). |
| 0050-a | Inline until recursion; only recursion is a call. | Invisible optimisation. (Budget: urgent 9.) |
| 0053-a | A callee with no signature types no parameter. | Don't guess. The unannotated directly-called lambda gets ADR-0195 ruling 4's "type annotation needed". |
| 0055-a | `List.maximum` returns `Option`, not −∞. | Elm, Rust, Swift. (−0 bug: urgent 7.) |
| 0057-b | Maps iterate in sorted key order. | Deterministic render text and cache keys; Elm `Dict`, `BTreeMap`. |
| 0063-a | A policy term's binders see only the declaration's parameters. | Terms run apart from the body; Dafny and SPARK contracts. |
| 0063-b | An element of a labelled collection carries its label. | Otherwise `for t in secrets` launders; Jif. |
| 0063-c | A lambda's parameters take the join of the call's other arguments. | Moot in precision under ADR-0129's driving-argument rule. |
| 0066-a | A nested declaration written before a `let` does not see it. | Swift refuses capture-before-declaration; no hoisting hazards. |
| 0068-a | Branches are related only when the value is used. | A discarded value cannot give a wrong answer; PW0618 covers `Result`. |
| 0084-a | A record or list has no implicit text form. | JS's `"[object Object]"`; `display` states the format. |
| 0089-a | `latest_wins` is not a concurrency mode. | `on_key_change supersede` covers it. |
| 0092-a | A query does not `depends_on` another resource; derivation is a materialization. | One way to declare derivation (ADR-0195 ruling 10). |
| 0102-a | An unfilled key position matches every value. | Sound over-invalidation; TanStack prefix matching. |
| 0114-a | A build-placed page reads no parameter; no enumeration yet. | Enumeration waits for a program, as an explicit clause over build input. |
| 0122-b | `entry_value` is its own protocol frame. | One meaning per frame. |

## Already settled (29)

| id | settled by |
|---|---|
| 0031-a | ADR-0052, ADR-0053, ADR-0086 |
| 0032-b | ADR-0033 |
| 0033-b | ADR-0049 |
| 0040-a | ADR-0056 |
| 0044-a | ADR-0059 and ADR-0206 |
| 0047-b | ADR-0196 |
| 0054-a | ADR-0179 |
| 0058-a, 0058-b | ADR-0131 |
| 0059-b | ADR-0076 |
| 0064-a, 0085-a | ADR-0129 |
| 0072-a, 0119-b | ADR-0130 and ADR-0136 |
| 0089-c | ADR-0152 |
| 0102-b | ADR-0145 and ADR-0150 |
| 0119-a | ADR-0131 and ADR-0138 |
| 0122-c | ADR-0157 |
| 0122-e | ADR-0172 |

Covered by ADR-0195:
- 0038 ×2 (rulings 2 and 7)
- 0041 (ruling 3)
- 0059 §1 ×2 (ruling 5)
- 0063-d (ruling 1)
- 0065-a (ruling 4)
- 0088-a, 0088-b (ruling 10)
- 0088-c (ruling 9)
- 0096-a (ruling 13)
- 0104-a (ruling 11)
- 0107-a (ruling 8)
- 0122-a (ruling 14)

## Suggested order

1. **Now: the urgent soundness defects.**
   - 0045-a part 1 (the `for`-loop transaction leak), with a mutant
   - urgent 2 (stale captures) and urgent 3 (verify the `todo` handler gap)
   - 0089-b, 0092-b, the 0047-a interim, 0108-a
   - 0101-a, 0101-b, 0101-c, 0101-d (audit step), 0105-a, 0100-a
   - urgent 7 (−0) and urgent 8 (`.value`)
2. **With the app layer:**
   - 0073-a (computed holes), then 0071-a
   - 0122-d (route-keyed speculation)
   - 0057-a and 0057-c (map keys, sorting on arrival)
   - 0099-a (`let _`), 0051-a (`while`), 0052-a, 0055-b, 0061-a, 0049-b
   - 0046-a (cheaper instantiation)
3. **After the app layer:** 0060-a (shared arms), 0078-a (effect rows on function types), 0056-a (Final_Sigma), and the 0047-a full parser split.
4. **Housekeeping:**
   - the 0032-a field made required
   - 0049-a wording
   - the NOTICE (urgent 10)
   - the inlining budget (urgent 9)
   - KNOWN_LIMITATIONS cleaned of every entry these settle, and ADR-0059's stale §4 and "Not done" (now ADR-0194, 0202, 0203, 0205)
