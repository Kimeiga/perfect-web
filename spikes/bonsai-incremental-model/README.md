# spike: bonsai-incremental-model

**Charter reference:** §14 Milestone 0 task 12 (added by charter v2), §5 design
donors.

## Question

Charter §5 lists two new donors and what **not** to assume about each:

| donor | borrow | do not assume |
|---|---|---|
| Jane Street Incremental | a stable dependency DAG, cutoffs, stabilization, incremental recomputation of arbitrary derived values | that its OCaml implementation should become the permanent cross-target runtime |
| Bonsai / Bonsai_web | purely functional state machines, a static computation DAG, lifecycle/scoping, whole-program incrementality, strong UI expect tests | that its virtual-DOM diff/patch loop, Js_of_ocaml target, or lifecycle APIs prevent forced layout or provide SSR/resumption/placement |

Does the incremental model actually deliver "an unrelated state update does not
recompute an expensive derived value", and where does that incrementality stop?

## Run it

```bash
just spike-bonsai
```

Evidence: `docs/evidence/M0/spike-bonsai-incremental-model.txt`.

## Toolchain — installed successfully, but not on the first three attempts

The charter allows recording a blocker instead of building. **No blocker was
needed** — the toolchain installs on Apple Silicon — but it took four attempts,
and the failures are worth recording because they will recur:

```text
opam 2.5.2, OCaml 5.2.0 switch "pw-bonsai"
bonsai      v0.16.0      incremental v0.16.1
virtual_dom v0.16.0      incr_dom    v0.16.0      js_of_ocaml 5.9.1
```

1. `timeout` is not a macOS builtin (it is `gtimeout` from coreutils).
2. `OPAMYES=1` does **not** authorize opam to invoke Homebrew for system
   dependencies. opam listed `libffi` and `zlib` and then aborted at the
   interactive prompt.
3. After installing those, `conf-pkg-config` failed: `"pkgconf": command not found`.
4. `brew install pkgconf gmp openssl@3` plus `opam install --assume-depexts`
   succeeded.

**Note the version resolution:** opam offers bonsai up to **v0.17.0**, but the
solver selected **v0.16.0** on OCaml 5.2.0. The v0.16 API is the older
`Computation.t` / `Value.t` style, not the v0.17 "cont" API. Anyone reading
Bonsai documentation must check which API version it describes.

## Results

### Incremental: unrelated updates do not recompute

`incr_demo/incr_demo.ml` builds a two-input graph where an instrumented
expensive node depends on `store_id` only, and a cheap node depends on
`cart_count` only — deliberately mirroring the charter's store page, where a cart
change must not recompute the menu.

```text
  initial stabilize                      expensive=1 cheap=1
  set cart_count := 1 (unrelated)        expensive=1 cheap=2
  set cart_count := 2 (unrelated)        expensive=1 cheap=3
  set cart_count := 3 (unrelated)        expensive=1 cheap=4
  set cart_count := 4 (unrelated)        expensive=1 cheap=5
  set cart_count := 5 (unrelated)        expensive=1 cheap=6
  set store_id := 48 (related)           expensive=2 cheap=6
  set store_id := 48 again (cutoff)      expensive=2 cheap=6

check:unrelated-update-does-not-recompute=pass  (1 -> 1 across 5 unrelated updates)
check:related-update-does-recompute=pass        (1 -> 2)
check:same-value-write-is-cut-off=pass          (2 -> 2)
```

The instrumentation is a counter incremented **inside** the expensive node, so no
framework claim can hide a recomputation.

### Where the incrementality stops — from installed source

Charter task 12 asks to *"confirm from source which portions use Jane Street
Incremental and which portions still produce a virtual DOM and diff/patch it"*.

Bonsai's own `dune-package` declares:

```text
ui_incr            <- Jane Street Incremental
incr_map
incr_map.collate
virtual_dom  (+ .html .svg .keyboard .layout .input_widgets .ui_effect ...)
js_of_ocaml
```

and bonsai ships `incr0.ml` / `annotate_incr.ml`, its incremental core.

Rendering, however, ends here — `virtual_dom/node.mli`:

```ocaml
module Patch : sig
  type node := t
  type t

  val create : previous:node -> current:node -> t
  val apply : t -> Dom_html.element Js.t -> Dom_html.element Js.t
  val is_empty : t -> bool
end
```

## Findings

**F-1 — the incremental DAG delivers exactly what charter §9.5 asks for, for
pure derived values.** Five unrelated updates, one expensive evaluation. Writing
the *same* value is cut off and costs nothing. This is the shape the project's
resource graph should have: *"a rerender or local reactive recomputation must not
create a new request unless the semantic resource key or policy changed."*

**F-2 — incrementality stops at the virtual DOM, and this is the whole reason
Bonsai is not the permanent renderer.** `Patch.create ~previous ~current`
compares two complete trees. Charter §8.4 says the opposite:

> *"When one value changes, update only dependent parts. Do not conceptually
> rerender the entire component and compare virtual trees."*

So Bonsai's static computation DAG avoids recomputing *values*, and then hands
the result to a diff/patch loop that walks trees anyway. The charter's warning —
do not assume the vdom loop "prevents forced layout or provides SSR/resumption/
placement" — is confirmed by construction, not by opinion.

**F-3 — this is precisely the trap charter v2's new §20 risk names.** *"Fine-
grained updates are mistaken for layout safety."* Bonsai is a strong instance:
genuinely fine-grained at the computation layer, and completely silent about
whether the resulting DOM writes interleave with geometry reads. The companion
`layout-phase-scheduler` spike measured that gap at **264×–848×**. Incrementality
and layout safety are orthogonal properties, and a system can have the first
while being catastrophic at the second.

**F-4 — what to borrow, and what to leave.**

*Borrow:*
- **Cutoff on equal values** — the "set the same value again" case must cost
  nothing. Directly applicable to charter §9.5's query-key semantics.
- **Explicit stabilization** — a named point where the graph becomes consistent
  maps cleanly onto §7.5A's frame transaction step 2, *"stabilize pure/incremental
  computations"*, before measurements are collected.
- **A static computation DAG** — dependencies known before running, which is what
  makes §9.4's materialization graph inspectable.
- **Expect tests over UI** — Bonsai's testing story is unusually strong and
  informs Milestone 7's golden suite.

*Leave:*
- The **virtual-DOM diff/patch loop** (F-2).
- The **OCaml/Js_of_ocaml runtime**, per charter §5's explicit warning.
- Bonsai's **lifecycle APIs** — the project deliberately has no general lifecycle
  effect (§7.5).

**F-5 — `incr_map` is worth studying separately for Milestone 6.** Bonsai depends
on `incr_map` and `incr_map.collate`: incremental operations over *maps*, so that
changing one key does not recompute a derived view of the whole collection. That
is structurally the same problem as *"`MenuChanged(store_47)` invalidates only
store 47"* (charter §14 M6 gate). Not exercised in this spike.

## Limitations

- **No Bonsai web application was built or rendered.** The library installs and
  its dependency graph and rendering API were inspected from source, but no
  `Bonsai_web` app was compiled to JavaScript and run in a browser. The
  incremental behaviour was demonstrated with **Incremental directly**, which is
  the mechanism under test, but it means Bonsai's own lifecycle/scoping model was
  read rather than exercised.
- **Bonsai v0.16, not v0.17.** The solver chose v0.16 on OCaml 5.2.0. The v0.17
  "cont" API differs substantially and was not evaluated.
- **The expect-test workflow was not run.** Charter task 12 mentions Bonsai's
  "unusually strong UI expect tests" as something to inspect; only their existence
  was confirmed via the `vdom_test_helpers` sublibrary.
- The expensive node is a synthetic loop, not a realistic derived value.
- Single-threaded, no `incr_map`, no collation, no async.
