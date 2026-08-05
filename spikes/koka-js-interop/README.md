# spike: koka-js-interop

**Charter reference:** §14 Milestone 0 tasks 7–8.

## Questions

1. Do Koka ADTs and effect handlers compile on Apple Silicon?
2. Can generated JavaScript be imported from Node as an ES module (or through a
   documented adapter)?
3. Can a `maybe`/`result` value cross the boundary **without unchecked guessing**?

A fourth question was pulled forward from Milestone 1 task 8 because the whole
Milestone 1 plan depends on it: **can machine-readable inferred types and effects
be obtained from Koka without a fork?**

## Run it

```bash
just spike-koka
# or directly:
koka --target=js --outputdir=build -o build/store kk/store.kk
node --test spikes/koka-js-interop/node/test.mjs
```

Evidence: `docs/evidence/M0/spike-koka-js-interop.txt`.
Pinned to **Koka 3.2.3** (macOS arm64). 16 Node tests, all passing.

## Results

| Question | Answer |
|---|---|
| 1. ADTs + handlers on Apple Silicon | **Yes**, unmodified. |
| 2. Importable from Node | **Yes** — plain `.mjs` ES modules, no adapter, no bundler. |
| 3. `maybe`/`result` without guessing | **Only with a hand-written decoder.** The raw boundary is unsafe; see F-4/F-5/F-7. |
| 4. Machine-readable effects without a fork | **Yes** — via the `.kki` interface file. See F-1. |

The effect rows extracted straight from `build/kk_store.kki`:

```text
calculate-subtotal     !{}   <- pure
available-items        !{}   <- pure
describe-order         !{}   <- pure
load-store             !{database-read, trace-effect}
load-store-subtotal    !{database-read, trace-effect}
load-store-for-js      !{}   <- pure (handlers discharged the effects)
main                   !{console}
```

That is the Milestone 1 gate item *"every effectful example has a visible
inferred effect"* demonstrated a milestone early.

## Findings

**F-1 — `.kki` interface files give machine-readable types AND effect rows. No
fork needed.** `koka --target=js` writes `<module>.kki` beside `<module>.mjs`.
It is plain text containing, per public declaration: the fully elaborated
signature *including the inferred effect row*, `[line,col,line,col]` source
spans, and ADT constructor tag numbers. `node/kki.mjs` is a ~120-line reader for
it. This resolves charter §14 M1 task 8 in the affirmative and makes task 9's
"narrow parser as a temporary bridge" the correct path.
**Caveat:** `.kki` is an internal format with no stability guarantee. The reader
pins `VERIFIED_AGAINST_KOKA = "3.2.3"` and throws on any other version, so an
upgrade cannot silently produce wrong effect data.

**F-2 — ADT tags are positional integers; never hardcode them.** A multi-
constructor type compiles to `{_tag: <int>, ...fields}` where the integer is
assigned by declaration order. **Inserting a constructor renumbers every later
tag.** Koka also exports `is_<ctor>` predicates, which are stable across
reordering. The decoder uses only the predicates. Hardcoding `_tag === 1` is
exactly the "unchecked guessing" charter §14 M0 task 8 forbids.

**F-3 — Koka has no generic `Result<T, E>`.** `error<a>` fixes the error side to
`exception`; `either<a,b>` has domain-meaningless `Left`/`Right`. The project
declares its own `qresult<a,e>` with `QOk`/`QErr`. Cheap, but it means the
charter §7.1 `Result` is *ours*, not inherited, and every stdlib function
returning `error<a>` needs an adapter at the seam.

**F-4 — single-field `value struct`s are ERASED.** `Money_usd(350)` returns
literally `350`; `Store_id("store_47")` returns `"store_47"`. At the JS boundary
a `money-usd` **is** a number and a `store-id` **is** a string. Koka's nominal
typing exists only inside its own type checker.
*Consequence:* charter §7.1's "nominal opaque domain types" and "typed units such
as `Money<USD>`" get **zero** runtime protection at the JS seam. The `pw`
compiler must carry nominal identity in its own manifest; it cannot recover it
from Koka's output. `decodeMoneyUsd` returns `_unverifiedNominalType: true` to
keep this visible rather than pretending.

**F-5 — the generated `is_*` predicates are discriminators, not validators.**
Two shapes exist, both unsafe on unknown input:

```js
export function is_just(maybe)  { return (maybe !== null); }   // is_just(42) === true
export function is_qok(qresult) { return (qresult._tag === 1); } // is_qok(null) THROWS
```

They are correct only when the argument is already known to be a value of that
exact Koka type — which is precisely what JS does not know at the boundary. Every
decoder in `node/decode.mjs` therefore confirms the structural shape itself
first, and uses the predicate only to choose among confirmed variants.

**F-6 — a multi-operation effect must be discharged by ONE `handler` block.**
Chaining `with fun read-store(..)` then `with fun read-menu(..)` installs *two*
handlers for the same effect, each defining one operation, and the second
operation ends up unhandled: `operator read-menu is not handled`. Ergonomic
trap, cost ~10 minutes; recorded so Milestone 1 does not rediscover it.

**F-7 — `Nothing` and `Nil` are BOTH `null`.** So is any other payload-free
constructor of a type Koka chooses to represent that way. A bare `null` arriving
at the boundary is genuinely ambiguous, and `is_nothing(Nil)` and `is_nil(Nothing)`
both return `true`. Runtime discrimination is **impossible**; the decoder must be
told statically which type it is decoding.
*Consequence:* charter §7.1's "no ambient `null` or `undefined` in application
code" holds inside Koka but **not** at the JS boundary. Reinforces F-4: the
type manifest must come from `pw`, not from the shape of Koka's JS.

**F-8 — Koka does NOT enforce exhaustiveness as an independent static rule.**
Measured both ways:

| case | result |
|---|---|
| non-exhaustive match, function declared **total** | compile **error**: `effects do not match` |
| non-exhaustive match, function declares **`exn`** | **compiles cleanly**, fails at runtime with `pattern match failure` |

Koka models a partial match as *"this function may raise"*, not as *"this match
is incomplete"*. Exhaustiveness is therefore guaranteed only for functions whose
effect row excludes `exn` — and `exn` is easy to acquire transitively.
*Consequences:*
- Charter §7.1 "exhaustive pattern matching" and §16.2 "unhandled ADT variant"
  **cannot be delegated to Koka**. Milestone 9A already lists "pattern
  exhaustiveness" as own-compiler work; this confirms it is mandatory, not
  optional.
- Milestone 1's compile-fail harness must assert that exhaustiveness cases are
  written with a **total** effect row, or the tests pass vacuously.
- Both negative cases are pinned as permanent tests, so a future Koka release
  that closes the gap will be noticed.

**F-9 — `-o <name>` collides with the module's own emitted file.** Compiling
`neg4.kk` with `-o out/neg4` makes the launcher overwrite the module, producing
`TypeError: $neg4.main is not a function`. Use a launcher name distinct from
every module name. Minor, but it cost time and will recur in Milestone 3's build
scripts.

## Limitations

- Only the JS backend was exercised. The C backend, the Wasm backend, and
  `--target=wasm` are untouched.
- No async/concurrency: charter §7.6 structured concurrency is **not** tested
  here. Koka's async story on the JS target is a known open risk (see
  `docs/RISK_REGISTER.md`).
- No `pw` source language exists yet, so nothing tests that authoring syntax
  stays decoupled from Koka syntax (Milestone 1 gate item).
- The fixture database is in-memory only; no SQLite adapter boundary yet
  (Milestone 1 task 3).
- Perceus / reference-counting behavior and performance were not measured.
- `kki.mjs` extracts effect rows and spans only, not a full type AST.
