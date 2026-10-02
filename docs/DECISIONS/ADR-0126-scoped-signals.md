# ADR-0126: scoped signals (proposed)

Status: **proposed**, 2026-10-02. Needs the owner's ruling before any of it
is built. Unblocks E14 tasks T06 (forms) and T11 (dialogs), and the DoorDash
target the project exists to reach.

## Why now

Charter §9.1 made UI state its own kind of state, and `docs/SEMANTICS.md`
named the primitive `signal`. Nothing else was settled, and since E7 the
renderer runs no handler that only changes the page. A DoorDash-shaped app is
mostly this kind of state:

| screen | UI state |
|---|---|
| store page | which category tab; the cart drawer open or shut |
| item modal | open for which item; chosen options; quantity |
| checkout | the address being typed; the tip chosen |
| search | the query text; the filters |

## What "no bugs" has to mean here

Each row is a bug class React allows and Pleris should make unwritable:

1. A sibling needs the state, so it is lifted to a parent and passed down
   through views that do not use it.
2. A view reads shared state with nothing providing it, and silently gets a
   default.
3. A change re-renders things that did not read it.
4. Two copies of one fact are kept in sync by hand and drift.
5. An async handler writes state after its view is gone.
6. A dialog's state allows impossible combinations (open with no item).

## Proposed rules

**R1. Declared where it lives.** A page, view or block declares a signal with
a type and an initial value. Its instance belongs to that scope's instance: a
signal inside a keyed `{#each}` is one per item and moves with its key.

**R2. Read by name, resolved by the compiler.** Any view used inside the
scope reads the signal by name. Which declaration it means is decided at
compile time, and the requirement is inferred upward through every view in
between, the way effect rows are. Views in between never mention it. This
removes bugs 1 and 2: no threading, and a read with nothing declaring it
around the use is a compile error naming the use site.

**R3. Writing is an effect.** Writing a signal appears in a handler's effect
row. A view's render has an empty row (charter §7.4), so a render cannot
write; only handlers can.

**R4. Fine-grained by construction.** The compiler knows which parts read
which signals, so a write updates exactly those parts, as resources already
do. This removes bug 3.

**R5. One source per fact.** A value computed from signals or resources is a
`derived` value, recomputed in dependency order, never a second signal kept in
sync by hand. This removes bug 4.

**R6. Scoped lifetime.** When a scope's instance is removed, its signals are
gone, and a pending handler's later write to them is dropped
(charter §7.6, structured concurrency). This removes bug 5.

**R7. Typed like everything else.** A signal's type is any serializable
Pleris type, so a modal is `Closed | Open(MenuItemId, Options)`, not a flag and
a nullable id. Matching it is exhaustive. This removes bug 6.

**R8. Placement and privacy.** Signals live in the browser. Server code
cannot read one; a command receives what it needs as arguments. A signal whose
initial value comes from private data makes its page private, by the label
rules that already exist.

**R9. Resumption.** The server renders each signal's initial value; the
browser resumes without re-running the page. A signal is ephemeral unless it
declares `persist url("tab")`, which round-trips it through the address bar,
for state worth a shareable link: tabs, filters, search text.

**R10. Events.** Handlers receive the platform's typed event, so an input can
write its signal (`e.value`). This settles ADR-0058's open question the same
way.

## A sketch

```text
page StorePage(id: StoreId) {
    signal drawer: Drawer = Drawer.Shut
    view { <Header /> <Menu /> <CartDrawer /> }
}

view Header()     { <button on:press={() => drawer = Drawer.Open}>Cart</button> }
view CartDrawer() { {#match drawer} {:Open} <aside>..</aside> {:Shut} {/match} }
```

`Header` and `CartDrawer` are siblings. Neither is passed anything and the
page passes nothing. If `CartDrawer` were used on a page that declares no
`drawer`, the build would fail at that use.

## Compile errors this adds

- a signal read with no declaration around the use;
- two declarations of one name around a use (no silent shadowing);
- a write outside a handler;
- a signal read on the server;
- a signal of a type with no serial form;
- a `let mut` in a view body (replaced by `signal`, so there is one way).

## Tests it needs before it is accepted

- siblings sharing one signal, with no intermediate view naming it;
- one signal per keyed item, surviving a reorder;
- a write after removal dropped;
- only reading parts updated;
- `persist url` surviving a reload and a shared link;
- each compile error above, as a rejected corpus program.

## Rulings needed

1. The keyword: `signal`, as SEMANTICS has it, or `state`.
2. Whether view composition (ADR-0072) inlines at compile time, which R2's
   static resolution assumes.
3. The storage choices besides ephemeral: `url` only, or also session
   storage.
4. Whether E3's `let mut` view state is removed outright.

## Prior art

Koka's scoped state effects and implicit parameters (static, typed); React
context and Jetpack Compose's CompositionLocal (runtime, with silent
defaults); Svelte stores (shared, unscoped); Elm (one model, no locality).
