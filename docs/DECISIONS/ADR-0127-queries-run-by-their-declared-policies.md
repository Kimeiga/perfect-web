# ADR-0127: queries run by their declared policies

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14 (E14-Q, second slice).

## Context

After ADR-0125 the development server ran the store's queries, and ran every
one of them on every render. What each query declares about reuse was read by
nothing at run time:

```pleris
public query Menu(id: StoreId) ...
    freshness 5.minutes   cache shared   key id   concurrency one_per_key
session query Cart(session: Session<SessionId>) ...
    freshness 0.seconds   cache private  key session
```

`runtime/pw-resource` has implemented these policies since E4 (freshness,
privacy-partitioned caching, one flight per key, retries, deadlines), and a
test fed it compiler manifests. Nothing connected the two in a running server.

## Decision

1. **The page plan carries each binding's policy** (`page_values::Policy`):
   cache partition, privacy, freshness, timeout, attempts, `parallel`, and the
   key as argument positions. The server needs nothing beside the plan.
2. **`pw-resource` decides.** The server fetches each binding through it, on a
   clock it keeps at wall time. `pw-resource` caches a token and the server
   keeps the value by token, a few per key.
3. **Defaults, each the conservative reading:**
   - **no `key` clause keys by every argument**, so two calls with different
     arguments never share an entry (consistent with PW0336, which refuses a
     key that leaves out an argument the body reads);
   - **no `freshness` is 0**: a value nobody said may be reused is not reused;
   - **no `cache` clause is not cached** (`Manifest::uncached`, new): one
     flight is still shared, and nothing outlives it.
4. **A private entry's key begins with the session**, as `pw-resource`
   requires of its host; a shared entry's does not. A forgotten session's
   private entries are evicted with its other state.
5. **A commit drops exactly what it made stale** (`Resources::invalidate_key`,
   new): each entry the command names in `invalidates`, and each entry an
   emitted event reaches through `invalidates_on`, bound by the names it gives.
   An event leaving a key unbound (`_`, ADR-0091) drops the whole query. A
   change to the deployment's menu drops `Menu`'s entries.
6. **An invalidated read reads again.** A flight a commit invalidates loses the
   right to publish; its reader is answered by a new read, not an error.
7. **`/metrics`** reports how many times each data-layer operation ran and how
   many values are kept (charter §10.5).

## Found while building

- **A document served after a menu change showed the old menu** (E7-P).
  `broadcast_menu` regenerated the menu fragment without invalidating it, and
  `Materializer::regenerate` leaves a current entry alone, so pages already
  open were patched and every page served later got the first rendering. Only
  once the server read the menu through its query did the two disagree in a
  test. The fragment is invalidated before it is regenerated.
- **Two presses at once rolled both back.** Each commit invalidated the
  session's cart entry while the other command re-read it; the invalidated
  read was treated as an error and the command failed. Fixed by decision 6.
- **A late invalidated flight could evict the value a newer reader was handed**
  if one value were kept per key. Kept: the last four per key.

## Acceptance

- `runtime/pw-resource/tests/concurrency_regressions.rs`:
  `an_uncached_result_is_not_kept_and_one_entry_can_be_dropped`.
- The development server's tests (25):
  - `a_shared_query_is_run_once_for_its_readers_and_a_private_one_for_each`:
    two readers, one `Store` and one `Menu`, two `Cart`s; past 30 s, `Store`
    again and `Menu` not;
  - `a_command_drops_the_entry_it_invalidates_and_no_other`;
  - `a_menu_change_drops_the_menus_kept_values`, which also shows the changed
    menu in a newly served page;
  - `an_entry_key_is_scoped_by_session_only_when_private`;
  - `concurrent_commands_on_one_session_all_commit`: 8 threads × 5 presses.
- The own-renderer suite in WebKit and Chromium (109 of 109 each), the E14
  contract on all three stacks (27 of 27), and T01's and T08's controls on
  Pleris.
- **Not recorded:** a mutation-control script for this ADR. Unlike most
  decisions since ADR-0038, these tests have not been shown to fail with each
  piece undone.

## Not claimed

- **`on_key_change cancel`.** A served page does not change its key, so nothing
  cancels on a key change; T07 needs a page that navigates.
- **`consistency`.** `read_your_writes` holds here because the cart is kept for
  no time and a commit drops the session's entry; nothing enforces a
  consistency mode as such.
- **T12's checker gap** (ADR-0124): a session's cart in a `public` query
  still checks. At run time its entries are keyed by its session argument, so
  this server would not serve one session's cart to another; the defect is the
  label rule, and its fix is a ruling on ADR-0085.
