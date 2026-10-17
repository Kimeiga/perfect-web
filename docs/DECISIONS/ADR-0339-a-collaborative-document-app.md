# ADR-0339: a collaborative document is merged by Automerge, edited in a mounted ProseMirror, and told on the stream

Status: accepted under the owner's delegation of 2026-10-02, by the
integrator's ruling of 2026-10-15 (below), its downloads still the owner's
to approve; a design note by track `docs-design` for NEXT's Google
Docs-shaped app (the first of the reference apps the capability matrix
chooses, in the owner's order relayed 2026-10-08), merged on train-8. Docs
only: nothing here is built, and nothing was downloaded. Every version,
licence and size below was read from the registries' metadata, and the
dependency trees were resolved from index metadata alone (`npm install
--package-lock-only --ignore-scripts`, `cargo generate-lockfile` with a
scratch `CARGO_HOME`; no tarball or crate fetched). Date: 2026-10-14, at
master 5da9c05. Builds on ADR-0075 (a mounted resource does not build),
ADR-0207 (a data source states what it guarantees), ADR-0258 and ADR-0270
(the relying party; the typed principal), ADR-0279 (a conversation read
from each side), ADR-0297 (a commit tells the pages that ask), ADR-0302 (a
predicate's words) and ADR-0333 (what a page may run).

## Ruling (the integrator, 2026-10-15)

Accepted under the owner's delegation of 2026-10-02, with the amendments
below; the downloads stay the owner's to approve, and nothing is fetched
before that approval.

1. **The three rulings stand**: Automerge behind `replicated`'s
   `rich_text` policy (reopened if D0's cases or budgets say so, before D1
   lands); Pleris renders every reader's document and an editor's page
   mounts ProseMirror at the page's authority, audited and said to be
   unconfined; presence and sync down the stream, one sync request up, a
   WebSocket only if D6's measurement fails (charter §11.3).
2. **No app-specific Rust (amends "How it fits" and question 4).** The
   integrator's ruling on the three-database note (ADR-0338, 2026-10-15) holds here
   too: the app's own data (documents' names and owners, roles, shared
   links, comments) is declared as tables and served by the generic
   adapters (that note's D1 to D3), not a hand-written layer. The
   replicated document's log, its snapshots and its sync are the
   platform's, generic over every program's `replicated` values, as the
   tables' primitives are. `examples/docs/` is the fourth app, with no
   `migrations/docs/` of its own: its tables' migrations come from their
   declarations. Phase 3 waits for D3 (the PostgreSQL adapter); phases 0
   to 2 do not.
3. **A predicate on a read** (question 2): `requires` on a query, as on a
   command (ADR-0258), with its words, so who may read a document is in
   the program where the checker sees it; an operation answering
   `NotShared` would hide it below the seam, which the three-database note
   refuses for the same reason.
4. **The frames** (question 3): added at protocol version 1, sent only to
   pages whose build holds the replica runtime; a test holds that a page
   without it never receives `ReplicaSync` or `Presence`.
5. **`vendor/`** (question 5): accepted; ADR-0333's tests are extended to
   it (same origin, each file's hash in the manifest), never loosened.
6. **A document read from each side** (question 6): confirmed; a derive per
   reader is measured by D6, not assumed affordable.
7. **Codes and ports** (question 1): PW6601 upward for this app's tracks
   (free at 2026-10-15 on master and every pushed branch); each track's
   Playwright base is given when it starts.
8. **The owner decisions that are design** (3 to 9), under the delegation:
   the editor at the page's authority, as stated, with a sandboxed frame
   only on a measured need (3); history kept on editors' devices, erasure
   by copy, said in the app (4); a shared link grants viewer, commenter or
   editor, as its document's owner chooses (viewer by default), and lasts
   until rotated, with an expiry its owner may set (5); offline edits
   promised within the browser's eviction rules, said on the page (6);
   presence as proposed, link guests anonymous (7); no suggestion mode
   (8); no WebSocket before D6's measurement (9). Decision 1 (the CRDT) is
   ruled 1 above. Decision 2 (the downloads) is the owner's alone.

## Context

### What the owner asked, and what it shares with the next app

- **NEXT, the first app the capability matrix chooses**: "rich-text
  documents; several editors at once, with live cursors and presence;
  comments on ranges; sharing and permissions (owner, editor, commenter,
  viewer, and link sharing), which the privacy checker holds; version
  history and restore; and offline edits merged on reconnect." Three
  rulings, "each researched when it is reached": how concurrent edits
  merge, the editor surface, and how presence and cursors ride the live
  transport "at a frequency far past anything it carries now". Any npm or
  crates.io download "waits for the owner's approval".
- **The end-to-end-encrypted messages after it** need "device-local
  storage under an explicit policy ... (charter §9.1's kind 4, shared with
  the Docs app)". So the device store this app builds is that app's too.

### What Pleris has, read at 5da9c05

- **Kind 4 is parsed, not modelled.** Charter §9.1: "Replicated/offline
  state: intentionally local and synchronized data with explicit conflict
  policy." A-011's `replicated OrderDraft` states `privacy`, `storage
  device`, `offline writable`, `sync on_reconnect`, `conflict
  merge_by_field(..)` and `on_conflict_unresolved surface_to_user`;
  `tests/one_parser.rs` lists it as "E11's replicated state; parsed, not
  modelled"; `policy.rs` places those clauses on "a replicated value" and
  gives them no meaning. The capability matrix: "Real-time collaboration
  and offline | `replicated` is parsed only (A-011)". Charter §9.3 derives
  "offline-writable" as "local replica plus explicit conflict policy".
- **A mounted resource is refused** (ADR-0075): A-007's `<map-container
  resource={StoreMap} center={center} />` "is not compiled". A-024's
  audited escape (`unsafe capability synchronous_geometry because "…"
  attributes_forced_layout_to VendorMap`, an `unsafe.imperative` acquire
  and release) is checked, R-044 refused for an incomplete record
  (`UNSAFE_AUDIT_INCOMPLETE`), and builds nothing.
- **The live transport is one way, host to page** (ADR-0035, ADR-0139,
  ADR-0155, ADR-0161, ADR-0175, ADR-0297):
  - each document a page shows is a subscriber with a queue of frames; a
    frame is forgotten only when the page's next request acknowledges it
    (ADR-0139); at most 256 wait (`MAX_WAITING`), a subscriber silent for
    120 s is forgotten (`IDLE`), one that has not asked for 3 s is passed
    by (`LIVE`), and a commit tells 8 sessions at once (`TELLERS`);
  - `/stream` writes newline-delimited JSON, looking at its queue every
    25 ms, held 80 passes (2 s) and opened again at once; the long poll
    holds 1 s. The host is a thread per connection: "a held connection is
    a held thread" (`stream_open`'s comment);
  - up, a page changes state by commands alone: a POST, a transaction,
    its interaction's record and its outbox (ADR-0121), each commit told
    to the pages that read what it dropped by deriving their documents
    again;
  - the frames are `ResourceChanged`, `Patch`, `PatchSet`, `Recovery` and
    `EntryValue`, and `pw-protocol` holds that a frame "must not name a
    transport".
- **The page's policy** (ADR-0333): `connect-src 'self'`; `script-src
  'self' 'wasm-unsafe-eval' '<the bootstrap's hash>'`; the runtime makes
  "no WebSocket, no `EventSource`, no `eval`". `serve_file` serves flat
  names from `dist`, nothing below it.
- **Privacy**: a label is a set of restrictions (`privacy.rs`), `Device`
  among them ("Belongs to one device and cannot be moved off it"). ADR-0279
  ruled that "No label names two principals": each participant reads a
  conversation from their own side, "the same rows read twice, each read
  private to its reader". `current_user()` is the host's (ADR-0270), a
  program makes no handle (ADR-0263, PW5037 to PW5039), and a `requires`
  predicate is the host's to evaluate, refused 403 with its words
  (ADR-0258, ADR-0302).
- **One host** (ADR-0246): a second host on one database hears nothing of
  the first's commits. **`deny.toml`** admits MIT, Apache-2.0 (and with
  the LLVM exception), BSD-2-Clause, BSD-3-Clause, Zlib, Unicode-3.0 and
  Unlicense: "Copyleft in particular should need an ADR, not a silent
  pass."

### How others merge concurrent edits (each read 2026-10-14)

- **Google Docs: operational transformation around a server.** John
  Day-Richter's three posts on the Google Drive Blog (2010-09-21 to -23):
  "We save your document as a revision log consisting of a list of these
  changes. ... To display a document, we replay the revision log from the
  beginning"; "The algorithm that we use to handle these shifts is called
  operational transformation (OT)"; a client "never send[s] more than one
  pending change at a time"; and "Only the server needs to be aware of the
  document's history and only the clients need to be aware of uncommitted
  changes."
- **The protocol descends from Jupiter.** The Wave whitepaper (Wang, Mah,
  Lassen, "Google Wave Operational Transformation", v1.1, July 2010): "The
  starting point for Wave OT was the paper 'High-latency, low-bandwidth
  windowing in the Jupiter collaboration system'" (Nichols, Curtis, Dixon,
  Lamping, UIST '95, pp. 111-120, as the whitepaper cites it), and Wave
  "requir[es] the client to wait for acknowledgement from the server
  before sending more operations".
- **ProseMirror: a central authority that orders steps.** Marijn
  Haverbeke, "Collaborative Editing in ProseMirror" (2015-07-28):
  "ProseMirror's algorithm is centralized, in that it has a single node
  (that all users are connected to) making decisions about the order in
  which changes are applied", and "Unlike OT, it does not try to guarantee
  that applying changes in a different order will produce the same
  document." The guide: the authority must "Track a current document
  version" and "Accept changes from editors, and when these can be
  applied, add them to its list of changes".
- **Figma: server-authoritative and CRDT-inspired, with no text merge.**
  Evan Wallace, "How Figma's multiplayer technology works" (2019-10-16):
  "we didn't want to use operational transforms (a.k.a. OTs), the standard
  multiplayer algorithm popularized by apps like Google Docs"; "Figma
  clients are web pages that talk with a cluster of servers over
  WebSockets. Our servers currently spin up a separate process for each
  multiplayer document"; offline, "the client downloads a fresh copy of
  the document, reapplies any offline edits on top of this latest state";
  "Figma isn't using true CRDTs though"; "simultaneous editing of the same
  text value doesn't work in Figma"; and comments are "stored in Postgres,
  not our multiplayer system".
- **Peritext: rich text as a CRDT.** Litt, Lim, Kleppmann and van
  Hardenberg (Ink & Switch, November 2021; CSCW 2022, PACMHCI 6 CSCW2
  Article 531, doi:10.1145/3555644):
  - OT "assumes that a document has a single, linear timeline of edits,
    which is managed by a cloud service";
  - control characters for formatting are "the approach used by the CRDT
    library Yjs", and "Yjs has the most full-featured rich-text CRDT
    available today ... However, it suffers from the anomaly shown in this
    section" (two overlapping bolds merging to a non-bold word);
  - an inserted character "inherits the bold/non-bold status of the
    preceding character", while "a link or comment span does not grow in
    the same way";
  - block elements are left open: "what should happen when users
    concurrently split, join, and move block elements?"
- **Interleaving, and what offline costs.** Weidner and Kleppmann, "The
  Art of the Fugue" (arXiv 2305.00583, v3 2025-10-21): concurrent
  insertions at one position "may interleave the inserted text passages
  ... it affects both CRDTs and Operational Transformation." Gentle and
  Kleppmann, "Collaborative Text Editing with Eg-walker" (EuroSys 2025):
  "OT algorithms are slow to merge files that have diverged substantially
  due to offline editing; CRDTs are slow to load and consume a lot of
  memory."
- **Local-first** (Kleppmann, Wiggins, van Hardenberg, McGranaghan, April
  2019; Onward! 2019, pp. 154-178): "we treat the copy of the data on your
  local device ... as the primary copy", and servers "not as central
  authorities, but as 'cloud peers'". Pleris takes its mechanism (a replica
  on the device), not its ownership model: here the host stays the
  authority on who may read and write.

### The libraries, read in their registries and repositories (2026-10-14)

- **Automerge.** `@automerge/automerge` 3.5.0 (2026-09-16, MIT; its Wasm
  3,644,225 bytes before compression, the package's 47,529,812 bytes
  being several builds of it) and the `automerge` crate 0.12.0 (the same
  day, MIT, rust-version 1.90.0): one Rust core, compiled for the browser.
  - Rich text: "Rather than representing rich text as a tree structure
    like HTML, we represent it as plain text annotated with spans and
    blocks" (Automerge 2.2, April 2024); marks carry an `expand` option;
    block markers carry a `type` and `parents`, and "text may appear in
    many spans, but should only ever be in a single block". Its schema is
    "(extremely minimal)" and "a work in progress": paragraphs, headings,
    code blocks, quotes, list items and images; no tables.
  - The same post shows "Conflicting structure changes in yjs can cause
    loss of text" (a list turned into paragraphs while an item is added):
    "very rare in online editing scenarios", but "much more likely during
    longer sessions of offline collaboration".
  - History: "Automerge stores the full history of documents"; "Automerge
    3.0 uses the same file format as Automerge 2" (July 2025, format since
    January 2023). It cannot be trimmed: "Discarding the history is not
    currently possible without replacing the document: if you don't have
    the same history, you can't merge the same changes" (issue #799,
    2023-12-24; shallow clone "on the list" 2024-09-23; converted to a
    discussion 2026-08-26). Issue #1244 (open since 2026-01-09): "Loading
    large doc blocks main thread for 10+ seconds".
  - API (docs.rs, 0.12.0): `spans`, `marks`, `get_cursor(obj, position,
    at: Option<&[ChangeHash]>)` and `get_cursor_position`, `spans_at`
    heads; the sync protocol "assumes a reliable in-order stream between
    two peers".
  - Binding: `@automerge/prosemirror` 0.2.0 (2026-02-25, MIT): "This plugin
    is beta quality software. The API will probably change a bit before a
    stable release and there are bugs, but it also works reasonably well."
  - Maintained: monthly written updates (July and August 2026), 3.4.1 and
    0.11.0 in August, 3.5.0 and 0.12.0 in September; a Rust repository
    (samod) "wire-compatible with the JavaScript one"; Keyhive, "our access
    control and encryption layer", in alpha.
- **Yjs, and yrs for the host.** `yjs` 13.6.33 (2026-09-23, MIT, 300,059
  bytes of unminified ESM) with `lib0` 0.2.119; `yrs` 0.28.0 (2026-09-17,
  MIT), whose `sync` module carries the sync protocol and `Awareness`.
  - Two implementations of one format: y-crdt "aims to maintain behavior
    and binary protocol compatibility with Yjs"; its README's table names
    yjs 13.6 against yrs 0.21.
  - Rich text: formatting attributes in the sequence (Peritext's anomaly);
    y-prosemirror 1.3.7 (2025-07-03): "This binding maps a Y.XmlFragment
    to the ProseMirror state" (Automerge's text-loss case).
  - History: "Set `doc.gc = false` in order to disable gc and be able to
    restore old content"; yrs: "in order to support revisions we cannot
    garbage collect deleted blocks", and a revision's main use is
    "rendering read-only state of the Doc or restoring document from its
    past state".
  - Yjs 14 is at release candidate (`@y/y` 14.0.0-rc.28, 2026-09-29, rc.0
    on 2026-02-25), with attribution "to implement a Google Docs-like
    versioning feature" whose API still moves ("rename concept
    AttributionManager => Renderer ... definitely a breaking change to all
    users of the current attribution api", a commit of 2026-06-16), and
    `@y/prosemirror` 2.0.0-15 (beta): "Suggestion mode and version diffs,
    rendered as attribution marks".
  - Its threat model (THREAT_MODEL.md, 2026-04-14): "It is therefore
    critical that untrusted peers are never allowed to write updates to
    the original Yjs document"; "Server-side filtering of updates is a
    common antipattern ... not a reliable security boundary"; and out of
    scope, "sanitizing rich text content for XSS".
  - Presence: "If a client doesn't receive updates from a remote peer for
    30 seconds, it marks the remote client as offline"
    (`outdatedTimeout = 30000`, renewed at half of it).
- **Loro.** `loro-crdt` 1.16.4 (2026-09-30, MIT; its Wasm 3,398,779 bytes)
  and the `loro` crate 1.16.2 (2026-09-21, MIT). Its site answers a bot
  check, so its documentation was read from its source, loro-dev/loro-docs.
  - "Loro 1.0 has stabilized the data format and will not have any
    breaking changes" (1.0, 2024-10-23); rich text "complying with
    Peritext's criteria" on Fugue (2024-01-22); time travel; shallow
    snapshots that drop history ("Sensitive data permanently removed from
    history"), after which "Peers can only sync if they have versions
    after the shallow snapshot point".
  - Binding: `loro-prosemirror` 0.4.4 (2026-08-22) maps each node to a map
    with `nodeName`, `attributes` and `children`: a tree, as Yjs's.
  - The crate's graph: 158 crates, 83 of them new to this workspace's
    `Cargo.lock`, among them `im`, `sized-chunks` and `bitmaps` (MPL-2.0+)
    and `xxhash-rust` (BSL-1.0), which `deny.toml` does not admit; and
    `loom` and `pretty_assertions` as normal dependencies of
    `loro-internal`.
- **diamond-types** (Eg-walker's implementation): plain text; the crate's
  1.0.0 is from 2022-08-25, ISC, which `deny.toml` does not admit.
- **OT libraries**: ShareDB 6.0.5 (2026-10-06, MIT) with its `rich-text`
  type 4.1.0, last published 2020-04-03; `prosemirror-collab` 1.3.1
  (2023-05-17), whose authority applies ProseMirror's steps in JavaScript.

### Every library considered

Sizes are the registry's: npm's unpacked size, crates.io's crate size.
Maintenance is read from the registries' release dates and, where named,
the repository's open issues (GitHub, 2026-10-14).

| Library | Version, published | Licence | Size | Maintenance | Here |
| --- | --- | --- | --- | --- | --- |
| `@automerge/automerge` | 3.5.0, 2026-09-16 | MIT | 47,529,812 B; Wasm 3,644,225 B | 138 versions since 2022-10-04; monthly in 2026; 97 open issues | chosen, the page |
| `automerge` (crate) | 0.12.0, 2026-09-16 | MIT | 409,627 B; 13 new crates | 37 versions since 2019-12-28 | chosen, the host |
| `@automerge/prosemirror` | 0.2.0, 2026-02-25 | MIT | 201,178 B | "beta quality"; 18 versions since 2024-03-15; 24 open issues | chosen, the binding |
| ProseMirror (`prosemirror-view` and the rest) | 1.42.6, 2026-09-25 | MIT | 905,889 B (the view) | 310 versions of the view since 2016-09-21 | chosen, the editor |
| `yjs` | 13.6.33, 2026-09-23; 14.0.0-rc.28 | MIT | 2,308,376 B; ESM 300,059 B | 314 versions since 2015-01-26; 144 open issues | lost; to revisit at Yjs 14 |
| `yrs` (crate) | 0.28.0, 2026-09-17 | MIT | 285,969 B; 8 new crates | 82 versions since 2020-10-20; 150 open issues (y-crdt) | lost |
| `y-prosemirror` | 1.3.7, 2025-07-03; 2.0.0-15 beta | MIT | 293,732 B | 84 versions since 2019-04-26 | lost |
| `loro-crdt` | 1.16.4, 2026-09-30 | MIT | 20,760,604 B; Wasm 3,398,779 B | 176 versions since 2023-04-07; 73 open issues | lost; the runner-up |
| `loro` (crate) | 1.16.2, 2026-09-21 | MIT; its graph MPL-2.0+ and BSL-1.0 | 243,656 B (`loro-internal` 1,452,054 B); 83 new crates | 46 versions since 2022-08-07 | lost |
| `loro-prosemirror` | 0.4.4, 2026-08-22 | MIT | 298,416 B | 21 versions since 2024-06-06 | lost |
| `diamond-types` | crate 1.0.0, 2022-08-25; npm 1.0.2, 2023-05-15 | ISC (the crate); ISC OR Apache-2.0 (npm) | 134,311 B; 383,968 B | nothing published since 2023 | lost |
| `sharedb` with `rich-text` | 6.0.5, 2026-10-06; 4.1.0, 2020-04-03 | MIT | 713,820 B; 16,319 B | ShareDB released this month; its rich-text type not since 2020 | lost |
| `prosemirror-collab` | 1.3.1, 2023-05-17 | MIT | 32,468 B | 24 versions since 2016-09-21 | lost |
| `@tiptap/core` | 3.32.0, 2026-10-13 | MIT | 3,300,912 B | 513 versions since 2020-11-16 | lost |
| `lexical` | 0.52.0, 2026-09-28 | MIT | 3,225,842 B | before 1.0; nightly builds | lost |
| `slate` with `@slate-yjs/core` | 0.138.2, 2026-10-14; 1.0.2, 2023-07-11 | MIT | 2,399,420 B; 1,756,880 B | before 1.0; its Yjs binding not since 2023 | lost |
| `quill` | 2.0.3, 2024-11-30 | BSD-3-Clause | 3,036,362 B | no release after 2.0.3 | lost |
| `@blocknote/core` | 0.56.0, 2026-10-14 | MPL-2.0 | 11,009,966 B | released often | lost |
| `tungstenite` (crate) | 0.30.0, 2026-07-11 | MIT OR Apache-2.0 | 71,531 B; 5 new crates | 50 versions since 2017-03-17 | only for D9 |

### The editor surface (each read 2026-10-14)

- **Google Docs left HTML** (Google Workspace Updates, 2021-05-11):
  "migrating the underlying technical implementation of Docs from the
  current HTML-based rendering approach to a canvas-based approach".
  Charter §1, item 10: "Semantic HTML and CSS, not an opaque canvas".
- **ProseMirror** renders the document as HTML in a `contenteditable`
  (prosemirror-view 1.42.6, 2026-09-25, 310 versions since 2016-09-21,
  MIT), and maps positions and coordinates through
  `getBoundingClientRect`, `getClientRects`, `elementFromPoint` and
  `caretRangeFromPoint` (`src/domcoords.ts`): the synchronous geometry
  charter §7.5A forbids ordinary code and A-024's escape exists for.
- **EditContext**, the way past `contenteditable`, is marked experimental
  and shipped in Chrome 121 alone (MDN's compatibility data).
- **The charter's non-goals**: "Proving every imperative widget safe ...
  Quarantine them behind a measurable imperative-resource boundary
  instead."

### The transport, the device and a shared link (each read 2026-10-14)

- **WebSocket** is in every engine; **WebTransport** in Chrome 97, Firefox
  114 and Safari 26.4 (MDN's compatibility data), over HTTP/3, which
  charter §11.3 places later. CSP Level 3: "'self' now matches https: and
  wss: variants of the page's origin". Charter §11.3: "Prove the
  application semantics over ordinary HTTP first." Charter §9.5: "at most
  one in-flight request unless policy says otherwise".
- **The device.** WebKit (2020-03-24): "deleting all of a website's
  script-writable storage after seven days of Safari use without user
  interaction on the site" (IndexedDB among them; a Home Screen web app
  keeps its own count). WebKit (2023-08-10): "By default, all origins use
  a best-effort mode", and a persistence request is granted "based on
  heuristics like whether the website is opened as a Home Screen Web
  App". The Storage Standard: "persistent buckets cannot be cleared
  without consent by the user." Charter §7.10 lists "browser storage"
  among the sources whose values start as `Unknown`.
- **A shared link is a capability URL.** The W3C TAG's "Good Practices for
  Capability URLs" (First Public Working Draft, 2014-02-18): URLs "appear
  within application logs ... and in browser history"; a site linked from
  the page "may be notified of the capability URL through the Referer HTTP
  header"; "Capability URLs should expire"; revoking one "has an impact
  on all those who had it".

## Decision

### 1. Concurrent edits merge in Automerge, behind `replicated`'s `rich_text` policy

- **The decision.** A collaborative document is a replicated value,
  charter §9.1's kind 4, whose conflict policy is `rich_text`: Automerge's
  rich text, by the `automerge` crate in the host and the same core as
  Wasm in an editor's page. Reused, never written (charter §2: "a
  universal CRDT system"), and behind an adapter: no program sees an
  Automerge type (charter §4, "no leakage of the temporary dependency's
  semantics into application source"; §3.6, "an adapter boundary around
  experimental dependencies"). Kind 4 offers a closed set of replicated
  types the platform provides, rich text first and A-011's records next;
  never a general replicated JSON.
- **Why Automerge**:
  1. **One implementation on both sides.** The host merges with the code
     the browser merges with. With Yjs, the host's merge is yrs's and the
     page's Yjs's, kept equal by a project's aim.
  2. **Peritext's rules for marks, block markers for structure.** Each
     mark states whether it grows at its boundary, as Peritext's examples
     8 and 9 ask; a block is a marker in the text, so a list turned into
     paragraphs and an item added beside it keep both. That is the case
     Automerge shows Yjs's binding losing, "much more likely during longer
     sessions of offline collaboration", and the owner's last requirement
     is exactly such a session.
  3. **History is the CRDT's.** A past version's render (`spans_at`), a
     diff by author (each change's actor), a comment's anchor at the
     version its page showed (`get_cursor(.., at)`) and a restore come
     from the document itself, with no second history store.
  4. **A stable format.** "Automerge 3.0 uses the same file format as
     Automerge 2" (July 2025; 2.0 was released in January 2023). That 3.5.0
     reads what 3.0 wrote is D0's to check, not this note's to assume.
  5. **Its host crate's licences are all ones `deny.toml` admits**: 13 new
     crate names, each MIT, MIT OR Apache-2.0, or Zlib (Downloads, below).
     Its advisories are `just audit`'s to check when it is added.
  6. **Maintained**: monthly releases and written updates through 2026,
     and rich-text algorithms that "were developed in concert with Martin
     Kleppmann" (the 2.2 post), one of Peritext's authors.
- **What it costs, said plainly**:
  - the ProseMirror binding is beta by its own word: D0 and D4 test it,
    and it is vendored at a pinned version;
  - history cannot be discarded: every editor's replica holds every
    deleted word, and erasing history means a new document (a copy).
    Viewers and commenters never receive the replica (ruling 2);
  - 3.6 MB of Wasm before compression, for editors alone, loaded after
    the page's first paint (ruling 2);
  - the crate is 0.x and requires `hexane` 1.0.0-alpha.5 (the newest
    stable `hexane` is 0.2.1): pinned with `=`, upgraded only by a track
    that runs D0's cases again;
  - loading a long history is slow by Automerge's own open issue: D0
    measures it;
  - the schema has no tables.
- **The switch condition.** D0 runs Peritext's examples, Automerge's
  structure case, the interleaving case and a long offline divergence
  against Automerge, stating each expected result before the run. Where
  Automerge loses an intent the case states and a candidate keeps it, or
  its load time or memory fails D0's budget, this ruling is reopened
  before D1 lands.

**Alternatives**

- **Yjs in the page, yrs in the host.** The most mature binding (since
  2019; Tiptap ships its own, `@tiptap/y-tiptap`, "Tiptap ProseMirror
  bindings for Yjs"), the smallest browser code (300 KB of unminified ESM
  against 3.6 MB of Wasm), the sync protocol and awareness in yrs, and a
  written threat model. Lost on four counts: two
  implementations of one format, whose compatibility table lags yrs by
  seven minor versions; Peritext's anomaly; the text-loss case under
  offline structure edits; and Yjs 14 mid-flight, so that Yjs 13 means
  migrating soon and Yjs 14 means building on a release candidate. **To
  revisit** when Yjs 14 and `@y/prosemirror` 2 are released stable (their
  suggestion mode and attributed diffs are what a Docs app wants) and yrs
  states compatibility with Yjs 14.
- **Loro.** Peritext's rules on Fugue, a format frozen since 1.0, time
  travel, and shallow snapshots that could hand an editor a replica
  without history. Lost: its host crate brings MPL-2.0+ and BSL-1.0
  crates that `deny.toml` does not admit (an ADR and the owner's ruling on
  weak copyleft in the host), 83 new crate names in all; its ProseMirror
  binding is a tree like Yjs's, so the structure case is not addressed;
  and a small team. The runner-up, if the owner admits MPL-2.0 and D0
  favours it.
- **diamond-types or Eg-walker.** Plain text, a crate last published in
  2022, ISC. Its ideas are in Loro.
- **OT, as Google Docs does** (ShareDB, `prosemirror-collab`). Pleris has
  the central server OT wants. Lost: the host could not read what it
  orders. `prosemirror-collab`'s authority applies ProseMirror's steps in
  JavaScript, and no maintained rich-text OT has a Rust implementation,
  so the host could not render the document for a viewer, show a past
  version or resolve a comment's range without a JavaScript engine in it.
  And offline is OT's weak case, by Eg-walker's measure and Peritext's
  "single, linear timeline". ShareDB's `rich-text` type was last published
  in 2020.
- **Figma's last writer wins per property.** A whole text value replaced
  by the last writer: "simultaneous editing of the same text value doesn't
  work in Figma."
- **A CRDT of Pleris's own.** Charter §2.

### 2. Pleris renders the document; an editor's page mounts ProseMirror

- **A reader's page is Pleris's.** A viewer and a commenter read the
  document as semantic HTML the host renders from the CRDT's spans (its
  blocks and marks): Pleris's renderer, streamed and resumable like any
  page, kept live by the stream's patches (ruling 3), with no editor code
  and no replica in the page. They never receive the replica, so never
  the history inside it.
- **An editor's page mounts ProseMirror** over that same rendered
  document. It is ADR-0075's refused case, built: a browser `resource`
  (A-007's form, affine, `placement browser`) whose `acquire` creates the
  editor on the page's element and whose `release` destroys it, loaded
  after the page's first paint, so the first paint is the host's render.
- **Why ProseMirror**: MIT; the base Tiptap and BlockNote build on; a
  binding exists for each of the three CRDTs; its schema is declared, so
  the program's types generate it; maintained; and it edits HTML, not a
  canvas.
- **The boundary**:
  - **What it is given**: its page's own replica, the schema, and for each
    peer a name and colour the host stamped (ruling 3). Never a handle, a
    session, a secret or another document: the compiler checks the
    mount's arguments.
  - **What it holds**: the page's authority. In one JavaScript realm it
    could do anything the page can: fetch this origin with the session's
    cookie, read the page's DOM. What bounds it is the page's policy
    (ADR-0333: this origin only, no `eval`), the vendored code at a pinned
    version and hash, and the audit record. Not a confinement, said so.
  - **What is audited** (A-024's form): `unsafe capability
    synchronous_geometry because "ProseMirror maps positions and
    coordinates through getBoundingClientRect, getClientRects,
    elementFromPoint and caretRangeFromPoint (prosemirror-view
    src/domcoords.ts)" attributes_forced_layout_to DocumentEditor`, so
    its forced layout is attributed to it (charter §7.5A).
- **The document is external input, decoded.** Any editor's client can
  write any change (Yjs's threat model: "Any peer with write access has
  the theoretical ability to corrupt a Yjs document"), and the library
  leaves "sanitizing rich text content for XSS" to the application. So
  the host decodes spans into the program's types (charter §7.10): an
  unknown block renders as a paragraph and an unknown mark is dropped; a
  link's address decodes to a `SafeUrl` (`https:`, `http:`, `mailto:`)
  or the link is dropped; an image only from this origin's uploads
  (ADR-0333's `img-src 'self'`); text is escaped. The generated
  ProseMirror schema checks a link's address again in its `toDOM`.
- **How the code reaches the page.** The approved packages are vendored
  into the build, `dist/vendor/<package>@<version>/…`, their bare
  specifiers rewritten to this origin's paths when vendored, so no import
  map and no inline script is added and ADR-0333's policy is unchanged;
  each file's SHA-256 is recorded in the build's manifest. `script-src
  'self' 'wasm-unsafe-eval'` already allows the Wasm.

**Alternatives**

- **Pleris's own editing engine on `contenteditable`**: selection mapping,
  IME composition, clipboard, undo, spellcheck and mobile keyboards, which
  is what prosemirror-view's 248,851 bytes of source (1.42.6's
  `dist/index.js`) and ten years of releases hold. A later question,
  measured against the mounted editor once it exists.
- **EditContext**: experimental, and in one engine.
- **A canvas**, as Google Docs since 2021: charter §1, item 10.
- **Tiptap** (MIT, 3.32.0): ProseMirror under a layer of its own, its
  collaboration extension tied to Yjs. **Lexical** (MIT, 0.52.0):
  collaboration only through `@lexical/yjs`. **Slate** (MIT, 0.138.2):
  `@slate-yjs/core` last published 2023-07-11. **Quill** (BSD-3-Clause,
  2.0.3): a Delta model of its own. **BlockNote** (0.56.0): MPL-2.0, and
  Tiptap underneath.
- **The editor in a sandboxed frame**: confinement, an opaque origin with
  no cookie, at the cost of selection, IME, focus and layout across a
  frame. Not first; the boundary to revisit on a measured need.
- **The editor for every reader**: it would ship the replica, and the
  history inside it, to every viewer.

### 3. Presence and cursors ride the stream down, and one sync request up

- **Down: two new frame kinds on the subscriber's queue.** `ReplicaSync`
  carries the host's next sync message for that page's replica,
  acknowledged and kept as a batch is (ADR-0139). `Presence` carries each
  peer's latest state, replaced and never appended per peer, so a slow
  page holds one state per peer, never a trail of them, and
  `MAX_WAITING` still bounds the queue. The stream's 25 ms pass coalesces
  both: a page hears at most 40 batches a second, whatever its peers send.
- **Up: `POST /pw-sync/{doc}`, one in flight per document and page.** Not
  a command: no transaction of the program's, no outbox, no interaction's
  record. It is idempotent by the CRDT itself, since a change already
  applied applies nothing. It carries the replica's sync message and the
  sender's presence. While one is in flight, new changes wait for the
  next: Wave's wait for acknowledgement, Google's one pending change, and
  charter §9.5's one in-flight request, and Automerge's sync protocol
  needs an in-order stream. Presence is sent at most every 100 ms.
- **The host answers once the changes are durable**: appended to the
  document's log (in memory, or on PostgreSQL in the request's
  transaction), then answered with the host's reply message, and then
  told to each other page of the document whose reader the access check
  still allows.
- **Identity is the host's.** A presence state's name and colour are the
  principal's (ADR-0270), never what the client says; y-prosemirror's
  README has the client set `name: 'My real name'` itself. A link guest is
  shown as anonymous. Each Automerge actor is bound to the user of the
  session that first sent it, and a change from an actor bound to another
  user is refused, so the history's "who changed this" is the host's
  word.
- **Viewers hear edits at a pace, not per keystroke.** An admitted change
  raises `Edited(doc)`, which drops `DocumentView(_, doc)`; the host tells
  a document's viewers at most 4 times a second, so a viewer's page shows
  an edit within 250 ms and a pass.
- **Presence's life**: gone 30 s after its last word (Yjs's and Loro's
  default), renewed every 15 s while the page is there, and ended at once
  on `pagehide`.
- **The budget, measured by D6** on loopback, in three engines, with 2, 5
  and 10 editors typing 10 characters a second: another editor sees a
  keystroke within 150 ms at p95, and a cursor within 150 ms; no frame is
  lost; the host's threads and CPU are recorded. Then again under the
  latencies of charter M11's lab, when it exists.
- **A WebSocket only if the measurement asks for one.** The corpus already
  says `transport websocket` (A-006); `tungstenite` 0.30.0 runs over the
  host's own `std::net::TcpStream` threads ("both synchronous (like
  TcpStream) and asynchronous usage"); CSP3's `'self'` already matches
  `wss:`. It is D9, built only if D6 fails its budget. WebTransport is in
  all three engines now, but needs HTTP/3: charter M12.

**Alternatives**

- **A WebSocket first**: a second transport beside the stream and the
  long poll before any evidence that they fall short, and the
  acknowledgement, forced reconnection and reload that ADR-0139, ADR-0155
  and ADR-0175 built and tested for the stream, built again for it.
- **A command per keystroke**: a transaction, an interaction's record and
  the outbox per keystroke, and every open page derived again per
  keystroke.
- **Presence inside the CRDT**: kept in the history for ever.
- **Presence as the client says**: a client could show itself as anyone.
- **WebRTC between peers**: a relay to run, and no host to hold who may
  read.

## How it fits Pleris

### Kind 4, modelled (a sketch; syntax provisional, ADR-0010)

```
type Block = Paragraph | Heading(HeadingLevel) | Quote | Bullet | Numbered | CodeBlock
type Mark  = Strong | Emphasis | InlineCode | Link(SafeUrl)

replicated Document(reader: capability.User<UserId>, doc: DocId)
    -> Result<RichText<Block, Mark>, DocumentError>
{
    privacy                User(reader)
    storage                device
    offline                writable
    sync                   live
    conflict               rich_text(grows = [Strong, Emphasis], fixed = [InlineCode, Link])
    on_conflict_unresolved surface_to_user
    requires               MayEdit(doc)
}

private query DocumentView(reader: capability.User<UserId>, doc: DocId)
    -> Result<Rendered<Block, Mark>, DocumentError>
    cache          private
    key            reader, doc
    invalidates_on Edited(doc), AccessChanged(doc)
```

- `RichText<B, M>` is the platform's; `B` and `M` are the program's closed
  types. From them the compiler generates the block and mark names the
  adapter writes, the ProseMirror schema, the host's renderer and the
  decoder. A mark whose growth is not stated is refused: as A-011's
  comment says, "there is no silent last-write-wins".
- A replica is read from its reader's side, ADR-0279's ruling extended:
  keyed by the reader's own handle, labelled the reader's, private.
  Another reader's replica of the same document is another value; the
  host's merge of all of them is the deployment's, as a conversation's
  rows are.

### Permissions, held by predicates over the reader's own handle

- **Roles are closed types**, and a link can never make an owner because
  its type has no such case:

  ```
  type Role     = Owner | Editor | Commenter | Viewer
  type LinkRole = LinkViewer | LinkCommenter | LinkEditor
  type Sharing  = Restricted | AnyoneWithLink(LinkRole)

  predicate MayView(doc: DocId)    says "You don't have access to this document."
  predicate MayComment(doc: DocId) says "You can view this document, not comment on it."
  predicate MayEdit(doc: DocId)    says "You can't edit this document."
  predicate OwnsDoc(doc: DocId)    says "Only its owner can share it."
  ```

- **`Access(doc)` is a host operation** answering the reader's role from
  the principal's grant, or the session's link grant (a guest's,
  ADR-0258), or nothing. Each predicate is evaluated through it, at a
  `TRACK SEAM (docs)` beside `MayMessage`.
- **Who sees what**: a viewer reads the rendered document and its
  comments; a commenter adds comments; an editor holds the replica, and
  with it the history; only editors and the owner are shown the history;
  only the owner shares. Every reader present is shown to every other
  reader of the document, and a link guest anonymously.
- **Comments are rows, outside the CRDT**, as Figma keeps its comments in
  PostgreSQL. A commenter never writes the document (Yjs's threat model:
  untrusted peers never write it). A comment's anchor is made by the host
  from the page's selection, a block and an offset at the version the page
  showed (`get_cursor(.., at: heads)`), and a program makes no anchor (a
  handle, ADR-0263's rule). A comment whose text was deleted is shown as
  detached.
- **A role lost, or lost offline.** The stream stops sending the
  document; the next sync is refused 403 `MayEdit` with its words; the
  page shows the changes the host never took, lets them be copied, and
  drops the replica (`on_conflict_unresolved surface_to_user`). A CRDT
  cannot take half an update back, so the replica goes whole.

### A shared link: a capability URL the platform makes, redeems and revokes

- **The token** is 256 bits from the operating system, as ADR-0258's
  states are. Visiting `/d/{doc}/l/{token}` redeems it into a grant on the
  session, carrying the token's generation, and answers 303 to the
  document's own address, so the token leaves the address bar, the
  history and the `Referer` (the TAG's redirect pattern).
- **Document pages** send `Referrer-Policy: no-referrer`; every link the
  host renders in a document carries `rel="noreferrer noopener"`; the
  token's path prefix is in `robots.txt` (the TAG's advice).
- **Revoking is rotating**: a new generation, and every grant of the old
  one is no grant. The share dialog is a page private to the owner,
  behind `OwnsDoc`. A program never holds the token: the platform makes,
  shows and redeems it, so no program can log it, cache it or put it in
  an event.

### The typed principal

- `Access(doc)` reads `current_user()`'s grants; the program never names
  a reader the host did not make (ADR-0263, ADR-0270).
- An actor is bound to a user, and a presence state stamped with one, by
  the host alone (ruling 3).
- A link guest is the session's own guest principal (ADR-0258), with a
  grant on that session alone.

### Device-local storage, shared with the end-to-end-encrypted messages

- **The device store**: IndexedDB, a partition per user (the principal's
  id, hashed), opened only for the signed-in user, cleared at sign-out,
  including a sign-out the stream tells from another tab;
  `navigator.storage.persist()` asked once; a quota error said on the
  page; what it holds read back through a decoder (charter §7.10).
- **What it holds**: for this app, the replica (Automerge's saved bytes)
  and the changes the host has not acknowledged. For the encrypted
  messages, keys (WebCrypto, non-extractable) and history, labelled
  `Device`: never to leave the device.
- **`storage device` is not the `Device` label.** A replica is kept on the
  device and is the reader's (`User<reader>`); it syncs. `Device` is what
  never leaves (the next app's keys and plaintext).
- **Said plainly**: a partition is a convention inside one origin, not an
  isolation, since any script of the origin can open any of its IndexedDB
  databases. Safari deletes the store after seven days of Safari use
  without a visit unless persistence is granted, and grants it by its own
  heuristics. A link guest edits online only: a guest has no partition.

### What the compiler checks, and what the runtime does

| Concern | The compiler checks | The runtime does |
| --- | --- | --- |
| Merge | `conflict` names a policy the type has; each mark states its growth; no last-write-wins on text | Automerge merges, the crate in the host, its Wasm in an editor's page |
| Reading | every read of a document keyed by the reader's own handle, `cache private`; a page's read gated by `MayView` | `Access(doc)` answers the role; the host renders per reader; frames only to sessions it allows |
| Writing | the sync names `MayEdit`, a comment `MayComment`, sharing `OwnsDoc`, each predicate with words (PW0351) | each sync request evaluated, refused 403 with its words; a role lost stops the frames |
| Roles | closed types; `LinkRole` has no owner; the ability of each role matched exhaustively | grants stored; a link grant per session, with its generation |
| Handles | a program makes no anchor, share link or `User<..>` (PW5037's rule) | the host makes them |
| Device | `storage device` only for a value keyed by its reader's handle; read back decoded | a partition per user, cleared at sign-out, persistence asked |
| Content | a link's address is a `SafeUrl`; the schema covers every case of `B` and `M` | the decoder drops what is unknown or unsafe; the renderer escapes |
| The editor | its arguments: no handle, secret or other document; its audit record complete | vendored and hashed, mounted after first paint, released on leaving |
| Offline refusal | `offline writable` states `on_conflict_unresolved` | unsent changes shown and copyable, the replica dropped |
| Presence | nothing: no program value holds it | stamped by the host, throttled, coalesced, timed out, sent only to readers |
| History | a restore `requires MayEdit` | versions by heads, a past version rendered, actors as users, a restore as a new change |

## The build plan, as tracks

Each track: its files, its codes (proposed, in a block the integrator
assigns; PW66xx and above are free at 5da9c05, PW61 being TodoMVC's), its
recipe (`just/<track>.just`, `e14-<name>` writing
`docs/evidence/E14/<name>.txt`), its acceptance and its mutation controls
(`scripts/<track>_mutations.py`, each `.pw` mutant still well typed).
Phase 1 is the foundation the encrypted messages share; phase 2 makes it
collaborative; phase 3 is the app; D9 is conditional.

### Phase 0: D0, `docs-merge`, the cases before the choice

- **Files**: `spikes/docs-merge/` (a Node harness for the Wasm package, a
  Rust test for the crate), `just/docs-merge.just`.
- **Codes**: none; it measures a library.
- **Cases**, each with its expected result written before the run:
  Peritext's examples 1 to 9; Automerge's structure case; the
  interleaving case, typed forwards and backwards; a mark across a
  concurrent paragraph split; two branches of 1,000 edits each merged
  (time and result); a comment's anchor across a deletion; the Automerge
  paper's editing trace (259,778 operations) loaded, timed, and its
  memory taken, in Node and in three engines; and the same changes applied
  by the crate and by the Wasm saving to the same bytes and the same spans.
- **Acceptance**: `e14-docs-merge`; ruling 1 stands, or is reopened.
- **Mutation controls**: the harness comparing text without marks; the
  merge step skipped; the crate's and the Wasm's saves not compared.
- **Optionally** (owner decision 2), the same cases against Yjs and Loro
  in Node, as exhibits for the comparison: never shipped.

### Phase 1: the foundation

**D1, `replicated`: a replicated rich text, in the compiler and the host.**

- **Files**: `compiler/pw-core/src/replicated.rs` (the clauses' meanings,
  `RichText`, the generated schema, renderer and decoder),
  `packages/pw-platform-web/` (the platform's `RichText`, `SafeUrl`),
  corpus fixtures (an accepted replicated document; a rejected one per
  rule), `compiler/pw-core/tests/replicated.rs`;
  `spikes/own-renderer/server/src/replicas.rs` (the adapter: apply a sync
  message, save, load, spans, cursors, render), `replicas_pg.rs` (the log
  and its snapshots), `migrations/docs/0001_replicas.sql`; `automerge =
  "=0.12.0"` in the server's `Cargo.toml`, a commit of its own.
- **Codes**: PW6601 a replicated value's `conflict` is a policy its type
  has; PW6602 every mark states its growth; PW6603 `storage device` holds
  only a value keyed by its reader's own handle; PW6604 `offline
  writable` states `on_conflict_unresolved`; PW6605 a link's address is a
  `SafeUrl`; PW6606 a replicated document's sync names its edit predicate.
- **Acceptance** (`e14-replicated`): each rule refused and its control
  accepted; two replicas' concurrent edits merged as D0 measured; the
  renderer escaping text and dropping `javascript:` and `data:` links; an
  unknown block rendered as a paragraph; a hostile change (an unknown type,
  10 MB of text) refused or bounded; the log on PostgreSQL surviving a
  restart and replaying to the same spans; a past version rendered.
- **Mutation controls**: each rule disabled; a `javascript:` link let
  through; an unknown block passed as raw markup; the log written after
  the answer; the size limit skipped.

**D2, `device-store`: the browser's store, under its policy.**

- **Files**: `spikes/own-renderer/public/pw-device-store.mjs`; the
  sign-out telling at `TRACK SEAM (docs)` in `identity.rs`;
  `e2e/device-store.spec.mjs`.
- **Codes**: none; the device's rules are D1's PW6603 and the runtime's.
- **Acceptance** (`e14-device-store`, three engines): a replica written,
  the page reloaded offline and the replica read back; sign-out clearing
  the user's partition, and another tab's; two users of one browser
  profile never opening each other's; `persist()` asked once and its
  answer recorded; a quota error said on the page; corrupt stored bytes
  decoded as an error, never used.
- **Mutation controls**: no clearing at sign-out; a partition by session
  instead of user; the decoder skipped; `persist()` not asked.

### Phase 2: collaborative

**D3, `replica-sync`: the sync request, the frames, offline and refusal.**

- **Files**: `spikes/own-renderer/server/src/docs.rs` (`/pw-sync/{doc}`,
  actors bound to users, limits, the frames), `runtime/pw-protocol`
  (`ReplicaSync`, `Presence`), `spikes/own-renderer/public/pw-replica.mjs`
  (one in flight, the offline queue, reconnection),
  `e2e/replica-sync.spec.mjs`.
- **Codes**: none; its compile-time rule is D1's PW6606, the rest the
  host's.
- **Acceptance** (`e14-replica-sync`): two pages editing at once
  converge, in three engines; a page offline edits, reconnects and merges;
  never two sync requests in flight for one page and document (the host
  counts); a viewer's sync refused 403 `MayEdit` with its words; a change
  from an actor bound to another user refused; a sync over the limit
  refused 413; a page whose reader lost access sent no further frame; no
  frame of the document's to a session without a role (a third user's
  pages and stream hold none of its text); an answer lost and the request
  sent again applying nothing twice.
- **Mutation controls**: two in flight allowed; `MayEdit` skipped; the
  actor's binding skipped; frames sent without the access check; the
  answer sent before the log is written.

**D4, `editor-mount`: ADR-0075's mounted resource, built for ProseMirror.**

- **Files**: the compiler's mount (ADR-0075's refusal lifted for a
  `resource` whose `acquire` mounts a declared foreign component;
  `template_ir.rs`), the generated ProseMirror schema,
  `scripts/vendor.py` (copies an approved package's files, rewrites its
  specifiers, records hashes), `serve_file`'s `vendor/` directory,
  `spikes/own-renderer/public/pw-editor.mjs`, `e2e/editor.spec.mjs`.
- **Codes**: PW6607 a mount is given no handle, secret or another
  document's replica; PW6608 the generated schema covers every case of
  `B` and `M`; the audit record's completeness is the existing
  `UNSAFE_AUDIT_INCOMPLETE`.
- **Acceptance** (`e14-editor-mount`): each rule refused at build and
  its control accepted; then in three engines: typing; bold, italic
  and a link by keyboard; pasted HTML holding a script and a `javascript:`
  link kept out by the schema; undo and redo; composition input where
  Playwright can drive it; ADR-0333's policy unchanged and nothing
  refused under it; the first paint the host's render and the editor
  mounted after it; a reader's page loading no editor code (its requests
  listed); the editor's synchronous layout attributed to
  `DocumentEditor`.
- **Mutation controls**: each rule disabled; the schema's `toDOM` without
  the address check; a vendored file whose hash differs accepted; editor
  code sent to a viewer's page.

### Phase 3: the app

**D5, `docs-sharing`: roles, links, and the app's pages.**

- **Files**: `examples/docs/app.pw` (the documents list, the document
  page, the share dialog), `docs.rs` and `docs_pg.rs` (grants, link
  grants, `Access`), `migrations/docs/0002_sharing.sql`,
  `e2e/docs-sharing.spec.mjs`.
- **Codes**: PW6609 a program makes no share link (PW5037's rule for a
  new handle); PW6610 a command that changes sharing `requires OwnsDoc`.
- **Acceptance** (`e14-docs-sharing`): each rule refused at build and its
  control accepted; every role against every ability, in memory and on
  PostgreSQL; a link redeemed into a grant and the
  redirect free of the token; the token in no log line, no shared cache
  entry and no other user's page; a rotation revoking; `Referrer-Policy`
  and `rel="noreferrer noopener"` on every rendered link; a third user
  seeing nothing, by page, `/pw-read`, stream and cache.
- **Mutation controls**: each rule disabled; each predicate inverted and
  skipped; a link granting ownership; the token left in the redirect; a
  rotation revoking nothing; the referrer policy dropped.

**D6, `docs-presence`: who is here, and where.**

- **Files**: `docs.rs` (presence per document, stamped, timed out),
  `spikes/own-renderer/public/pw-presence.mjs`, the editor's decorations
  for other cursors and selections, `e2e/docs-presence.spec.mjs`, a
  latency script writing `docs/evidence/E14/docs-presence.txt`.
- **Codes**: none; no program value holds presence.
- **Acceptance** (`e14-docs-presence`): who is here; others' cursors and
  selections; a guest shown anonymously; a session without a role told
  nothing; presence gone 30 s after its last word and at once on
  `pagehide`; at most one state per peer per pass; the budget at 2, 5 and
  10 editors.
- **Mutation controls**: a name the client sent accepted; presence
  appended instead of replaced; presence sent to a session without a
  role; the timeout never firing.
- **The decision point**: a failed budget starts D9.

**D7, `docs-comments`: comments on ranges.**

- **Files**: `examples/docs/app.pw` (`Comment`, `comment`, `reply`,
  `resolve`), `docs.rs` (anchors), `docs_pg.rs`,
  `migrations/docs/0003_comments.sql`, `e2e/docs-comments.spec.mjs`.
- **Codes**: PW6611 a program makes no anchor.
- **Acceptance** (`e14-docs-comments`): an anchor a program makes refused
  at build, and its control; a commenter's selection made an anchor at
  the page's version, highlighted; the anchor following
  concurrent edits; a deleted range's comment shown detached; a viewer's
  comment refused `MayComment`; comments private as the document is.
- **Mutation controls**: PW6611 disabled; an anchor resolved at the latest
  version instead of the page's; `MayComment` skipped; an anchor sent by
  the client accepted.

**D8, `docs-history`: versions and restore.**

- **Files**: `docs.rs` (named versions and checkpoints, by heads),
  `examples/docs/app.pw` (the history page, `restore`),
  `e2e/docs-history.spec.mjs`.
- **Codes**: PW6612 a `restore` `requires MayEdit`.
- **Acceptance** (`e14-docs-history`): a `restore` without `requires
  MayEdit` refused at build, and its control; a past version rendered; a
  diff by author, actors shown as users; a restore made as a new change
  that every replica merges, an offline editor's later changes kept; a
  viewer's request for history refused.
- **Mutation controls**: PW6612 disabled; a restore that replaces the
  document (breaking every replica) instead of a new change; history shown
  to a viewer; an author taken from the client.

### Conditional: D9, `live-socket`

Only if D6 fails its budget: `tungstenite` (owner decision 9), A-006's
`transport websocket`, the same frames (`pw-protocol` names no
transport), the stream and the long poll kept as fallbacks.

## Downloads for the owner's approval

Read from the npm registry, crates.io and jsDelivr's file listings on
2026-10-14; resolved from metadata alone.

**Shipped to an editor's page** (npm, vendored by D4), 16 packages,
50,169,930 bytes unpacked, all MIT:

| Package | Version | Published | Unpacked bytes |
| --- | --- | --- | --- |
| `@automerge/automerge` | 3.5.0 | 2026-09-16 | 47,529,812 (its Wasm 3,644,225; the rest are other builds of it) |
| `@automerge/prosemirror` | 0.2.0 | 2026-02-25 | 201,178 |
| `prosemirror-model` | 1.25.12 | 2026-09-21 | 530,158 |
| `prosemirror-state` | 1.4.4 | 2025-10-23 | 184,018 |
| `prosemirror-view` | 1.42.6 | 2026-09-25 | 905,889 |
| `prosemirror-transform` | 1.12.2 | 2026-09-25 | 326,309 |
| `prosemirror-commands` | 1.7.2 | 2026-08-05 | 133,102 |
| `prosemirror-keymap` | 1.2.3 | 2025-05-04 | 26,164 |
| `prosemirror-history` | 1.5.1 | 2026-09-29 | 69,244 |
| `prosemirror-changeset` | 2.4.4 | 2026-09-25 | 128,015 |
| `prosemirror-schema-basic` | 1.2.5 | 2026-09-25 | 30,353 |
| `prosemirror-schema-list` | 1.5.1 | 2025-03-04 | 54,555 |
| `orderedmap` | 2.1.1 | 2023-05-17 | 14,835 |
| `rope-sequence` | 1.3.4 | 2023-05-17 | 27,917 |
| `w3c-keyname` | 2.2.8 | 2023-06-07 | 8,381 |
| `ordered-map` | 0.1.0 | 2014-11-04 | not stated (see Found) |

What the page loads is the Wasm and the modules it imports, not the
package's other builds; its size compressed is D0's to measure (the build
has no minifier today).

**The host's crate**: `automerge = "=0.12.0"` (2026-09-16, MIT, 409,627
bytes). Its lockfile holds 45 crates: 22 in `Cargo.lock` at the same
version, 10 another version of a crate it holds (of which `itertools`
0.15.0 cannot share the lock's 0.14.0, and the rest may resolve to the
lock's), and 13 names new to it, 1,789,218 bytes:

| Crate | Version | Licence | Bytes |
| --- | --- | --- | --- |
| `automerge` | 0.12.0 | MIT | 409,627 |
| `borsh` | 1.8.1 | MIT OR Apache-2.0 | 85,266 |
| `cfg_aliases` | 0.2.2 | MIT | 6,527 |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 | 80,244 |
| `hex` | 0.4.3 | MIT OR Apache-2.0 | 13,299 |
| `hexane` | 1.0.0-alpha.5 | MIT | 259,029 |
| `leb128` | 0.2.7 | MIT OR Apache-2.0 | 13,874 |
| `smol_str` | 0.3.6 | MIT OR Apache-2.0 | 26,335 |
| `tracing` | 0.1.44 | MIT | 463,135 |
| `tracing-attributes` | 0.1.31 | MIT | 39,733 |
| `tracing-core` | 0.1.36 | MIT | 63,967 |
| `unicode-segmentation` | 1.13.3 | MIT OR Apache-2.0 | 112,325 |
| `zlib-rs` | 0.6.8 | Zlib | 215,857 |

and `itertools` 0.15.0 (MIT OR Apache-2.0, 164,131 bytes). Every licence
is one `deny.toml` admits.

**For D0's comparison only, never shipped** (owner decision 2), in Node:
`yjs` 13.6.33, `y-prosemirror` 1.3.7, `y-protocols` 1.0.7, `lib0`
0.2.119, `isomorphic.js` 0.2.5 (with ProseMirror, 15 packages, 11,698,448
bytes, all MIT); `loro-crdt` 1.16.4 and `loro-prosemirror` 0.4.4 (with
ProseMirror and `lib0`, 14 packages, 29,084,963 bytes, all MIT). The
`yrs` crate 0.28.0 would add 8 names (509,518 bytes, all admitted); the
`loro` crate is not proposed, its graph failing `deny.toml`.

**Only for D9** (owner decision 9): `tungstenite` 0.30.0 (2026-07-11, MIT
OR Apache-2.0, 71,531 bytes), with `data-encoding` 2.11.1 (MIT), `http`
1.5.0, `httparse` 1.10.1 and `sha1` 0.11.0 (MIT OR Apache-2.0): 5 new
names, 265,991 bytes.

## Owner decisions

1. **The CRDT**: Automerge, as this note decides; or Yjs with yrs, the
   most mature, two implementations, with Yjs 14 near; or Loro, which
   needs MPL-2.0+ and BSL-1.0 admitted to the host.
2. **The downloads**: the shipped list and the host's crate above; and
   whether D0 may also take Yjs and Loro, in Node, for the comparison.
3. **A foreign editor at the page's authority**: ProseMirror runs in the
   page's own realm, bounded by the policy, the pinned code and the audit,
   not confined. Accept it so, or ask for a sandboxed frame first, at the
   cost of selection, IME and focus across a frame.
4. **History on editors' devices**: with Automerge every editor's device
   keeps every deleted word, and erasing history means a copy of the
   document. Accept it, or ask for replicas without history (Loro's
   shallow snapshots, or Yjs with garbage collection).
5. **Shared links**: links that last until rotated, or links that expire,
   as the TAG advises; and whether a link may grant editing.
6. **Offline promises**: Safari deletes a site's store after seven days of
   Safari use without a visit, and grants persistence by its heuristics.
   Promise offline edits within that, said on the page, or offline
   editing only in an installed web app.
7. **Presence**: every reader shown to every other reader while present,
   and link guests anonymously.
8. **Suggestion mode** is not in this app: Yjs's threat model wants an
   untrusted suggester on a separate fork, a design of its own. Confirm.
9. **A WebSocket** only on a failed measurement (charter §11.3), or
   `tungstenite` approved now.

## Questions for the integrator

1. **The codes and ports** of D0 to D9: their block (PW66xx and above are
   free) and their Playwright bases.
2. **A predicate on a read.** `requires` is a command's (ADR-0258); the
   reader's page needs a query refused by role with its words, as a
   command is. Or keep it as the data operation's answer (`NotShared`),
   as a conversation carries `closed`?
3. **`pw-protocol`'s new frames**: version 2, or added at version 1 for a
   page that ignores a kind it does not know?
4. **The app's home**: `examples/docs/` as a fourth app on the development
   server, with its own data layer and `migrations/docs/`?
5. **`serve_file`'s `vendor/` directory**: a change to a rule ADR-0333's
   tests rely on (flat names from `dist`).
6. **ADR-0279's ruling extended**: a document read from each side, its
   render private to each reader, so a document many read costs a derive
   per reader (ADR-0270's superset). Confirm.

## Not claimed

- **The editor confined**: it holds the page's authority (ruling 2).
- **Merges past D0's cases.** Automerge guarantees convergence; its own
  August 2026 update points at the Livelymerge note that "convergence is
  not the same as correctness".
- **Erasure**: a deleted word stays in every editor's replica (Automerge).
- **Offline durability past the browser's eviction.**
- **Presence past the measured editors**, and in a lab's latencies until
  M11.
- **More than one host** (ADR-0246): one host process holds a document.
- **End-to-end encryption of documents**: the next app's; Keyhive is
  alpha.
- **Suggestion mode; tables; images beyond this origin's uploads; export
  to DOCX or PDF; print and page layout; search inside documents.**
- **A link's secrecy past the TAG's mitigations**: a link forwarded, an
  extension that reads URLs, or a history synced elsewhere.
- **Telling by principal**: a document's viewers are told as ADR-0270's
  superset tells.
- **A screen reader's run of the editor**, and mobile keyboards past
  Playwright's three engines.
- **What a minified and compressed editor page weighs**: D0 measures it.

## Found while researching

- **A stale row in the capability matrix**: "XSS and CSRF | ... | No
  Content-Security-Policy is generated (KNOWN_LIMITATIONS)". ADR-0333
  generates one since 2026-10-14; the matrix was checked at 73e462b
  (2026-10-08). The integrator's file.
- **`@automerge/prosemirror` 0.2.0 declares `ordered-map` `^0.1.0`**: a
  package published once, 2014-11-04, by an unrelated author, with no
  repository, whose tarball holds only its `package.json` by jsDelivr's
  listing, and which no file of the binding's `src/` imports. Harmless
  today; `^0.1.0` would take any 0.1.x published under that name. D4
  vendors only the files the build imports.
- **`automerge` 0.12.0 requires `hexane` 1.0.0-alpha.5**, a pre-release,
  where the newest stable `hexane` is 0.2.1.
- **Loro's crate graph** holds MPL-2.0+ and BSL-1.0 crates, and `loom` and
  `pretty_assertions` as normal dependencies.
- **GitHub reports Yjs's licence as NOASSERTION**: the LICENSE file is the
  MIT text with two copyright holders; npm states MIT.
- **y-crdt's compatibility table** names yrs 0.21 and yjs 13.6; yrs is at
  0.28.0, and Yjs 14 is a release candidate.
- **WebTransport is in all three engines** (Safari 26.4, by MDN's data),
  over HTTP/3.
- **loro.dev answers a script with a bot check**; its documentation was
  read from its source repository, and nothing was done to pass the
  check.
