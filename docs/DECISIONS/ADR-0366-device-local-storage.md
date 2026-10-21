# ADR-0366: device-local storage: a module's signal the device keeps, read before it is shown, never sent

Status: proposed by track `device-store` for the integrator's ruling; a
design note (step 1) and then its first version (step 2), built in the same
track. Date: 2026-10-20. Milestone: E14. Charter §9.1's fourth kind of state,
§9.3 ("browser/device-only → browser execution"), §7.8 (`Device`), §7.10
(browser storage starts as `Unknown`). Revisits ADR-0130's third ruling ("A
signal is ephemeral, or persisted in the URL; nothing else ... Revisit when a
task needs state that survives a reload"), as that ruling asked. Builds on
ADR-0144 (a module's signal, provided where it lives), ADR-0129 and ADR-0282
(a value's label follows it; what a value holds is one label), ADR-0227 and
ADR-0229 (what the browser computes from a signal), and the TodoMVC track's
`fragment` clause (a signal kept in step with something outside the page).
ADR-0339's D2 (`device-store`) is this track; its D1 and the
end-to-end-encrypted messages build on it (decision 8).

## Context

### What waits on it

- **TodoMVC's official acceptance.** The unchanged Cypress suite passes 3 of
  29 (`tracks/todomvc.md`, the correction of 2026-10-16 is authoritative):
  the tests assert the browser's own storage, and TodoMVC keeps its todos in
  the host's memory, by session. The ruling of 2026-10-09 (NEXT) said its
  device version "waits for the Docs app's device-local storage".
- **The Docs app's offline edits** (ADR-0339 D2: "IndexedDB, a partition per
  user ... `navigator.storage.persist()` asked once; a quota error said on
  the page; what it holds read back through a decoder").
- **The end-to-end-encrypted messages** (NEXT, "device-only"): "a placement
  label ... by which the compiler proves a message's plaintext and the private
  keys never reach the server: not in a command's argument, a query, a log, a
  cache or an event".

### What the official TodoMVC tests ask of the store (read 2026-10-20)

`spikes/todomvc-cypress/cypress/e2e/spec.cy.js` (track `todomvc`, upstream
`ff43b02e`), for a framework in none of its tables:

- `checkItemSaved`: a spy on `win.localStorage.__proto__.setItem`
  (`Storage.prototype.setItem`) "should have been called". A property
  assignment (`localStorage[k] = v`) does not count; `setItem` must be called.
- `checkNumberOfTodosInLocalStorage(n)`: some key's text, `JSON.parse`d, has
  `length === n` ("assuming it is an array"). A version envelope around the
  list at that key fails it.
- `checkNumberOfCompletedTodosInLocalStorage(n)`: `_.filter(items,
  {completed: true})`: each item an object with a boolean `completed`.
- `checkTodosInLocalStorage(text)`: some key's text includes the title.
- The persistence test toggles a todo, waits for storage, then `cy.reload()`
  and expects the same todos, the first `completed`.
- Its `beforeEach` treats a first `setItem` call, or a first listener for
  `input`/`keydown`/`change`, as "the app has started".

TodoMVC's own spec, `app-spec.md` (read 2026-10-20): "Your app should
dynamically persist the todos to localStorage. If the framework has
capabilities for persisting data (e.g. Backbone.sync), use that. Otherwise,
use vanilla localStorage. If possible, use the keys `id`, `title`,
`completed` for each item. Make sure to use this format for the localStorage
name: `todos-[framework]`. Editing mode should not be persisted."

So the first version's store is `localStorage`, under a name the program
chooses (`todos-pleris`), holding the list as a JSON array of
`{id, title, completed}`, written with `setItem`.

### What Pleris has (read at af5c1758)

- **A signal lives in the browser** (ADR-0130, R8): "Server code cannot read
  one", its first value is in the document (R9), a handler alone writes it
  (PW5300), and it is read only where the browser reads it again (PW5301).
  A module may declare one and a page provide it (ADR-0144).
- **`Device` exists as a label and nowhere else.** `privacy.rs`: "Belongs to
  one device and cannot be moved off it"; `placement.rs`: "Device-bound
  values exist only where the device is" (`World::Browser` alone); a shared
  cache refuses it (`safe_in_shared_cache`); `log<Public>` refuses any
  non-public value (ADR-0129, decision 6). No program value carries it: a
  label comes "only from declarations" (`labels.rs`).
- **No sink refuses a value going from the browser to the server.** Every
  label rule so far stops a value going the other way (a secret into markup,
  a session's value into a shared cache). A command's arguments, a query's
  key and a navigation's address are built in the browser and sent.
- **What the browser computes** (ADR-0227, ADR-0229, ruling 0073-a): a text
  part, an attribute or a block's subject computed from one signal at the top
  of a page; a block a signal decides rendered again whole by the server's
  renderer built for the browser; an `{#each}` over a signal's list only
  inside such a block, by path. A probe on 2026-10-20 (`pw build` of a page
  whose list is a signal) was refused twice, by name: "computes a value in a
  row of `row`, a list no query gives, which no host computes yet (ruling
  0073-a)", and "`{#each}` reads its list by path, and `rows_of(todos)` is
  computed".
- **A signal kept in step with the outside** has a precedent: the TodoMVC
  track's `fragment filter "/" => "all", ..` (PW6101-PW6106), which the
  runtime follows at start and on `hashchange`, with the declared default
  the server renders.

### The platform (primary sources, each read 2026-10-20)

- **Web Storage** (WHATWG HTML, "Web storage"): `setItem` "Throws a
  QuotaExceededError if the new value couldn't be set", before the map
  changes; each change "Dispatches a storage event on Window objects holding
  an equivalent Storage object", excluding the one that made it; the event
  carries `key`, `oldValue`, `newValue`, `url`, `storageArea`; "authors are
  encouraged to assume that there is no locking mechanism". Strings only,
  synchronous, the main thread's.
- **The Storage Standard** (WHATWG, Living Standard, last updated
  2026-10-19): `localStorage` is a storage endpoint with a quota of 5 × 2^20
  bytes (5 MiB); a bucket's mode is "best-effort" or "persistent";
  `persist()` asks the "persistent-storage" permission; `estimate()` is "an
  implementation-defined rough estimate"; "Whenever a storage bucket is
  cleared by the user agent, it must be cleared in its entirety".
- **Quotas and eviction** (MDN, "Storage quotas and eviction criteria", last
  modified 2026-01-05): best-effort data "persists as long as the origin is
  below its quota, the device has enough storage space, and the user doesn't
  choose to delete the data"; an origin may use up to 60% of the disk in
  Chromium, the smaller of 10% of the disk and a 10 GiB group limit in Firefox
  (50% when persistent), about 60% in Safari's browser app; eviction is least
  recently used origin first and "skips over origins that have been granted
  data persistence"; "In Firefox, when a site chooses to use persistent
  storage, the user is notified with a UI popup", where Safari and Chromium
  "automatically approve or deny the request based on the user's history of
  interaction". Its two sentences on Web Storage disagree (a 10 MiB maximum
  and "up to 5 MiB of local storage"), so the first version measures it
  (Acceptance).
- **WebKit** (2020-03-24, "Full Third-Party Cookie Blocking and More"): ITP
  deletes "all of a website's script-writable storage after seven days of
  Safari use without user interaction on the site", LocalStorage and IndexedDB
  among them; "Web applications added to the home screen are not part of
  Safari and thus have their own counter". (2023-08-10, "Updates to Storage
  Policy", Safari 17): "all origins use a best-effort mode"; a persistence
  request is granted "based on heuristics like whether the website is opened
  as a Home Screen Web App"; the policy covers "localStorage, Cache API,
  IndexedDB, Service Worker, and File System".
- **IndexedDB** holds structured-cloneable values (bytes, `CryptoKey`) in
  transactions, asynchronously, under the origin's bucket quota. It tells no
  other tab of a change; `versionchange` fires on other connections when an
  upgrade or deletion "was requested elsewhere" (MDN, 2025-05-02).
- **Web Cryptography** (W3C Editor's Draft, 2026-08-11): "CryptoKey objects
  are serializable objects", their `[[extractable]]` slot kept through
  serialization; "it is expected that most authors will make use of the
  Indexed Database API" to keep them.
- **BroadcastChannel** (WHATWG HTML): a message goes to every channel of the
  same name and storage key, the source removed ("Remove source from
  destinations").

## Decision

### 1. A value the device keeps is a module's signal, provided by the device

```
type Todo = Todo { id: String, title: String, completed: Bool }

// What this device keeps of its todos: the list itself, under TodoMVC's
// name, never sent anywhere.
signal todos: List<Todo> = []
    storage  device "todos-pleris"
    privacy  Device
    conflict last_write_wins
```

- **A module's signal with a value and a `storage device` clause.** ADR-0144's
  module signal names a value pages and views share and has no value of its
  own (PW5306); a page provides it. A signal the device keeps is provided by
  the device: its written value is the default the device starts with, and no
  `provide` may give it another (PW6808).
- **Read and written as any signal is.** A handler assigns it (PW5300); a
  template part, a handler or a page query's key reads it (PW5301); what a
  page computes from it is the browser's (decision 7). Every page and view
  that names it holds the same value, the device's, in every tab.
- **It states whose it is.** `privacy Device`: it belongs to this device and
  never leaves it (decision 5). `storage device` says where it rests,
  `privacy` where it may go: ADR-0339's "`storage device` is not the `Device`
  label". The first version takes `Device` alone; a device value that is a
  user's (`privacy User(reader)`, partitioned by user, cleared at sign-out) is
  D1's.
- **It states what two tabs' writes do.** `conflict last_write_wins`: the
  whole value written last is every tab's (decision 6). A-011's rule holds:
  "the policy is part of the declaration, so there is no silent
  last-write-wins". It is the only policy the first version offers.
- **Its name is the program's** (`"todos-pleris"`), a stable name that a
  rename of the Pleris binding does not change: the data outlives the code
  that wrote it. One name per program (PW6805); letters, digits, `-`, `_` and
  `.` only.
- **Its type has a device form** (PW6807): data the browser's wire carries
  (ADR-0130's R7), and in the first version no function, no type that
  contains itself (ADR-0205's nodes) and no opaque type with a `where`
  predicate, whose decoder would have to run the predicate (Not claimed).
- **Its default is data**, as a signal's first value is (backend
  `signals.rs`: "A first value that computes is refused").

### 2. The store: `localStorage` for the first version, IndexedDB for bytes and keys

- **The first version's backend is `localStorage`.** It is what TodoMVC's
  spec and its official tests name; its read is synchronous, so the runtime
  reads it in its first task (decision 3); its `storage` event tells every
  other tab of a write with no channel of Pleris's (decision 6). Its limits
  are stated: strings only, a few MiB an origin (measured, Acceptance), the
  main thread's, no transaction across keys.
- **IndexedDB is the backend for what `localStorage` cannot hold**: bytes,
  a `CryptoKey`, a replica's saved document and a collection that grows (D1's
  replicas, the messages' keys and history). It is chosen by the platform from
  the value's type, never written by the program: the declaration says what
  the value is and whose, not which browser API keeps it. Changing a value's
  backend is the runtime's migration; no program changes.
- **What is stored**: the value's wire form (`js_pure`, ADR-0130): a record an
  object keyed by its field names, a list an array, a case `{"$case": ..}`, an
  opaque type its representation, an `Int` its exact digits. A `List<Todo>` is
  `[{"id":"1","title":"buy some cheese","completed":false}]`, which is what
  TodoMVC's tests read.
- **Its key** is the declared name at the first version, and `name@v` at
  version `v` from 2 (decision 4). A key the program does not declare is never
  read, written or removed.
- **Quota.** A write `setItem` refuses (`QuotaExceededError`) leaves the
  previous stored value as it was (the HTML algorithm throws before the map
  changes), keeps the page's value (the person sees what they did), and is
  said where the change was made, in the platform's words, through the page's
  announcer (ADR-0302's `tell`): "This device has no room to keep that." The
  next write tries again.
- **Eviction and `persist()`.** The first version keeps a best-effort bucket
  and asks nothing: Firefox would show a permission prompt at a todo list's
  load, and Safari and Chromium answer by heuristics no request changes. What
  that means is said, not hidden: the browser may clear the origin's data
  under storage pressure, Safari after seven days of use without a visit, and
  all of it at once. The IndexedDB backend asks `persist()` once an origin,
  on the first write a person makes (D1's "asked once"), and keeps the
  answer for the page to show.

### 3. What the server renders before the device is read, and what is shown

- **The server renders the declared default**, as it renders any signal's
  first value; it never has the device's.
- **Nothing the device decides is shown until the device has been read.** The
  compiler marks each element holding a part a device value decides (an
  attribute of the element, or a text part or block whose nearest element it
  is) with `data-pw-device`. The document's root carries `data-pw-device=
  "reading"`, and the head's style, allowed by its hash (ADR-0333), keeps
  those elements `visibility: hidden` while it does. A `<noscript>` style
  shows them, at the default, where no script will run. The rest of the page
  is shown and usable as it arrives: the TodoMVC header and its focused
  input are not the device's.
- **The runtime reads the device first.** In `attach`, before any handler
  binds: each device value the page holds is read, decoded, migrated
  (decision 4), set as the signal's value, and every part that reads it is
  rendered again (decision 7's computed parts and blocks); then the root's
  mark is removed, in the same task as the last render. So the first frame
  that shows a device-decided element shows the device's value, or the
  default where the device holds none.
- **What is read is decoded, never trusted** (charter §7.10): a validating
  decoder, made by the compiler from the type (a record's fields present and
  of their types, a list's elements, a case the type has, an `Int` a safe
  integer), turns the stored text into the value or a refusal. A refusal
  leaves the default in place, is logged, and the stored text is copied to
  `name!unreadable` before the first write replaces it: corrupt bytes are
  never used and never silently lost.

### 4. Versions and migrations

```
signal settings: Settings = Settings { theme: "light", compact: false }
    storage  device "settings"
    privacy  Device
    conflict last_write_wins
    version  2
    migrate  settings_from_1

fn settings_from_1(old: SettingsV1) -> Settings !{} { .. }
```

- **A value states its version** (`version`, 1 where none is written) and,
  for each version before it, the pure function that takes a value of that
  version to the next: `migrate f1, f2, ..` in order (PW6806). The first
  function's parameter type is version 1's type; each one's result is the
  next one's parameter; the last one's result is the declared type; each
  performs nothing (`!{}`).
- **Each version has its own key** (`settings`, `settings@2`), so a stored
  value says its version by where it is, and one `setItem` writes a value and
  its version at once.
- **At boot**, the runtime reads the declared version's key; where it holds
  nothing, it looks for the newest earlier version that does, decodes it with
  that version's type, runs the functions after it in order, writes the
  result under the new key and only then removes the old one. A failure at
  any step is decision 3's refusal, with the old value left where it was.
- **An older page stops writing.** A tab still running the build before the
  migration would write the old key, which no newer page reads. A page that
  finds, at boot or by a `storage` event, a version of its value newer than
  its own keeps showing what it has, writes nothing more of it, and says so:
  "This page is out of date. Reload to keep your changes."
- **The released versions are held to, in a later version** (the tables'
  `schema.lock`, ADR-0350, applied here): `pw build` would write
  `device.lock`, each value's name, version and the canonical form of its
  type, and a build given the released lock would refuse a value whose type
  changed at the same version: a changed type with no new version and
  migration is a person's data unread after the next deploy. The first
  version does not build it (Not claimed); until it does, the decoder refuses
  such a value at read, keeps it aside and shows the default (decision 3), so
  nothing is misread, though the person's data is unread until a migration
  is written.

### 5. Device-only data never crosses to the server

- **The label comes from the declaration.** A read of a signal declared
  `privacy Device` carries `Device`, and from there it follows the value as
  every label does (ADR-0129, ADR-0282): through calls, a body's value,
  branches, assignments, a loop's elements, and the conditions a sink is
  reached under.
- **The browser's ways to the server are each a sink**, the new rules:
  - **PW6801**: a command's argument, and a command call reached under a
    device value's condition (it tells the condition);
  - **PW6802**: a page query's key, which the browser sends to read it again
    (ADR-0152);
  - **PW6803**: what the browser sends of the page itself: a navigation's
    arguments (ADR-0280), an address-bearing attribute (`href`, `src`,
    `srcset`, `action`, `formaction`, `poster`, `cite`, `data`, `ping`, the
    list `template_ir` holds for unsafe schemes), and a form control's value
    or binding inside a `<form>`.
- **What is already a sink for it, unchanged**: a public log (ADR-0129's
  sixth decision), a shared cache (`safe_in_shared_cache`), and placement: a
  declaration holding `Device` runs only in the browser (`may_hold`), and
  server code cannot read a signal at all (R8).
- **What the device keeps is the device's** (**PW6804**): a value assigned to
  a device signal holds no session's, user's, organization's or secret's
  label. A session's cart kept on the device would outlive the session on a
  shared computer, which is what `privacy User(..)`'s partition and its
  clearing at sign-out (D1) are for.
- **Held at run time too**: the browser tests record every request each page
  makes and find no stored value in any of them (Acceptance).
- **A handler's capture of one is written where the device is.** A resume
  manifest may hold a captured value only where its label fits the document
  it lands in (PW5007). A device value exists only in the browser, which
  renders the parts that read it and writes their handlers' captures into the
  document it holds, on the device; the server writes a capture of one only
  at its declared default, a constant of the program's own text. So `Device`
  is no restriction on that document (`resume.rs`, `written_by_its_device`);
  every other label is one, as before. TodoMVC's row handlers capture their
  `row`, made from its todos (`resumable(captures = { row })`).

### 6. Several tabs

- **Every tab shows the last write.** A write is `setItem`; every other tab's
  runtime hears the `storage` event, decodes `newValue` as at boot, and sets
  its signal without writing it back; a `clear()` (`key` null) reads every
  value again. The declared policy is the only one offered,
  `last_write_wins`, on the whole value.
- **What it costs, said**: two tabs writing within one event's delivery each
  write a whole list, and the later one wins; the earlier tab then shows it.
  A change is lost visibly, never silently diverged. There is no lock (the
  HTML spec's own advice); a merge by key is a later policy.
- **The IndexedDB backend** tells other tabs by `BroadcastChannel` (IndexedDB
  tells none), and upgrades through its own versioned open and
  `versionchange`.

### 7. What the browser computes for a value it alone holds

A device value is rendered by the browser alone, so what a page can show of
one is what the browser can compute (ruling 0073-a's later steps). This
track builds the two steps TodoMVC needs:

- **A value computed from several browser-held values** (signals, device
  values among them): one function, an argument for each; the host computes
  the first render from their first values, the browser every render after.
- **Inside a block the browser renders, a computed value is the browser's**,
  including the block's `{#each}` list where every value its function reads
  is a signal: computed before the block renders, and read by its path, as a
  computed hole is (ADR-0226). The block is rendered again for each value a
  computed part inside it reads.
- **Inside a block a host decides, it is refused by name**, since the browser
  does not render that block again and the value would stay its first:
  "computes a value from the signals `notes` and `shown` inside a block a
  value other than a signal decides, which the browser does not render again
  (ruling 0073-a)".

Rows stay read by path: a list's function makes the rows whole (`Row { id,
title, completed, class, editing }`), so no value is computed inside a row
(refused by name, as before; a row's item is typed from the list the device
keeps, so the refusal is this one and not a type the build lacks).

- **A block rendered again keeps its nodes.** The browser replaced such a
  block whole each time a signal it reads changed, so a row was another
  element after each press: focus, a hover, what assistive technology held
  and a test's handle on a row were lost, and TodoMVC's own tests 12 and 13,
  which hold a todo's `<li>` across its toggle, failed once its list was the
  browser's to render. Now the new rendering is morphed into the old: a node
  of the same kind (its tag, `data-pw` and `id`) is kept and its attributes
  and text set; a keyed list's row is matched by its key's anchor and kept,
  moved or removed; only what is new is inserted and only what is gone
  removed; a control's `checked` and `value` follow the rendering where it
  changed them (ADR-0142); and a modal dialog that goes is closed first, its
  focus given back (ADR-0141).

#### The two refusals decision 7 lifts, what each said and why, and what
still fires

*Ruled by the integrator, 2026-10-20 (ruling 3 on this note's questions):*
decision 7 is accepted on condition that this note names each ruling-0073-a
refusal it lifts, its original reason, why that reason cannot apply to a
value that never leaves the device, and a test that each refusal still fires
for a value a host holds. This is that, written at the train-12 merge; its
test and mutants are the train's.

*Clarified and accepted under the owner's delegation, 2026-10-20:* this
scope includes every browser signal, provided every input is a signal.
Host-, query- and parameter-derived values retain the refusals below. The
host renders the program's declared initial values; subsequent signal
values belong to the browser (ADR-0130). The `Device` rules add confinement,
not a prerequisite for computing from browser-local state. The mixed
host-and-signal witness and all three mutation controls below remain
required.

**What is lifted is lifted for the browser's values**: the page's signals,
the values the device keeps among them. A signal's first value is the
declaration's, which a host renders; every later one is the browser's alone
(ADR-0130). A value the device keeps is not in a host at all: PW6801 to
PW6804 refuse each way it could go there, the `Device` label follows what
is computed from it, and a public log, a shared cache and placement refuse
it as before (decision 5). A **host's** value is a query's, a page's or a
view's parameter, and anything computed from one. The lift looks at each
value a computed part reads (`computed_part`'s `browsers`): a part whose
every input is a signal is the browser's; a part with a host's value among
its inputs is refused as it was.

**1. A value computed from several values.**

- *What it said* (`page_values.rs`, `computed_part`): "part N computes a
  value from `a` and `b`, and a host computes one from one value (ruling
  0073-a)".
- *Its reason.* ADR-0226 and ADR-0227 give the refusal as "not yet"
  (ADR-0226 decision 5: "one from several values, or from none"; ADR-0227
  decision 5: "one from a signal and a query's value together"). The reason
  that holds for a host's value is that a computed part is one function
  computed by the one party that holds each input and hears of each change:
  a host for a query's value, which it reads again; the browser for a
  signal's, which no host hears of. From a signal and a query's value
  together neither is that party: the host never hears the signal change,
  and the browser has the query's value only as the page was rendered, so
  whichever computed the part would show a value no longer true of one of
  its inputs. Ruling 0073-a itself says the browser computes "when the
  inputs are signals or speculated values".
- *Why it cannot apply.* Every input is the browser's: it holds each and
  hears of each change, as it does of one. No input is a host's that could
  change without it; a host renders the first value from each input's
  declared default, and the browser replaces it. For a value the device
  keeps a host has no value that could disagree, since none reaches it, and
  what is computed from the value carries `Device` and goes where the value
  goes.
- *Still fires, for a host's value.* A query's value read with the device's,
  with a signal's or with another query's is refused in the same words:
  `a_value_a_host_holds_is_still_refused_where_decision_7_lifts_the_refusals`
  (`compiler/pw-core/tests/device_store.rs`: `total(notes, likes)`, the
  device's value and a query's, and `sum(likes, more)`, two queries'),
  `what_a_host_does_not_compute_is_refused_by_name` (`computed_holes.rs`) and
  `what_the_browser_does_not_compute_yet_is_refused_by_name`
  (`computed_signals.rs`). Its mutant, "a value from a browser value and a
  host's together is built as the browser's"
  (`scripts/device_store_mutations.py`), is killed by the first and by no
  other test of that file.

**2. A value computed inside a block a signal decides, and the list such a
block reads.**

- *What it said*: "part N computes a value inside a block a signal decides,
  and the browser, which renders that block again, computes none yet (ruling
  0073-a)" (`page_values.rs`, `rendered_again`), and for a block's list,
  "`{#each}` reads its list by path, and `f(x)` is computed"
  (`template_ir.rs`, `lower_block_at`).
- *Its reason.* A block a signal decides is rendered again, whole, by the
  browser, from the page's signals alone (ADR-0130, ADR-0137; ADR-0229
  decision 4). A value a host computes from a query's value is not among
  them, so the second rendering would not have it and the part would stay at
  its first value; and the browser computed only at the top of a page, never
  before a block renders (ADR-0227 decision 1). A list is read by its path
  and the renderer computes none (ADR-0073).
- *Why it cannot apply.* The value the lift builds is not a host's. The
  browser computes it from the signals it holds before it renders the block
  and gives the block its value by path, as it does a computed hole's
  (ADR-0226), computing it again and rendering the block again when any of
  its inputs changes (the plan's `reads`): nothing stays at a first value. A
  list is the same, its function's inputs signals and its rows keyed as any
  list's. For a value the device keeps there is no host's value to be
  missing from the second rendering.
- *Still fires, for a host's value.* A query's value computed inside a block
  a signal decides, beside the device's or alone, and a list computed from a
  query's value, are refused in the same words: the device-store test above
  (`total(notes, likes)` and `plus_one(likes)` inside `{#if
  List.length(notes) > 0}`, and `{#each merged(notes, pinned)}` and `{#each
  only(pinned)}` in it), `what_a_host_does_not_compute_is_refused_by_name`
  (`computed_holes.rs`: a query's value inside `{#if open}`),
  `what_a_condition_does_not_compute_yet_is_refused_by_name`
  (`computed_conditions.rs`) and `the_ir_blocks_a_computed_list_and_a_
  foreign_key` (`template_values.rs`: a list computed from a view's
  parameter). Its mutants, "a value a host computes inside a block a signal
  decides is let through" and "a list computed from a host's value is built
  as the browser's", are each killed by the device-store test and by no
  other test of that file.

**Not lifted.** A value computed from the browser's values inside a block a
host decides is refused by name, in new words (the browser does not render
that block again; "the browser computes one at the top of the page" is no
longer the reason): `a_value_computed_from_the_devices_inside_a_hosts_block_
is_refused_by_name`. One in a loop's row is refused as before:
`a_value_computed_in_a_browser_rows_is_refused_by_name`. The rest of ruling
0073-a's list (a page's parameter, an arm from the names it binds, a view
that contains itself, a speculated value, an attribute's text) is not
touched by decision 7.

### 8. How the Docs app and the encrypted messages build on it

- **D1's replicated document** is kind 4 with a sync and a conflict policy,
  stored by the same runtime module: `storage device` with `privacy
  User(reader)`, on the IndexedDB backend, in a partition per user (the
  principal's id, hashed) cleared at sign-out; `persist()` asked once.
  Decisions 3 to 6 hold for it unchanged; its sync is ADR-0339's.
- **The messages' keys and history** are `privacy Device` on the IndexedDB
  backend: keys as platform handles over non-extractable `CryptoKey`s, which
  no program value holds, and a history whose plaintext decision 5 keeps on
  the device. Their decryption and rendering in the browser is decision 7
  extended to a query's ciphertext.

### 9. Codes

A block of its own for this track, **PW68xx** (`Owner::DeviceStore`): no
branch head and no ADR uses it (checked 2026-10-20 across every local and
`origin` branch). ADR-0339's PW6703 ("`storage device` holds only a value
keyed by its reader's own handle") stays D1's: it is the user partition's
rule, which the first version does not offer.

| code | rule |
| --- | --- |
| PW6801 | a device value reaches a command's argument, or decides a command call |
| PW6802 | a device value keys a page's query |
| PW6803 | a device value is written where the browser sends it: a navigation, an address, a form |
| PW6804 | a device signal is assigned a session's, user's, organization's or secret's value |
| PW6805 | a device signal's clauses: `storage device "name"` (one name a program), `privacy Device`, `conflict last_write_wins`, at module level, with a value |
| PW6806 | a device signal's versions: a pure migration for each earlier version, chained by type, the last to the declared type |
| PW6807 | a device signal's type has a device form |
| PW6808 | a device signal is provided by the device: no `provide` gives it |

## Alternatives

- **A placement** (`placement device` on a query or a table). Placement says
  where code runs (charter §7.9's worlds); a value at rest is not code, and
  the Docs note keeps the two apart. Reading a device value does make what
  reads it the browser's, which placement already derives from the label.
- **A source** (ADR-0207) whose operations run on the device. A source
  states a database's guarantees to host operations, and "no database" is
  the browser world's (charter §7.3); a device value has no operation but
  read and assign, which a signal has.
- **A new resource kind** (`device Todos -> List<Todo>`, read with `let todos
  = device Todos()`). A second way to hold browser state, against ADR-0130's
  fourth ruling ("there is one way to hold UI state"), with its own read and
  write rules to keep equal to a signal's.
- **`replicated` with no sync.** A replica with nothing to sync with; its
  conflict policy is between devices, and a device-only value has none.
- **A page clause, as `fragment` is.** A device value outlives the page and is
  shared by pages and tabs; its name, versions and migrations need one place.
- **A page's own signal with `storage device`.** Two forms for one thing; a
  page that wants one declares it in its module.
- **IndexedDB alone.** It cannot pass TodoMVC's own tests, which read
  `localStorage`, and its asynchronous read cannot be in the runtime's first
  task. It is the backend for what `localStorage` cannot hold (decision 2).
- **Cookies.** Sent with every request: the opposite of device-only.
- **The version inside the stored value** (`{"v":2,"value":[..]}`). Fails
  TodoMVC's tests, and a reader of the key must parse it to know what it is.
  A version key beside the value is two writes, which a crash can tear.
- **Show the default, then the device's value** (hydrate and swap). The
  default is wrong content for every person whose device holds something.
  **Hide the whole page** until the device is read: TodoMVC's focused input is
  not focusable while hidden, and the rest of the page waits for no reason.
- **An inline script in the head that reads the store before the body
  paints.** It cannot render the parts; at most it hides them, which the
  server's mark and the head's style do without a script.
- **`persist()` at every load.** A prompt in Firefox at a todo list's first
  visit, and nothing changed in Safari or Chromium.
- **A tab's write merged into another's** (a CRDT per value). The Docs app's;
  a whole-value policy, stated, is the first version's.

## Acceptance (when built)

- **The compiler** (`compiler/pw-core/tests/device_store.rs`): each of
  PW6801 to PW6808 refused with its control accepted; a device value through
  a helper, a branch, an assignment and a loop reaches each sink; the
  rejected corpus gains an exhibit for PW6801 and PW6802.
- **What the browser computes** (decision 7): several signals' value at the
  top and in a block, and a computed `{#each}` list in a block, each built,
  with the host's first render and the browser's agreeing; and each
  refusal it lifts still firing for a value a host holds (the section under
  decision 7, ruling 3).
- **The runtime, in three engines** (`e2e/device-store.spec.mjs`, a demo
  page): a value persists across a reload; two tabs agree; a schema change
  migrates (a version-1 value read by a version-2 page); a stale tab stops
  writing; an unreadable value is refused and kept aside; a full store is
  said on the page; no request carries a stored value; and the store's
  `localStorage` limit measured in each engine.
- **TodoMVC** on the device: its own browser tests in three engines, and the
  official Cypress suite, unchanged, as the count of 29 it passes.
- **A mutation script** (`scripts/device_store_mutations.py`), each rule and
  each runtime step's mutant killed; `just e14-device-store` records it all.

## Not claimed

- **IndexedDB, `persist()`, a user's partition, sign-out clearing**: D1's,
  on decisions 2 and 8.
- **A merge of two tabs' writes**; a write's atomicity across two values.
- **An opaque type's `where` predicate held on read**: refused by PW6807
  until the decoder runs it.
- **A value computed inside a row** of a browser-rendered list, and a list
  rendered again row by row (a block is rendered again whole, then morphed):
  decision 7's later steps.
- **`device.lock`** (decision 4): a type changed at the same version is not
  refused by the build; the decoder refuses it at read.
- **Every channel off the device.** The browser's own history and sync (a
  page's title, its address), extensions, a person's copy and paste, timing,
  and a foreign component mounted with a device value (ADR-0075's mount, not
  built; ADR-0339's PW6707 is its rule) are outside what a compiler sees.
- **Safari's seven days and storage pressure**: said, not prevented.
- **The web's code delivery** (NEXT's E2EE item): the server that serves the
  code that reads the store could serve other code.

## The first version, built (2026-10-20)

Recorded by `just e14-device-store` (`docs/evidence/E14/device-store.txt`)
and `just e14-device-store-cypress` (`docs/evidence/E14/device-store-cypress.txt`).

- **The compiler** (`compiler/pw-core/tests/device_store.rs`, 13 tests):
  each of PW6801 to PW6808 refused with its control clean, each refusal
  naming where its value is the device's (charter §16.3); a device value
  reaches PW6801 directly, through a helper's body, a branch, a page's
  signal it was assigned to and a call it decides; PW6802 through the
  page's signal a query is keyed by; PW6803 at a link, an image's address
  and a form's control; PW6804 from a session's captured value. Decision
  7's values build, and are refused by name inside a row and inside a
  host's block. The corpus gains A-054, R-108 (PW6801) and R-109 (PW6802)
  (C28; the enforcement table 89 of 89), and the generality witnesses
  `device_value_to_a_command` and `device_value_keys_a_query` (62 of 62).
- **The host** (`tests::todomvc::` and `tests::device_store::`, 5 tests):
  TodoMVC's document renders the declared default, marks and hides what the
  device decides while the device is read, its policy allowing both of the
  head's styles by their hashes, names what the device keeps in its
  manifest, and no host holds a todo: TodoMVC's host layer
  (`server/src/todomvc.rs`) is gone. A page whose browser computes nothing
  but a step between versions is served the module that holds it.
- **The browser, three engines**: the demo (`e2e/device-store.spec.mjs`
  over `examples/demo/device.pw`, 15 tests, 45 of 45, a value computed from
  the device's value and the page's and a list reordered among them) and
  TodoMVC
  (`e2e/todomvc.spec.mjs`, 50 tests, 150 of 150); and the whole browser
  suite, since the block morph (decision 7) is every signal block's: 1391
  of its 1416 passed and 22 skipped as their specs say, and the 3 that
  failed are the finding below, one test in each engine, which a page that
  keeps nothing on the device never reaches (`readDevice` returns at once).
- **The store's limit, measured**: `localStorage` holds 5,242,880 UTF-16
  code units an origin in Chromium, Firefox and WebKit (Playwright 1.58's
  builds): the Storage Standard's 5 × 2^20, counted in code units, not
  bytes. MDN's 10 MiB sentence is not what these engines do.
- **A finding the demo's tests made**: the runtime set each value as it
  read it; reading the next one yielded, the render the first had begun
  started and was no longer the one the mark waited for, and an earlier
  value's part was shown at its default for as long as its computing took
  (in each engine: "what the device decides stays hidden until it is
  rendered at the device's value"). Each value is now read first and all
  are set at once, in one render the mark waits for.
- **TodoMVC's own Cypress suite, unchanged**: 24 of 29 pass (3 before
  this track). The acceptance (all 29) is not met. The five that fail, fail
  for any app under the Cypress the suite is run with (15.21.1, which
  upstream's `^15.14.2` resolves to), each shown with no app at all
  (`spikes/todomvc-cypress-probes`):
  - tests 4 and 9 (a hook), and 10 and 11, which the failed hook skips:
    `checkItemSaved` calls `.invoke('reset')` on a spy, and a spy has
    `resetHistory` and no `reset` (the probe prints `reset:undefined
    resetHistory:function`; Sinon removed it in 5.0.1, and Cypress has
    bundled a Sinon after it since 4.0.0);
  - test 24, the persistence test: it aliases each todo its `createTodo`
    command found, a command ending in `.then()`, and reads the alias again
    after `cy.reload()`, which Cypress cannot do ("Cypress cannot requery
    the page after commands such as `cy.should()`"), on a static page too;
    an alias `cy.get` made is read again, the probe's control.
  Before the morph, tests 12 and 13 failed as well: they hold a todo's
  `<li>` across its toggle, and a block replaced whole made it another
  element.
- **The mutation controls** (`scripts/device_store_mutations.py`): each
  rule, each step of the build, the host and the runtime, and the morph
  undone in turn. The first whole run killed 54 of 58; each of the four
  that survived named a test missing, now written: the host's policy of the
  head's styles, a page whose one computing is a step, a list reordered,
  and a value computed again for its second input. The count of the run
  after them is in the evidence.

## Questions for the integrator

1. **The block PW68xx** with `Owner::DeviceStore`, or codes in ADR-0339's
   PW67xx after PW6712 (the Docs app's tracks')? Taken meanwhile: PW68xx,
   cheap to renumber before merge.
2. **TodoMVC's persistence moves to the device**, and with it the app's
   host data layer (`todomvc.rs`, its server tests) goes: no host data is
   left. Its 2026-10-09 ruling said "a second version". Taken meanwhile: one
   version, the device's, the Rust layer removed in its own commit (cheap to
   reverse: history keeps it).
3. **Decision 7 lifts two of ruling 0073-a's refusals** for browser-held
   values only. Taken meanwhile: built in this track, in commits of their
   own, each with its tests and mutants.
4. **TodoMVC's official acceptance, as the correction of 2026-10-16 states
   it (all 29 pass, Cypress exits 0), cannot be met by any app under any
   Cypress since 4.0.0** (The first version, built): the spec resets its
   spy with `.invoke('reset')`, and a spy has had no `reset` since Cypress
   4.0.0 took Sinon from 3.2.0 to 8.1.1 (Cypress's changelog) and Sinon
   5.0.1 removed it (Sinon's `CHANGES.md`: deprecated in 4.1.4). Upstream's
   own `package.json` asks `^15.14.2`. How should the acceptance be stated:
   as the 24 of 29 with the five named and shown, against the suite with
   its spy's call corrected upstream, or otherwise? Taken meanwhile:
   nothing reclassified; the count and the probes are recorded, and the
   acceptance stays unmet.
5. **`device.lock`** (decision 4) is not built. Taken meanwhile: Not
   claimed; the decoder refuses a changed type at read.
6. **The published generality figure** moved from 60/60 to 62/62 (this
   track's two invariants, corpus C28): `tests/generality.rs` holds it, and
   STATUS's "generality is 60/60" is the integrator's to move.

## The integrator's rulings (2026-10-20)

The answers to the questions above, from the integrator's message of about
11:40Z, recorded at the train-12 merge:

1. **PW68xx stays** this track's own block (a new capability, charter 9.1
   kind 4); its owner is registered in `codes.rs` as `DeviceStore`.
2. **Deleting TodoMVC's Rust host layer is accepted**: it is the steering
   goal, no app-specific Rust.
3. **Decision 7's two lifted refusals are accepted on condition** that this
   note names each, its original reason, why that reason cannot apply to a
   value that never leaves the device, and a test that each still fires for
   a value a host holds. Met at the train-12 merge: the section under
   decision 7, its test and its three mutants.
4. **TodoMVC's acceptance is stated as** the official suite, run unchanged,
   on the Cypress version tastejs/todomvc itself pins (found in that
   repository's `package.json`; Cypress is an owner-approved download for
   TodoMVC, and the version's source and size are stated when it is
   fetched): all 29 there is acceptance met. Also reported: 24 of 29 on
   Cypress 15.21.1, the five others shown impossible for any app on Cypress
   4.0 and later (Sinon's `spy.reset` removed; an alias read again after a
   reload) by the app-less minimal tests committed here. The pinned-version
   run is not made; NEXT holds it.
5. **`device.lock` later is accepted**: listed as not claimed in the first
   version.
6. **STATUS's generality is 62/62**: the integrator writes it at the
   merge.

## Numbered on train-12 (2026-10-20)

- **Numbered ADR-0366** (the integrator's ruling of 2026-10-20: a train is
  numbered before it is pushed, after master's ADR-0363, in train order),
  merged from `5d4280b0` (`track/device-store`). `ADR-XXXX` reads `ADR-0366`
  in the track's code, recipes, tests, corpus and evidence (65 mentions in
  36 files), as in this file's name and title.


## Generic integration and trustworthy controls (2026-10-20)

The generic solver made the old device-list row control equivalent: removing
`of_a_module_signal` from the fallback in `infer.rs` left the focused row test
green. Its baseline and mutant each executed one passing test. The replacement
removes only Device List declarations from `Signatures::signal_type`, the shared
resolved identity authority read both by inference and the generic value typer.
It leaves non-device lists and scalar device signals alone. On the integrated
source it produces seven real assertion failures out of fifteen compiler tests,
including `generic_reverse_keeps_a_device_rows_type_and_privacy`.

That regression renders `List.reverse(notes)` rows and accesses their nominal
record fields, then proves a row value is refused at a server command by PW6801.
The same command over a page-local list checks clean. This joins the generic
solver's row type and the Device privacy boundary in one program; neither an
unknown-name error nor a type error satisfies the negative case. The pre-existing
Device-label mutation also exercises this integrated privacy assertion.

The integration exposed a separate evidence defect: this campaign counted failed
builds and command timeouts as kills. These are now refused producers. Cargo's
native exit and libtest summaries use the existing `cargo_test_results` parser.
The caller additionally freezes the complete executed test inventory, refuses
ignored/measured tests and authenticates every failed test's panic site as an
assert macro in the selected test source. Production panics, setup/unwrap/expect
panics, compilation errors, stopped processes and incomplete results earn no
kill. A test helper's asserted build result remains a genuine test assertion;
its previous `expect` is now an explicit `assert!`. This source attribution is
for the owned Rust tests, not a sandbox for hostile output.

For browser tests, line output alone cannot establish whether a failure happened
in a body assertion, hook, setup or timeout. The integrator approved a structured
producer. `assertion-result-reporter.mjs` uses the pinned Playwright 1.58.0 public
Reporter, TestResult and TestStep interfaces, verified against the installed
`playwright/types/testReporter.d.ts` and the tagged primary source:

- [Reporter types at v1.58.0](https://github.com/microsoft/playwright/blob/v1.58.0/packages/playwright/types/testReporter.d.ts)
- [JSON reporter at v1.58.0](https://github.com/microsoft/playwright/blob/v1.58.0/packages/playwright/src/reporters/json.ts)

The built-in JSON reporter omits expect/hook step detail, so it cannot prove this
contract. The small reporter follows `host-plan-reporter.mjs`'s existing lifecycle:
fail closed on invalid callbacks, publish atomically only at `onExit`, refuse an
existing receipt. Each parent command provides a fresh random nonce and fresh
path. `browser_test_results` validates that receipt, exact native exit agreement
and the frozen baseline inventory. The reporter requires one begin/end for each
selected test, no retries, skipped/timedOut/interrupted results, expected failures
or global errors. Every failed result error must match a failed expect step in
the test body, with no hook ancestor. The normal line reporter and canonical
`run_suite.sh` still retain ordinary red assertion output and traces.

`device_browser_protocol.py` executes nine real pinned Playwright cases without
launching a browser: green and body assertion are valid; beforeEach/afterEach
assertions, runtime errors, test and setup timeouts, skipping and retry recovery are
invalid. The focused Python suite checks the transport, reporter lifecycle,
Cargo producer, campaign verdict and final restored build. Cancellation kills
and reaps the owned process group before restoring source; a real child-process
regression holds this order. Its twenty named
mutation controls use the existing canonical `guide_python` assertion receipts.
`just e14-device-producer` records these checks; whole device/TodoMVC campaigns
remain required on the combined remote train. No full-campaign result is claimed
by the local probes in `docs/evidence/tooling/device-integration-probes.txt`.

## Integration amendment: qualified signal reads (2026-10-20)

A device signal keeps the type and privacy of its resolved declaration whether
read through a selective import (`word`) or a module path (`prefs.word`). The
integration review found that the latter checked without PW6801 at a command
boundary: the value typer considered qualified declarations only as callables,
and the label reader never asked the signal's label for a field-shaped path.
The build already refused its handler because `prefs` was not a bound receiver;
this repair restores the semantic diagnostic, not an executable data-leak fix.

Use the existing Term namespace resolver and canonical DefId. The existing
lexical path predicate remains authoritative: a local record called `prefs`
reads its own `word` field, and is never relabelled as a module's device signal.
The diagnostic identifies the declaration which supplied the Device label.

This amendment does not add qualified signal lowering. A permitted browser-only
read through a module path may still check and then receive the existing build
refusal; selective imports remain the supported backend form. Regression tests
must execute the real build and retain that refusal until backend support is
separately designed, tested and approved.

The integration review also found that a nested declaration's origin reader
used only its immediate binders, unlike the type and label readers. The same
lexical environment must include captured outer binders: in an argument made
from a public record `prefs.word` and imported Device `secret`, the diagnostic
names `secret` even when `prefs` also names an imported module. The nested
function in this witness remains independently refused by PW0339 and PW5301;
this corrects the diagnostic's origin, not which programs can execute.


### An assertion operand is not an assertion failure (2026-10-20)

Independent integration review found a remaining attribution gap: an `unwrap`
panic inside an assertion operand shares the assertion's source line. Rust
1.97.1 reports its operand column (27 or 37 in the native probes), while an
ordinary failed `assert_eq!` and a failed `assert!` with a custom message both
point to the assertion macro's first column (5). The device producer now checks
that exact source column as well as the selected test path, line and macro.
It continues to accept custom assertion messages; message text is not authority.

[`device-assertion-columns.txt`](../evidence/tooling/device-assertion-columns.txt)
records the native four-case fixture, commands and actual exit-101 output. The
old producer accepts all four as assertion failures; the new one accepts only
the two real assertions and refuses panics in the compared operand and message
operand. A seventeenth regression and twentieth producer control hold this rule.
Whole campaign validation remains part of the combined remote train.

### Native browser matcher proof (2026-10-20)

A failed Playwright `expect` step is not sufficient assertion evidence. Native
Chromium probes showed that a closed page and `expect(42).toHaveText(...)` each
failed inside that category and earned a receipt. Both are producer/API errors,
not observations contradicting an application's expected behavior.
The [native probes and local control logs](../evidence/tooling/browser-assertion-probes.txt)
retain the before/after outcomes and their actual producer output.

The two device/TodoMVC specs now import a small public `expect` wrapper. It
preserves native comparison, `.not`, custom messages and locator retries. Only
a thrown error with the native `matcherResult`, the invoked matcher name and
the corresponding boolean pass result receives a fresh nonce-bound Error cause.
Any existing cause stays beneath that marker. Playwright's
[public custom matcher example](https://playwright.dev/docs/test-assertions#add-custom-matchers)
reads the native `matcherResult`; its public
[`TestError.cause`](https://playwright.dev/docs/api/class-testerror#test-error-cause)
is the serialized transport. The pinned 1.58 probe confirms that it does not
serialize `matcherResult`. The failed step is
already serialized before an outer wrapper catches the error, so the reporter
requires the marker on every final error and retains exact message/stack pairing
with body expect steps. Hooks, runtime errors, timeouts, skips, retries and
incomplete inventories remain invalid. Unsupported assertion modes fail closed.

The sole existing polling witness uses positive `expect.poll(callback).toBe`.
Native poll drops `matcherResult` at its deadline and leaves caught failing
child expect steps. A scoped public `expect.extend` matcher therefore polls
with the same `Object.is` comparison and returns a native matcher result once.
Its supported contract is a callback, expected value, optional string message,
finite positive timeout (default 5000ms) and nonempty finite positive intervals
(default 100/250/500/1000ms, repeating the last). It does not expose `.not`, soft,
promise or other polling matchers. A completed mismatch can fail the assertion;
a callback exception, a callback still pending at its deadline, or a callback
that completes after that deadline cannot. No application behavior changes.

The native protocol covers 26 cases, including actual Chromium mismatches and
the two false-kill reproductions, custom messages, negation, preserved causes,
polling, NaN and signed zero. Its default 23 cases and ordinary unit checks
need no browser binary. Three browser tests require `PW_BROWSER_PROOF=1` in the
explicit `e14-browser-assertions` recipe; its actual
`scripts/playwright_assertion_mutations.py` command selects browser setup.
That recipe freezes all ten regression methods and proves eight additional
controls through their named native body assertions. The existing twenty device
producer controls remain intact. These local checks do not replace final
combined hosted verification or the full changed-page browser suites.

The control's outer unittest producer uses the shared process owner's bounded
cooperative cleanup; the direct Playwright child owner uses immediate cleanup.
The shared inherited budget reserves time for each enclosing owner (ADR-0376).
Native timeout and SIGTERM probes of this exact nested topology captured the
runner, worker and detached Chromium processes at a live-page barrier. None of
the eight captured processes was running before source restoration in either
case; the probe reaped adopted zombie children before exiting. This is evidence
for the configured producer, not arbitrary detached subprocesses. Each
JavaScript mutant also passes native `node --check` before its selected witness;
invalid syntax is refused and cannot earn an assertion kill.

### Train-12: mutation witnesses assert the device contract before inspecting it

The first strict hosted campaign at `f15117de` (verify `39526960864`) ran
its unmutated core/browser/TodoMVC baselines, then reported 53 of 62 kills,
nine refused producers and no survivors. The nine refusals stay failed
historical evidence. A Rust match made non-exhaustive, an `expect`/panic
before a comparison, a browser parser error, a matcher receiving the wrong
input type, and an unfinished wait are not mutation kills.

The Map/Set operator now returns no storage-form refusal from its existing
exhaustive arm. It preserves the intended omitted rejection while compiling.
The positive core and host witnesses explicitly assert an executable page
plan, a successful build of the valid TodoMVC fixture, and the named migration
module before extracting them. Filesystem, process and other setup errors
remain ordinary errors; this is not a blanket conversion of setup failures.

Browser witnesses assert that required storage entries are strings and valid
JSON before comparing decoded values. Missing entries and malformed persisted
data now fail their actual storage contract instead of throwing from JSON.parse
or misusing a string matcher. Existing absence checks use the equivalent
`toBe(undefined)` native comparator, which carries authenticated matcher
metadata in the pinned tool. All expected persisted values remain unchanged.
The test that holds back rendering asserts completion of the device read with
a bounded poll, and releases its own route in `finally` even on failure.
A separate shared polling fix owns the final-interval deadline boundary; this
change does not relax callback timeout or assertion provenance requirements.

All 62 original fault models remain. Complete native inventory, native exit
agreement, assertion provenance, mixed-error refusal, bound-stop refusal and
source restoration are unchanged. Local targeted checks cover the repaired
witnesses; the complete campaign and all three browser engines still require
the combined hosted verification before acceptance.

Local native checks after these repairs retain green baselines of 15 core,
5 host and 15 Chromium tests. Each of the nine previously refused operators
now produces a complete authenticated assertion failure against its frozen
inventory: both core operators, both host operators, and the five browser
operators. The browser failing counts are respectively 8 (read at start),
7 (write), 2 (other tab), 1 (old key), and 1 (unreadable backup), with all 15
tests completed in every run. The two polling cases use the separately
reviewed final-interval fix. The 17 strict device-producer regressions remain
green. Mutated sources and generated demo assets were restored; these targeted
runs do not replace the full hosted 62-operator campaign.
### Train-12: a final polling interval cannot start a new callback

A sleep clamped to the overall polling deadline can wake slightly early.
The previous loop then started another callback with less than a millisecond
left, making ordinary completed mismatches end in an invalid callback timeout.
The loop now marks its final interval before sleeping and ends afterward.
An actually started callback that throws, remains pending or finishes late is
still refused; no timeout becomes assertion evidence.

A deterministic native timer witness fails on the former loop and passes on
the repaired one. The complete current protocol passes 27 cases, and the
11-method native regression inventory kills all nine instrument controls.
The [raw before/after proof](../evidence/tooling/browser-poll-final-interval.txt)
retains commands, source hashes and native output. These local checks do not
replace the complete hosted browser assertion and device campaigns.
