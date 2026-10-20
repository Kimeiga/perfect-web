# ADR-XXXX: device-local storage: a module's signal the device keeps, read before it is shown, never sent

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
- **The released versions are held to** (the tables' `schema.lock`,
  ADR-0350, applied here): `pw build` writes `device.lock`, each value's
  name, version and the canonical form of its type; a build given the
  released lock refuses a value whose type changed at the same version
  (PW6806): a changed type with no new version and migration is a person's
  data unread after the next deploy.

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
  including the block's `{#each}` list: computed before the block renders, and
  read by its path, as a computed hole is (ADR-0226).

Rows stay read by path: a list's function makes the rows whole (`Row { id,
title, completed, class, editing }`), so no value is computed inside a row.
A block is rendered again whole, as today.

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
| PW6806 | a device signal's versions: a pure migration for each earlier version, chained by type; the released lock's type at its version |
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
  with the host's first render and the browser's agreeing.
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
  rendered again row by row: decision 7's later steps.
- **Every channel off the device.** The browser's own history and sync (a
  page's title, its address), extensions, a person's copy and paste, timing,
  and a foreign component mounted with a device value (ADR-0075's mount, not
  built; ADR-0339's PW6707 is its rule) are outside what a compiler sees.
- **Safari's seven days and storage pressure**: said, not prevented.
- **The web's code delivery** (NEXT's E2EE item): the server that serves the
  code that reads the store could serve other code.

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
