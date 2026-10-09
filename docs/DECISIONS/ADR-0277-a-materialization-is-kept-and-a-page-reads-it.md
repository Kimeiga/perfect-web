# ADR-0277: a materialization is kept, and a page reads it

Status: accepted under the owner's delegation of 2026-10-02; the second part
of ruling 10's last piece, "materializations made real" (ADR-0195, ADR-0210's
0092-a), after ADR-0273. Date: 2026-10-08. Milestone: E14.

## Context

- **ADR-0273 made a typed materialization a value its body derives.** Its
  body was checked and held to its type, and its `query R(..)` reads were the
  graph's edges. Nothing compiled it, nothing kept it, and a page reading
  one was refused (PW5108): "the host serves none yet".
- **The pieces were there, unjoined.**
  - The backend refused `query R(..)` in any body.
  - A contract had no kind for a materialization, so no component was built.
  - `pw-materialize` kept entries of text, regenerated one at a time, and
    had no order for a chain.
  - The development server hand-wired two entries, the cart's and the
    menu's, and an event reached query entries alone.
- **A read is a dependency, not authority.** A page's `query Store(id)` is
  recorded in its contract as a dependency on the `Store` component, which
  holds its own `database.read` (architect ruling, 2026-08-07, in
  `contract.rs`). A materialization's read is the same, but its dependency
  may be another materialization, whose value is an entry the host keeps,
  not a component's export to call. So the host must answer the read, not
  wire one component to another.
- **A host function is `'static`** (`pw_host::engine::HostFn`): one cannot
  borrow the server that would answer a read.
- **Prior art.**
  - Noria (OSDI '18) keeps materialized views and fills a missing one by an
    upquery to what it reads.
  - Incremental view maintenance (DBSP, VLDB '23) computes a view's change
    from its inputs'.
  - Next.js's `revalidateTag` regenerates a tagged value on the next request,
    serving the stale one meanwhile.
  - Twitter's home timelines were kept per reader and written on each post
    (Krikorian, QCon SF 2012).

## Decision

1. **A materialization that derives its value is a component**, compiled as
   a query is: its parameters to its type. Each `query R(..)` its body reads
   is a call of the platform's read of R, `pw:host/reads#<R>`, by R's whole
   path, taking R's parameters and answering R's value, its `Ok` where R
   answers a `Result`. Its contract names what each read reads (`reads`) and
   no capability: a read is a dependency, linked with no grant, and what it
   reads holds its own authority.
2. **The host keeps its value as the materializer's entry**, keyed by its
   arguments, encoded so that it reads back whole: each case and field by
   name, each number by its width. A reader asking for one that is missing,
   or out of date, has it made, once for every reader asking at once
   (`stampede single_flight`). Where it could not be made, the reader is
   served the last good value, where `fallback last_known_good` keeps one.
3. **A read is answered from what it reads, and the body asked again.** A
   body only reads, and asks the same of the same answers. So where it asks
   for one not yet answered, the host answers it and runs the body again
   with what it has learned, until it asks for nothing more. A query is read
   through its kept answer, where a page's plan gives its policy, so that a
   change reads it once for the page and for what derives from it. A
   materialization is read from its entry.
4. **A page reads one as it reads a query**, `let x = query M(args)`: a
   public one (`partition public`), kept for every reader. A private one,
   kept per reader, is refused at the page by name: the host keeps none yet.
   A query reading one stays refused: derivation is a materialization's
   (ADR-0210).
5. **An event makes the chain again, in its order, and tells its readers.**
   It reaches each kept entry by the materialization's own `invalidates_on`,
   or through what it reads (ADR-0102). Every entry it reaches is out of date
   before any is made again, and each is made after every one it reads, so
   none is asked for twice. Each open document that reads an entry made
   again with another value, at its key, is read again and sent what
   changed; no other is, not even another document of the same session, and
   none where the value is what it was.
6. **The store shows a chain**: `MenuSize(id)`, its menu's sections and
   items counted from `Menu(id)`, and `MenuLine(id)`, "3 items in 1
   section", from `MenuSize(id)`. The store's page shows the line, live as
   the menu changes.

## Acceptance

- **`compiler/pw-core/tests/materializations_kept.rs`, 3 tests**, each with
  its control:
  - each read is the platform's and names what it reads, answering the
    menu's `Ok` (a query reads nothing so);
  - one that derives its value is a component that imports its reads, and
    the WIT declares them (a fragment that declares no type is no
    component);
  - a page that reads one depends on it and binds it (and no fragment).
- **`compiler/pw-core/tests/materialization_bodies.rs`**: a page reads a
  public one; a private one and a query reading one are refused (PW5108).
- **`runtime/pw-materialize/tests/chain_order.rs`, 2 tests**: each comes
  after what it reads, through any number of reads (the controls: by path
  where none reads another; a cycle left in path order).
- **`spikes/own-renderer/server/src/materializations.rs`, 3 tests**: every
  kind of value reads back as it was, and what no entry holds is refused.
- **`spikes/own-renderer/server/src/tests/materializations.rs`, 5 tests**:
  - a page is served what the chain derives, every reader's;
  - a change makes the chain again in its order, the count then the line,
    each once, and tells the document that reads it;
  - a document that reads another key is told nothing, though its session
    holds one that is;
  - an entry made again with the value it had tells no one: a rename counts
    the same, and its document is not read again;
  - what could not be made again is served its last good value, and is made
    once the fault is gone.
- **`spikes/own-renderer/e2e/materialized.spec.mjs`, in three engines**: the
  store's page shows the line with scripts off; an item added reaches every
  open page of the store, and one taken away; another store's page is told
  nothing.
- **Found by the load test** (`sustained_load_leaves_the_server_bounded`):
  told on every making, each of 300 renames read a thousand open store
  pages again, and the run outlasted the idle window, which forgot every
  visitor. A rename leaves the count as it was, so nothing is told.
- **The tests this changed**, each updated for what it now holds:
  - the store's components, two more;
  - its insert's patches, the line's text with them;
  - its page's last part, its accessibility tree and its count of identity
    markers, the line among them;
  - its graph, contracts, WIT, page plan, static values and committed
    components, regenerated, the two materializations among them;
  - the entries a thousand departed visitors leave: the menu's, and now its
    count's and its line's, every reader's;
  - and the host's import mirror, with a read's rule.
- **`scripts/materializations_kept_mutations.py`, 21 mutants**, recorded
  by `just e14-materializations-kept`:
  - the compiler's: a read lowered, answering R's `Ok`; one that derives its
    value lowered at all; a read naming what it reads; a page reading a
    public one, and refused a private one;
  - the host's: a read linked with no grant;
  - the materializer's: a chain's order;
  - the server's: a page's read answered from the entry; an event reaching
    a kept entry; the chain made in its order; an entry out of date before
    it is made again; a document told only where the value changed; each
    document that reads the entry at its key told, and no other; a menu
    change's entries told; a query read through its kept answer; a body's
    answers remembered; a fault holding; the last good value served; and a
    value read back at its width.

  21 of 21 killed. The first run killed 19. "Each document of a reader's
  session is told" and "a document is told whatever key it reads" survived:
  the test of a document that reads another key held only that the document
  was sent no operation, and a document read again whose page did not
  change is sent none either. The test holds now that the document is sent
  no telling at all, and that the control's document is sent one.
- **The scripts whose code or tests this changed, run whole**: 16 scripts,
  166 of 166 mutants killed (`materialization_body`, `query_reads`,
  `pages`, `not_found`, `cross_session`, `telling`,
  `command_invalidations`, `every_entry`, `row_reads`, `menu_changed`,
  `availability`, `slots`, `descriptions`, `accessibility`, `titles`,
  `patch_set`).

## Not claimed

- **A private materialization**, kept per session or user: the partition by
  principal first. A timeline kept per reader and written on each post, as
  Twitter's was, waits on it.
- **Incremental maintenance**: an entry made again recomputes its body
  whole, reading each of its reads again.
- **`regenerate on_read`**, served stale and made again on the next read, as
  Next.js does: `on_invalidation` is the only value the host runs.
- **The store's `MenuFragment`**, a fragment that declares no type, is still
  hand-wired: making it a typed value the page renders is its own change.
- **A body asked again for each read it lacks**: each run learns one answer,
  so a body of n reads runs n + 1 times when it is made. It is pure, and the
  host bounds it at 64 questions.
- **Telling a command's readers precisely**: a command's commit tells the
  documents that read an entry it made again, as a menu change does; a
  query's drops still tell each session that reads the query (ADR-0219).

## Alternatives

- **A component import of what it reads**, `pw:app/<R>`: the contract's
  form for a page's dependency. The host would have to answer a
  materialization's read from an entry, not an export, so the platform's
  read it is, with the dependency named in `reads`.
- **Borrowing the server in a host function**: `HostFn` is `'static`, and
  an `unsafe` lifetime would make every read a promise the type system no
  longer checks.
- **Reading every dependency before the body runs**: a read's arguments may
  come from an earlier read, a timeline reading whom you follow and then
  their posts, so the body must be asked.
