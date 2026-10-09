# Known limitations

Reviewed 2026-09-15 against master `4a9bcddc095b3b4d0eac0e0c2ddb8a0143de03d9`
and the evidence-gate repair. Current milestone state is owned by
[STATUS](STATUS.md); implementation order is in [NEXT](NEXT.md).

The former file was primarily an E0 snapshot and still said no compiler or Linux
CI existed. Its complete bytes are retained in
[the historical limitations file](KNOWN_LIMITATIONS-history-2026-09-15.md).
Those historical statements must not override current source or test results.

## Value checking and generated execution

**What the parser does not read yet** (found with ADR-0237, which made
lowering report what it parses):

- **An `{#each}`'s list and key are read as written** (ADR-0242): parsed by
  the grammar, and not lowered into the body's arena, so a computed list is
  not typed, and a key's names are not resolved as terms.
- **A clause, a `return` and a block after a statement are statements**
  (ADR-0243, ADR-0038): a block separates its statements around them, and
  their readers read each pair as one. The grammar has no node for any of
  them.

**E9-V1..V6 are met** (2026-09-24, ADR-0031). What the value relations do not
decide is Undecided, counted by `pw audit-values`, and never reported as
agreement:

- **A member of a value of unknown type is not judged** (ADR-0048). Member
  existence is a relation now (PW0610), and a read whose value's type nothing
  states is undecided, as every relation's is.
- **A sum type's case is typed where it is written through its type**
  (2026-09-26, ADR-0059): `Shape.Circle(3)`, its fields, its arity, and a
  case the type lacks (PW0608). A case written alone, `Empty` or
  `Circle(3)`, is the case of the one visible type with it (ADR-0198), or,
  where several have it, of the type expected where it is written
  (ADR-0201); PW0022 names them where nothing expected chooses. Until
  2026-09-26 no case had a type, and `Shape.Bogus(1)` passed.
- **What the value relations still leave undecided** since ADR-0065, which
  typed a value that holds at every type (`None`, `[]`, `todo`, ..): 1 of the
  store's relations, 1 of kiokun's, 17 of the accepted corpus's. Since
  ADR-0178 a listener's `_` is decided, which had left two of the store's
  undecided and one of the corpus's. Each is its own construct:
  - a `measure { .. }` block;
  - a dimensioned literal (`8.px`);
  - a `derived` value;
  - a painter's context;
  - a declared function named as a value (`Money.add`);
  - a handle from a `use` of a host call;
  - a type named as a value (`Element`).

  A declared function's result that its arguments do not fix stays
  unknown. ADR-0195 (ruling 4) rules it typed from what is expected of it,
  and refused as "type annotation needed" where nothing fixes it; not built
  yet.
- **A record literal whose first field is shorthand** (`P { x }`) parses as
  the name `P` followed by a block `{ x }`. A brace is a record only as `{ }`
  or `{ name:`, so that `if x { .. }` stays a block. `let p = P { x }`
  checks and means something else; `P { a: 1, x }` is a record. Found
  writing ADR-0063. ADR-0195 (ruling 1) rules Rust's rule: a type name and
  `{` open a record everywhere but the head of `if`, `while`, `for` and
  `match`, where a record is parenthesized; not built yet.
- **A `()` body discards its last value** (A-018). A branch mismatch in
  statement position is not refused; as a result, it is.
- **Generic layouts at the boundary.** Phantom parameters map to one WIT
  layout; a generic whose layout depends on its arguments is refused, because
  specialization is not implemented.
- **Named function values are not instantiated.** A generic function named as
  a value (`List.map` passed along) has its type parameters as holes.
- **The unit value `()` lowers to `Expr::Error`.** Its type is therefore
  unknown. This is harmless to the relations, which never guess, but it is a
  lowering gap.

A scope/resource policy is now attached to the resolved type rather than its
spelling, preserving the existing type-level policy. This is not a per-value
ownership proof. Private RPCs now require conditional principal preservation;
that records a runtime obligation, not its execution. Stable nominal capture
identities distinguish modules; complete transitive schema-evolution and host
semantic-ABI validation remain separate work. Foreign code and backend internal
record/variant layouts still need their documented integration proofs.

**E10-I is established** (2026-09-24, ADR-0032): the store's commands compile
to components and run through the E8 host, and the Rust closure path is deleted.
The component backend is narrow, and everything outside it is refused by name
rather than approximated:

- **A generic type does not cross the component boundary** (ADR-0062).
  Inside a component and a module, a generic record, sum type or opaque type
  is laid out per instance, its arguments fixed by its fields or its use. In
  a query's parameter or result it has no WIT form, unless its parameter is
  phantom. An instance only a lambda's branches name is refused by name.
- **A type that contains itself compiles** (ADR-0194, ADR-0202):
  `replies: List<Comment>` is laid out by its type, and one held in place,
  `Option<Node>` or `Add(Expr, Expr)`, is boxed. Each crosses a boundary as
  its nodes. Not yet:
  - **crossing a boundary** through another declaration (two types that hold
    each other), held deeper than one `List` or `Option` of itself, holding
    another such type, or as an opaque type: refused by name where its WIT is
    written. Each compiles inside a component;
  - **on the browser's wire** it crosses as its nodes (ADR-0205): a
    signal's value, which a handler reads and sets, a command's argument,
    and a speculation's value, which the server writes by its query's type
    (ADR-0233). Refused by name: a handler's capture of one, a command's
    declared error holding one, two types that hold each other, and a case
    of one as a browser's argument;
  - **nested deeper than 128 in a host** (`NESTED_DEPTH`): a host reads such
    a value's nodes instead.
- **`==` compares primitives only.** Two records, two sum-type values or two
  lists are not compared by either backend, and the refusal is by name
  (`Eq` on a nominal type).
- **Straight-line bodies, at E10-I.** Import calls and scalar constants were
  supported, and values moved flat or in their canonical layout. Matches over
  `Option` and `Result`, field reads and their cases came after (ADR-0036),
  and then computation (ADR-0039, below).
- **The invocation region is the memory strategy** (ADR-0046, measured). It
  is a bump region reclaimed by each export's post-return, and nothing can
  outlive an invocation. A call's peak is its whole allocation: kiokun's
  Search reaches 3.2 MB for the one-letter query `T`.
- **Each call instantiates afresh** (ADR-0032). At 7–14 µs it costs more than
  most kiokun calls themselves (ADR-0046).
- **An opaque value is built and read inside a component** (ADR-0054), as
  its representation retyped, a generic one too since ADR-0062; it crosses
  the boundary as that representation.
- **The data layer is not Pleris.** `store:data/carts` is the deployment's
  (`owner: external`), as the contract records.

**The backend matches over `Option` and `Result`**, reads fields, and builds
their cases (2026-09-25, ADR-0036). **It computes** (2026-09-25, ADR-0039):
`Int` and `Float` arithmetic, comparisons, `&`, `|`, `!`, `if`, string
literals, interpolation, pipelines, records built, and calls to other
declarations, inlined, or compiled beside the export when they recurse, at
the types a generic callee's arguments give it (ADR-0050). An early
`return`, `?`, `for` loops and `let mut` bindings compile (ADR-0051). Still
refused by name:

- a `Float` literal pattern, and an or-pattern that binds a name (nested
  and literal patterns compile to a decision tree since ADR-0060);
- (resolved by ADR-0076) an arm no case reaches, which the checker
  computed and did not report: it is PW0333 now;
- a `return`, a `?` or an assignment inside a lambda a list operation runs,
  and an assignment to a field (ADR-0051);
- a named argument to a standard-library operation, which the checker
  gives to the parameter of its name and the backend refuses by name: an
  operation takes its arguments in order (ADR-0081);
- `%` on a `Float`, and a `Float` interpolated: their semantics are not
  decided;
- list and string operations beyond the ones the standard library declares
  (ADR-0040, below).

- **A trap's cause is read from a function's name** (ADR-0267). Each trap
  of the component's own calls a function the name section names by its
  cause, and the host reads the cause from the frame the trap stopped in;
  Wasm's own it names by their code. A tool that strips the name section
  takes the causes with it, the traps staying where they were, and an
  engine that inlines functions may omit the frame, which the host's
  engine is configured not to. Until ADR-0267 traps were not distinguished
  by cause: an `Int` overflow and a zero divisor read alike.
- **An operand the checker cannot type is undecided** (ADR-0043). PW0609
  refuses operands of two known types that an operator does not take, and
  counts the rest as undecided; the backend refuses what remains.
- **There is no implicit conversion between `Int` and `Float`.**
  `Float.from_int` converts where a count meets a measurement (ADR-0043).
- **The logical operators are `&` and `|`.** `&&` does not parse. They
  short-circuit.

- **Some matches are not analysed for exhaustiveness.** Each is counted
  Blocked by the match audit, with its reason: neither proven nor refused.
  - A `Float` literal pattern.
  - A scrutinee the value relations cannot type, including a name bound at
    two sites.

  A pattern nested under `Some`, `Ok`, `Err` or a declared case, a literal,
  a `Bool`, and a scrutinee no pattern takes apart (a record, a list) are
  analysed since 2026-09-26 (ADR-0060); until then each was Blocked.

  Until 2026-09-25 matches over `Option`, `Result` and calls were not checked
  at all, and four other shapes were proven exhaustive when they were not
  (ADR-0038).
- **A case written alone is chosen by what is written where it is, not by
  its later use** (ADR-0201). Where two types have `Empty`, `let s = Empty`
  then `take(s)` is PW0022: the `let` states no type, and the rule does not
  infer one from `take`. Write `let s: Shape = Empty`, or `Shape.Empty`.
- **A `let` name or a parameter may begin with a capital** (ADR-0197). A
  pattern's name is a case by its capital and a binding otherwise, and a
  case is declared with one (PW0625); a `let` and a parameter bind a name
  whatever its first letter, since their grammar binds nothing else.
- **`return` is a statement, not an expression.** Its value is the statement
  after it, in a block or on its line in a match arm (ADR-0038).
- **A template arm takes one case apart** (ADR-0042, ADR-0061). A template's
  `{#match}` takes an `Option`, a `Result` or a declared sum type apart, an
  arm binding each field of its case. A nested or literal pattern in an arm
  is not read: the runtime has no decision tree. `{:else}` in `{#each}` is refused;
  `{#if xs}` around the list says the same.
- **An interpolated attribute is refused in a `style`**, and a URL with holes
  must begin with text (ADR-0042).
- **A computed value builds from one value** (ADR-0226 to ADR-0229): a text
  hole, an attribute's whole value or a block's subject, at the top of a page
  from a query's value, which a host computes, or a signal's, which the
  browser computes after the host's first; inside a block a host renders,
  from a query's value; and in a loop's row from the row's item, which a host
  computes for each row, and the speculation module for each row it renders.
  These check and do not build (ruling 0073-a, later):
  - one from several values, from none, or from a page's parameter;
  - one the browser would compute inside a block, or one from a query's
    inside a block a signal decides;
  - one in an arm from the names it binds, or in a view that contains
    itself;
  - an attribute from a speculated value, or a speculated row's value from
    a field of its item (a block's subject from one is the module's since
    ADR-0235);
  - a hole in an attribute's text.

  A directive other than `on:` (`style:width={w}`) does not build either
  (ADR-0073).
- **A value computed from a signal shows its last value until the page's
  module has loaded** (ADR-0227): the first change loads it, once per page.
- **A `Float` is not written by a template** (ADR-0074). It has no format
  yet, so `{price}` over a `Float` is refused (PW0609); the host drops a
  `Float` it is given.
- **A mounted resource does not build** (ADR-0075). The template IR has no
  representation for an element that mounts a resource
  (`resource={StoreMap}`, A-007), and refuses it by name.
- **A stream region's limits** (ADR-0148):
  - **Without JavaScript, outside Chrome 150 and later**, a streamed region
    stays its placeholder. The settled arm is in the document, inert, in its
    `<template for>`.
  - **WebKit paints nothing until a page holds about 200 characters of
    text**, or has loaded. A smaller shell is blank in Safari until its
    regions settle, however it is rendered.
  - **A region is rendered once per document.** A change to its query after
    it settles is not sent. Its keyed rows are addressed in the document's
    own domain, not shared as a public fragment's are.
  - **WebKit paints a streaming page only once it says enough** (ADR-0165,
    ADR-0166). While a document is still arriving, WebKit holds its first
    paint until it has more than 200 non-whitespace characters of text, or
    an image over 32 by 32 pixels. A page that says less is shown in Safari
    only when its streams have settled. The store says enough since its
    descriptions (ADR-0166); a compiler cannot know a page's text before its
    data, so nothing checks another page.
  - **A stream sits at the top of a page, or in a view composed there.** One
    inside a block or a loop's row is refused at build, as is a signal shown
    in one, or a view holding one.
  - **`fallback` is not executed for a stream's query**: the plan refuses a
    page with one.
  - **Every page that binds a query is served and kept current by its own
    plan** (ADR-0190), and speculates from its own module (ADR-0191). The
    store's page alone has its shared list, the menu, kept as one fragment
    for every reader, and a change to public data other than the menu
    reaches no open page. A change the store makes to a session's own
    resource reaches its pages (ADR-0193), for the order alone so far.
  - **A query's budget bounds the region, not the query.** The query runs on
    after its region is given the host's failure, and what it answers is
    kept as its policy says.
- **A template shows each case by name; a function need not** (ADR-0153).
  A function a template calls can map a state to nothing with a catch-all,
  and the template shows what it returns.
- **A command a page sends is keyed by its interaction** (ADR-0154), and the
  runtime still sends each request once: with every one keyed it could retry
  safely, as RFC 9110 then allows, and does not yet.
- **A key a page changes** (ADR-0152):
  - **Only a page's own `let` query is keyed by a signal.** A view's signal,
    and a `<stream>`'s query, are not.
  - **A key is a `String`, an `Int` or a `Bool`** (PW5308). A record or a
    list would need a codec.
  - **The key a page shows is recorded per session**, as what it shows is.
    Two tabs of one session that choose different keys are patched as one.
  - **Keyed reads run in a browser for `cancel` only**: the charter's store
    tests 6-8 in Chromium, Firefox and WebKit (`e2e/keyed.spec.mjs`), and
    T07's controls in Chromium for `keep` and `supersede`. The server's tests
    cover each policy.
- **What a page shows is recorded per session** (ADR-0151, as before it).
  Two pages of one session read at once are both served, and the later is
  the one later changes are derived against.
- **A fragment changed at its source reaches the pages when a document
  reads it** (ADR-0150, ADR-0178). When a shared fragment's query value
  changes with no event, the pages open are told once a new document reads
  the change: within the query's freshness, or not at all for a store no
  one opens.
- **`pw diff`'s limits** (ADR-0149):
  - **Effects are the declared rows.** A query's inferred effects appear as
    its component's capabilities.
  - **Client bytes** are the compiled handlers and speculations; the runtime
    is not counted.
  - **No state machines**, since the language has none; a sum type's cases
    are domain changes.
- **A view composes when its body is its markup, its signals and its
  `provide`s** (ADR-0136, ADR-0144). Refused by name (PW5020):
  - a view with other bindings of its own;
  - a prop that is not a value path, a literal included (`label="Add"`).
- **A view that contains itself is an instance made at run time**
  (ADR-0203), each use rendered in a frame of its own. Refused by name
  (PW5020): one with no `{#if}`, `{#match}` or `{#each}` on the way back to
  it, one that holds or provides a signal, and one that shows a `<stream>`.
  A page nests at most 500 elements, and the renderer refuses one deeper
  (`TooDeep`). A change inside an instance renders it again whole: a list
  in it is not kept in place, and what has focus there loses it. An
  instance at the top of a page given a signal and anything else is
  refused, as the browser holds the signals alone.

  A signal given to a view as a prop is shown, not changed: a view's
  handler that would capture one is refused (PW5301). A view changes a
  signal it names, its own or one provided to it (ADR-0144). A view's
  handler that captures a restricted parameter is refused at the view, even
  where every page using it could hold the value. The Marko adapter composes
  no view that holds or names a signal.
- **The server patches a page's text parts, its lists and the blocks its
  queries decide, by difference** (ADR-0145, ADR-0146). What a loop's row
  reads of another query is refused by the plan. An interpolated attribute
  renders and is not patched, and neither is a shared list other than the
  menu. A query's error is the server's failure, not a value a page shows.
  kiokun's pages are static.
- **A clause's words are the clause's** (ADR-0047). Every name resolves in
  lexical scope, but `scope application`'s `application`, or `load`'s
  `on_first_interaction`, is a word of its clause and not a name. A word
  outside its clause's closed set is left to the analysis that reads the
  clause, and most clauses written inside a block have none.
- **The standard library is small** (ADR-0040, ADR-0055).
  - `List` has `length`, `get`, `take`, `drop`, `slice`, `reverse`,
    `concat`, `map`, `filter`, `fold`, `any`, `all`, `find`, `sort_by`,
    `group_by` (by a `String` key, adjacent runs), `sum` and `maximum` (of
    `Float`s).
  - `String` has `length`, `slice`, `codepoints`, `from_codepoints`,
    `starts_with`, `ends_with`, `contains`, `join`, `trim`,
    `to_lower_ascii`, and `to_lower` and `to_upper` (ADR-0056).
  - `Float` has `from_int` (ADR-0043).

  - `Map` and `Set` (ADR-0057), keyed by an `Int`, a `String`, a `Bool` or
    an opaque type over one (ADR-0248), in ascending key order.

  There is no `split` and no `range`. `enumerate` is removed, since the
  language has no tuple type (ADR-0055).
- **A map's key is an `Int`, a `String`, a `Bool`, or an opaque type over
  one** (ADR-0057, ADR-0248), and another key type is refused at check
  (PW0627), where it is written or where a call instantiates it. A map or set
  a query is given, or a host answers, is sorted on arrival, and a key twice
  stops the invocation (ADR-0259); one inside another value is refused,
  since nothing would sort it. A `for` loop reads a map or set through
  `Map.keys` or `Set.to_list`.
- **Case mapping is per code point** (ADR-0056). A word-final capital sigma
  lowers to `σ`, where Unicode's default conversion gives `ς`. There is no
  case folding and no locale rule. The mapping is Unicode 17.0's, as the
  pinned Rust knows it.
- **A member with one declaration needs no import** (ADR-0254). A function
  of another module whose first parameter takes a type is that type's
  member wherever the type is; only where several modules declare one is the
  module's import what decides it, and a module that sees several or none is
  refused (PW0628). A page's planned reads ask as no module.
- **An affine value has no borrow** (ADR-0045). A function whose row does not
  release a transaction may use it, and one whose row does must end it once
  on every path. There is no way to say "this function reads the value and
  hands it back"; a `use` block releases its value and is the scoped form. A
  release inside a loop is refused even when the loop would run once.
- **An acquisition is followed from a name** (ADR-0250, ADR-0269). One is
  held by a binding, `let x = ..`, `use x = ..` or `let h =
  Maps.create(..)?`; by what takes apart the `Result` or `Option` carrying
  it, an arm's `Ok(h)` or `Some(h)`, or `let h = r?`; ended where it is
  made; given to a caller, by a declaration that answers one; or held by a
  resource's `acquire` clause. Anywhere else it is refused, even where it
  would be ended: a bare handle matched, `match h { x => .. }`, a carrying
  value nested in another or bound by an or-pattern, and `let o =
  Some(Database.begin())`, which no release takes. `use _ = ..` is refused, not read as a scope holding a value it
  never names, and so is an acquisition no name holds inside a keyword's
  block, `unsafe.imperative { Maps.create(..) }`. In a clause other than
  `acquire`, a `release`, a key, a `draw` or a transition, an acquisition
  is refused, not followed into what reads the clause's value (ADR-0251).
- **A function value read from a record field is not called** (ADR-0052).
  A function is a value: a lambda or a declaration's name is stored,
  returned, passed and called. But `r.check(5)` reads as a method call. A
  lambda whose use fixes no parameter types is refused by name.
- **A function passed to a generic declaration is refused by name**
  (ADR-0062, found 2026-09-26). `apply(n, x => x + 1)`, where `fn apply<A,
  B>(a: A, f: fn(A) -> B) -> B`, is "a lambda whose type nothing fixes":
  the backend types an argument by its parameter's declared type, and does
  not carry into it what the earlier arguments fixed. A list operation's
  function, and a non-generic declaration's, are typed.
- **A statement keyword cannot name a value** (PW0013, ADR-0041, ruling
  needed). `let query = ..` is refused rather than read as a `query ..`
  statement at every use.

**The kiokun slice** (ADR-0037, ADR-0041) is one shard of kiokun.com, with
its logic in Pleris and its data layer in the host:
- **Search covers the loaded shard only.** A lookup reaches every shard.
  kiokun.com searches in SQLite FTS5, and an index over every shard is the
  deployment's.
- **The index is Rust.** Candidate retrieval, case folding and a stub's target
  are the host's, as the database's are kiokun.com's. The ranking is Pleris.
- **The ranking is held to the slice's port, not to kiokun.com's live search**
  (ADR-0041, corrected). kiokun.com's `/api/search` decides which queries are
  CJK differently (hangul yes, astral Han no) and searches script variants.
- **Korean is looked up, not searched.** An entry shows its Korean words,
  Japanese names and character (ADR-0037, amended). The index has Chinese and
  Japanese rows; kiokun.com's Korean rows need its romanization and ranking
  ported first. Pitch accent is not in kiokun's entries.
- **A call is a fresh instance** (ADR-0032). Bulk work needs a query over a
  list, as `shards.Places` is: a call a word made the whole shard's load three
  times slower.

**Resumable handler bodies are compiled** (2026-09-25, ADR-0033), to one ES
module each, and the page's elements carry what the handlers read. A body
computes (ADR-0058): it is lowered as a query's is, and its commands are
awaited in order. What remains:

- **A handler is given its event's record, as the platform declares it**
  (ADR-0138): `press`, `input`, `change`, `keydown`, `submit`. An element
  does not refine its event: a checkbox's `change` gives `value`, not
  `checked`. `bind:value` and forms wait for ADR-0131's later rulings. A
  function or a command named as a handler, `on:input={save}`, is
  `(e) => save(e)` (ADR-0199).
- **A handler reads what its command answered, never the value**
  (ADR-0157): `Result<(), E>`, whether it committed and its declared error
  if not. A handler takes it apart or discards it by name; one that gives it
  to the runtime is refused (ADR-0159, PW0618), and so is a command named
  as a handler, `on:submit={save}`, that answers one (ADR-0199). A command called
  inside a function value, or a function value that reads what the handler
  captured, is refused, and so is a command parameter that is not a
  primitive or an opaque type over one. A command is called by a page's
  handler only (PW0339).
- **The development server hosts the store's two commands only.** A handler
  that calls another command compiles, and is tested under Node.
- **Captures are not a patched part.** An element carries the capture paths
  its handler reads, as rendered. A patch that changes a captured field without
  re-rendering the element leaves the old value there: for example, E7-P's
  keyed rename, which replaces only the item's text. The store's handler reads
  only `item.id`, the loop's key, which a keyed patch never changes.
- **No invariant relates two fields** (ADR-0180): a delivery estimate's
  least minutes are at most its most because its estimator says so.
- **An opaque type's invariant is bounds on an `Int`, or on a `String`'s
  length** (ADR-0179, ADR-0225). `where value >= 1` and `where
  String.length(value) <= 280` are checked at every construction, by the
  build, and at every boundary, by the host. A length is in code points: no
  grapheme clusters, no normalization. Any other predicate is refused
  (PW0623) until a program needs it. A test narrows what it tests in
  an `if`'s branches and across `&` and `|`; a test followed by an early
  `return` narrows nothing after it. A keyed read's key and a route's
  parameter are not held to an invariant: none of the store's is a type
  that states one.
- **A privacy label follows the control flow since ADR-0129.** A branch a
  secret chooses carries its label, and so does a sink inside it. A name
  bound over a labelled collection's elements carries its label (ADR-0063).
- **A call through an unannotated lambda is related to nothing**
  (ADR-0068). `let f = (x) => x + 1` then `f("a")`: the lambda's parameter
  takes its type from no use, so the call has no function type to check. A
  call through an annotated value or a parameter is checked.
- **The name check keeps its own scope walk** (ADR-0066). `names.rs`
  resolves scopes for PW0021 itself, where every other analysis reads
  `crate::lexical`. They differ in one rule: the name check binds a keyword
  statement's first word, to keep a modifier quiet, except the resource a
  `query` or `subscription` names, which it resolves (ADR-0108).
- **A clause's key is checked for names, count and types, not labels**
  (ADR-0088). A secret passed in `emits` or `invalidates` reaches the graph
  unlabelled, and what a key performs (`current_session()`'s
  `session.read`) is not the declaration's. ADR-0195 (ruling 9) rules a key
  labelled by what built it, a secret refused, and what a key performs the
  declaration's effect; not built yet.
- **`requires` is held at the invocation boundary** (ADR-0115).
  Predicate names belong to the deployment's authorization vocabulary and
  command-parameter arguments are carried by index in the component contract.
  The host refuses an export whose preconditions were not evaluated, and a
  deployment evaluator's false, unknown, error or missing argument refuses the
  invocation before the component runs. **The own-renderer development server
  is not a production identity provider:** it deliberately models every local
  demo session as `SignedIn` and knows no other predicate. Production identity
  verification and application-specific predicate implementations remain
  deployment integrations.
- **A policy's value is checked by its domain wherever it is written**
  (ADR-0089, ADR-0247): heading a declaration, or as a clause in a block,
  `observe .. { scope application }` or a `handler_policy { .. }`. **Some
  clauses are read by nothing**, judged and then unread: no analysis,
  generator or runtime names `respects`, `intrinsic_height` or a
  `handler_policy` (RISK_REGISTER's R5 for the last). Where a block's
  clause may be written is not judged either: `respects` in a resource's
  block is judged by its domain and not refused for its place (0047-a's
  full split). A `requires` predicate is checked as deployment
  vocabulary over command parameters (ADR-0115); a `privacy` label
  constructor's name, a length, or
  a `conflict` strategy's field.
- **A `replicated` declaration is parsed, and nothing else** (A-011): no
  syntax reads one, no runtime keeps one, and its `privacy` clause labels
  nothing. A declaration's label comes from its `session` or `private`
  keyword alone, so `privacy User(consumer)` leaves `OrderDraft` public to
  every analysis. Its values are checked by their domains (ADR-0089).
- **Which declarations a policy belongs to is checked for the four graph
  clauses only** (ADR-0092).
- **A cache key is held to every parameter its body reads** (ADR-0107),
  including one read only for an effect such as a trace, whose value the
  entry does not depend on. Telling the two apart needs the flow of values
  to the result; ADR-0195 (ruling 8) rules it computed by the
  information-flow labels, and the rule here stands until it is.
- **A command's write is matched to its readers by domain, not by key**
  (ADR-0101). PW5106 requires a command writing `Carts` to reach each
  cart reader with no staleness window. It does not check that the event
  it emits carries the key of the entry it wrote, and PW5107 does not check
  that `invalidates Cart(..)` names the entry a command speculates on
  (ADR-0105). A reader with a positive
  `freshness` is exempt (A-026), and a row naming only the family,
  `!{ database }`, matches nothing (A-025).
- **A Marko page refreshes a binding only for a query that a command in
  its own module names in `invalidates`** (`marko.rs`). A query reached only
  by an event, or invalidated by a command in another module, keeps its
  first value on a Marko page, and a speculation on it stays. PW5106 and
  PW5107 accept both kinds of reach, and the dev server honours both
  (ADR-0105).
- **A handler names a function or a command, or is a lambda written in the
  attribute** (ADR-0199). A local's function value, React's
  `onClick={handle}`, is refused when checked (PW0614): a lambda written
  where it is used shows what it captures and the signals it writes, and a
  declaration is how a behaviour is shared.
- **A command computes its events and the entries it invalidates**
  (ADR-0208, ADR-0209). Each goes from the command's component to the
  platform, and is acted on once its writes commit. An event carrying a value
  that is not a key (a record, a list) checks, and is refused when the
  command runs. `invalidates Cart(_)` is every entry (ADR-0256); `_` in
  a `depends_on` key is a name, and resolves to nothing.
- **Accounts are a development provider's, and sessions are in memory**
  (ADR-0258). No production provider adapter: no OIDC discovery, no JWKS, no
  ID token signature verification; `identity::Provider` is the seam. One
  issuer; no account linking, email, recovery or sign-in rate limits.
  Sessions do not outlive the process, expire, or get listed and revoked. A
  stale tab's command refused with 403 shows the runtime's generic failure.
  The program reads who its reader is through a data layer op: since
  ADR-0263 `context.current_user()` is the host's operation,
  `pw:host/principal#read`, which no host answers until track
  `notifications` (W3) lands, and `current_organization()`'s no host
  answers at all.
- **An upload's image outlives its post only where it was copied**
  (ADR-0260, ADR-0261): a deleted post's image is served no more, and its
  blob is collected where no post names it, but a copy a browser or a CDN
  kept under `immutable` stays, and a blob a failed commit put stays in the
  store, served to no one. A budget of uploads is the server's memory, as a
  lease is, so a
  restart or a second host forgets it; the development blob store is a
  directory, and there is no production store, CDN, signed address,
  private image, re-encoding or resizing. Metadata is served as uploaded,
  an Exif position among it; EXIF orientation is read in JPEG alone. A
  post carries one image, attaching it navigates the page and loses a draft
  typed before it, and an image is not part of a post's speculation.
- **The follows timeline is a query** (ADR-0257): read on each render
  from the posts of those followed, not a materialization fanned out on
  write. A reader is told of its own follow by its session, and another
  session of its user by nothing until it reads again; a user's page lists
  their newest 20 posts. No blocks, mutes, follow requests, private
  accounts or lists of followers.
- **A row shown before the server answers waits, as the feed says**
  (ADR-0275). Its id is the page's, `pending-..`; the feed's timeline row
  links nowhere and acts on nothing until the server's row replaces it. A
  reply shown so still links to itself, `/post/pending-reply-{n}`, not
  found: the thread's replies are a view that contains itself, which
  computes no value in its template yet (ruling 0073-a). An id the client
  makes and the server accepts, with which nothing would wait, is not
  built.
- **`pw fmt` keeps a policy's value as written** (ADR-0276). A clause's
  value is tokens, which the grammar gives no expression's nodes, so the
  formatter keeps their gaps as written, one space where there was any, and
  spaces no value as it spaces an expression. It is a canonical spacing,
  not a layout (ADR-0013): it breaks and joins no line. A `_` out of place in
  a number, `5_` or `5__000`, has no message of its own. `examples/history`
  is held to no format.
- **Notifications tell a superset, and are the feed's own** (ADR-0270,
  ADR-0274). An event naming a user reads again every open page of the
  query, not only that user's; another user's page derives from its own
  kept entry and is sent nothing. A like, a reply and a follow notify; a
  mention does not, an unlike there is none of, and there is no grouping,
  no other language, no push and no email. The feed still reads its
  timelines and relations by the session through `identified_by`, and a
  stream reads no `current_user()`.
- **A materialization is kept public, and made again whole** (ADR-0255,
  ADR-0273, ADR-0277). One that derives its value is compiled, kept as the
  materializer's entry, made again in its chain's order when an event
  reaches it, and read by a page; but only a public one (`partition
  public`): one kept per session or user waits for a partition by
  principal, and the follows timeline is built on queries meanwhile. Each
  making recomputes its body whole, asking it again for each read it
  lacks; `regenerate on_read` is not run. One that declares no type is a
  fragment the host renders, the store's still wired by hand.
- **A loop's name that shadows a page's binding is read as the binding**
  (found 2026-10-08, ADR-0277). The page plan types a read by its root's
  name among the page's bindings before its loops: on the store's page, a
  binding named `line` beside the cart's `{#each cart.lines as line}` was
  read for `line.name` inside the loop, and the build refused. The checker
  accepts the shadowing; the binding is named `summary`.
- **A source's guarantees are held to the database the feed opens**
  (ADR-0207, ADR-0246): the host measures a PostgreSQL feed's isolation and
  whether it may write, and refuses to serve on a shortfall. The store's
  data, and every other layer, are held to the host's own database's
  guarantees, not measured. Not counted either: a command's several writes
  to a source without transactions, which commit apart, and its reads
  across two sources, which no transaction makes one snapshot.
- **The feed on PostgreSQL is one host's** (ADR-0246): delivery is the
  committing host's, so a second host on the same database hears nothing
  of the first's commits; a host's commands are serialized by its layer's
  lock, so a serialization failure between concurrent writers is refused
  as any refused commit, and never measured; an interaction's idempotency
  is remembered in the host's memory, not committed with its writes; and
  the browser suite runs on the in-memory layer.
- **A `style` attribute's value is escaped by refusing what executes**
  (`expression(`, a script scheme in `url(`), then as an attribute. A style
  can still load a URL the value names. A `<style>` element holds text only
  (ADR-0094), and a stylesheet's braces open holes, so none is written.
- **A `<link>` in markup may name any origin** (ADR-0189): a relation the
  body allows, a stylesheet or a preconnect, from anywhere. A link the head
  holds is refused in markup (PW5035), and `http-equiv` is the host's
  (ADR-0186). ADR-0195 (ruling 13) rules an `href` a literal or a build's
  asset, a cross-origin one's origin declared by the program, and the host's
  Content-Security-Policy generated from that list; not built yet.
- **A single patch that addresses nothing is ignored** (ADR-0168): a patch
  set that does is refused and the page read again, but a lone
  `ReplaceText` or `SetAttribute` is not. It hid E7-P's scrambled list until
  2026-10-03.
- **A member read is computed for a page in two places only** (ADR-0169):
  text at the top of a page, from a query's value (ADR-0125), and a row of a
  loop over a query's list, from its item. Anywhere else it is a build
  refusal:
  - an attribute, or what a block decides by, at the top of a page;
  - a loop over a field of a row's item, or over a stream's answer;
  - a signal's member, which the browser reads by field.
- **`display` writes US dollars as en-US does** (ADR-0169). Other locales
  and currencies have no `display`.
- **A press is sent only once its handler's code has arrived** (ADR-0268).
  A command's request outlives its page, kept alive within 64 KiB, but a
  press whose handler is still loading when the page is left is never
  made, and a request's answer whose page is gone is read by no one: a
  refusal is not shown, and the next page reads what is. A page whose
  other scripts keep bodies alive can find a command refused by the
  browser's bound, which this runtime does not see.
- **A speculation reaches the top of a page** (ADR-0122, ADR-0172): its
  text, and each attribute, block and loop there that reads the speculated
  value, which the browser renders again. A speculated read inside another
  block, or a list inside a row, is a build refusal. A region renders from
  the speculated value, the page's signals and its own names; one that
  reads anything else is refused.
- **A browser sends a record field by field** (ADR-0172), each field named
  as the host finds it: a record with a field whose name would come back as
  another, `opensMinute`, is refused. A variant is not sent.
- **Availability is a `Bool`** (ADR-0178), as charter §15.1 declares: no
  count of what is left. The store's recommendations are not asked again for
  a stock change, and may name an item sold out since; a cart line is not
  marked when its item sells out, and the command refuses a further
  increase.
- **A page does not say what it shows was the last kept** (ADR-0177).
  While a public query's origin fails, its last value is shown, and nothing
  tells the reader it is from before. `fallback empty` is checked and read
  by nothing.
- **A command's request that hangs waits** (ADR-0173). A command declares
  no `timeout`, so a request with its connection open and no answer is not
  a failure, and is not sent again.
- **A press whose resends all fail says nothing a reader hears** (ADR-0173).
  Its control is marked and its line goes; a handler cannot match "not
  sent". The runtime sends again without asking whether the browser is
  online.
- **Each Add carries the whole item** (ADR-0172), its description too,
  though the command reads its id and the transition its name and price. A
  narrower record is the program's to declare.
- **A list inside a shared query's value is rendered once** (ADR-0170). It
  is rendered with its document and not patched again. Only the menu's
  public fragment is patched for every reader (E7-P), its categories' lists
  inside it included (ADR-0181).
- **An unkeyed list inside a row renders the row again** when it changes
  (ADR-0181): its instances have no address. A keyed one is changed where
  it is.
- **A comment in markup is `<!-- -->`** (ADR-0167), and no page renders it.
  HTML's other bogus comments, `<!x>` and `<?x>`, are read as elements. A
  line of markup text that begins with `//` or `/*` is refused (PW5028),
  except in a `<style>` or a `<script>`.
- **A hole cannot hold a string** (ADR-0049). `"{f("a")}"` ends the outer
  token at the inner quote. Escapes are defined, and every backend reads one
  decoder; policy strings (`because`, `route`, `host`) are read as written.

## Research requirements are not implemented guarantees

The census records temporal authority, unknown external outcomes, compatibility
across live versions, composed capacity budgets, optimistic overlap, and unmanaged
browser/foreign behavior. Some related mechanisms already exist, but the census
itself does not prove complete enforcement. Read the per-record status and the
[research/implementation boundary](../research/failures/IMPLEMENTATION.md).

## Evidence and test coverage

The new recipe tests replace producers in isolated temporary trees. They prove
exit-status propagation for the tested recipe shapes, not compiler correctness,
browser behavior, host isolation, performance, or production readiness.

`errexit` and `pipefail` do not make shell execution universally fail-safe.
Independently invoked scripts and shell contexts that intentionally handle errors
need their own tests. Redirected reports may be incomplete after failure; this
repair does not make report publication atomic or validate every printed claim.

No new real-device, screen-reader, cross-browser, database-fault, or production
rollout test was run locally in the September 15 review. Prior observations keep
their original scope. The false-green recipe defect does not prove old tests
failed, but a recipe's zero exit status alone was insufficient evidence.

**`optimistic` and `idempotent_by` are executed since 2026-10-02**, each
within stated bounds (ADR-0120 found both unexecuted):
- `idempotent_by` (ADR-0121): once per interaction, in memory, for the last
  64 interactions of a session;
- `optimistic` (ADR-0122): in the own renderer only, for a part outside any
  block, where the page binding's key is an invocation-context call. A new
  cart line takes the name and price the page showed (ADR-0172), and the
  server's answer replaces it: a speculation never invents a value
  (ADR-0195, ruling 14).
E14's shared store contract still excludes both, for every stack.

**A signal lives in a page, a use of a view, or a `provide`** (ADR-0133,
ADR-0144). Each use of a view holds its own instance. A view used in a
loop's row holds none yet (PW5307): each row would need its own, kept by the
row's key. A first value is data, not a computation. The browser sets a text part or an
attribute in place outside any loop, at top level or in a block a signal
decides, and renders such a block again for the rest (ADR-0142). The plan
refuses a signal read anywhere else, and a block it renders that reads
anything but the signals and its own names (ADR-0137). `bind:value` binds a
signal of `String`; another type waits for a codec (ADR-0131). So a dialog over a query's value, an
attribute a signal decides, and a signal's list outside its block, wait for
the browser to hold more than the signals. Until 2026-10-02 two of these were
listed here as refused, and built. A block renders again whenever any signal it reads changes, even when
the arm shown does not read it. The browser's renderer is 90 KB gzipped,
loaded on the first block a press renders.

**A modal dialog is a `<dialog>` a signal's block renders** (ADR-0141), the
browser's own, and it handles `close`. A dialog's return value from a
`<form method="dialog">` waits for forms (ADR-0131). A dialog in a view is
shown by the view's own signal or one provided to it (ADR-0144). A page
that reads queries holds signals on the store's route only (ADR-0140); any
other page with queries is not served yet (E14-Q).

**A response's caching is said for a session's responses alone**
(ADR-0184). A session's response says `private, no-store`, and a build's
file names no session but says nothing of how long it may be kept, which is
the deployment's. A page declared `cache shared` is not served yet, so no
response says `public`. The engines in the browser suite ask for a page
again on going back whatever it says, so `no-store`'s effect on a browser
that restores pages is held by its header alone.

**A page states its title only when it is served at a route** (ADR-0183).
PW5029 refuses a routed page that states none. A page without a route is
titled by the host that serves it, as before: the demos' pages by their
names, and the benchmark's store "Store". A title reads a page's parameters
and its queries' values; on a page that binds a query, its parameters since
ADR-0231. Refused at build:
- one that reads a signal;
- one that reads a value a press speculates.

**A page's parameter is read where a host renders** (ADR-0231). Every page
is rendered with its parameters, each one text as its address gives it. A
text part, a title, an attribute, a handler's captures and an instance's
argument read one. The browser holds them only where a speculation renders:
- a region it renders again with a speculation reads them, since a
  speculating document carries them (ADR-0236);
- one read inside a block a signal decides is refused at build (ADR-0137).

A value computed from one waits on ruling 0073-a. Its representation,
`{id.value}`, is read as any opaque value's is (ADR-0232).

**An opaque value's representation is read by its own path** (ADR-0232):
`{p.id.value}` is `p.id`, as the checker types the base. A `.value` neither
the checker's types nor the value relations type the base of is read as a
field, and does not render.

The store has no change event of its own, so its title, like its heading,
changes when the page's values are next read. A title, or metadata, that
reads a member function is refused where the page is planned: a host
computes one in a text part of the page's body, or a loop's row.

**The runtime is sent as it is written** (ADR-0188): comments and all, about
45% of its script's bytes. Its bounds are on Brotli's compression of it, as a
static host sends it. Minifying it waits on the owner (a minifier is a
download). The development server compresses nothing.

**The store contains a menu item only far down its page** (ADR-0187): when
28 items precede it in its list, or 16 lists precede its list. That is at
least 2,400 CSS pixels at 16 px text, in a single column. Not covered:
- a grid of items, which puts the 29th item higher;
- a first screen taller than 2,400 CSS pixels;
- sixteen categories with no items, about 1,300 px;
- text smaller than 16 px.

**A page's metadata is a name and a content** (ADR-0186). A page states its
description and Open Graph properties with `<meta>` at the top of its view.
They are written into the head as the page is served, and set again by
nothing. Not supported:
- `media` and `lang` on metadata, which are refused. So a page has one
  `theme-color` for light and dark alike.
- A `<link>` in the head: a canonical URL, an alternate language, an icon.
- Structured data as JSON-LD.
- Microdata's own rules. A `<meta itemprop>` is written where it is, and
  whether it is inside an item is not read.

No page is required to state a description.

**A page writes a `<link>` only where HTML allows one in the body**
(ADR-0189): its relations body-ok, or an item's property. It cannot state a
canonical or an alternate address. A `<style>` in markup is written where it
is, into the body, where browsers apply it and HTML does not put it. Styling
is charter §8.3's.

**A page that is absent is answered with the host's own page** (ADR-0163).
`not_found_on` names one case of one declared error, and a host answers it
404 with a page that holds nothing of the program's. A program declares no
not-found view yet, for this or for an address no route names, which the
development server answers 404 as plain text. A keyed read that answers the
case after the page is shown fails as any read does.

**Ids and references are read in the declaration that renders them**
(ADR-0185), as a label is (below). Not read:
- a reference or an id across a page and the views it composes;
- what the program computes;
- whether an ARIA attribute is allowed on its element's role, and what a
  role requires: ADR-0182's audit reads these at run time;
- a `<label for>` that names an element that is not a control.

**An element's children are its elements, for nine elements** (ADR-0204).
PW5012 reads what `<ul>`, `<ol>`, `<dl>`, `<table>`, `<thead>`, `<tbody>`,
`<tfoot>`, `<tr>` and `<select>` hold as the page holds it: each element
written there, a block's rows and branches, and what a view renders at its
top. Not read: text in one of them, and the content models of the other
elements.

**A form control is named only in the declaration that renders it**
(ADR-0143). PW5014 matches a `<label for>`, a wrapping `<label>` and an
`aria-labelledby`, written as text in the same view or page. These are not
matched:
- a computed `for`, `id` or IDREF;
- an `id` inside `{#each}`, which names no single row.

Wrapping the control in its label is the repair for each. A name the
program computes, such as `aria-label={x}` or a label's interpolated text,
is taken as a name, though it may be empty when the page runs. A label in a
page for a field in a view is refused, though HTML allows it. `title` and a
placeholder name nothing, though axe-core accepts both.

**Test 14's audit is the store's page, by rules a test decides**
(ADR-0182). The spec reads the store in three engines, as served and after
each kind of change, by axe's and WCAG 2.2 AA's rules that apply to it.
Not covered:
- **axe-core itself.** It is not installed: that needs the owner's OK for a
  network install.
- **A person with a screen reader.** The procedure is in
  `docs/research/screen-reader-smoke-test.md`, and no one has run it yet.
- **Target size at AAA** (44 by 44 pixels). The store's buttons are the
  browsers' own, about 20 pixels tall.
- **Forced colors, text spacing, and text zoomed alone.**
- **The other pages**: the demos' and the kiokun slice's.
- **A phone's layout in Firefox**, which Playwright does not emulate.
- **Duplicate ids and ARIA references at build** are checked since
  ADR-0185, within the declaration that renders them.

**An element left open is reported at the end of the file** (ADR-0200). A
close tag closes the innermost element whatever its name, so in
`<main><div>A</main>` the `</main>` closes `<div>`, `<main>` takes every `}`
after it, and `pw check` says "unclosed block" where the file ends. A
`{#..}` block left open is named where it is (PW5019). Matching a close tag
to its element by name waits for its own ADR, beside HTML's optional end
tags.

**A string's hole cannot hold a string** (ADR-0200). `"{f("a")}"` does not
parse; write the value to a binding first. Swift's and Kotlin's
interpolations read one.

**A label does not follow a value through storage, a later call of a
function value, or time** (ADR-0129). A value's label follows it through
calls, bodies, branches and assignments, and a public log takes only a public
value. Not tracked:
- a database write inside a secret branch: writes are not a sink the charter
  names, and a stored value's label is not carried through storage;
- a function value bound to a name and called elsewhere: it carries what it
  computes (ADR-0079), not the conditions of where it is called;
- how long a branch takes.
No program lowers a label except a host binding, whose signature is its
contract; an audited `declassify` waits for the first program that needs
one.

**The development server honours query policies except one** (ADR-0127,
ADR-0152): `consistency`, which ADR-0207 holds to what its source reads,
and nothing at run time enforces as such. `on_key_change`
runs since ADR-0152, for a binding a page's signal keys. Freshness, cache partition, key, retries,
timeout and one-flight-per-key are `pw-resource`'s, and a commit drops exactly
what it invalidates. **Its sandbox is checked, not
enforced** (ADR-0124, E14-I): no hidden file is copied in, and a process in
it can read the repository by absolute path.

**Every page's resume manifest names the document schema `cart-doc`**
(found by ADR-0220), and so does the browser's resume decision. The two
agree for every program, so the check compares a constant with itself and
tells no document from another.

**A `<select>` is not bound to a signal, and a `<textarea>`'s value is a
signal or text** (ADR-0221, PW5036). A select's chosen option is marked
`selected={..}` until a computed attribute in a row, from a signal (ADR-0227,
ADR-0228), can mark the one whose value a signal holds. A textarea a query fills waits for a patch the runtime
applies to its default value.

**A pending post is by "You"** (ADR-0222): a transition sees the value it
changes and its command's arguments, not the page's own user. A value of a
type that contains itself is speculated on as its nodes (ADR-0233), and a
view's instance given one is rendered again with it (ADR-0234). A target keyed
by a command's parameter matches a page's binding keyed by the page's parameter
its handlers pass unchanged (ADR-0236); one passed through a name the handler
binds, or from a composed view, is refused by name. An arm whose target's
resource a page does not show is not the page's, and the page waits for the
server on it (ADR-0238); one whose resource it shows under another key
refuses the page.

**A commit reaches another session's open page by the query it reads, not
by its key** (ADR-0219). A like drops one post's `Thread`, and every open
thread page is read again; each other one is sent no change, from the cache.
A reply is a post, and drops every `Thread` (ADR-0231).

**E7 gate 8 is unstable on this machine** (2026-10-02, E7-G8). About half
of runs see one long animation frame of 52-63 ms, with no script attributed
and 4-6 ms blocking, at HEAD and at `6545029` alike. `just e10-bench` stops at
that gate; `just e10-close-bench` records every run instead. The detector
counts a frame no page script made long, and whether it should is not decided
([control](evidence/E10/gate8-control-2026-10-02.md)).

**Ignored tests are not run by CI.** Benchmarks such as
`runtime/pw-host/tests/bench.rs` are `#[ignore]`d and need artifacts `just ci`
does not build. ADR-0115 broke one of them (world-level exports, fixed in
`0ad72c7`), and only re-recording E10's gate found it.

## Historical measurement interpretation

ADR-0027 supersedes the inference that an undefined frame-level
`forcedStyleAndLayoutDuration` establishes browser non-support. That value is
read from script-attribution records by the repaired instrumentation. Missing
observations and observed zero must remain distinct. Old raw measurements are
preserved, not replaced with invented results.
