## M. Forms, drafts, routes, and navigation

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| M01 | Form draft cannot express temporary invalid input | Model/spec: user types a minus sign before a number | Editable text and validated domain values have different state spaces. | Draft models; typed decoding | missing | A+C+D | Derive draft, parse errors, and submit decoding from the domain contract without treating them as duplicate schemas. P0. | T+U | [S43] [R1] |
| M02 | Client validation substitutes for authoritative validation | Model/spec: direct POST bypasses UI | Every external command input must be validated and authorized at execution. | Server forms; shared decoders | specified but unproven | C+D | Generate both usability checks and authoritative decoding from the same predicates; keep runtime-only checks explicit. | T+U | [S24] [R1] |
| M03 | Submit behavior changes when JavaScript is absent | Model/spec: form works only after hydration | An operation declared progressively enhanced must preserve its command semantics through native submission. | Native HTML forms; enhanced form adapters | specified but unproven | C+D+G | Derive action, method, decoding, outcome, and redirect for both paths; do not claim every rich app must work without JS. | U+X | [R1] [R11] |
| M04 | Two activation paths submit one intent twice | Model/spec: Enter plus click/replayed handler | One logical submission must have one intent identity across equivalent entry paths. | Generated form intent; idempotency | specified but unproven | C+D | Unify submit events, pending state, and command identity; still permit explicit repeated submissions. | U+R | [S13] [R1] |
| M05 | Late result overwrites a newer draft | Model/spec: server validation for old input arrives late | A response may update only the draft version it describes or a deliberate reconciliation target. | Draft revision tokens | missing | C+D | Separate field draft revision from resource revision and preserve subsequent edits. | U+R | [S43] [R1] |
| M06 | Navigation leaves stale work attached | Model/spec: old route completion changes new page | Route lifetime and retained/shared-resource lifetime must be represented independently. | Scoped routes; shared resource owners | partially covered | C+D | Cancel subscriptions on exit, gate late completions, and preserve explicitly retained work. | U+R | [R1] [R4] |
| M07 | History and URL disagree with application state | Model/spec: Back restores wrong filters | Navigable state and history entries must have an explicit reversible representation. | Typed routes; history snapshots | specified but unproven | C+D+H | Derive route encoding/decoding and distinguish push, replace, transient, and durable state. | U+D | [R1] [S41] |
| M08 | Scroll or focus restoration destroys user context | Model/spec: restored list jumps to top | Navigation restoration must respect browser-owned state and the intended target. | Native navigation; scoped restoration | specified but unproven | D+F+H | Preserve anchors, selection, nested scrollers, and meaningful focus; policies differ for Back, new navigation, and modal close. | U+D | [S41] [S45] [R1] |
| M09 | Route encoding or redirect admits the wrong target | Model/spec: double-decoded route parameter | Routing and authorization must use the same normalized identity and allowed destination. | Typed route codecs; redirect allowlists | specified but unproven | C+D+E | Decode once at a defined boundary and restrict redirect authority; preserve round-trip tests. | T+X | [S22] [S71] [R1] |
| M10 | Recovery reload discards unsaved work | Model/spec: schema mismatch reloads edited form | Compatibility recovery must not silently destroy acknowledged local user input. | Draft persistence; recovery negotiation | partially covered | D+H | Offer migrate, export, retry, or explicit discard according to policy; a successful reload is not automatically safe recovery. P0. | U+D | [R4] [R11] [S46] |

## N. Accessibility and browser-owned interaction

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| N01 | Control lacks a usable semantic name or role | WCAG; WAI-ARIA patterns | An interactive operation must expose perceivable purpose and operable semantics. | Native elements; accessibility diagnostics | specified but unproven | B+C+D+G+H | Check structural relationships statically, derive native semantics, and require human evaluation of meaningful labels. | U+D | [S44] [S45] [R1] |
| N02 | Keyboard behavior diverges from pointer behavior | Model/spec: clickable div cannot be activated by keyboard | Supported input modalities must reach the same intended operation. | Native buttons/links; control libraries | specified but unproven | C+D+G | Prefer native semantics; generated custom controls need keyboard and touch behavior tests, not only role attributes. | U | [S44] [S53] |
| N03 | Focus trap or return target is wrong | WAI modal dialog pattern | Modal focus and background interactivity must match the actual interaction state. | Native dialog; tested focus scopes | specified but unproven | C+D+H | Derive trapping/inertness and meaningful return behavior; initial focus remains content-sensitive policy. | U+D | [S45] |
| N04 | Visual state disagrees with accessibility tree | Model/spec: aria-modal without actual modality | Semantic accessibility state must represent real interaction and visibility. | Native state properties; ARIA mappings | specified but unproven | C+D+G | Derive attributes from the same control state; test dynamic trees with assistive technologies. | U | [S45] [S44] |
| N05 | Async updates are silent or excessively announced | Model/spec: repeated live-region loading announcements | Assistive feedback must report meaningful state changes without overwhelming the user. | Status semantics; bounded announcements | missing | C+D+H | Associate announcements with semantic outcomes and user intent; expose concise policy for urgency and aggregation. | U+D | [S44] |
| N06 | Composition, autocorrect, or undo is overwritten | React IME issue 8683 | Browser editing sessions must not be replaced by stale application values. | Composition-aware controls | missing | C+D+F+G | Model in-progress editing, committed text, selection, and replacement; test real IMEs and mobile keyboards. P0. | U | [S43] [S48] |
| N07 | Browser services lose access to content | Model/spec: virtualization defeats find/copy/translation | Rendering optimization must preserve the declared native document affordances. | Semantic DOM; accessible rendering modes | specified but unproven | C+D+H | Test find-in-page, copy, translation, selection, and printing; never require universal virtualization. | U+P | [S53] [S47] [R11] |
| N08 | Accessibility depends on one automated score | WCAG mixed evaluation model | Automated rule compliance is not whole-application accessibility proof. | Manual assistive-tech testing; conformance review | missing | G+H+F | Require keyboard, screen-reader, zoom/reflow, contrast, motion, and task-completion evidence across supported configurations. | V+U | [S44] |
| N09 | Pointer-only precision or gesture requirement | Model/spec: drag is the only way to reorder | Essential actions need an operable alternative when the user cannot perform a specific gesture. | Native input; alternative actions | specified but unproven | C+H+G | Model the action independently of drag/touch presentation and provide keyboard/non-drag access. | U+D | [S44] |
| N10 | Motion or viewport change hides necessary content | Model/spec: mobile keyboard obscures focused input | Responsive presentation must preserve task visibility under user and device preferences. | Responsive layouts; preference-aware controls | specified but unproven | D+F+G+H | Test keyboard viewport changes, zoom, reduced motion, orientation, and safe-area behavior on real mobile devices. | U+D | [S44] [R1] |

## O. Internationalization, text, time, and numeric meaning

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| O01 | Text slicing splits a user-perceived character | Unicode segmentation model | Text operations must use the intended unit, not assume code units equal characters. | Grapheme-aware text APIs | missing | A+B+C | Distinguish byte, code-point, and grapheme operations; make unsafe low-level indexing explicit. | T | [S66] |
| O02 | Mixed-direction content changes presentation meaning | W3C bidi examples | Embedded text direction must not escape into surrounding semantic content unexpectedly. | Bidi isolation; direction-aware markup | specified but unproven | C+D | Derive safe isolation at relevant dynamic-text boundaries and retain explicit document direction. | U | [S65] [S68] [R1] |
| O03 | Translation drops required variables or cases | Model/spec: pluralized message loses quantity | Translations must satisfy the source message's typed interpolation and selection contract. | Typed message catalogs | missing | B+C+G | Check placeholders and required locale cases; translation quality remains human work. | T+U | [S65] [R1] |
| O04 | Localized display string reused as machine value | Model/spec: formatted date reparsed as canonical timestamp | Presentation formatting must not silently become a storage or protocol representation. | Typed formatting/encoding APIs | missing | A+B+C | Separate display text from canonical data and derive each from one underlying value. | T | [S67] [S71] |
| O05 | Wall time resolves ambiguously | Temporal time-zone examples | Converting a local date/time to an instant requires a zone and ambiguity policy. | Temporal-style types | missing | B+D+H | Use distinct date, instant, zoned time, duration, and schedule types; expose gap/fold policy only at conversion boundaries. | T+D | [S67] |
| O06 | Recurring schedule drifts with timezone changes | Model/spec: daily local appointment stored as fixed UTC interval | Recurrence must preserve the intended civil-time or elapsed-time meaning. | Explicit recurrence rules | missing | D+H | Store scheduling intent and zone/version policy, not just the next timestamp; test DST and rule changes. | T+D | [S67] |
| O07 | Money mixes currency or rounding authority | Model/spec: tax rounding differs between UI and charge | Monetary arithmetic must preserve currency, scale, and the chosen rounding stage. | Decimal/money types | missing | A+B+D+H | Make monetary rules authoritative on the server and derive display; provider adapters enforce their own amount contract. | T+D | [S14] [S71] |
| O08 | Unicode normalization changes identity unexpectedly | Model/spec: visually similar account names treated inconsistently | Identifier equality must be a declared domain policy shared by lookup, uniqueness, and authentication. | Canonical identifier codecs | missing | C+D+H | Choose normalization/case handling per domain; preserve original display text and do not conflate confusable detection with equality. | T+D | [S65] [S71] |
| O09 | Locale-dependent ordering destabilizes persisted cursors | Model/spec: sorted page cursor reused under another locale | Ordering-dependent identities must bind the ordering policy and its compatibility version. | Versioned collation; scoped cursors | missing | C+D+H | Bind locale/collation to cursor contracts or use stable domain order; do not infer a universal sorting policy. | T+D | [S65] [S70] |

## P. CSS, layout, and rendering cost

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| P01 | Generated read/write interleaving forces repeated layout | Layout-thrashing example | Owned layout reads and writes must obey a phase discipline. | Measure/plan/mutate scheduling | partially covered | B+C+D | Reject synchronous owned measurement after owned mutation in a batch; foreign code enters an explicit layout boundary. | E+U | [S60] [R1] |
| P02 | Clean-layout proof ignores external invalidation | Model/spec: font or foreign script changes geometry | Scheduling one's own writes does not prove the browser has no pending style/layout work. | Trace-based validation; phase boundaries | partially covered | D+G+F | State the narrower guarantee of no generated thrashing; measure actual layout cost and external invalidators. | P+V | [S60] [R5] |
| P03 | Observer feedback never converges | Model/spec: resize callback changes its measured size | Observer-driven feedback needs a stable or explicitly bounded update policy. | Deferred observer processing | specified but unproven | C+D+H | Batch observer input and bound feedback; expose unstable feedback rather than promise any next-frame loop converges. | U+D | [R1] [S60] |
| P04 | Containment or virtualization changes semantics | Model/spec: offscreen content no longer searchable | Layout optimization is admissible only if it preserves required document behavior. | Opt-in containment; semantic rendering modes | partially covered | C+G+H | Compare optimized and reference behavior for selection, focus, printing, sticky layout, and accessibility. | P+U | [S53] [S60] [R1] |
| P05 | CSS scope or cascade violates component contract | Model/spec: host stylesheet overrides private control state | Style ownership must define permitted host influence and semantic requirements. | Scoped CSS; layers; design tokens | specified but unproven | C+D+G | Derive stable scoping and layer order; preserve deliberate theme/inheritance hooks and test host styles. | U+X | [R1] [S69] |
| P06 | Physical axes break RTL or vertical layout | CSS Writing Modes model | Layout expressed in logical relationships must not be compiled as fixed physical coordinates. | Logical properties; writing modes | specified but unproven | C+G | Retain logical axes through layout APIs; physical positioning remains explicit when intended. | U | [S69] [R1] |
| P07 | Images, fonts, or embeds cause uncontrolled shifts | CLS documented causes | Layout must reserve or deliberately negotiate space for asynchronous content. | Intrinsic dimensions; fallback metrics | specified but unproven | C+D+H | Derive asset dimensions when known; require layout policy for unknown embeds and test post-load shifts. | U+P | [S63] [R1] |
| P08 | Animation uses more work than its visual contract requires | Model/spec: geometry animation blocks interaction | Animation plans must preserve visuals and interaction while respecting work budgets. | Compositor-friendly plans; profiling | specified but unproven | C+D+G | Prefer equivalent cheaper plans when proven; do not universally replace geometric effects with transforms. | P+U | [S60] [S61] [R1] |
| P09 | Large DOM or style graph exceeds device capacity | Layout-cost model | Rendered structure must fit measured device budgets without destroying required content behavior. | Incremental rendering; optional virtualization | specified but unproven | D+G+H | Measure node count, style/layout time, memory, and interaction latency; choose domain-compatible rendering strategies. | P+D | [S60] [S44] |


[S13]: ../SOURCES.md#s13
[S14]: ../SOURCES.md#s14
[S22]: ../SOURCES.md#s22
[S24]: ../SOURCES.md#s24
[S41]: ../SOURCES.md#s41
[S43]: ../SOURCES.md#s43
[S44]: ../SOURCES.md#s44
[S45]: ../SOURCES.md#s45
[S46]: ../SOURCES.md#s46
[S47]: ../SOURCES.md#s47
[S48]: ../SOURCES.md#s48
[S53]: ../SOURCES.md#s53
[S60]: ../SOURCES.md#s60
[S61]: ../SOURCES.md#s61
[S63]: ../SOURCES.md#s63
[S65]: ../SOURCES.md#s65
[S66]: ../SOURCES.md#s66
[S67]: ../SOURCES.md#s67
[S68]: ../SOURCES.md#s68
[S69]: ../SOURCES.md#s69
[S70]: ../SOURCES.md#s70
[S71]: ../SOURCES.md#s71
[R1]: ../SOURCES.md#r1
[R4]: ../SOURCES.md#r4
[R5]: ../SOURCES.md#r5
[R11]: ../SOURCES.md#r11
