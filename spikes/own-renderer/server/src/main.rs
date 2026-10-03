//! E7-P's development host.
//!
//! **Not E8's production host.** Its deletion condition is explicit: E8
//! replaces it with the capability-constrained production host while preserving
//! the `pw-protocol` browser boundary. Saying that plainly is what stops a
//! temporary adapter quietly becoming architecture.
//!
//! # What it exists to remove
//!
//! ```text
//! before                        after
//!
//! browser                       browser
//!   ↓                             ↓ pw-protocol
//! Node spike                    Rust dev server
//!   ↓                             ├── pw-resource
//! hand-built approximations       ├── pw-materialize
//!                                 ├── pw-render
//!                                 └── the compiler's emitted graph
//! ```
//!
//! The Node spike reconstructed E6's semantics in JavaScript — a cart counter,
//! an events array, a version integer. Every one of those was a second
//! implementation of something the Rust runtime already owned, and validating
//! an internal service boundary that exists to be deleted is wasted work.
//!
//! # What the browser knows
//!
//! `ResourceEntryId`, `Version`, `PartAddress`, `StreamFrame`, `Patch`. Not
//! `EntryKey`, not SQLite, not outbox rows, not compiler graph nodes. The
//! boundary is asserted, not assumed — see `e2e/protocol-boundary.spec.mjs`.
//!
//! # Its commands are compiled Pleris (E10-I)
//!
//! `add_to_cart` and `clear_cart` run as the components the compiler built
//! from `examples/store/app.pw` (`docs/evidence/E10/<component id>.wasm`),
//! admitted against each artifact's real imports and executed through the E8
//! host's engine. This server supplies only the DEPLOYMENT: the platform's
//! session operation, and `store:data/carts`, the data layer whose staged write
//! commits with its event only when the compiled command returns `Ok`. The Rust
//! closures that computed the commands are deleted, and
//! `the_commands_run_as_compiled_components` keeps them deleted.
//!
//! # Deliberately not
//!
//! A deployment abstraction, a production authentication system, a plugin host,
//! a Wasm executor of its own (the one it uses is `pw-host`'s), an HTTP/3
//! experiment, or a distributed materializer. This development deployment
//! explicitly models every local session as `SignedIn` so the store demo can
//! exercise command authorization; any other predicate is refused. Real identity
//! verification belongs to the deployment, not to this test server.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use pw_document::{IdentityDomain, LocalPartId, PartAddress, Partition, TemplateSchemaId};
use pw_host::engine::{HostFn, Val};
use pw_host::{Admission, ComponentContract, Granted, Limits, Node, Topology, admit};
use pw_materialize::{Clock, EntryKey, FragmentPolicy, Materializer};
use pw_protocol::{
    CURRENT, CausalBasis, Patch, PatchOp, PatchSet, Recovery, ResourceEntryId, StreamFrame,
    Targeted, Version,
};
use pw_render::{Env, PartId, Settled, Template, Value};
use pw_resource::{DevelopmentIdentityKey, EntryIdentity};

/// One subscriber's undelivered frames, and where its cursor is.
///
/// # Why frames are not removed when they are read
///
/// The first transport drained the queue on read and wrote the frames to the
/// socket. A page that reloaded left an in-flight long poll behind; that
/// request's thread woke, took the frames the reload had not yet asked for,
/// wrote them to a socket nobody was reading, and returned. The frames were
/// gone and the new page waited forever — reproducibly, and only when a
/// reload raced a change, which is why it looked like flakiness.
///
/// So delivery is acknowledged rather than assumed. Each frame carries a
/// sequence number; a subscriber asks for everything after the last sequence
/// it APPLIED; and the server drops a frame only once a later request proves
/// the client got past it. Writing to a dead socket now loses nothing.
///
/// # Why a subscriber is bounded, and forgotten
///
/// A frame leaves the queue only when the subscriber acknowledges it, and a
/// menu change is pushed to every subscriber. So a visitor who closed the tab
/// kept every later change forever: 1,000 visitors and 300 menu changes held
/// 600,000 frames (`docs/evidence/E10/load.txt`). A queue now holds at most
/// [`MAX_WAITING`] frames. A subscriber that falls further behind gets one
/// `Recovery::Reload` instead, the protocol's answer to "this document cannot
/// continue", and nothing more until it is re-rendered. A subscriber that has
/// not asked for [`IDLE`] is forgotten, and if it asks again it is told to
/// reload, because what it missed can no longer be said.
struct Subscriber {
    /// The last sequence assigned. Sequences start at ONE, so that the
    /// initial cursor — zero, meaning "nothing acknowledged yet" — is smaller
    /// than every frame. With zero-based sequences the very first frame a
    /// subscriber ever received was numbered zero, `since=0` read it as
    /// already acknowledged, and it was never delivered. It cost a real
    /// invalidation and looked like a transport that simply had nothing to say.
    ///
    /// Serving a document takes a sequence number too, so a served page's
    /// cursor is never zero. A poll with a nonzero cursor from a subscriber
    /// the server has forgotten is then known to be a page that missed
    /// frames, never mistaken for a new one.
    last_seq: u64,
    frames: Vec<(u64, StreamFrame)>,
    /// When it last asked for frames or was served a document.
    seen: std::time::Instant,
    /// It fell more than [`MAX_WAITING`] frames behind: its queue is one
    /// `Recovery::Reload`, and further frames are dropped until it is
    /// re-rendered.
    behind: bool,
    /// **Every frame pushed to it, kept or dropped** (ADR-0151). A document
    /// is read outside the table, and this says whether a change reached its
    /// session meanwhile. Not `last_seq`, which a document's cursor moves
    /// too: two documents read at once would each make the other read again.
    pushed: u64,
}

impl Default for Subscriber {
    fn default() -> Subscriber {
        Subscriber::at(0)
    }
}

impl Subscriber {
    /// A document's subscriber, its sequence starting at the document's
    /// cursor (ADR-0161): every frame for it comes after the document.
    fn at(cursor: u64) -> Subscriber {
        Subscriber {
            last_seq: cursor,
            frames: Vec::new(),
            seen: std::time::Instant::now(),
            behind: false,
            pushed: 0,
        }
    }
}

/// How long the kitchen takes to answer (E14, T02): long enough that pages
/// opened together are asking at once.
const KITCHEN_MS: u64 = 300;

/// The most frames one subscriber may have waiting.
const MAX_WAITING: usize = 256;

/// **What a command answered** (ADR-0157): whether it committed, and its
/// value in the wire form a handler's compiled code decodes, `Ok` or a
/// declared `Err`. A command that trapped or was refused answered no value,
/// and `why` says what happened instead.
#[derive(Debug, Clone, PartialEq)]
struct Answered {
    committed: bool,
    result: Option<serde_json::Value>,
    why: Option<String>,
}

impl Answered {
    /// The answer as an interaction keeps it, so a retried request is given
    /// the first one's (ADR-0121). `result` is written only when there is
    /// one: a command declaring no `Result` answers `null`, which is an
    /// answer, and a command that trapped answers none.
    fn kept(&self) -> String {
        let mut kept = serde_json::json!({ "committed": self.committed, "why": self.why });
        if let Some(result) = &self.result {
            kept["result"] = result.clone();
        }
        kept.to_string()
    }

    fn from_kept(kept: &str) -> Answered {
        let v: serde_json::Value = serde_json::from_str(kept).unwrap_or_default();
        Answered {
            committed: v["committed"] == true,
            result: v.get("result").cloned(),
            why: v["why"].as_str().map(str::to_string),
        }
    }
}

/// **A document's keyed reads** (ADR-0152, ADR-0161): each binding a signal
/// keys, by its name.
#[derive(Default)]
struct Keyed {
    reads: BTreeMap<String, KeyRead>,
}

/// **One binding's reads** (ADR-0152).
#[derive(Default)]
struct KeyRead {
    /// The latest read the browser asked for, by its number.
    latest: u64,
    /// The key the page shows: each signal's value in the last read applied.
    /// A signal absent here shows its first value.
    shown: BTreeMap<String, serde_json::Value>,
    /// A `cancel` read's hold on its key's flight, with the read's number.
    /// Let go when a newer read is asked or the browser leaves, so that
    /// `pw-resource` stops the flight when nobody else holds it.
    hold: Option<(u64, pw_resource::Subscription<Arc<Val>>)>,
    /// **One `keep` read at a time**: each waits for the one before it.
    turn: Arc<Mutex<()>>,
}

/// **Which key a binding a signal keys is read for** (ADR-0152).
enum Keys<'a> {
    /// A new document's: each signal's first value.
    First,
    /// What the session's page shows: the key last applied for each binding.
    Shown,
    /// A read the browser asked for: this binding's new key, the others as
    /// shown.
    Asked {
        binding: &'a str,
        key: &'a BTreeMap<String, serde_json::Value>,
    },
}

/// What came of a keyed read (ADR-0152).
#[derive(Debug, PartialEq, Eq)]
enum KeyOutcome {
    /// It was the latest, and its patch set is in the session's frames.
    Applied,
    /// A newer read was asked, its page was replaced, or it was let go:
    /// nothing it read is applied.
    Superseded,
}

/// Whether a read was stopped: a flight's cancellation, as a source sees it.
type Stopped = Arc<dyn Fn() -> bool + Send + Sync>;

/// How long the menu's slow category takes, and which it is (E14, T07): a
/// test sets them through `/bench/category`.
#[derive(Clone, Debug, Default)]
struct Categories {
    slow: Option<String>,
    delay_ms: u64,
}

/// How many times a document is read, at most (ADR-0151): each attempt but
/// the last outside the subscriber table, and one is repeated only when a
/// change reached its session while it was read.
const DOCUMENT_ATTEMPTS: u32 = 3;

/// How long a subscriber may go without asking before it is forgotten. The
/// long poll holds for one second and the stream for two, so a live page asks
/// far more often than this.
const IDLE: std::time::Duration = std::time::Duration::from_secs(120);

/// `{"cursor":..,"frames":[reload]}`: the batch a forgotten subscriber gets.
fn reload_batch(cursor: u64) -> String {
    let frame = StreamFrame::Recovery {
        protocol: CURRENT,
        recovery: pw_protocol::Recovery::Reload,
    };
    format!(
        "{{\"cursor\":{cursor},\"frames\":[{}]}}",
        serde_json::to_string(&frame).expect("frame")
    )
}

/// **A served document** (ADR-0161): its session, and its number, which is
/// also the cursor it was served at. Each is its own subscriber: what it
/// shows, its frames and its keyed reads are its own, so serving a session's
/// second tab leaves what its first was waiting for. Until 2026-10-03 a
/// session had one subscriber, and serving a document cleared it.
type Doc = (String, u64);

/// **A page's parameters**, as its address gives them (ADR-0162): `id`,
/// from `/stores/{id}`.
type Params = BTreeMap<String, String>;

/// Each of `session`'s documents in `map`, in the order they were served.
fn documents_of<V>(map: &BTreeMap<Doc, V>, session: &str) -> Vec<Doc> {
    map.range((session.to_string(), 0)..=(session.to_string(), u64::MAX))
        .map(|(doc, _)| doc.clone())
        .collect()
}

/// Forget every document that has not asked for [`IDLE`]. Says which, and
/// each session left with none, whose own state goes with it.
fn forget_idle(
    queue: &mut BTreeMap<Doc, Subscriber>,
    now: std::time::Instant,
) -> (Vec<Doc>, Vec<String>) {
    let idle: Vec<Doc> = queue
        .iter()
        .filter(|(_, w)| now.saturating_duration_since(w.seen) >= IDLE)
        .map(|(doc, _)| doc.clone())
        .collect();
    for doc in &idle {
        queue.remove(doc);
    }
    let mut gone: Vec<String> = idle
        .iter()
        .map(|(session, _)| session.clone())
        .filter(|session| documents_of(queue, session).is_empty())
        .collect();
    gone.dedup();
    (idle, gone)
}

impl Subscriber {
    fn push(&mut self, frame: StreamFrame) {
        self.pushed += 1;
        if self.behind {
            return;
        }
        self.last_seq += 1;
        if self.frames.len() >= MAX_WAITING {
            self.frames.clear();
            self.frames.push((
                self.last_seq,
                StreamFrame::Recovery {
                    protocol: CURRENT,
                    recovery: pw_protocol::Recovery::Reload,
                },
            ));
            self.behind = true;
            return;
        }
        self.frames.push((self.last_seq, frame));
    }

    /// Everything after `since`, and the cursor a client that applies it all
    /// should report next time.
    fn after(&self, since: u64) -> (u64, Vec<&StreamFrame>) {
        let frames: Vec<&StreamFrame> = self
            .frames
            .iter()
            .filter(|(s, _)| *s > since)
            .map(|(_, f)| f)
            .collect();
        let cursor = self
            .frames
            .iter()
            .map(|(s, _)| *s)
            .filter(|s| *s > since)
            .max()
            .unwrap_or(since);
        (cursor, frames)
    }

    /// Forget what the client has acknowledged.
    fn acknowledge(&mut self, through: u64) {
        self.frames.retain(|(s, _)| *s > through);
    }
}

/// The build this server serves. One value, used as the compatibility
/// generation everywhere it is needed, so nothing derives a second one.
const BUILD: &str = "B1";

/// The deployment's PRF key, from a provider rather than a literal.
const IDENTITY: DevelopmentIdentityKey = DevelopmentIdentityKey;

/// **What a session's document shows** (ADR-0145): each part's text, by its
/// number, and each list its queries fill, item by item. A change is sent as
/// the difference from it, so a part that did not change is not patched and
/// an item whose key stayed keeps its nodes.
#[derive(Debug, Clone, Default)]
struct Shown {
    texts: BTreeMap<u32, String>,
    lists: BTreeMap<String, Vec<Value>>,
    /// Each block a query decides, as rendered (ADR-0146).
    blocks: BTreeMap<u32, String>,
}

/// **The recommender the store's data layer reaches** (ADR-0148): what
/// `store:data/recommendations#for-store` answers, after how long, and how it
/// fails. A test sets it through `/bench/recommendations`.
#[derive(Debug, Clone)]
struct Recommender {
    /// How long an answer takes. Charter §15.5: 1200 ms.
    delay_ms: u64,
    /// `declared`: the query's declared error, `NoneAvailable`. `host`: the
    /// call itself fails, as a source that is down does.
    fail: Option<String>,
    /// What it recommends, `(id, name)`.
    items: Vec<(String, String)>,
}

impl Default for Recommender {
    fn default() -> Recommender {
        Recommender {
            delay_ms: 1200,
            fail: None,
            items: vec![
                ("cortado".to_string(), "Cortado".to_string()),
                ("cold-brew".to_string(), "Cold Brew".to_string()),
            ],
        }
    }
}

/// **The estimator the store's data layer reaches, for one session** (E14,
/// T10): what `store:data/estimates#current` answers, after how long, and how
/// it fails. A test sets it through `/bench/estimate`.
#[derive(Debug, Clone)]
struct Estimator {
    /// How long an estimate takes. Charter §15.5: 400 ms.
    delay_ms: u64,
    /// `declared`: the query's declared error. Anything else: the call itself
    /// fails, as an estimator that is down does.
    fail: Option<String>,
    minutes: i64,
}

impl Default for Estimator {
    fn default() -> Estimator {
        Estimator {
            delay_ms: 400,
            fail: None,
            minutes: 25,
        }
    }
}

struct Server {
    /// The templates the compiler emitted, deserialized once.
    templates: Vec<Template>,
    /// The keyed collection E7-P's structural patches operate on.
    ///
    /// Mutable, because a list that never changes cannot demonstrate that a
    /// change preserves identity. Each item is `(key, name)` and the key is
    /// what `{#each menu as item key item.id}` declares.
    menu: Mutex<Vec<(String, String)>>,
    /// The materializer's clock, advanced once per regeneration.
    ///
    /// A version is `Entry.generated_at`, which is a clock reading — so a clock
    /// that never moves gives every entry version 0 and the browser's staleness
    /// comparison compares nothing. Advancing here rather than inventing a
    /// counter keeps the version the MATERIALIZER's, which is the gate.
    clock: Clock,
    /// The one materializer. Versions come from here and from nowhere else.
    materializer: Materializer,
    /// Per-session cart lines — `(item, quantity)` — behind the
    /// materializer's command boundary. This is the deployment's DATA LAYER:
    /// `store:data/carts#add` is its operation, and the compiled command calls
    /// it through the host.
    carts: Mutex<BTreeMap<String, Lines>>,
    /// The graph the compiler emitted: what each command emits and what each
    /// entry listens for. Read once; a test replaces it.
    graph: pw_materialize::Graph,
    /// The compiled components, by component id, read from
    /// `docs/evidence/E10/` as the compiler wrote them. Data, like the
    /// contracts: this server links no compiler. Compiled once, with the
    /// artifact's imports read once — both are properties of the bytes.
    components: BTreeMap<String, Loaded>,
    /// The document's static assets.
    dist: std::path::PathBuf,
    /// **Where the compiler's browser artifacts are** (ADR-0123): the
    /// handler modules and speculation modules. `pw build`'s output directory
    /// for a running server; the document directory for the unit tests,
    /// which `run.sh` fills the same way.
    artifacts: std::path::PathBuf,
    /// **The store page's speculation manifest** (ADR-0122), as `pw
    /// emit-speculations` wrote it to `dist/speculations/`: which bindings
    /// the page speculates on, and so which entries' values it is sent.
    /// `None` when the build wrote none.
    speculation: Option<serde_json::Value>,
    /// **What the store page shows, as reads of what its queries return**
    /// (ADR-0125): `pw build`'s `pages/store.page.StorePage.json`. Each
    /// binding names the query component it runs; each part, the steps from
    /// that binding's value. Until 2026-10-02 this server computed the page's
    /// values itself and ran none of the store's queries.
    plan: serde_json::Value,
    /// Every page's plan, by its template path (ADR-0130): what a page
    /// whose values are its signals alone renders from.
    plans: BTreeMap<String, serde_json::Value>,
    /// **The store's queries, run by their declared policies** (ADR-0127):
    /// freshness, cache partition, key, concurrency, timeout and retries are
    /// `pw-resource`'s to decide, on its own clock, which this server keeps at
    /// wall time. It caches each value itself, and hands it to the fetch.
    ///
    /// Until 2026-10-02 it cached a token, and the value was in a table here
    /// bounded to four per key. Eight threads committing on one session could
    /// invalidate more than four flights between a reader being handed the
    /// cached token and reading its value, and the reader found it gone
    /// (`concurrent_commands_on_one_session_all_commit`, 1 run in 3).
    queries: pw_resource::Resources<Arc<Val>>,
    query_clock: (
        pw_resource::Clock,
        std::time::Instant,
        std::sync::atomic::AtomicU64,
    ),
    /// A number for each `concurrency parallel` flight's own key.
    parallel: std::sync::atomic::AtomicU64,
    /// How many times each data-layer operation ran: what `/metrics` reports,
    /// and what shows a policy saved a request.
    calls: Arc<Mutex<BTreeMap<String, u64>>>,
    /// **Frames waiting for each document** (ADR-0161).
    pending: Mutex<BTreeMap<Doc, Subscriber>>,
    /// **What each document shows** (ADR-0145, ADR-0161), as it was served
    /// and then patched: what a change is derived against. Locked after
    /// `pending` and `keyed`, never before.
    shown: Mutex<BTreeMap<Doc, Shown>>,
    /// **The next document's number** (ADR-0161), server-wide and from one,
    /// so a document's cursor is never zero.
    documents: std::sync::atomic::AtomicU64,
    /// **Each document's parameters** (ADR-0162), as its address gave them:
    /// which store it shows. Registered with its subscriber, before it is
    /// read, and forgotten with it.
    params: Mutex<BTreeMap<Doc, Params>>,
    /// **Each session's order, by its status's case** (E14, T04): what
    /// `store:data/orders#current` answers. The kitchen sets it through
    /// `/bench/order`; no order is `None`.
    orders: Mutex<BTreeMap<String, String>>,
    /// **The recommender** (ADR-0148): what a streamed region's query reads,
    /// with the delay and failure a test sets.
    recommender: Mutex<Recommender>,
    /// **Each session's estimator** (E14, T10), as a test set it; the default
    /// for a session no test has.
    estimators: Mutex<BTreeMap<String, Estimator>>,
    /// **What the store has posted on its notice board** (E14, T09): what
    /// `store:data/notices#current` answers. A test posts a notice through
    /// `/bench/notice`, and nothing is told it changed.
    notice: Mutex<String>,
    /// **How long the kitchen takes now, in minutes** (E14, T02): what
    /// `store:data/kitchen#prep-minutes` answers, after [`KITCHEN_MS`]. A
    /// test changes it through `/bench/prep`, and nothing is told.
    prep_minutes: Mutex<i64>,
    /// **The items sold out** (charter §15.4, §15.5's forced stale item): what
    /// `store:data/menus#is-available` answers no for, inside the command
    /// that adds one. A test sells one out through `/bench/stock`, and the
    /// page that shows it is not told.
    sold_out: Mutex<std::collections::BTreeSet<String>>,
    /// **Each document's keyed reads** (ADR-0152, ADR-0161).
    keyed: Mutex<BTreeMap<Doc, Keyed>>,
    /// **The menu's categories** (E14, T07): which is slow, and how slow.
    categories: Mutex<Categories>,
    /// How many reads of a category saw they were stopped, and ended early
    /// (E14, T07).
    category_stopped: Arc<std::sync::atomic::AtomicU64>,
    /// **The items each store's menu fragment was last rendered from**: a
    /// fragment is rendered again when the `Menu` query's value is not what
    /// it shows.
    menu_rendered_from: Mutex<BTreeMap<String, Vec<(String, String)>>>,
    /// **The compiler's contracts, and the node this server is.**
    ///
    /// E8's last gate item asks for the command path to go through the host
    /// rather than a Rust closure. This is the half that is real today: the
    /// authority to run a command is DECIDED — `admit` against a topology,
    /// producing a `Granted` or a refusal — instead of being ambient because
    /// the function happens to be callable.
    ///
    /// The BODY is no longer a Rust closure (E10-I, 2026-09-24): each command
    /// runs as the component the compiler built from its `.pw` declaration,
    /// admitted against the imports the artifact actually has.
    contracts: Vec<ComponentContract>,
    topology: Topology,
    /// **Each idempotent command's outcome, by interaction** (ADR-0121).
    ///
    /// `pw-resource`'s reservation: a key is reserved before its command
    /// runs, a duplicate waits for and shares the first outcome, and an
    /// unknown outcome is never re-executed. The key binds the interaction to
    /// its session and its command; [`Server::interactions`] binds it to the
    /// arguments and bounds how many are kept.
    commands: pw_resource::Resources,
    /// Per session, the interaction keys held in `commands`, oldest first,
    /// each with the arguments it was first sent with.
    interactions: Mutex<BTreeMap<String, std::collections::VecDeque<(String, String)>>>,
}

/// How many interactions' outcomes one session keeps (ADR-0121). A retry of
/// an interaction older than this many later ones runs again: the bound's
/// cost, stated rather than unbounded memory.
const INTERACTIONS_PER_SESSION: usize = 64;

/// The semantic identity of one session's cart.
///
/// Built from the shape the compiler's bridge produces. The server does not
/// invent it: `EntryIdentity` is the one answer to "which entry", and a second
/// construction here would be the divergence the split exists to prevent.
fn cart_identity(session: &str) -> EntryIdentity {
    EntryIdentity::new(
        "store.page.Cart",
        &[session],
        Partition::Session {
            id: session.to_string(),
        },
    )
    .generation(BUILD)
}

/// A store's menu's entry identity — PUBLIC, and carrying the same
/// generation a session-scoped entry does. Partition and compatibility are
/// orthogonal. One per store (ADR-0162): a change to one store's menu is not
/// another's.
fn menu_identity(store: &str) -> EntryIdentity {
    EntryIdentity::new("store.page.Menu", &[store], pw_resource::Partition::Public)
        .generation(BUILD)
}

/// The parameters of the store page for the store `id` (ADR-0162).
fn store_params(id: &str) -> Params {
    BTreeMap::from([("id".to_string(), id.to_string())])
}

fn cart_entry(session: &str) -> ResourceEntryId {
    ResourceEntryId::derive(&cart_identity(session), &IDENTITY)
}

/// **The compiler's contracts, as data.**
///
/// Read from `docs/evidence/E8/component-contracts.json`, which `just
/// e8-contracts` regenerates from the store demo. ADR-0018: neither side links
/// the other, so this is the artifact and not a call into the compiler.
///
/// Panics rather than defaulting to an empty list. An empty list means every
/// command has no contract and is refused, which looks like a security posture
/// and is actually a missing file.
/// **The compiled components**, as the compiler wrote them.
///
/// `just e10-component` writes `docs/evidence/E10/<component id>.wasm`, named
/// by the contract's own id so nothing here derives a name. Panics on a
/// missing artifact rather than running without it: a command with no
/// compiled body has no body at all now.
/// A cart's lines, `(item, quantity)`, as the data layer holds them.
type Lines = Vec<(String, i64)>;

/// One compiled component: ready to run, and what its artifact imports.
struct Loaded {
    prepared: pw_host::engine::Prepared,
    imports: Vec<String>,
}

#[cfg(test)]
fn components() -> BTreeMap<String, Loaded> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/evidence/E10");
    let mut out = BTreeMap::new();
    for id in [
        "store.page.add_to_cart",
        "store.page.clear_cart",
        "store.page.Store",
        "store.page.Menu",
        "store.page.Cart",
        "domain.line_count",
    ] {
        let path = dir.join(format!("{id}.wasm"));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "{}: {e}\nrun `just e10-component` to compile the store's commands",
                path.display()
            )
        });
        let imports = pw_host::engine::imports_of(&bytes)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let prepared = pw_host::engine::Prepared::compile(&bytes)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        out.insert(id.to_string(), Loaded { prepared, imports });
    }
    out
}

/// **What one build gave the server** (ADR-0123, ADR-0125): where its
/// browser artifacts are, and what it runs the store by.
struct Built {
    artifacts: std::path::PathBuf,
    templates: Vec<Template>,
    contracts: Vec<ComponentContract>,
    components: BTreeMap<String, Loaded>,
    graph: pw_materialize::Graph,
    plan: serde_json::Value,
    /// Every page's plan, by its template path (ADR-0130).
    plans: BTreeMap<String, serde_json::Value>,
}

/// Every component a build wrote, by its file name (ADR-0123).
fn components_in(dir: &std::path::Path) -> Result<BTreeMap<String, Loaded>, String> {
    let mut out = BTreeMap::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let Some(id) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".wasm"))
        else {
            continue;
        };
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let imports =
            pw_host::engine::imports_of(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let prepared = pw_host::engine::Prepared::compile(&bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        out.insert(id.to_string(), Loaded { prepared, imports });
    }
    Ok(out)
}

#[cfg(test)]
fn contracts() -> Vec<ComponentContract> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/evidence/E8/component-contracts.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}\nrun `just e8-contracts` to regenerate the contracts",
            path.display()
        )
    });
    ComponentContract::from_json(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **The node this dev server is.**
///
/// An origin with the five capabilities the store demo's contracts require —
/// written out rather than derived from those contracts, which is the whole
/// point: a topology derived from what a program asks for grants everything
/// every program asks for, and admission becomes a formality.
///
/// `docs/evidence/E8/artifact-audit.txt` is the same shape against real
/// components.
fn dev_topology() -> Topology {
    Topology {
        nodes: vec![Node {
            name: "dev-origin".to_string(),
            world: "origin".to_string(),
            grants: [
                "database.read<Carts>",
                "database.read<Menus>",
                // A session's order (E14, T04).
                "database.read<Orders>",
                "database.read<Stores>",
                "database.write<Carts>",
                // The recommender (ADR-0148), a source reached over the network.
                "network.fetch",
                // A session's delivery estimate (E14, T10).
                "database.read<Estimates>",
                // The store's notice board (E14, T09).
                "database.read<Notices>",
                // The kitchen's prep time (E14, T02).
                "database.read<Kitchen>",
                // The menu by category (E14, T07).
                "database.read<Categories>",
                // 2026-08-10. The store gained the `import context.{
                // current_session }` it had been missing since E4, so the page
                // and both commands read the session. A dev origin that does
                // not publish it refuses all three — which is admission
                // working, and is what this list not being derived from the
                // contracts is for.
                "session.read",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }],
    }
}

impl Server {
    /// **The server for what `pw build` built** (ADR-0123): its templates,
    /// contracts, components, graph, handlers and speculations, all from one
    /// build directory. Until 2026-10-02 the commands and contracts came from
    /// `docs/evidence/` and the graph was compiled into this binary, so a
    /// program changed and rebuilt kept running its old commands.
    fn from_build(dist: std::path::PathBuf, build: std::path::PathBuf) -> Result<Server, String> {
        let read = |rel: &str| {
            std::fs::read_to_string(build.join(rel))
                .map_err(|e| format!("{}: {e}", build.join(rel).display()))
        };
        let templates: Vec<Template> = serde_json::from_str(&read("templates.json")?)
            .map_err(|e| format!("templates.json: {e}"))?;
        let contracts = ComponentContract::from_json(&read("contracts.json")?)
            .map_err(|e| format!("contracts.json: {e}"))?;
        let graph = pw_materialize::Graph::from_json(&read("graph.json")?)
            .map_err(|e| format!("graph.json: {e:?}"))?;
        let components = components_in(&build.join("components"))?;
        let plan: serde_json::Value =
            serde_json::from_str(&read("pages/store.page.StorePage.json")?)
                .map_err(|e| format!("pages/store.page.StorePage.json: {e}"))?;
        let mut plans = BTreeMap::new();
        for entry in std::fs::read_dir(build.join("pages")).map_err(|e| format!("pages: {e}"))? {
            let path = entry.map_err(|e| format!("pages: {e}"))?.path();
            let Some(page) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{page}: {e}"))?;
            plans.insert(
                page.to_string(),
                serde_json::from_str(&text).map_err(|e| format!("{page}: {e}"))?,
            );
        }
        Ok(Server::with(
            dist,
            dev_topology(),
            Built {
                artifacts: build,
                templates,
                contracts,
                components,
                graph,
                plan,
                plans,
            },
        ))
    }

    /// The server on the committed artifacts, on a topology the caller chooses.
    ///
    /// Exists so a test can run the command path against a node that grants
    /// nothing. A refusal that only ever happens in production is a refusal
    /// nobody has seen work.
    #[cfg(test)]
    fn on(dist: std::path::PathBuf, templates: Vec<Template>, topology: Topology) -> Server {
        Server::with(
            dist.clone(),
            topology,
            Built {
                artifacts: dist,
                templates,
                contracts: contracts(),
                components: components(),
                graph: pw_materialize::Graph::from_json(GRAPH).expect("the committed graph parses"),
                plan: serde_json::from_str(include_str!(
                    "../../../../docs/evidence/E10/pages/store.page.StorePage.json"
                ))
                .expect("the committed page plan parses"),
                plans: BTreeMap::new(),
            },
        )
    }

    fn with(
        dist: std::path::PathBuf,
        topology: Topology,
        Built {
            artifacts,
            templates,
            contracts,
            components,
            graph,
            plan,
            plans,
        }: Built,
    ) -> Server {
        let speculation = speculation_manifest(&artifacts);
        let query_clock = pw_resource::Clock::new();
        let clock = Clock::new();
        let materializer = Materializer::new(clock.clone(), BUILD);
        materializer.declare("store.page.Cart", FragmentPolicy::default());
        materializer.declare("store.page.Menu", FragmentPolicy::default());
        Server {
            templates,
            clock,
            materializer,
            graph,
            carts: Mutex::new(BTreeMap::new()),
            components,
            menu: Mutex::new(default_menu()),
            dist,
            artifacts,
            pending: Mutex::new(BTreeMap::new()),
            shown: Mutex::new(BTreeMap::new()),
            documents: std::sync::atomic::AtomicU64::new(1),
            params: Mutex::new(BTreeMap::new()),
            orders: Mutex::new(BTreeMap::new()),
            recommender: Mutex::new(Recommender::default()),
            estimators: Mutex::new(BTreeMap::new()),
            notice: Mutex::new("Open until 7 pm".to_string()),
            prep_minutes: Mutex::new(12),
            sold_out: Mutex::new(std::collections::BTreeSet::new()),
            keyed: Mutex::new(BTreeMap::new()),
            categories: Mutex::new(Categories::default()),
            category_stopped: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            menu_rendered_from: Mutex::new(BTreeMap::new()),
            contracts,
            topology,
            speculation,
            plan,
            plans,
            queries: pw_resource::Resources::caching(query_clock.clone()),
            query_clock: (
                query_clock,
                std::time::Instant::now(),
                std::sync::atomic::AtomicU64::new(0),
            ),
            parallel: std::sync::atomic::AtomicU64::new(0),
            calls: Arc::default(),
            commands: pw_resource::Resources::new(pw_resource::Clock::new()),
            interactions: Mutex::new(BTreeMap::new()),
        }
    }

    /// **May this component run here, and with what?**
    ///
    /// One `admit` call, and no other source of truth. The command path calls
    /// this before doing anything, so authority is decided rather than assumed
    /// — the difference between a command that is callable and a command that
    /// is permitted.
    fn authorise(&self, component_id: &str) -> Result<Granted, String> {
        let Some(c) = self
            .contracts
            .iter()
            .find(|c| c.component_id == component_id)
        else {
            // A command with no contract is not a command this build produced.
            // Refused rather than run: "I have no record of this" and "this is
            // fine" must never be the same answer.
            return Err(format!("no contract for `{component_id}`"));
        };
        let node = &self.topology.nodes[0].name;
        // **The artifact's own imports**, read from the component the host is
        // about to run — not a list anyone wrote down. Until E10-I the body
        // was a Rust closure and the audit was given nothing.
        let actual = match self.components.get(component_id) {
            Some(loaded) => loaded.imports.clone(),
            None => return Err(format!("no compiled component for `{component_id}`")),
        };
        match admit(c, &self.topology, node, &actual) {
            Admission::Admit { .. } => {
                let a = admit(c, &self.topology, node, &actual);
                Granted::from(&a, &BTreeMap::new())
                    .ok_or_else(|| format!("`{component_id}` was admitted but granted nothing"))
            }
            Admission::Refuse(refusals) => Err(format!(
                "`{component_id}` may not run on `{node}`: {refusals:?}"
            )),
        }
    }

    /// The storage key, derived from the same identity the wire id is.
    fn cart_key(&self, session: &str) -> EntryKey {
        EntryKey::from_identity(&cart_identity(session))
    }

    /// How many items the data layer holds for the session: the tests' view
    /// of the state. The page's count is `domain.line_count`'s (ADR-0125).
    #[cfg(test)]
    fn cart_value(&self, session: &str) -> i64 {
        self.carts
            .lock()
            .expect("carts")
            .get(session)
            .map(|lines| lines.iter().map(|(_, q)| q).sum())
            .unwrap_or(0)
    }

    /// **Run one compiled command through the host.**
    ///
    /// Authority first — `admit` against the artifact's real imports — then
    /// the component, with the deployment's implementation of each granted
    /// operation. The command's result is whatever the COMPONENT returns.
    fn run(
        &self,
        component_id: &str,
        session: &str,
        host: &BTreeMap<String, HostFn>,
        args: &[Val],
    ) -> Result<Vec<Val>, String> {
        let granted = self.authorise(component_id)?;
        let contract = self
            .contracts
            .iter()
            .find(|c| c.component_id == component_id)
            .ok_or_else(|| format!("no contract for `{component_id}`"))?;
        let export = contract.exports[0]
            .component
            .clone()
            .ok_or_else(|| format!("`{component_id}`'s contract does not locate its export"))?;
        self.components
            .get(component_id)
            .ok_or_else(|| format!("no compiled component `{component_id}`"))?
            .prepared
            .call_authorized_within(
                contract,
                &granted,
                &Limits {
                    fuel: Some(50_000_000),
                    memory_bytes: Some(16 * 1024 * 1024),
                    table_elements: Some(1_000),
                },
                host,
                &[&export.interface, &export.function],
                args,
                |predicate, _bound| match predicate {
                    // This is a DEVELOPMENT identity model, not authentication:
                    // every session the local server issued is the signed-in demo
                    // principal. The important property here is that `requires`
                    // is evaluated explicitly and an unknown predicate cannot run.
                    "SignedIn" if !session.is_empty() => Ok(true),
                    "SignedIn" => Ok(false),
                    other => Err(format!(
                        "the development deployment has no authorization predicate `{other}`"
                    )),
                },
            )
    }

    /// The platform's session operation: the request's session, and nothing
    /// else the component could ask it for.
    fn session_operation(session: &str) -> HostFn {
        let session = session.to_string();
        Arc::new(move |_args: &[Val]| Ok(vec![Val::String(session.clone())]))
    }

    /// The materializer's version for this entry.
    ///
    /// Zero when nothing has been materialized yet. There is no counter here;
    /// asking the materializer is the only way the server learns a version.
    fn version(&self, session: &str) -> Version {
        let key = self.cart_key(session);
        Version(
            self.materializer
                .entry(&key)
                .map(|e| e.generated_at)
                .unwrap_or(0),
        )
    }

    /// **Run a command the compiler built, by its component id.**
    ///
    /// No code here knows which command it is running. The component runs
    /// its compiled body, for example
    ///
    /// ```text
    /// command add_to_cart(item: MenuItemId, quantity: PositiveInt) -> Result<Cart, CartError>
    /// {
    ///     Carts.add(current_session(), item, quantity)
    /// }
    /// ```
    ///
    /// and what this server supplies is the DEPLOYMENT: the platform's session
    /// operation, and [`Server::data_layer`], all of `store:data/carts`. The
    /// linker gives the component only what its contract was granted and its
    /// artifact imports. Every write STAGES; only if the component returns
    /// without an `Err` are the staged cart and its event committed together
    /// (ADR-0019), so a command that fails, or traps, commits nothing and the
    /// browser receives nothing.
    ///
    /// `fail` makes the data layer answer `cart-expired`, the rollback path the
    /// browser tests exercise.
    fn command(
        &self,
        component_id: &str,
        session: &str,
        args: &[Val],
        fail: bool,
    ) -> Result<(), String> {
        let answered = self.command_answered(component_id, session, args, fail)?;
        if answered.committed {
            return Ok(());
        }
        Err(format!(
            "{component_id} failed: {}",
            answered.result.unwrap_or_default()
        ))
    }

    /// [`Server::command`], and what the command answered (ADR-0157): its
    /// value, `Ok` or a declared `Err`, for the handler that called it. The
    /// outer error is a command that trapped or was refused, which answered
    /// no value.
    fn command_answered(
        &self,
        component_id: &str,
        session: &str,
        args: &[Val],
        fail: bool,
    ) -> Result<Answered, String> {
        // One lock across the call and the commit: two presses in the same
        // instant both read the lines, and one of them would be lost —
        // `lazy-handler.spec.mjs` clicks twice concurrently to find exactly that.
        {
            let mut carts = self.carts.lock().expect("carts");
            let current = carts.get(session).cloned().unwrap_or_default();
            let staged: Arc<Mutex<Option<Lines>>> = Arc::default();
            let mut host = Self::data_layer(session, current, staged.clone(), fail);
            host.insert(
                "pw:host/session#read".to_string(),
                Self::session_operation(session),
            );
            // Whether an item can be ordered now (charter §15.4), read inside
            // the command that adds it.
            let sold_out = self.sold_out.lock().expect("sold out").clone();
            host.insert(
                "store:data/menus#is-available".to_string(),
                Arc::new(move |args: &[Val]| match args {
                    [Val::String(item)] => Ok(vec![Val::Bool(!sold_out.contains(item))]),
                    other => Err(format!("menus#is-available received {other:?}")),
                }),
            );

            // What the command declares it emits (ADR-0104), computed before
            // it runs: a value this server cannot compute is refused, and a
            // refused command has written nothing.
            let events = declared_events(&self.graph, component_id, session)?;
            let out = self.run(component_id, session, &host, args)?;
            let result = out.first().map(answer_json);
            if let [Val::Result(Err(_))] = out.as_slice() {
                // A declared error: nothing commits, and the handler is told
                // which (ADR-0157).
                return Ok(Answered {
                    committed: false,
                    result,
                    why: None,
                });
            }
            let Some(lines) = staged.lock().expect("staged").take() else {
                // Nothing was written, so there is nothing to commit.
                return Ok(Answered {
                    committed: true,
                    result,
                    why: None,
                });
            };
            let total: i64 = lines.iter().map(|(_, q)| q).sum();
            let emitted = events.clone();
            self.materializer.command(|tx| {
                Materializer::set_state(tx, &format!("cart:{session}"), &total.to_string());
                Ok::<_, String>(events)
            })?;
            carts.insert(session.to_string(), lines);
            self.invalidate_queries(component_id, session, &emitted);
            // The materializer drains the committed event and regenerates the
            // entry it invalidates. The version moves because the RESOURCE
            // moved.
            drop(carts);
            self.drain(session);
            Ok(Answered {
                committed: true,
                result,
                why: None,
            })
        }
    }

    /// **A command whose arguments arrived from a browser, as JSON.**
    ///
    /// A compiled handler runs in the user's browser, so its arguments are a
    /// claim. They are typed by the command component's OWN parameters, read
    /// from the artifact by the host, and refused otherwise, before anything
    /// runs. The outer error is a malformed request (the arguments); the inner
    /// one is a command that ran and did not commit.
    ///
    /// An idempotent command (its contract's `idempotent_by`, ADR-0121) runs
    /// at most once per `interaction`: a retried request gets the first
    /// request's outcome, and nothing runs again. Without an interaction it is
    /// refused before anything runs, and so is an interaction sent again with
    /// other arguments.
    #[cfg(test)]
    fn command_json(
        &self,
        component_id: &str,
        session: &str,
        json: &[serde_json::Value],
        interaction: Option<&str>,
    ) -> Result<Result<(), String>, String> {
        let answered = self.command_answer(component_id, session, json, interaction)?;
        Ok(match answered.committed {
            true => Ok(()),
            false => Err(answered.why.unwrap_or_else(|| {
                format!(
                    "{component_id} failed: {}",
                    answered.result.unwrap_or_default()
                )
            })),
        })
    }

    /// [`Server::command_json`], and what the command answered (ADR-0157).
    /// A retried interaction is given the first request's answer, its value
    /// included.
    fn command_answer(
        &self,
        component_id: &str,
        session: &str,
        json: &[serde_json::Value],
        interaction: Option<&str>,
    ) -> Result<Answered, String> {
        let loaded = self
            .components
            .get(component_id)
            .ok_or_else(|| format!("no compiled component for `{component_id}`"))?;
        let export = self
            .contracts
            .iter()
            .find(|c| c.component_id == component_id)
            .and_then(|c| c.exports.first())
            .and_then(|e| e.component.clone())
            .ok_or_else(|| format!("`{component_id}`'s contract does not locate its export"))?;
        let args = loaded
            .prepared
            .arguments(&[&export.interface, &export.function], json)?;
        let answered = |run: Result<Answered, String>| match run {
            Ok(answered) => answered,
            Err(why) => Answered {
                committed: false,
                result: None,
                why: Some(why),
            },
        };
        let Some(key_type) = &export.idempotent_by else {
            return Ok(answered(self.command_answered(
                component_id,
                session,
                &args,
                false,
            )));
        };
        let id = interaction.ok_or_else(|| {
            format!(
                "`{component_id}` is idempotent_by {key_type}, and the request carries no interaction id"
            )
        })?;
        if id.is_empty()
            || id.len() > 128
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!("`{id}` is not an interaction id"));
        }
        let key = format!("{session}\u{1f}{component_id}\u{1f}{id}");
        let sent = serde_json::Value::Array(json.to_vec()).to_string();
        {
            let mut held = self.interactions.lock().expect("interactions");
            let queue = held.entry(session.to_string()).or_default();
            match queue.iter().find(|(k, _)| *k == key) {
                Some((_, first)) if *first != sent => {
                    return Err(format!(
                        "interaction `{id}` was first sent with other arguments"
                    ));
                }
                Some(_) => {}
                None => {
                    queue.push_back((key.clone(), sent));
                    while queue.len() > INTERACTIONS_PER_SESSION {
                        if let Some((oldest, _)) = queue.pop_front() {
                            self.commands.forget_command(&oldest);
                        }
                    }
                }
            }
        }
        let outcome = self
            .commands
            .try_command(&key, || {
                answered(self.command_answered(component_id, session, &args, false)).kept()
            })
            .map_err(|e| format!("interaction `{id}`'s outcome is unknown: {e:?}"))?;
        Ok(Answered::from_kept(&outcome))
    }

    /// **The deployment's data layer: `store:data/carts`, whole.**
    ///
    /// Every operation the interface declares, whichever command is running;
    /// the host links only those the component imports and was granted. Each
    /// write stages the session's new lines in `staged`, and nothing here
    /// commits. A command's `Ok` does that, in [`Server::command`]. The
    /// session an operation is passed must be the one the host gave the
    /// component, or the component is acting for someone else.
    fn data_layer(
        session: &str,
        current: Lines,
        staged: Arc<Mutex<Option<Lines>>>,
        fail: bool,
    ) -> BTreeMap<String, HostFn> {
        let expired = || {
            vec![Val::Result(Err(Some(Box::new(Val::Variant(
                "cart-expired".into(),
                None,
            )))))]
        };
        let op = |name: &'static str,
                  f: fn(&mut Lines, &[Val]) -> Result<(), String>|
         -> (String, HostFn) {
            let this_session = session.to_string();
            let current = current.clone();
            let staged = staged.clone();
            let run: HostFn = Arc::new(move |args: &[Val]| {
                let Some(Val::String(s)) = args.first() else {
                    return Err(format!("carts#{name} received {args:?}"));
                };
                if *s != this_session {
                    return Err(format!("carts#{name} was passed another session"));
                }
                if fail {
                    return Ok(expired());
                }
                let mut staged = staged.lock().expect("staged");
                let mut lines = staged.clone().unwrap_or_else(|| current.clone());
                f(&mut lines, &args[1..])?;
                let cart = cart_value(&lines);
                if name != "current" {
                    *staged = Some(lines);
                }
                Ok(vec![Val::Result(Ok(Some(Box::new(cart))))])
            });
            (format!("store:data/carts#{name}"), run)
        };
        BTreeMap::from([
            op("add", |lines, args| {
                let [Val::String(item), Val::S64(quantity)] = args else {
                    return Err(format!("carts#add received {args:?}"));
                };
                match lines.iter_mut().find(|(i, _)| i == item) {
                    Some((_, q)) => *q += quantity,
                    None => lines.push((item.clone(), *quantity)),
                }
                Ok(())
            }),
            op("clear", |lines, args| {
                if !args.is_empty() {
                    return Err(format!("carts#clear received {args:?}"));
                }
                lines.clear();
                Ok(())
            }),
            op("current", |_, args| {
                if !args.is_empty() {
                    return Err(format!("carts#current received {args:?}"));
                }
                Ok(())
            }),
        ])
    }

    /// **Forget the subscribers that stopped asking, and what was kept for
    /// them.** Their queues go, and so do their sessions' materialized cart
    /// entries: a returning page is served a new document, and `drain`
    /// regenerates a missing entry from state. 1,000 departed visitors had
    /// kept 1,001 entries (`docs/evidence/E10/load.txt`).
    ///
    /// In its own short hold of the subscriber table, released before any
    /// entry is evicted: `drain` takes the materializer and then the table,
    /// so evicting while holding the table would take them the other way.
    fn forget_idle_subscribers(&self) {
        let (documents, forgotten) = {
            let mut queue = self.pending.lock().expect("pending");
            forget_idle(&mut queue, std::time::Instant::now())
        };
        // What each forgotten document showed and read (ADR-0161), each in
        // its own hold: `read_keyed` takes `keyed` before `shown`.
        {
            let mut keyed = self.keyed.lock().expect("keyed");
            for doc in &documents {
                keyed.remove(doc);
            }
        }
        {
            let mut shown = self.shown.lock().expect("shown");
            for doc in &documents {
                shown.remove(doc);
            }
        }
        {
            let mut params = self.params.lock().expect("params");
            for doc in &documents {
                params.remove(doc);
            }
        }
        for session in forgotten {
            self.materializer.evict(&self.cart_key(&session));
            // And the query values kept for it alone (ADR-0127): a private
            // entry's key begins with its session.
            let own = format!("session={session}");
            let mine = |k: &str| k == own || k.starts_with(&format!("{own}\u{1f}"));
            self.queries.evict_where(|k| mine(&k.key));
            // And its interactions: a forgotten session's retry runs again,
            // which is the bound's stated cost (ADR-0121).
            let keys = self
                .interactions
                .lock()
                .expect("interactions")
                .remove(&session)
                .unwrap_or_default();
            for (key, _) in keys {
                self.commands.forget_command(&key);
            }
        }
    }

    /// **The deployment's catalogue** (ADR-0125): `store:data/stores#get` and
    /// `store:data/menus#for-store`, what the `Store` and `Menu` queries read.
    /// This server holds two stores (ADR-0162): 47, whose menu is the keyed
    /// list E7-P mutates, and 48. Any other is answered `not-found`.
    ///
    /// Its slow sources poll `stopped`, to end early a read nobody is
    /// waiting for (ADR-0152).
    fn catalog_within(&self, stopped: Option<Stopped>) -> BTreeMap<String, HostFn> {
        let menu = self.menu.lock().expect("menu").clone();
        let not_found = || {
            vec![Val::Result(Err(Some(Box::new(Val::Variant(
                "not-found".into(),
                None,
            )))))]
        };
        let get: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if store_named(id).is_some() => {
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::Record(vec![
                    ("id".into(), Val::String(id.clone())),
                    (
                        "name".into(),
                        Val::String(store_named(id).unwrap_or_default().into()),
                    ),
                    (
                        "hours".into(),
                        Val::Record(vec![
                            ("opens-minute".into(), Val::S64(7 * 60)),
                            ("closes-minute".into(), Val::S64(19 * 60)),
                        ]),
                    ),
                ])))))])
            }
            [Val::String(_)] => Ok(not_found()),
            other => Err(format!("stores#get received {other:?}")),
        });
        let for_store: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if store_named(id).is_some() => {
                let items = match id.as_str() {
                    STORE_ID => menu.clone(),
                    _ => second_menu(),
                };
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::List(
                    items
                        .iter()
                        .map(|(id, name)| {
                            Val::Record(vec![
                                ("id".into(), Val::String(id.clone())),
                                ("name".into(), Val::String(name.clone())),
                            ])
                        })
                        .collect(),
                )))))])
            }
            [Val::String(_)] => Ok(not_found()),
            other => Err(format!("menus#for-store received {other:?}")),
        });
        // What the recommender answers (ADR-0148), after its delay: a slow
        // source, which a streamed region does not wait for.
        let recommender = self.recommender.lock().expect("recommender").clone();
        let recommend: HostFn = Arc::new(move |args: &[Val]| {
            let [Val::String(_)] = args else {
                return Err(format!("recommendations#for-store received {args:?}"));
            };
            std::thread::sleep(std::time::Duration::from_millis(recommender.delay_ms));
            match recommender.fail.as_deref() {
                Some("host") => Err("the recommender is down".to_string()),
                Some(_) => Ok(vec![Val::Result(Err(Some(Box::new(Val::Variant(
                    "none-available".into(),
                    None,
                )))))]),
                None => Ok(vec![Val::Result(Ok(Some(Box::new(Val::List(
                    recommender
                        .items
                        .iter()
                        .map(|(id, name)| {
                            Val::Record(vec![
                                ("id".into(), Val::String(id.clone())),
                                ("name".into(), Val::String(name.clone())),
                            ])
                        })
                        .collect(),
                )))))]),
            }
        });
        // What the store has posted (E14, T09). Every call is counted, as
        // every data-layer call is: what `/bench/calls` reports.
        let posted = self.notice.lock().expect("notice").clone();
        let notice: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if id == STORE_ID => Ok(vec![Val::Result(Ok(Some(Box::new(
                Val::String(posted.clone()),
            ))))]),
            [Val::String(_)] => Ok(vec![Val::Result(Err(Some(Box::new(Val::Variant(
                "not-found".into(),
                None,
            )))))]),
            other => Err(format!("notices#current received {other:?}")),
        });
        // How long the kitchen takes now (E14, T02), after [`KITCHEN_MS`]:
        // a slow source. The minutes are the ones it was asked with.
        let minutes = *self.prep_minutes.lock().expect("prep minutes");
        let prep: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if id == STORE_ID => {
                std::thread::sleep(std::time::Duration::from_millis(KITCHEN_MS));
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::S64(minutes)))))])
            }
            [Val::String(_)] => Ok(not_found()),
            other => Err(format!("kitchen#prep-minutes received {other:?}")),
        });
        // The menu's items in a category (E14, T07), the slow category after
        // its delay. A read that is stopped meanwhile ends early, and says so:
        // what `/bench/calls` counts as stopped.
        let categories = self.categories.lock().expect("categories").clone();
        let in_menu = self.menu.lock().expect("menu").clone();
        let ended_early = self.category_stopped.clone();
        let in_category: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id), Val::String(category)] if id == STORE_ID => {
                if categories.slow.as_deref() == Some(category.as_str()) {
                    let until = std::time::Instant::now()
                        + std::time::Duration::from_millis(categories.delay_ms);
                    while std::time::Instant::now() < until {
                        if stopped.as_ref().is_some_and(|s| s()) {
                            ended_early.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            return Err("stopped: nobody is waiting for it".to_string());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::List(
                    in_menu
                        .iter()
                        .filter(|(item, _)| {
                            category.as_str() == "all" || category_of(item) == category.as_str()
                        })
                        .map(|(id, name)| {
                            Val::Record(vec![
                                ("id".into(), Val::String(id.clone())),
                                ("name".into(), Val::String(name.clone())),
                            ])
                        })
                        .collect(),
                )))))])
            }
            [Val::String(_), Val::String(_)] => Ok(not_found()),
            other => Err(format!("menus#in-category received {other:?}")),
        });
        BTreeMap::from([
            ("store:data/stores#get".to_string(), get),
            ("store:data/menus#for-store".to_string(), for_store),
            ("store:data/notices#current".to_string(), notice),
            ("store:data/kitchen#prep-minutes".to_string(), prep),
            ("store:data/menus#in-category".to_string(), in_category),
            (
                "store:data/recommendations#for-store".to_string(),
                recommend,
            ),
        ])
    }

    /// `pw-resource`'s clock, brought to wall time (ADR-0127).
    fn sync_query_clock(&self) {
        let (clock, started, seen) = &self.query_clock;
        let now = started.elapsed().as_millis() as u64;
        let before = seen.swap(now, std::sync::atomic::Ordering::SeqCst);
        if now > before {
            clock.advance(now - before);
        }
    }

    /// **A binding's query, run by its declared policies** (ADR-0127). The
    /// key is the arguments its `key` names (all of them when it names none),
    /// and a private entry's key is scoped by the session, as `pw-resource`
    /// requires of its host.
    fn fetch_binding(
        &self,
        session: &str,
        binding: &serde_json::Value,
        args: &[Val],
    ) -> Result<Val, Unread> {
        let resource = binding["resource"].as_str().unwrap_or_default();
        let answer = self.fetch_answer(session, binding, args)?;
        // A declared error the page says means it is not found (ADR-0163),
        // told from every other failure.
        if let Val::Result(Err(Some(e))) = &answer
            && let Val::Variant(case, _) = e.as_ref()
            && binding["not_found"]
                .as_array()
                .is_some_and(|cases| cases.iter().any(|c| c == case.as_str()))
        {
            return Err(Unread::NotFound(format!("{resource} answered {case}")));
        }
        Ok(unwrapped(resource, answer)?)
    }

    /// **A query's answer, run by its declared policies** (ADR-0127): its
    /// whole result, `Ok` or `Err`, as a stream shows it (ADR-0148). A
    /// declared error is given to each reader waiting on the flight that
    /// answered it, and is not kept, as a host's failure is not: the next
    /// reader asks again.
    fn fetch_answer(
        &self,
        session: &str,
        binding: &serde_json::Value,
        args: &[Val],
    ) -> Result<Val, String> {
        let key = self.answer_key(session, binding, args)?;
        // Held while it runs (ADR-0152): `pw-resource` stops a flight when
        // its last holder lets go, and a keyed read's `cancel` is a holder
        // letting go. Every other reader holds it too, so a flight is stopped
        // only when nobody is waiting for it.
        let _held = self.queries.subscribe(&key);
        self.fetch_answer_at(session, binding, args, &key)
    }

    /// **The `pw-resource` key a binding's read of `args` runs under**: its
    /// entry's, or for `concurrency parallel` one of its own.
    fn answer_key(
        &self,
        session: &str,
        binding: &serde_json::Value,
        args: &[Val],
    ) -> Result<pw_resource::Key, String> {
        let resource = binding["resource"].as_str().unwrap_or_default();
        let policy = &binding["policy"];
        let mut key = entry_key(session, policy, args)
            .ok_or_else(|| format!("`{resource}`'s key names an argument it was not given"))?;
        if policy["parallel"] == true {
            // `concurrency parallel`: no shared flight, so a key of its own.
            let n = self
                .parallel
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            key.push_str(&format!("\u{1f}#{n}"));
        }
        Ok(pw_resource::Key::new(resource, &key))
    }

    /// [`Server::fetch_answer`], under a key the caller computed and holds.
    fn fetch_answer_at(
        &self,
        session: &str,
        binding: &serde_json::Value,
        args: &[Val],
        key: &pw_resource::Key,
    ) -> Result<Val, String> {
        let resource = binding["resource"].as_str().unwrap_or_default();
        let manifest = runtime_manifest(resource, &binding["policy"]);
        let key = key.clone();
        // A flight a commit invalidated is not an error: its result was about
        // to be stale, and a reader asks again (found 2026-10-02: two presses
        // at once made one command's re-read fail on the other's commit).
        //
        // Once more through `pw-resource`, then directly. Asking again
        // through it can be invalidated again by each commit that lands
        // during the read, and eight times was no bound on that: eight
        // threads committing on one session invalidated a reader eight times
        // in about one run of a hundred. A direct read starts after the
        // invalidation, so it is as new as the commit that caused it, and
        // nothing can take its result away.
        for _ in 0..2 {
            match self.fetch_once(session, resource, &manifest, &key, args) {
                pw_resource::Fetched::Cancelled(pw_resource::StopReason::Invalidated) => continue,
                pw_resource::Fetched::Fresh(v)
                | pw_resource::Fetched::FromCache(v)
                | pw_resource::Fetched::Deduplicated(v) => {
                    if matches!(*v, Val::Result(Err(_))) {
                        self.queries.invalidate_key(&key);
                    }
                    return Ok(Val::clone(&v));
                }
                pw_resource::Fetched::Failed(e) => return Err(format!("{resource}: {e}")),
                pw_resource::Fetched::Cancelled(r) => {
                    return Err(format!("{resource}: stopped, {r:?}"));
                }
                pw_resource::Fetched::TimedOut => return Err(format!("{resource}: timed out")),
            }
        }
        self.answer(resource, session, args)
    }

    /// One fetch of a binding's query through `pw-resource`.
    fn fetch_once(
        &self,
        session: &str,
        resource: &str,
        manifest: &pw_resource::Manifest,
        key: &pw_resource::Key,
        args: &[Val],
    ) -> pw_resource::Fetched<Arc<Val>> {
        self.sync_query_clock();
        self.queries
            .fetch_cancellable(manifest, key, |_attempt, cancellation| {
                // What a slow source polls, to end early a read nobody is
                // waiting for any longer (ADR-0152).
                let cancellation = cancellation.clone();
                let stopped: Stopped = Arc::new(move || cancellation.is_cancelled());
                self.answer_within(resource, session, args, Some(stopped))
                    .map(Arc::new)
            })
    }

    /// **Drop what a commit made stale** (ADR-0127): each entry a command
    /// names in `invalidates`, and each entry an emitted event reaches through
    /// a query's `invalidates_on`. An event that leaves a key unbound (`_`,
    /// ADR-0091) drops every entry of that query.
    fn invalidate_queries(&self, command: &str, session: &str, events: &[pw_materialize::Event]) {
        let policy_of = |resource: &str| {
            self.plan["bindings"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|b| b["resource"] == resource)
                .map(|b| b["policy"].clone())
        };
        let drop_key = |resource: &str, args: Option<Vec<Val>>| {
            let Some(policy) = policy_of(resource) else {
                // No binding reads it, so nothing of it is kept.
                return;
            };
            match args.and_then(|a| entry_key(session, &policy, &a)) {
                Some(k) => self
                    .queries
                    .invalidate_key(&pw_resource::Key::new(resource, &k)),
                None => self.queries.invalidate(resource),
            }
        };
        for e in &self.graph.edges {
            if e.from == command && e.kind == pw_materialize::EdgeKind::Invalidates {
                let args: Option<Vec<Val>> = e
                    .key
                    .iter()
                    .map(|a| (a == "current_session()").then(|| Val::String(session.into())))
                    .collect();
                drop_key(&e.to, args);
            }
        }
        for event in events {
            for e in &self.graph.edges {
                if e.kind != pw_materialize::EdgeKind::InvalidatedBy || e.to != event.name {
                    continue;
                }
                // The query's parameters, bound from the event's arguments by
                // the names its `invalidates_on` gives them.
                let params = self
                    .graph
                    .nodes
                    .iter()
                    .find(|n| n.path == e.from)
                    .map(|n| n.params.clone())
                    .unwrap_or_default();
                let mut args = vec![None; params.len()];
                for (i, name) in e.key.iter().enumerate() {
                    if let (Some(p), Some(v)) =
                        (params.iter().position(|q| q == name), event.args.get(i))
                    {
                        args[p] = Some(Val::String(v.clone()));
                    }
                }
                drop_key(&e.from, args.into_iter().collect());
            }
        }
    }

    /// **Run a query's compiled component** (ADR-0125), with the data layer,
    /// the session and the catalogue, and return its answer: its declared
    /// result, `Ok` or `Err`. Nothing a query does is committed: it reads.
    fn answer(&self, component_id: &str, session: &str, args: &[Val]) -> Result<Val, String> {
        self.answer_within(component_id, session, args, None)
    }

    /// [`Server::answer`], its slow sources told how to see that the read was
    /// stopped (ADR-0152).
    fn answer_within(
        &self,
        component_id: &str,
        session: &str,
        args: &[Val],
        stopped: Option<Stopped>,
    ) -> Result<Val, String> {
        let current = self
            .carts
            .lock()
            .expect("carts")
            .get(session)
            .cloned()
            .unwrap_or_default();
        let mut host = Self::data_layer(session, current, Arc::default(), false);
        host.insert(
            "pw:host/session#read".to_string(),
            Self::session_operation(session),
        );
        // The session's order, as its status's case (E14, T04). A case the
        // program's type does not have is refused by the component's types,
        // as any value the host gives is.
        let order = self.orders.lock().expect("orders").get(session).cloned();
        let this_session = session.to_string();
        host.insert(
            "store:data/orders#current".to_string(),
            Arc::new(move |args: &[Val]| {
                let Some(Val::String(s)) = args.first() else {
                    return Err(format!("orders#current received {args:?}"));
                };
                if *s != this_session {
                    return Err("orders#current was passed another session".to_string());
                }
                let status = order.clone().map(|case| Box::new(Val::Variant(case, None)));
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::Option(status)))))])
            }),
        );
        // The session's delivery estimate (E14, T10), after the estimator's
        // delay: a slow source, which a streamed region does not wait for.
        let estimator = self
            .estimators
            .lock()
            .expect("estimators")
            .get(session)
            .cloned()
            .unwrap_or_default();
        let this_session = session.to_string();
        host.insert(
            "store:data/estimates#current".to_string(),
            Arc::new(move |args: &[Val]| {
                let Some(Val::String(s)) = args.first() else {
                    return Err(format!("estimates#current received {args:?}"));
                };
                if *s != this_session {
                    return Err("estimates#current was passed another session".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(estimator.delay_ms));
                match estimator.fail.as_deref() {
                    Some("declared") => Ok(vec![Val::Result(Err(Some(Box::new(Val::Variant(
                        "location-unavailable".into(),
                        None,
                    )))))]),
                    Some(_) => Err("the estimator is down".to_string()),
                    None => Ok(vec![Val::Result(Ok(Some(Box::new(Val::Record(vec![(
                        "minutes".into(),
                        Val::S64(estimator.minutes),
                    )])))))]),
                }
            }),
        );
        host.extend(self.catalog_within(stopped));
        let host = host
            .into_iter()
            .map(|(name, f)| {
                let calls = self.calls.clone();
                let counted_name = name.clone();
                let counted: HostFn = Arc::new(move |args: &[Val]| {
                    *calls
                        .lock()
                        .expect("calls")
                        .entry(counted_name.clone())
                        .or_default() += 1;
                    f(args)
                });
                (name, counted)
            })
            .collect();
        let out = self.run(component_id, session, &host, args)?;
        out.into_iter()
            .next()
            .ok_or_else(|| format!("{component_id} returned nothing"))
    }

    /// **A query's value**: its answer's `Ok` value. A declared error is a
    /// failure to a reader that shows only a value (ADR-0147).
    fn query(&self, component_id: &str, session: &str, args: &[Val]) -> Result<Val, String> {
        unwrapped(component_id, self.answer(component_id, session, args)?)
    }

    /// **Each binding of the store page, by its query** (ADR-0125): a page
    /// parameter is what `document`'s address gave it (ADR-0162),
    /// `current_session()` is the request's session, and a signal's key is
    /// what `document` shows (ADR-0161).
    fn bindings(&self, session: &str, document: u64) -> Result<BTreeMap<String, Val>, String> {
        Ok(self.bindings_where(session, Some(document), |_| true, &Keys::Shown)?)
    }

    /// One binding's value, by its name, read for none of the session's
    /// documents: a binding no signal keys is every document's.
    fn binding(&self, session: &str, name: &str) -> Result<BTreeMap<String, Val>, String> {
        Ok(self.bindings_where(session, None, |b| b == name, &Keys::Shown)?)
    }

    /// The bindings whose names `wanted` takes, each run by its policies, a
    /// binding a signal keys read for the key `keys` says (ADR-0152), and
    /// what `document` shows (ADR-0161).
    fn bindings_where(
        &self,
        session: &str,
        document: Option<u64>,
        wanted: impl Fn(&str) -> bool,
        keys: &Keys,
    ) -> Result<BTreeMap<String, Val>, Unread> {
        let mut out = BTreeMap::new();
        for b in self.plan["bindings"].as_array().into_iter().flatten() {
            if !wanted(b["binding"].as_str().unwrap_or_default()) {
                continue;
            }
            let args = self.args_of(session, document, b, keys)?;
            out.insert(
                b["binding"].as_str().unwrap_or_default().to_string(),
                self.fetch_binding(session, b, &args)?,
            );
        }
        Ok(out)
    }

    /// **A binding's arguments**, as a host computes them: a page parameter
    /// is what `document`'s address gave it (ADR-0162), `current_session()`
    /// the request's session, and a page signal the key `keys` says
    /// (ADR-0152), as `document` shows it (ADR-0161).
    fn args_of(
        &self,
        session: &str,
        document: Option<u64>,
        b: &serde_json::Value,
        keys: &Keys,
    ) -> Result<Vec<Val>, String> {
        let binding = b["binding"].as_str().unwrap_or_default();
        let signals: Vec<&str> = b["signals"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str())
            .collect();
        b["args"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|a| match a.as_str() {
                // A page parameter, as the document's address gave it
                // (ADR-0162).
                Some(name)
                    if self.plan["params"]
                        .as_array()
                        .is_some_and(|ps| ps.iter().any(|p| p == name)) =>
                {
                    document
                        .and_then(|d| {
                            self.params
                                .lock()
                                .expect("params")
                                .get(&(session.to_string(), d))
                                .and_then(|p| p.get(name).cloned())
                        })
                        .map(Val::String)
                        .ok_or_else(|| format!("the page's `{name}` is given no value here"))
                }
                Some("current_session()") => Ok(Val::String(session.into())),
                Some(signal) if signals.contains(&signal) => {
                    let value = match keys {
                        Keys::Asked {
                            binding: asked,
                            key,
                        } if *asked == binding => key.get(signal).cloned(),
                        Keys::First => None,
                        Keys::Shown | Keys::Asked { .. } => document.and_then(|d| {
                            self.keyed
                                .lock()
                                .expect("keyed")
                                .get(&(session.to_string(), d))
                                .and_then(|k| k.reads.get(binding))
                                .and_then(|r| r.shown.get(signal).cloned())
                        }),
                    }
                    .unwrap_or_else(|| self.first_value(signal));
                    key_val(&value).ok_or_else(|| format!("`{signal}` is no key: {value}"))
                }
                other => Err(format!("an argument this server cannot compute: {other:?}")),
            })
            .collect()
    }

    /// A page signal's first value, as the build computed it (ADR-0130).
    fn first_value(&self, signal: &str) -> serde_json::Value {
        self.plan["signals"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|s| s["name"] == signal)
            .map(|s| s["initial"].clone())
            .unwrap_or(serde_json::Value::Null)
    }

    /// **A part's value**, by the steps the compiler planned: a record field,
    /// or a member function run as its own component.
    fn read_part(
        &self,
        bindings: &BTreeMap<String, Val>,
        part: &serde_json::Value,
    ) -> Result<Val, String> {
        let binding = part["binding"].as_str().unwrap_or_default();
        let mut value = bindings
            .get(binding)
            .cloned()
            .ok_or_else(|| format!("no binding `{binding}`"))?;
        for step in part["steps"].as_array().into_iter().flatten() {
            value = if let Some(field) = step["field"].as_str() {
                let wit = field.replace('_', "-");
                match &value {
                    Val::Record(fields) => fields
                        .iter()
                        .find(|(n, _)| *n == wit)
                        .map(|(_, v)| v.clone())
                        .ok_or_else(|| format!("no field `{field}`"))?,
                    other => return Err(format!("`.{field}` read from {other:?}")),
                }
            } else if let Some(member) = step["member"].as_str() {
                let out = self.run(member, "", &BTreeMap::new(), &[value])?;
                out.into_iter()
                    .next()
                    .ok_or_else(|| format!("{member} returned nothing"))?
            } else {
                return Err(format!("a step this server does not know: {step}"));
            };
        }
        Ok(value)
    }

    /// The text a part shows, found by its template path.
    fn part_text(&self, session: &str, path: &str) -> Result<String, String> {
        let part = self.plan["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["path"] == path)
            .ok_or_else(|| format!("the plan has no part `{path}`"))?
            .clone();
        // The part's own binding, and no other (ADR-0151): every binding
        // was read here, so a command asked each of the page's queries for
        // the cart's count, and a slow one held up every command.
        let bindings = self.binding(session, part["binding"].as_str().unwrap_or_default())?;
        Ok(match val_to_value(&self.read_part(&bindings, &part)?) {
            Value::Text(t) => t,
            Value::Int(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            other => return Err(format!("`{path}` has no text form: {other:?}")),
        })
    }

    /// Consume committed events and regenerate what they invalidate.
    fn drain(&self, session: &str) {
        let key = self.cart_key(session);
        let invalidated = self
            .materializer
            .drain(&self.graph, std::slice::from_ref(&key));
        // The count the page shows, as the page computes it: the cart query's
        // value, read through `domain.line_count` (ADR-0125).
        let value = match self.part_text(session, "cart.line_count") {
            Ok(value) => value,
            Err(why) => return self.unshowable_session(session, &why),
        };
        if std::env::var("PW_TRACE").is_ok() {
            eprintln!(
                "drain session={session} invalidated={} value={value} existing={}",
                invalidated.len(),
                self.materializer.entry(&key).is_some()
            );
        }

        // First materialization, or a regeneration the drain asked for.
        //
        // The clock advances first, because a version IS a clock reading:
        // `Entry.generated_at`. A clock that never moves gives every entry
        // version 0, and then the browser's staleness comparison compares
        // nothing while looking exactly like it works. Advancing here rather
        // than keeping a counter is what keeps the version the MATERIALIZER's.
        let existing = self.materializer.entry(&key).is_some();
        if !existing || !invalidated.is_empty() {
            let because = invalidated.first().map(|(_, id)| *id);
            self.clock.advance(1);
            self.materializer
                .regenerate(&key, because, || Ok(value.to_string()));
        }

        // Frames only for an INVALIDATION. The first materialization is the
        // value the document was rendered with, and a frame for it would tell
        // the browser to replace a part with what it already shows — harmless
        // here, and wrong in general because it spends a version the page then
        // treats as the newest it has seen.
        if invalidated.is_empty() {
            return;
        }

        // What each of the session's pages shows now, from its queries
        // (ADR-0145), each read for its own document (ADR-0161), outside the
        // table.
        let documents = documents_of(&self.pending.lock().expect("pending"), session);
        let mut read = Vec::new();
        for doc in documents {
            let store = self.store_of(&doc);
            let now = self
                .bindings(session, doc.1)
                .and_then(|bindings| Ok((self.showing(session, &store, &bindings)?, bindings)));
            match now {
                Ok((now, bindings)) => read.push((doc, store, bindings, now)),
                Err(why) => self.unshowable(&doc, &why),
            }
        }

        // Two frames per change: the entry advanced, and every place in the
        // document the server DERIVED from it, as one set (ADR-0145). A
        // subscription and a patch are logically separate — a server may
        // derive a patch from a change rather than must.
        let entry = cart_entry(session);
        let version = self.version(session);
        let mut failed = Vec::new();
        let mut queue = self.pending.lock().expect("pending");
        for (doc, store, bindings, now) in read {
            // And the value itself, to a page that speculates on it
            // (ADR-0122), from the snapshot the patches are derived from.
            // Until 2026-10-03 it was read again after the table was let go,
            // and pushed in a second hold: a page could be sent one change in
            // two batches, and a command committed in between gave the frame
            // a value later than its version.
            let value = self
                .speculates_on_cart()
                .and_then(|binding| bindings.get(&binding))
                .map(val_to_json);
            // Against what the document shows. With none served, none shows.
            let patches = {
                let mut shown = self.shown.lock().expect("shown");
                match shown.get(&doc) {
                    Some(was) => match self.derive(session, &store, &bindings, was, &now) {
                        Ok(patches) => {
                            shown.insert(doc.clone(), now);
                            patches
                        }
                        Err(why) => {
                            failed.push((doc, why));
                            continue;
                        }
                    },
                    None => Vec::new(),
                }
            };
            // A document forgotten while it was read is told nothing.
            let Some(waiting) = queue.get_mut(&doc) else {
                continue;
            };
            waiting.push(StreamFrame::ResourceChanged {
                protocol: CURRENT,
                entry: entry.clone(),
                version,
            });
            // Sent with no patches too: the document reflects the new version
            // when nothing it shows changed.
            waiting.push(StreamFrame::PatchSet(PatchSet {
                protocol: CURRENT,
                basis: CausalBasis::of(entry.clone(), version),
                patches,
            }));
            // In the same hold, so one change reaches a page whole.
            if let Some(value) = value {
                waiting.push(StreamFrame::EntryValue {
                    protocol: CURRENT,
                    entry: entry.clone(),
                    version,
                    value,
                });
            }
        }
        drop(queue);
        for (doc, why) in failed {
            self.unshowable(&doc, &why);
        }
    }

    // --- E7-P: keyed collections -------------------------------------
    //
    // Architect ruling, 2026-08-07:
    //
    // > DOM identity should survive moves. A `MoveInstance` that deletes and
    // > recreates the node is not equivalent.
    //
    // Everything below therefore derives ONE thing on the server — which
    // instance, and what its markup is — and sends it. The browser does not
    // re-render, and it does not diff: it has no template and no previous
    // model to diff against.

    /// The menu's loop part, from the compiler's manifest.
    fn menu_part(&self) -> (&Template, LocalPartId) {
        let template = self
            .templates
            .iter()
            .find(|t| t.name == "StorePage")
            .expect("StorePage");
        let part = template
            .manifest()
            .into_iter()
            .find(|p| p.kind == "each")
            .expect("the store page has a keyed loop");
        (template, LocalPartId(part.id.0))
    }

    /// The text part inside the loop body that shows an item's name.
    fn item_name_part(&self) -> LocalPartId {
        let (template, _) = self.menu_part();
        let part = template
            .manifest()
            .into_iter()
            .find(|p| p.value == "item.name")
            .expect("the loop body shows the item's name");
        LocalPartId(part.id.0)
    }

    fn menu_address(&self) -> PartAddress {
        let (template, part) = self.menu_part();
        PartAddress::new(&TemplateSchemaId(template.schema.clone()), part)
    }

    /// The identity domain a session's document was rendered in.
    ///
    /// Derived HERE and used by both the full render and every patch, so a
    /// token in a patch is the token already in the page. Two derivations
    /// would agree until one of them changed.
    fn domain(&self, session: &str) -> IdentityDomain {
        IdentityDomain::document(
            "StorePage(47)",
            Partition::Session {
                id: session.to_string(),
            },
            BUILD,
        )
        .keyed("own-renderer-spike-key")
    }

    /// The menu fragment's OWN identity domain — public, and the same for
    /// every reader.
    ///
    /// Not the document's. `store.page.Menu` is declared `public … cache
    /// shared`: one answer for everybody. If its instance tokens came from the
    /// enclosing document they would be per-session, and two readers of one
    /// shared cache entry would get different bytes — which is the exact
    /// composition `IdentityDomain` exists to make unrepresentable, arriving
    /// through the back door of "the page renders the fragment".
    fn fragment_domain(&self, store: &str) -> IdentityDomain {
        IdentityDomain::document(
            &format!("store.page.Menu({store})"),
            Partition::Public,
            BUILD,
        )
        .keyed("own-renderer-spike-key")
    }

    /// The fragment's render environment.
    ///
    /// Takes the items rather than locking for them. Locking here put the menu
    /// mutex on two points of one call path — the page render and the fragment
    /// render beneath it — and a non-reentrant mutex against itself is a hang,
    /// not an error. The lock is taken once, by whoever is about to use the
    /// list, and passed down.
    ///
    /// It must NOT set the materialized fragment either: that is what the PAGE
    /// splices in, and putting it here made `menu_env` call `menu_fragment`
    /// call `menu_env` — unbounded recursion that presented as a server which
    /// accepted connections and answered none.
    fn menu_env(&self, store: &str, items: &[(String, String)]) -> Env {
        Env::new()
            .set("menu", menu_value(items))
            .in_domain(self.fragment_domain(store))
    }

    /// The menu fragment's bytes, materialized once and reused.
    ///
    /// Regenerated only when the entry is stale, and stored in the
    /// materializer under the PUBLIC entry key. Two readers get the same bytes
    /// because they are the same bytes — the entry is read, not re-rendered —
    /// and that is what makes `cache shared` a fact about the system rather
    /// than a claim about two renders agreeing.
    fn menu_fragment(&self, store: &str, items: &[(String, String)]) -> String {
        let key = self.menu_key(store);
        // Kept while it shows the `Menu` query's value. Until 2026-10-03 it
        // was kept until a command invalidated it, so a value that changed at
        // its source, read again once its freshness was spent, was not what
        // a new document showed.
        let mut rendered_from = self.menu_rendered_from.lock().expect("rendered from");
        if let Some(entry) = self.materializer.entry(&key)
            && !entry.stale
            && rendered_from.get(store).is_some_and(|was| *was == items)
        {
            return entry.body;
        }
        let (template, part) = self.menu_part();
        let env = self.menu_env(store, items);
        let html = pw_render::render_part(template, part, &env, &self.templates)
            .expect("the menu fragment renders");
        self.clock.advance(1);
        self.materializer.invalidate(&key, 0);
        self.materializer
            .regenerate(&key, None, || Ok(html.clone()));
        rendered_from.insert(store.to_string(), items.to_vec());
        html
    }

    /// The public entry whose version every patch to a store's menu is
    /// caused by.
    fn menu_key(&self, store: &str) -> EntryKey {
        EntryKey::from_identity(&menu_identity(store))
    }

    fn menu_version(&self, store: &str) -> Version {
        Version(
            self.materializer
                .entry(&self.menu_key(store))
                .map(|e| e.generated_at)
                .unwrap_or(0),
        )
    }

    /// Regenerate the menu entry and broadcast one structural patch per
    /// subscriber.
    ///
    /// Per subscriber, because an instance token is scoped to the DOCUMENT it
    /// appears in: two sessions render the same item at different tokens, and
    /// a single broadcast frame would address at most one of them. The
    /// alternative — one shared token — is the public-fragment case, and it is
    /// a different partition rather than a shortcut for this one.
    fn broadcast_menu(&self, op: MenuOp) -> Result<(), String> {
        // A departed visitor is forgotten before the fan-out, so a change is
        // not queued for pages nobody is reading.
        self.forget_idle_subscribers();
        // The subscriber table is taken FIRST and held throughout.
        //
        // It is what serializes a change against a document being served: a
        // page rendered from the new list must not also receive the patch that
        // produced it, and a page rendered from the old list must receive it.
        // Without the interlock both orders are possible and the second one
        // applies a change twice — an item inserted, then inserted again.
        let mut queue = self.pending.lock().expect("pending");
        {
            // Refusals happen before anything is regenerated: a rejected
            // operation must leave no version behind, or the page would be
            // told to catch up to a change that did not happen.
            let mut items = self.menu.lock().expect("menu");
            op.apply(&mut items)?;
        }
        // The deployment's menu changed, which is `MenuChanged(47)`: it drops
        // what the program says depends on it, the entries of each query
        // that declares `invalidates_on MenuChanged(id)` for this store, and
        // no other store's (§15.6 test 11, ADR-0164). Until 2026-10-03 every
        // store's kept `Menu` was dropped here, by the query's name.
        self.invalidate_queries(
            "",
            "",
            &[pw_materialize::Event::new(
                "Events.MenuChanged",
                &[STORE_ID],
            )],
        );

        // The fragment is re-materialized, in its own domain, once.
        // The menu E7-P changes is store 47's (ADR-0162).
        let items = self.menu.lock().expect("menu").clone();
        let (template, part) = self.menu_part();
        let env = self.menu_env(STORE_ID, &items);
        let html = pw_render::render_part(template, part, &env, &self.templates)
            .map_err(|b| format!("{b:?}"))?;
        self.clock.advance(1);
        // Invalidated first: `regenerate` leaves a current entry alone, so
        // without this the fragment kept its first rendering and every
        // document served after a change showed the old menu, while pages
        // already open were patched (found 2026-10-02, ADR-0127).
        self.materializer.invalidate(&self.menu_key(STORE_ID), 0);
        self.materializer
            .regenerate(&self.menu_key(STORE_ID), None, || Ok(html));
        self.menu_rendered_from
            .lock()
            .expect("rendered from")
            .insert(STORE_ID.to_string(), items.clone());

        let entry = ResourceEntryId::derive(&menu_identity(STORE_ID), &IDENTITY);
        let version = self.menu_version(STORE_ID);

        // ONE patch, for every reader.
        //
        // A public fragment has one identity, so its instances have one set of
        // tokens, so one address reaches every document containing it. The
        // previous version derived a patch per subscriber because the fragment
        // inherited each document's domain — which was per-session bytes for a
        // shared cache entry, dressed up as a broadcast.
        let operation = op
            .patch(template, part, &items, &env, &self.templates)
            .map_err(|b| format!("{b:?}"))?;
        let target = op.target(
            &self.menu_address(),
            part,
            self.item_name_part(),
            &env,
            part,
        );

        // To each document that shows the store (§15.6 test 11): another
        // store's menu is not this one's.
        let params = self.params.lock().expect("params");
        let readers = |doc: &Doc| {
            params
                .get(doc)
                .is_some_and(|p| p.get("id").map(String::as_str) == Some(STORE_ID))
        };
        if std::env::var("PW_TRACE").is_ok() {
            eprintln!("broadcast {:?} to {} document(s)", op, queue.len());
        }
        for (_, waiting) in queue.iter_mut().filter(|(doc, _)| readers(doc)) {
            waiting.push(StreamFrame::ResourceChanged {
                protocol: CURRENT,
                entry: entry.clone(),
                version,
            });
            waiting.push(StreamFrame::Patch(Patch {
                protocol: CURRENT,
                basis: CausalBasis::of(entry.clone(), version),
                target: target.clone(),
                operation: operation.clone(),
            }));
        }
        Ok(())
    }

    /// The document, and an empty queue for whoever receives it.
    ///
    /// A freshly rendered document already reflects every change committed
    /// before it was rendered. Delivering the patches for those changes as
    /// well would apply them twice — and the second application is not a
    /// no-op, because inserting an item is not idempotent. A reload is the
    /// ordinary way to reach that state, so this is not an edge case.
    ///
    /// Clearing rather than replaying is right because a patch is a
    /// TRANSITION. It is only meaningful against the state it was derived
    /// from, and this document was not derived from that state.
    ///
    /// Returns the cursor the document should subscribe from, so a reloaded
    /// page does not ask for frames the reload already made meaningless.
    #[cfg(test)]
    fn serve_document(&self, session: &str) -> (String, u64) {
        let (rendered, cursor, _) = self
            .serve_document_with_entries(session)
            .unwrap_or_else(|e| panic!("{e}"));
        (rendered, cursor)
    }

    /// The document, its cursor, and the values of the entries it speculates
    /// on (ADR-0122), all read under one hold of the subscriber table, so the
    /// values are the ones the rendered parts show.
    #[cfg(test)]
    fn serve_document_with_entries(
        &self,
        session: &str,
    ) -> Result<(String, u64, serde_json::Value), String> {
        self.serve_document_settled(session, &store_params(STORE_ID), &[])
            .map(|(html, cursor, entries, _)| (html, cursor, entries))
            .map_err(String::from)
    }

    /// [`Server::serve_document`], for the store `id` (ADR-0162), or why it
    /// could not be read (ADR-0163).
    #[cfg(test)]
    fn serve_store_document(&self, session: &str, id: &str) -> Result<(String, u64), Unread> {
        self.serve_document_settled(session, &store_params(id), &[])
            .map(|(html, cursor, _, _)| (html, cursor))
    }

    /// The document as [`Server::serve_document_with_entries`] serves it,
    /// with what its streams' queries settled to before it was written
    /// (ADR-0148), and the environment it was rendered in, which renders
    /// each streamed region's arm when its query settles.
    fn serve_document_settled(
        &self,
        session: &str,
        params: &Params,
        settled: &[(u32, Settled)],
    ) -> Result<(String, u64, serde_json::Value, Env), Unread> {
        self.drain(session);
        self.forget_idle_subscribers();
        // The document's values are read, and it is rendered, OUTSIDE the
        // subscriber table (ADR-0151). Until 2026-10-03 the table was held
        // throughout, so a page waiting for a slow query kept every other
        // page waiting, and every command's frames; and two pages read at
        // once never shared a query's flight, whatever its `concurrency`
        // said. The table is held only to install the document.
        // The document's number first (ADR-0161), and its subscriber with
        // it, so a change that reaches the session while it is read reaches
        // it too.
        let doc: Doc = (
            session.to_string(),
            self.documents
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );
        // Its parameters with it (ADR-0162): which store it reads.
        self.params
            .lock()
            .expect("params")
            .insert(doc.clone(), params.clone());
        let served = (|| {
            for _ in 1..DOCUMENT_ATTEMPTS {
                let pushed = self.subscribed(&doc);
                let read = self.render_store_document(&doc, settled)?;
                let mut queue = self.pending.lock().expect("pending");
                if let Some(served) = self.installed(&mut queue, &doc, read, Some(pushed)) {
                    return Ok(served);
                }
            }
            // The last attempt is read inside the table, as every document
            // was before: nothing can reach the session between the read and
            // the install, so a page is always served.
            let mut queue = self.pending.lock().expect("pending");
            let read = self.render_store_document(&doc, settled)?;
            Ok(self
                .installed(&mut queue, &doc, read, None)
                .expect("nothing reaches a session inside the table"))
        })();
        // A document that could not be read is no page, and waits for
        // nothing.
        if served.is_err() {
            self.pending.lock().expect("pending").remove(&doc);
            self.params.lock().expect("params").remove(&doc);
        }
        served
    }

    /// **The document's subscriber, before it is read** (ADR-0151,
    /// ADR-0161), and how many frames have reached it. Registered first, so a
    /// change that reaches the session while its document is read is pushed
    /// to it and counted, even for a session's first page.
    fn subscribed(&self, doc: &Doc) -> u64 {
        let mut queue = self.pending.lock().expect("pending");
        let waiting = queue
            .entry(doc.clone())
            .or_insert_with(|| Subscriber::at(doc.1));
        waiting.seen = std::time::Instant::now();
        waiting.pushed
    }

    /// **A document read, installed** in the table the caller holds: what
    /// was waiting is cleared, the document takes its cursor, and what it
    /// shows is recorded with it, so a change is sent as the difference
    /// (ADR-0145). `None` when a frame reached the session after `pushed`:
    /// the document may show a value from before that change, and clearing
    /// the change's frames would lose it, so it is read again.
    fn installed(
        &self,
        queue: &mut BTreeMap<Doc, Subscriber>,
        doc: &Doc,
        (html, shown, env): (String, Shown, Env),
        pushed: Option<u64>,
    ) -> Option<(String, u64, serde_json::Value, Env)> {
        let waiting = queue
            .entry(doc.clone())
            .or_insert_with(|| Subscriber::at(doc.1));
        if pushed.is_some_and(|pushed| waiting.pushed != pushed) {
            return None;
        }
        // What reached it while an attempt before this one was read: this
        // read includes it.
        waiting.frames.clear();
        waiting.behind = false;
        waiting.seen = std::time::Instant::now();
        self.shown.lock().expect("shown").insert(doc.clone(), shown);
        // Its keyed reads start with it (ADR-0152).
        self.keyed
            .lock()
            .expect("keyed")
            .insert(doc.clone(), Keyed::default());
        // Its cursor is its number, never zero.
        Some((html, doc.1, self.speculated_entries(&doc.0), env))
    }

    /// **The cart, as the page's speculation module decodes it** (ADR-0122):
    /// the WIT `domain-cart` record by its Pleris field names, as
    /// `cart_value` builds it for a component. Every line is priced 450, as
    /// the data layer prices it.
    fn cart_json(&self, session: &str) -> serde_json::Value {
        // The cart query's value, by its Pleris field names (ADR-0125).
        let cart = self
            .query(
                "store.page.Cart",
                session,
                &[Val::String(session.to_string())],
            )
            .unwrap_or_else(|e| panic!("the cart query failed: {e}"));
        val_to_json(&cart)
    }

    /// Does the page speculate on the session's cart? Read from the manifest:
    /// a binding of `store.page.Cart` keyed by `current_session()`, the one
    /// key this server computes, as it computes an event's (ADR-0104).
    fn speculates_on_cart(&self) -> Option<String> {
        self.speculation.as_ref()?["bindings"]
            .as_array()?
            .iter()
            .find(|b| {
                b["resource"] == "store.page.Cart"
                    && b["key"] == serde_json::json!(["current_session()"])
            })
            .and_then(|b| b["binding"].as_str().map(str::to_string))
    }

    /// Each speculated binding's entry, version and value, for the document.
    fn speculated_entries(&self, session: &str) -> serde_json::Value {
        let mut out = serde_json::Map::new();
        if let Some(binding) = self.speculates_on_cart() {
            out.insert(
                binding,
                serde_json::json!({
                    "entry": cart_entry(session),
                    "version": self.version(session),
                    "value": self.cart_json(session),
                }),
            );
        }
        serde_json::Value::Object(out)
    }

    /// The versions a committed command's writes produced (ADR-0122): what a
    /// page reconciles a speculation against.
    fn committed_basis(&self, session: &str) -> serde_json::Value {
        serde_json::json!([{ "entry": cart_entry(session), "version": self.version(session) }])
    }

    /// Every handler identity this build's templates name.
    ///
    /// Answered from the compiler's template IR rather than from a list here.
    /// A list would be a second answer to "which handlers exist", and the
    /// first thing it would do is disagree.
    fn handler_identities(&self) -> std::collections::BTreeSet<String> {
        self.templates
            .iter()
            .flat_map(|t| t.manifest())
            .filter(|p| p.kind == "event" && !p.value.is_empty())
            .map(|p| p.value.clone())
            .collect()
    }

    /// **Each handler this build compiled, with the capture schema its
    /// document presents** (ADR-0132): the identity, and the paths it
    /// captures joined, which is empty for a handler that captures nothing.
    /// What `/pw-handlers` tells the browser's resume decision.
    fn handler_table(&self) -> BTreeMap<String, String> {
        handler_table(&self.templates)
    }

    /// **A page's parameters, as its address gives them** (ADR-0130): each
    /// one text. A page whose parameter is not given is not rendered with a
    /// guess.
    fn page_params(&self, path: &str, query: &str) -> Result<BTreeMap<String, String>, String> {
        let plan = self
            .plans
            .get(path)
            .ok_or_else(|| format!("no page `{path}` in this build"))?;
        let mut out = BTreeMap::new();
        for p in plan["params"].as_array().into_iter().flatten() {
            let name = p.as_str().unwrap_or_default();
            let given = query
                .split('&')
                .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
                .and_then(percent_decoded)
                .ok_or_else(|| format!("`{path}` is given no `{name}`"))?;
            out.insert(name.to_string(), given);
        }
        Ok(out)
    }

    /// **A page whose values are its signals, its parameters and its
    /// streams** (ADR-0130, ADR-0136, ADR-0148): rendered at each signal's
    /// first value, as `pw build`'s plan states it, each parameter as the
    /// address gives it, `?item=cortado`, and each stream as its query settled
    /// or pending. The document, and the environment it was rendered in. A
    /// page that binds a query with `let` is the store's route's.
    fn render_page(
        &self,
        path: &str,
        params: &BTreeMap<String, String>,
        session: &str,
        settled: &[(u32, Settled)],
    ) -> Result<(String, Env, &Template), String> {
        let plan = self
            .plans
            .get(path)
            .ok_or_else(|| format!("no page `{path}` in this build"))?;
        if plan["bindings"].as_array().is_some_and(|b| !b.is_empty()) {
            return Err(format!(
                "`{path}` binds a query; this route renders a page's signals and streams"
            ));
        }
        let template = self
            .templates
            .iter()
            .find(|t| t.path == path)
            .ok_or_else(|| format!("no template `{path}`"))?;
        let mut env = Env::new();
        for (name, given) in params {
            env = env.set(name, Value::Text(given.clone()));
        }
        for (part, outcome) in settled {
            env = env.settle(PartId(*part), outcome.clone());
        }
        let env = with_signals(env, plan).in_domain(
            IdentityDomain::document(
                path,
                Partition::Session {
                    id: session.to_string(),
                },
                BUILD,
            )
            .keyed("own-renderer-spike-key"),
        );
        let body = pw_render::render(template, &env, &self.templates)
            .map_err(|e| format!("`{path}` does not render: {e:?}"))?;
        Ok((
            signal_document(&body, template, plan, &self.templates),
            env,
            template,
        ))
    }

    /// Where `pw emit-handlers` wrote the module for a handler identity.
    fn handler_module(&self, identity: &str) -> std::path::PathBuf {
        self.artifacts
            .join("handlers")
            .join(format!("{identity}.mjs"))
    }

    /// Handler identities the document names and no compiled module exists
    /// for. The server refuses to start while there are any: a document whose
    /// buttons cannot load their code is a build that did not finish.
    fn uncompiled_handlers(&self) -> Vec<String> {
        self.handler_identities()
            .into_iter()
            .filter(|id| !self.handler_module(id).is_file())
            .collect()
    }

    #[cfg(test)]
    fn render_store(&self, session: &str) -> String {
        self.render_store_showing(session)
            .unwrap_or_else(|e| panic!("{e}"))
            .0
    }

    /// **The store's document, and what it shows** (ADR-0145), or why it
    /// cannot be rendered: a query that failed is a page that cannot be
    /// shown, answered as such, never a server that stops (E14, T04).
    #[cfg(test)]
    fn render_store_showing(&self, session: &str) -> Result<(String, Shown), String> {
        // A document of the default store, read and not served.
        let doc: Doc = (
            session.to_string(),
            self.documents
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );
        self.params
            .lock()
            .expect("params")
            .insert(doc.clone(), store_params(STORE_ID));
        let read = self.render_store_document(&doc, &[]);
        self.params.lock().expect("params").remove(&doc);
        read.map(|(html, shown, _)| (html, shown))
            .map_err(String::from)
    }

    /// **The page a path names by its route**, and the parameters the path
    /// gives it (ADR-0162): among the pages this build planned with one.
    /// One route is one page's (PW0341), so the first is the only one.
    fn routed(&self, path: &str) -> Option<(String, Params)> {
        self.plans.iter().find_map(|(page, plan)| {
            let route = plan["route"].as_str()?;
            route_params(route, path).map(|params| (page.clone(), params))
        })
    }

    /// The store a document shows (ADR-0162): its `id`.
    fn store_of(&self, doc: &Doc) -> String {
        self.params
            .lock()
            .expect("params")
            .get(doc)
            .and_then(|p| p.get("id").cloned())
            .unwrap_or_else(|| STORE_ID.to_string())
    }

    /// The store's document, what it shows, and the environment it was
    /// rendered in, with what its streams' queries settled to (ADR-0148). A
    /// streamed region not given is rendered pending, with its placeholder.
    fn render_store_document(
        &self,
        doc: &Doc,
        settled: &[(u32, Settled)],
    ) -> Result<(String, Shown, Env), Unread> {
        let session = doc.0.as_str();
        let store = self.store_of(doc);
        let template = self.store_template();
        // Every value is a query's (ADR-0125): each binding runs its compiled
        // component, and each part reads the binding's value by the steps the
        // compiler planned. The menu's items are the `Menu` query's.
        // A new document's keys are its signals' first values, as the
        // browser holds them when it loads (ADR-0152).
        let bindings = self
            .bindings_where(session, Some(doc.1), |_| true, &Keys::First)
            .map_err(|e| e.of("the store page's queries"))?;
        let shown = self
            .showing(session, &store, &bindings)
            .map_err(|e| format!("the store page's values: {e}"))?;
        let mut env = self.document_env(session, &store, &bindings);
        for (part, outcome) in settled {
            env = env.settle(PartId(*part), outcome.clone());
        }
        let html = pw_render::render(template, &env, &self.templates)
            .map_err(|b| format!("the store page does not render: {b:?}"))?;
        Ok((html, shown, env))
    }

    /// **A binding read again, for the key the browser asks for**
    /// (ADR-0152). `seq` numbers the browser's reads of the binding, and
    /// `document` is the cursor of the page that asks. The read is applied
    /// only if it is the latest for a page that is still the session's:
    /// what its page shows is derived from it, and sent as one patch set in
    /// the session's frames, in order with every other change. Under
    /// `on_key_change cancel` a newer read lets go of this one's flight, and
    /// so does `left`, which says whether the browser has gone.
    fn read_keyed(
        &self,
        session: &str,
        binding: &str,
        seq: u64,
        document: u64,
        key: &BTreeMap<String, serde_json::Value>,
        left: &(dyn Fn() -> bool + Sync),
    ) -> Result<KeyOutcome, String> {
        let b = self.plan["bindings"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|b| b["binding"] == binding)
            .ok_or_else(|| format!("no binding `{binding}`"))?;
        let signals: std::collections::BTreeSet<String> = b["signals"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect();
        if signals.is_empty() {
            return Err(format!("`{binding}` is keyed by no signal"));
        }
        if key
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            != signals
            || key.values().any(|v| key_val(v).is_none())
        {
            return Err(format!(
                "`{binding}`'s key is its signals' values: {signals:?}"
            ));
        }
        let doc: Doc = (session.to_string(), document);
        // A read for a document the server does not hold is no page's, and
        // takes nothing (ADR-0161): not even its arguments, which are the
        // document's (ADR-0162).
        if !self.keyed.lock().expect("keyed").contains_key(&doc) {
            return Ok(KeyOutcome::Superseded);
        }
        let asked = Keys::Asked { binding, key };
        let args = self.args_of(session, Some(document), b, &asked)?;
        let flight = self.answer_key(session, b, &args)?;
        let cancel = b["policy"]["on_key_change"] == "cancel";
        let keep = b["policy"]["on_key_change"] == "keep";

        // The latest, for this page? A `cancel` read takes its hold here, and
        // the one before it lets go.
        let hold = self.queries.subscribe(&flight);
        let (hold, let_go, turn) = {
            let mut keyed = self.keyed.lock().expect("keyed");
            let Some(page) = keyed.get_mut(&doc) else {
                return Ok(KeyOutcome::Superseded);
            };
            let read = page.reads.entry(binding.to_string()).or_default();
            if seq <= read.latest {
                return Ok(KeyOutcome::Superseded);
            }
            read.latest = seq;
            let turn = read.turn.clone();
            if cancel {
                let before = read.hold.replace((seq, hold));
                (None, before, turn)
            } else {
                (Some(hold), None, turn)
            }
        };
        // Outside the table: letting go may stop a flight.
        drop(let_go);
        // Under `keep`, one read of the binding at a time: a read waits for
        // the one before it, which runs to its end, and one that is no longer
        // the latest when its turn comes is dropped unread. The browser
        // sends each read at once, so the server knows the latest while the
        // one before it still runs, and never applies that one.
        let _turn = keep.then(|| turn.lock().expect("turn"));
        if keep && self.superseded(session, binding, seq, document, false) {
            return Ok(KeyOutcome::Superseded);
        }

        // While it reads, a browser that leaves lets go of it too.
        let done = std::sync::atomic::AtomicBool::new(false);
        let mine = &doc;
        let value = std::thread::scope(|scope| {
            if cancel {
                let done = &done;
                scope.spawn(move || {
                    while !done.load(std::sync::atomic::Ordering::SeqCst) {
                        if left() {
                            self.let_go(mine, binding, seq);
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                });
            }
            let value = self.fetch_answer_at(session, b, &args, &flight);
            done.store(true, std::sync::atomic::Ordering::SeqCst);
            value
        });
        let value = match value {
            Ok(value) => unwrapped(b["resource"].as_str().unwrap_or_default(), value)?,
            // Stopped: let go of, by a newer read or a browser that left.
            Err(_) if self.superseded(session, binding, seq, document, cancel) => {
                return Ok(KeyOutcome::Superseded);
            }
            Err(why) => return Err(why),
        };
        drop(hold);

        // What the page shows for the new key: this binding's value, and the
        // others as they are.
        let mut bindings =
            self.bindings_where(session, Some(document), |n| n != binding, &Keys::Shown)?;
        bindings.insert(binding.to_string(), value);
        let store = self.store_of(&doc);
        let now = self.showing(session, &store, &bindings)?;
        let mut queue = self.pending.lock().expect("pending");
        let mut keyed = self.keyed.lock().expect("keyed");
        let Some(read) = keyed
            .get_mut(&doc)
            .and_then(|k| k.reads.get_mut(binding))
            .filter(|r| r.latest == seq)
        else {
            return Ok(KeyOutcome::Superseded);
        };
        let patches = {
            let mut shown = self.shown.lock().expect("shown");
            let Some(was) = shown.get(&doc) else {
                return Ok(KeyOutcome::Superseded);
            };
            let patches = self.derive(session, &store, &bindings, was, &now)?;
            shown.insert(doc.clone(), now);
            patches
        };
        read.shown = key.clone();
        if read.hold.as_ref().is_some_and(|(n, _)| *n == seq) {
            read.hold = None;
        }
        let entry = keyed_entry(session, binding);
        let Some(waiting) = queue.get_mut(&doc) else {
            return Ok(KeyOutcome::Superseded);
        };
        waiting.push(StreamFrame::ResourceChanged {
            protocol: CURRENT,
            entry: entry.clone(),
            version: Version(seq),
        });
        waiting.push(StreamFrame::PatchSet(PatchSet {
            protocol: CURRENT,
            basis: CausalBasis::of(entry, Version(seq)),
            patches,
        }));
        Ok(KeyOutcome::Applied)
    }

    /// **Let go of a `cancel` read's flight** (ADR-0152), if it is still the
    /// read holding it: a browser that left.
    fn let_go(&self, doc: &Doc, binding: &str, seq: u64) {
        let hold = {
            let mut keyed = self.keyed.lock().expect("keyed");
            keyed
                .get_mut(doc)
                .and_then(|k| k.reads.get_mut(binding))
                .filter(|r| r.hold.as_ref().is_some_and(|(n, _)| *n == seq))
                .and_then(|r| r.hold.take())
        };
        drop(hold);
    }

    /// Whether a read is no longer the one its page waits for: a newer one
    /// was asked, its page was replaced, or, under `cancel`, it was let go.
    fn superseded(
        &self,
        session: &str,
        binding: &str,
        seq: u64,
        document: u64,
        cancel: bool,
    ) -> bool {
        let keyed = self.keyed.lock().expect("keyed");
        match keyed.get(&(session.to_string(), document)) {
            None => true,
            Some(k) => k.reads.get(binding).is_none_or(|r| {
                r.latest != seq || (cancel && r.hold.as_ref().is_none_or(|(n, _)| *n != seq))
            }),
        }
    }

    /// **A session's page whose values cannot be read** (E14, T04): its
    /// document cannot be kept current, and is told to read itself again,
    /// which answers why. Never a panic: one failed query is one page that
    /// cannot be shown, not a server that stops for everyone.
    fn unshowable(&self, doc: &Doc, why: &str) {
        if std::env::var("PW_TRACE").is_ok() {
            eprintln!("unshowable document={doc:?}: {why}");
        }
        let mut queue = self.pending.lock().expect("pending");
        if self.shown.lock().expect("shown").remove(doc).is_some()
            && let Some(waiting) = queue.get_mut(doc)
        {
            waiting.push(StreamFrame::Recovery {
                protocol: CURRENT,
                recovery: Recovery::Reload,
            });
        }
    }

    /// [`Server::unshowable`], for every document of a session: what none of
    /// them can show.
    fn unshowable_session(&self, session: &str, why: &str) {
        let documents = documents_of(&self.pending.lock().expect("pending"), session);
        for doc in documents {
            self.unshowable(&doc, why);
        }
    }

    /// **A document, and each streamed region's arm as its query settles**
    /// (ADR-0148). With nothing left to settle it is one response of known
    /// length, as every document was. Otherwise all but the document's end is
    /// written at once; each region's patch follows, in the order the queries
    /// settle; and the response ends with the last, closing the connection,
    /// which is what tells the browser the document is complete.
    #[allow(clippy::too_many_arguments)]
    fn respond_streaming(
        &self,
        stream: &mut TcpStream,
        session: &str,
        fresh: bool,
        page: &str,
        template: &Template,
        env: &Env,
        settling: &mut Settling,
    ) {
        if settling.waiting.is_empty() && settling.settled.is_empty() {
            respond(
                stream,
                200,
                "text/html; charset=utf-8",
                session,
                fresh,
                page.as_bytes(),
            );
            return;
        }
        let cookie = if fresh {
            format!("set-cookie: pw-session={session}; Path=/; SameSite=Lax\r\n")
        } else {
            String::new()
        };
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\n{cookie}\
             connection: close\r\n\r\n"
        );
        let shell = page.strip_suffix(DOCUMENT_END).unwrap_or(page);
        if stream
            .write_all(head.as_bytes())
            .and_then(|_| stream.write_all(shell.as_bytes()))
            .and_then(|_| stream.flush())
            .is_err()
        {
            return;
        }
        while let Some((part, outcome)) = settling.next() {
            let patch = |outcome: Settled| {
                pw_render::settled_patch(
                    template,
                    PartId(part),
                    &env.clone().settle(PartId(part), outcome),
                    &self.templates,
                )
            };
            // An arm that does not render is a region whose query failed, as
            // far as the page can say: never a placeholder forever.
            let written = patch(outcome).or_else(|why| {
                if std::env::var("PW_TRACE").is_ok() {
                    eprintln!("stream {part} did not render: {why:?}");
                }
                patch(Settled::Failed(None))
            });
            let Ok(written) = written else { continue };
            if stream
                .write_all(written.as_bytes())
                .and_then(|_| stream.flush())
                .is_err()
            {
                return;
            }
        }
        let _ = stream.write_all(DOCUMENT_END.as_bytes());
        let _ = stream.flush();
        let _ = stream.shutdown(std::net::Shutdown::Write);
    }

    /// The store page's template.
    fn store_template(&self) -> &Template {
        self.templates
            .iter()
            .find(|t| t.name == "StorePage")
            .expect("StorePage")
    }

    /// **What the store's document renders from**: each binding's whole
    /// value, its parts' values, the menu's public fragment, and its signals
    /// at their first values, in the session's identity domain. A list a
    /// session's query fills is its binding's value: until 2026-10-03 it was
    /// set a second time, from what the document shows, which since ADR-0146
    /// is the same value under the same name.
    fn document_env(&self, session: &str, store: &str, bindings: &BTreeMap<String, Val>) -> Env {
        // Both bound BEFORE the chain. A `MutexGuard` produced inside a method
        // argument lives until the end of the whole STATEMENT, so locking the
        // menu inside the chain and locking it again inside `menu_fragment`
        // deadlocked a non-reentrant mutex against itself — presenting as a
        // request that simply never returned.
        let items = menu_items(&bindings["menu"]);
        let fragment = self.menu_fragment(store, &items);
        // Each binding's whole value, so a block a query decides, and what is
        // inside it, reads any field of it (ADR-0146).
        let mut env = Env::new();
        for (name, value) in bindings {
            env = env.set(name, val_to_value(value));
        }
        let mut env = env
            .set("menu", menu_value(&items))
            // The public fragment, EMITTED rather than rendered. Its instance
            // tokens are the fragment's own, so every reader's document
            // contains the same bytes and one patch addresses all of them.
            .materialized(self.menu_part().1, &fragment);
        for part in self.plan["parts"].as_array().into_iter().flatten() {
            let path = part["path"].as_str().unwrap_or_default();
            let value = self
                .read_part(bindings, part)
                .unwrap_or_else(|e| panic!("part `{path}`: {e}"));
            env = env.set(path, val_to_value(&value));
        }
        // Its signals, at their first values (ADR-0140).
        with_signals(env, &self.plan)
            // A page, not a materialization: its domain is the route identity
            // and its partition. The generation is carried whatever the
            // partition is — the two are orthogonal.
            .in_domain(self.domain(session))
    }

    /// **The lists a session's queries fill** (ADR-0145): each collection
    /// the page iterates whose binding is not cached shared. A shared one is
    /// a fragment every reader shares, patched once for all of them (the
    /// menu, E7-P).
    fn own_lists(&self) -> Vec<String> {
        let shared = |name: &str| {
            self.plan["bindings"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|b| b["binding"] == name && b["policy"]["cache"] == "shared")
        };
        self.plan["collections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c.as_str())
            .filter(|c| !shared(c))
            .map(str::to_string)
            .collect()
    }

    /// **What a session's document shows, from its queries' values**
    /// (ADR-0145): each part's text, and each list a session's query fills.
    fn showing(
        &self,
        session: &str,
        store: &str,
        bindings: &BTreeMap<String, Val>,
    ) -> Result<Shown, String> {
        let mut shown = Shown::default();
        for part in self.plan["parts"].as_array().into_iter().flatten() {
            let id = part["part"].as_u64().unwrap_or_default() as u32;
            let value = val_to_value(&self.read_part(bindings, part)?);
            let text = text_of(&value).ok_or_else(|| format!("part {id} has no text form"))?;
            shown.texts.insert(id, text);
        }
        for list in self.own_lists() {
            let value = bindings
                .get(&list)
                .ok_or_else(|| format!("no binding `{list}`"))?;
            let Value::List(items) = val_to_value(value) else {
                return Err(format!("`{list}` is not a list"));
            };
            shown.lists.insert(list, items);
        }
        // Each block a query decides, as it renders now (ADR-0146).
        let env = self.document_env(session, store, bindings);
        let template = self.store_template();
        for block in self.plan["blocks"].as_array().into_iter().flatten() {
            let id = block.as_u64().unwrap_or_default() as u32;
            let html = pw_render::render_part(template, PartId(id), &env, &self.templates)
                .map_err(|b| format!("block {id}: {b:?}"))?;
            shown.blocks.insert(id, html);
        }
        Ok(shown)
    }

    /// **The patches that turn what a document shows into what it should**
    /// (ADR-0145): each part whose text changed, and each list's change as
    /// keyed operations.
    fn derive(
        &self,
        session: &str,
        store: &str,
        bindings: &BTreeMap<String, Val>,
        was: &Shown,
        now: &Shown,
    ) -> Result<Vec<Targeted>, String> {
        let template = self.store_template();
        let schema = TemplateSchemaId(template.schema.clone());
        let mut out = Vec::new();
        for (id, text) in &now.texts {
            if was.texts.get(id) != Some(text) {
                out.push(Targeted {
                    target: PartAddress::new(&schema, LocalPartId(*id)),
                    operation: PatchOp::ReplaceText { text: text.clone() },
                });
            }
        }
        let env = self.document_env(session, store, bindings);
        for (list, items) in &now.lists {
            // A list at the top of the page only. One inside a block would
            // have no range while the block is not shown, and patching it
            // would read the page again at every change. No page served here
            // has one: a block a query decides is not rendered by this server
            // yet, and one a signal decides reads no query (ADR-0137).
            let top = template.chunks.iter().any(|c| {
                matches!(c, pw_render::Chunk::Dynamic(pw_render::Part::Each { collection, .. })
                    if collection == list)
            });
            if !top {
                continue;
            }
            let old = was.lists.get(list).map(Vec::as_slice).unwrap_or(&[]);
            out.extend(list_patches(
                template,
                &schema,
                list,
                old,
                items,
                &env,
                &self.templates,
            )?);
        }
        // Each block a query decides whose rendering changed, rendered again
        // where it is (ADR-0146).
        for (id, html) in &now.blocks {
            if was.blocks.get(id) != Some(html) {
                out.push(Targeted {
                    target: PartAddress::new(&schema, LocalPartId(*id)),
                    operation: PatchOp::ReplaceRange { html: html.clone() },
                });
            }
        }
        Ok(out)
    }
}

/// **A region a document leaves for its query to fill** (ADR-0148): which
/// part, the plan's entry for it (its query's component and policies), and
/// what this request gives the query.
struct StreamRun {
    part: u32,
    stream: serde_json::Value,
    args: Vec<Val>,
    streamed: bool,
}

/// **Each stream a plan names, with what this request gives its query**
/// (ADR-0148): a page parameter as `params` holds it, and
/// `current_session()` as the request's session.
fn stream_runs(
    plan: &serde_json::Value,
    session: &str,
    params: &BTreeMap<String, String>,
) -> Result<Vec<StreamRun>, String> {
    let mut out = Vec::new();
    for s in plan["streams"].as_array().into_iter().flatten() {
        let args = s["args"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|a| match a.as_str() {
                Some("current_session()") => Ok(Val::String(session.into())),
                Some(name) => params
                    .get(name)
                    .map(|v| Val::String(v.clone()))
                    .ok_or_else(|| {
                        format!("a stream's query is given `{name}`, which is not given")
                    }),
                None => Err(format!(
                    "a stream's argument this server cannot compute: {a}"
                )),
            })
            .collect::<Result<Vec<Val>, String>>()?;
        out.push(StreamRun {
            part: s["part"].as_u64().unwrap_or_default() as u32,
            stream: s.clone(),
            args,
            streamed: s["streamed"] == true,
        });
    }
    Ok(out)
}

/// **A document's streams, their queries running** (ADR-0148): each on a
/// thread of its own, started before the document is rendered, and each
/// given its `timeout` from when it started. A stream whose budget is spent
/// is given the host's failure; its answer, when it comes, is kept as its
/// policy says, for the next reader.
struct Settling {
    answers: std::sync::mpsc::Receiver<(u32, Result<Val, String>)>,
    /// Each stream not settled yet: whether it is streamed, and when its
    /// budget is spent.
    waiting: BTreeMap<u32, (bool, Option<std::time::Instant>)>,
    /// Settled and not taken yet, in the order they settled.
    settled: std::collections::VecDeque<(u32, Settled)>,
}

impl Settling {
    fn start<'scope, 'env>(
        scope: &'scope std::thread::Scope<'scope, 'env>,
        server: &'env Server,
        session: &'env str,
        runs: Vec<StreamRun>,
    ) -> Settling {
        let (tx, answers) = std::sync::mpsc::channel();
        let mut waiting = BTreeMap::new();
        let started = std::time::Instant::now();
        for run in runs {
            let budget = run.stream["policy"]["timeout_ms"]
                .as_u64()
                .map(|ms| started + std::time::Duration::from_millis(ms));
            waiting.insert(run.part, (run.streamed, budget));
            let tx = tx.clone();
            scope.spawn(move || {
                let answer = server.fetch_answer(session, &run.stream, &run.args);
                let _ = tx.send((run.part, answer));
            });
        }
        Settling {
            answers,
            waiting,
            settled: std::collections::VecDeque::new(),
        }
    }

    /// One answer, or each budget spent by now, or nothing left to come.
    fn receive(&mut self) {
        use std::sync::mpsc::RecvTimeoutError;
        let next = self.waiting.values().filter_map(|(_, at)| *at).min();
        let got = match next {
            Some(at) => self
                .answers
                .recv_timeout(at.saturating_duration_since(std::time::Instant::now())),
            None => self
                .answers
                .recv()
                .map_err(|_| RecvTimeoutError::Disconnected),
        };
        match got {
            Ok((part, answer)) => {
                if self.waiting.remove(&part).is_some() {
                    self.settled.push_back((part, settled_of(answer)));
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                let now = std::time::Instant::now();
                let spent: Vec<u32> = self
                    .waiting
                    .iter()
                    .filter(|(_, (_, at))| at.is_some_and(|at| at <= now))
                    .map(|(part, _)| *part)
                    .collect();
                for part in spent {
                    self.waiting.remove(&part);
                    self.settled.push_back((part, Settled::Failed(None)));
                }
            }
            // Every query answered or is gone: what still waits has nothing
            // coming.
            Err(RecvTimeoutError::Disconnected) => {
                for part in std::mem::take(&mut self.waiting).into_keys() {
                    self.settled.push_back((part, Settled::Failed(None)));
                }
            }
        }
    }

    /// **What the document is rendered with**: each stream the page waits
    /// for, settled, and each streamed one that has settled already, which
    /// the document shows at once rather than as a placeholder.
    fn for_document(&mut self) -> Vec<(u32, Settled)> {
        while self.waiting.values().any(|(streamed, _)| !streamed) {
            self.receive();
        }
        while let Ok((part, answer)) = self.answers.try_recv() {
            if self.waiting.remove(&part).is_some() {
                self.settled.push_back((part, settled_of(answer)));
            }
        }
        self.settled.drain(..).collect()
    }

    /// The next streamed region to settle, as it does; `None` when all have.
    fn next(&mut self) -> Option<(u32, Settled)> {
        while self.settled.is_empty() && !self.waiting.is_empty() {
            self.receive();
        }
        self.settled.pop_front()
    }
}

/// What every document ends with, written last: after each streamed
/// region's patch, when there are any (ADR-0148).
const DOCUMENT_END: &str = "</body>\n</html>\n";

/// A structural change to a keyed collection.
///
/// One type, holding both halves of the change: what it does to the server's
/// list, and what patch describes it. Splitting them is how a list and its
/// patches drift — the patch generator gets an off-by-one the state does not
/// have, and the page ends up with an order the server never held.
#[derive(Debug, Clone)]
enum MenuOp {
    /// Insert `(id, name)` before or after `at`.
    Insert {
        id: String,
        name: String,
        /// The instance to anchor to, or the head/tail of the collection.
        at: Option<String>,
        before: bool,
    },
    Remove {
        id: String,
    },
    /// Move `id` after `after`, or to the front when `after` is `None`.
    Move {
        id: String,
        after: Option<String>,
    },
    /// Change one item's ordinary field. Not structural — and included here to
    /// prove that it does NOT disturb its siblings.
    Rename {
        id: String,
        name: String,
    },
}

impl MenuOp {
    fn position(items: &[(String, String)], id: &str) -> Result<usize, String> {
        items
            .iter()
            .position(|(k, _)| k == id)
            .ok_or_else(|| format!("no item `{id}`"))
    }

    fn apply(&self, items: &mut Vec<(String, String)>) -> Result<(), String> {
        match self {
            MenuOp::Insert {
                id,
                name,
                at,
                before,
            } => {
                // A duplicate key is REFUSED, not disambiguated. Two instances
                // with one key means one address for two places, which is
                // `docs/RISK_QUEUE.md` 25 arriving through the front door.
                if items.iter().any(|(k, _)| k == id) {
                    return Err(format!("duplicate key `{id}`"));
                }
                if id.is_empty() {
                    return Err("an item with no key has no address".into());
                }
                let at = match at {
                    Some(a) => {
                        let p = Self::position(items, a)?;
                        if *before { p } else { p + 1 }
                    }
                    // No anchor: the head for `before`, the tail for `after`.
                    // This is the only way an item can enter an empty
                    // collection without re-rendering the document.
                    None if *before => 0,
                    None => items.len(),
                };
                items.insert(at, (id.clone(), name.clone()));
            }
            MenuOp::Remove { id } => {
                let at = Self::position(items, id)?;
                items.remove(at);
            }
            MenuOp::Move { id, after } => {
                let from = Self::position(items, id)?;
                let item = items.remove(from);
                let to = match after {
                    None => 0,
                    Some(a) => Self::position(items, a)? + 1,
                };
                items.insert(to, item);
            }
            MenuOp::Rename { id, name } => {
                let at = Self::position(items, id)?;
                items[at].1 = name.clone();
            }
        }
        Ok(())
    }

    /// The patch describing this change, with markup the renderer produced.
    fn patch(
        &self,
        template: &Template,
        part: LocalPartId,
        items: &[(String, String)],
        env: &Env,
        others: &[Template],
    ) -> Result<PatchOp, pw_render::Blocked> {
        let each = part;
        let token = |id: &str| pw_render::instance_token_of(each, &item(id, ""), "id", env);
        Ok(match self {
            MenuOp::Insert {
                id,
                name,
                at,
                before,
            } => {
                let html =
                    pw_render::render_instance(template, each, &item(id, name), env, others)?;
                let instance = at.as_deref().map(token);
                if *before {
                    PatchOp::InsertBefore { instance, html }
                } else {
                    PatchOp::InsertAfter { instance, html }
                }
            }
            MenuOp::Remove { id } => PatchOp::RemoveInstance {
                instance: token(id),
            },
            MenuOp::Move { id, after } => PatchOp::MoveInstance {
                instance: token(id),
                after: after.as_deref().map(token),
            },
            MenuOp::Rename { name, .. } => {
                // The item's TEXT part, addressed inside the instance by
                // `target` below. A rename that reinserted the item would also
                // work visually and would destroy the node — which is the whole
                // distinction this milestone exists to make.
                let _ = items;
                PatchOp::ReplaceText { text: name.clone() }
            }
        })
    }

    /// For `Rename`, the address is inside the instance rather than the loop.
    fn target(
        &self,
        base: &PartAddress,
        scope: LocalPartId,
        inner: LocalPartId,
        env: &Env,
        each: PartId,
    ) -> PartAddress {
        match self {
            MenuOp::Rename { id, .. } => {
                let token = pw_render::instance_token_of(each, &item(id, ""), "id", env);
                PartAddress::new(&base.template, inner).within(scope, token)
            }
            _ => base.clone(),
        }
    }
}

/// A cart as the WIT's `domain-cart` record: its lines, each with an item,
/// a quantity and a unit price.
fn cart_value(lines: &[(String, i64)]) -> Val {
    Val::Record(vec![(
        "lines".into(),
        Val::List(
            lines
                .iter()
                .map(|(item, quantity)| {
                    Val::Record(vec![
                        ("item-id".into(), Val::String(item.clone())),
                        ("quantity".into(), Val::S64(*quantity)),
                        (
                            "unit-price".into(),
                            Val::Record(vec![("minor-units".into(), Val::S64(450))]),
                        ),
                    ])
                })
                .collect(),
        ),
    )])
}

/// **A binding's policy, as `pw-resource`'s manifest** (ADR-0127).
/// **A query's value, from its answer** (ADR-0147): the `Ok` value, and a
/// declared error as a failure.
fn unwrapped(component_id: &str, answer: Val) -> Result<Val, String> {
    match answer {
        Val::Result(Ok(Some(v))) => Ok(*v),
        Val::Result(Err(e)) => Err(format!("{component_id} answered {e:?}")),
        v => Ok(v),
    }
}

/// **What a stream's query settled to, from its answer** (ADR-0148): its
/// `Ok` value; `Some` of its declared error; or `None`, for the host's
/// failure, a spent budget or a trap.
fn settled_of(answer: Result<Val, String>) -> Settled {
    match answer {
        Ok(Val::Result(Ok(Some(v)))) => Settled::Ready(val_to_value(&v)),
        Ok(Val::Result(Ok(None))) => Settled::Ready(Value::Record(BTreeMap::new())),
        Ok(Val::Result(Err(Some(e)))) => Settled::Failed(Some(val_to_value(&e))),
        Ok(Val::Result(Err(None))) => Settled::Failed(Some(Value::Record(BTreeMap::new()))),
        Ok(v) => Settled::Ready(val_to_value(&v)),
        Err(_) => Settled::Failed(None),
    }
}

fn runtime_manifest(resource: &str, policy: &serde_json::Value) -> pw_resource::Manifest {
    let mut m = pw_resource::Manifest::new(resource)
        .freshness(policy["freshness_ms"].as_u64().unwrap_or(0))
        .attempts(policy["attempts"].as_u64().unwrap_or(1) as u32);
    if let Some(t) = policy["timeout_ms"].as_u64() {
        m.timeout = t;
    }
    if policy["cache"] == "private" || policy["privacy"] != "public" {
        m = m.private();
    }
    if policy["cache"] == "none" {
        m = m.uncached();
    }
    m
}

/// **An entry's key** (ADR-0127): the arguments the policy's `key` names, by
/// position, and for a private entry the session first. `None` when an
/// argument the key names was not given.
fn entry_key(session: &str, policy: &serde_json::Value, args: &[Val]) -> Option<String> {
    let mut parts = Vec::new();
    if policy["cache"] == "private" || policy["privacy"] != "public" {
        parts.push(format!("session={session}"));
    }
    for i in policy["key"].as_array().into_iter().flatten() {
        let v = args.get(i.as_u64()? as usize)?;
        parts.push(val_to_json(v).to_string());
    }
    Some(parts.join("\u{1f}"))
}

/// **A key's value, as a component is given it** (ADR-0152): a `String`, an
/// `Int` or a `Bool`, which PW5308 holds a key to.
fn key_val(v: &serde_json::Value) -> Option<Val> {
    match v {
        serde_json::Value::String(s) => Some(Val::String(s.clone())),
        serde_json::Value::Bool(b) => Some(Val::Bool(*b)),
        serde_json::Value::Number(n) => n.as_i64().map(Val::S64),
        _ => None,
    }
}

/// **A keyed binding's entry, for one session** (ADR-0152): its version is
/// the number of the read its page last applied.
fn keyed_entry(session: &str, binding: &str) -> ResourceEntryId {
    ResourceEntryId::derive(
        &EntryIdentity::new(
            &format!("store.page.{binding}#key"),
            &[session],
            Partition::Session {
                id: session.to_string(),
            },
        )
        .generation(BUILD),
        &IDENTITY,
    )
}

/// **A menu item's category** (E14, T07): hot or cold, by the item.
fn category_of(item: &str) -> &'static str {
    match item {
        "cold-brew" => "cold",
        _ => "hot",
    }
}

/// The store `/StorePage.html` and `/` show (ADR-0125), the first of the two
/// this server holds (ADR-0162).
const STORE_ID: &str = "47";
const STORE_NAME: &str = "Blue Bottle";

/// **Why a page's values could not be read** (ADR-0163): its address names
/// nothing, as the page declares, or it cannot be shown now. Told apart by
/// type, so no failure's text can make it the other.
#[derive(Debug)]
enum Unread {
    /// A binding's query answered the case the page's `not_found_on` names:
    /// answered 404.
    NotFound(String),
    /// Any other failure (ADR-0147): answered 503.
    Failed(String),
}

impl Unread {
    /// The same failure, said of `what`.
    fn of(self, what: &str) -> Unread {
        match self {
            Unread::NotFound(why) => Unread::NotFound(format!("{what}: {why}")),
            Unread::Failed(why) => Unread::Failed(format!("{what}: {why}")),
        }
    }
}

impl From<String> for Unread {
    fn from(why: String) -> Unread {
        Unread::Failed(why)
    }
}

impl std::fmt::Display for Unread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unread::NotFound(why) => write!(f, "not found: {why}"),
            Unread::Failed(why) => f.write_str(why),
        }
    }
}

impl From<Unread> for String {
    fn from(unread: Unread) -> String {
        unread.to_string()
    }
}

/// **What a page whose address names nothing is answered** (ADR-0163), with
/// 404: accessible, and nothing of the page's.
const NOT_FOUND_PAGE: &str = "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
     <title>Not found</title>\n</head>\n<body>\n<main>\n<h1>Not found</h1>\n\
     <p>Nothing is at this address.</p>\n</main>\n</body>\n</html>\n";

/// **The second store** (ADR-0162): its id and name. Its menu is its own,
/// and nothing changes it, so a change to store 47's is seen not to reach
/// it (§15.6 test 11).
const SECOND_STORE: (&str, &str) = ("48", "Harbor Coffee");

/// The second store's menu.
fn second_menu() -> Vec<(String, String)> {
    [
        ("drip", "Drip Coffee"),
        ("matcha", "Matcha Latte"),
        ("scone", "Blueberry Scone"),
    ]
    .map(|(id, name)| (id.to_string(), name.to_string()))
    .to_vec()
}

/// A store's name, by its id: the stores this server holds.
fn store_named(id: &str) -> Option<&'static str> {
    match id {
        STORE_ID => Some(STORE_NAME),
        _ if id == SECOND_STORE.0 => Some(SECOND_STORE.1),
        _ => None,
    }
}

/// A component value as the renderer reads it: a record by its Pleris field
/// names (the WIT's kebab case undone), an `Int` as an `Int`.
fn val_to_value(v: &Val) -> Value {
    match v {
        Val::String(s) => Value::Text(s.clone()),
        Val::Bool(b) => Value::Bool(*b),
        Val::S64(n) => Value::Int(*n),
        Val::S32(n) => Value::Int(i64::from(*n)),
        Val::List(items) => Value::List(items.iter().map(val_to_value).collect()),
        Val::Record(fields) => Value::Record(
            fields
                .iter()
                .map(|(n, v)| (n.replace('-', "_"), val_to_value(v)))
                .collect(),
        ),
        // A case, as a `{#match}` arm names it (ADR-0146): `Some`, `None`,
        // `Ok` and `Err` as Pleris writes them, and a declared case by its
        // WIT name (ADR-0061). A payload of several fields is a list.
        Val::Variant(case, payload) => Value::Variant {
            case: case.clone(),
            payload: payload.as_deref().map(|v| Box::new(val_to_value(v))),
        },
        Val::Enum(case) => Value::Variant {
            case: case.clone(),
            payload: None,
        },
        Val::Option(v) => Value::Variant {
            case: if v.is_some() { "Some" } else { "None" }.to_string(),
            payload: v.as_deref().map(|v| Box::new(val_to_value(v))),
        },
        Val::Result(r) => {
            let (case, v) = match r {
                Ok(v) => ("Ok", v),
                Err(v) => ("Err", v),
            };
            Value::Variant {
                case: case.to_string(),
                payload: v.as_deref().map(|v| Box::new(val_to_value(v))),
            }
        }
        Val::Tuple(items) => Value::List(items.iter().map(val_to_value).collect()),
        other => Value::Text(format!("{other:?}")),
    }
}

/// A value as a text part shows it, as the renderer writes it: `None` for a
/// list or a record, which a text part does not show.
fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::Text(t) => Some(t.clone()),
        Value::Int(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The value at a dotted path from a record: `quantity` from a line.
fn field_at<'v>(v: &'v Value, path: &str) -> Option<&'v Value> {
    path.split('.')
        .filter(|s| !s.is_empty())
        .try_fold(v, |v, field| match v {
            Value::Record(fields) => fields.get(field),
            _ => None,
        })
}

/// **A list's change, as keyed operations** (ADR-0145). The list the
/// template iterates over `list` becomes `new`:
/// - an item whose key left is removed;
/// - a new one is inserted where it now is;
/// - one out of place is moved;
/// - one whose value changed is set in place, text part by text part, or
///   replaced when its markup holds more than text.
///
/// An item whose key stayed keeps its nodes, as E7-P keeps the menu's: a
/// focus, a scroll, and anything a test marked on it stay.
fn list_patches(
    template: &Template,
    schema: &TemplateSchemaId,
    list: &str,
    old: &[Value],
    new: &[Value],
    env: &Env,
    others: &[Template],
) -> Result<Vec<Targeted>, String> {
    let Some((each, binding, key, body)) = each_over(&template.chunks, list) else {
        return Err(format!("`{}` iterates no list `{list}`", template.path));
    };
    let target = PartAddress::new(schema, LocalPartId(each.0));
    let token = |v: &Value| pw_render::instance_token_of(each, v, &key, env);
    let render = |v: &Value| {
        pw_render::render_instance(template, each, v, env, others).map_err(|b| format!("{b:?}"))
    };
    // Where the instance after `prev` goes: after it, or at the head.
    let insert = |prev: &Option<pw_document::InstanceToken>, html: String| match prev {
        None => PatchOp::InsertBefore {
            instance: None,
            html,
        },
        Some(p) => PatchOp::InsertAfter {
            instance: Some(p.clone()),
            html,
        },
    };
    let at = |operation: PatchOp| Targeted {
        target: target.clone(),
        operation,
    };
    // The text parts an item's markup is, when it is text alone: each read
    // from the item by its path.
    let texts: Option<Vec<(PartId, String)>> = body
        .iter()
        .filter_map(|c| match c {
            pw_render::Chunk::Dynamic(p) => Some(p),
            _ => None,
        })
        .map(|p| match p {
            pw_render::Part::Text { id, value, .. } => value
                .strip_prefix(&format!("{binding}."))
                .map(|field| (*id, field.to_string())),
            _ => None,
        })
        .collect();

    let wanted: Vec<pw_document::InstanceToken> = new.iter().map(token).collect();
    let mut out = Vec::new();
    // What the document holds, as each operation leaves it.
    let mut current: Vec<(pw_document::InstanceToken, Value)> = Vec::new();
    for v in old {
        let t = token(v);
        if wanted.contains(&t) {
            current.push((t, v.clone()));
        } else {
            out.push(at(PatchOp::RemoveInstance { instance: t }));
        }
    }
    let mut prev: Option<pw_document::InstanceToken> = None;
    for (i, item) in new.iter().enumerate() {
        let t = wanted[i].clone();
        match current.iter().position(|(c, _)| *c == t) {
            None => {
                out.push(at(insert(&prev, render(item)?)));
                current.insert(i, (t.clone(), item.clone()));
            }
            Some(found) => {
                if found != i {
                    out.push(at(PatchOp::MoveInstance {
                        instance: t.clone(),
                        after: prev.clone(),
                    }));
                    let moved = current.remove(found);
                    current.insert(i, moved);
                }
                if current[i].1 != *item {
                    let was = current[i].1.clone();
                    let in_place = texts.as_ref().and_then(|texts| {
                        texts
                            .iter()
                            .map(|(id, field)| {
                                let old = field_at(&was, field).and_then(text_of)?;
                                let now = field_at(item, field).and_then(text_of)?;
                                Some((old != now).then(|| {
                                    Targeted {
                                        target: PartAddress::new(schema, LocalPartId(id.0))
                                            .within(LocalPartId(each.0), t.clone()),
                                        operation: PatchOp::ReplaceText { text: now },
                                    }
                                }))
                            })
                            .collect::<Option<Vec<_>>>()
                    });
                    match in_place {
                        Some(changes) => out.extend(changes.into_iter().flatten()),
                        // More than text: the instance is rendered again, where
                        // it is.
                        None => {
                            out.push(at(PatchOp::RemoveInstance {
                                instance: t.clone(),
                            }));
                            out.push(at(insert(&prev, render(item)?)));
                        }
                    }
                    current[i].1 = item.clone();
                }
            }
        }
        prev = Some(t);
    }
    Ok(out)
}

/// The `{#each}` over `list`: its part, the name each item is bound to, its
/// key's path, and its markup. Searched through every block, as the
/// renderer's parts are.
fn each_over<'t>(
    chunks: &'t [pw_render::Chunk],
    list: &str,
) -> Option<(PartId, &'t str, String, &'t [pw_render::Chunk])> {
    for c in chunks {
        let pw_render::Chunk::Dynamic(p) = c else {
            continue;
        };
        if let pw_render::Part::Each {
            id,
            collection,
            binding,
            key,
            body,
        } = p
            && collection == list
        {
            return Some((*id, binding.as_str(), key.clone().unwrap_or_default(), body));
        }
        for region in p.nested() {
            if let Some(found) = each_over(region, list) {
                return Some(found);
            }
        }
    }
    None
}

/// A component value as a speculation module decodes it (ADR-0122).
fn val_to_json(v: &Val) -> serde_json::Value {
    match v {
        Val::String(s) => serde_json::json!(s),
        Val::Bool(b) => serde_json::json!(b),
        Val::S64(n) => serde_json::json!(n),
        Val::S32(n) => serde_json::json!(n),
        Val::List(items) => serde_json::Value::Array(items.iter().map(val_to_json).collect()),
        Val::Record(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(n, v)| (n.replace('-', "_"), val_to_json(v)))
                .collect(),
        ),
        // A case as a handler's compiled code reads one (ADR-0157, `js_pure`'s
        // `decode`): its name as the WIT spells it, and its payload, several
        // fields as an array.
        Val::Result(r) => {
            let (case, payload) = match r {
                Ok(v) => ("ok", v),
                Err(v) => ("err", v),
            };
            case_json(case, payload.as_deref())
        }
        Val::Option(o) => match o {
            Some(v) => case_json("some", Some(v)),
            None => case_json("none", None),
        },
        Val::Variant(name, payload) => case_json(name, payload.as_deref()),
        Val::Enum(name) => case_json(name, None),
        Val::Tuple(items) => serde_json::Value::Array(items.iter().map(val_to_json).collect()),
        other => serde_json::json!(format!("{other:?}")),
    }
}

/// **A command's answer, for the handler that called it** (ADR-0157): `Ok`
/// without its value, which reaches the page from the resource, so the page
/// has one source for what it shows; or the declared `Err`, whole. A result
/// that is not a `Result` answers nothing.
fn answer_json(v: &Val) -> serde_json::Value {
    match v {
        Val::Result(Ok(_)) => serde_json::json!({ "$case": "ok" }),
        Val::Result(Err(e)) => case_json("err", e.as_deref()),
        _ => serde_json::Value::Null,
    }
}

/// A case on the wire: `{ "$case": name }`, and its payload as `value`.
fn case_json(name: &str, payload: Option<&Val>) -> serde_json::Value {
    match payload {
        Some(v) => serde_json::json!({ "$case": name, "value": val_to_json(v) }),
        None => serde_json::json!({ "$case": name }),
    }
}

/// The menu's `(id, name)` items, from the `Menu` query's value.
fn menu_items(menu: &Val) -> Vec<(String, String)> {
    let Val::List(items) = menu else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let Val::Record(fields) = item else {
                return None;
            };
            let field = |name: &str| {
                fields
                    .iter()
                    .find(|(n, _)| n == name)
                    .and_then(|(_, v)| match v {
                        Val::String(s) => Some(s.clone()),
                        _ => None,
                    })
            };
            Some((field("id")?, field("name")?))
        })
        .collect()
}

fn default_menu() -> Vec<(String, String)> {
    [
        ("espresso", "Espresso"),
        ("cortado", "Cortado"),
        ("cold-brew", "Cold Brew"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect()
}

fn item(id: &str, name: &str) -> Value {
    let mut f = BTreeMap::new();
    f.insert("id".to_string(), Value::Text(id.into()));
    f.insert("name".to_string(), Value::Text(name.into()));
    Value::Record(f)
}

fn menu_value(items: &[(String, String)]) -> Value {
    Value::List(items.iter().map(|(id, name)| item(id, name)).collect())
}

/// The containment the large-menu case relies on.
///
/// `content-visibility: auto` rather than virtualization: a menu is
/// semantically a list of independent items, so every item stays in the
/// document, findable and in the accessibility tree, and the ones off screen
/// are simply not laid out until they approach the viewport. Virtualization
/// removes items from the document, which is a correctness cost that has to be
/// justified per case rather than adopted as a default.
///
/// `contain-intrinsic-size` is not optional with it: without a placeholder
/// size the scrollbar jumps as items are realized, which is the visible defect
/// that makes people abandon containment and reach for virtualization.
const STYLE: &str = "#menu li { content-visibility: auto; contain-intrinsic-size: auto 42px; }";

/// **The events a command declares it emits**, with their values
/// (ADR-0104).
///
/// Read from the command's `emits` edges in the compiler's graph: `emits
/// CartChanged(current_session())` is `Events.CartChanged` carrying the
/// session. Until 2026-09-26 this server committed `CartChanged` after any
/// cart write, whatever the command declared, so a command that emitted
/// nothing worked here and nowhere else. A value the server cannot compute
/// is refused, never guessed.
fn declared_events(
    graph: &pw_materialize::Graph,
    command: &str,
    session: &str,
) -> Result<Vec<pw_materialize::Event>, String> {
    graph
        .edges
        .iter()
        .filter(|e| e.kind == pw_materialize::EdgeKind::Emits && e.from == command)
        .map(|e| {
            let values = e
                .key
                .iter()
                .map(|arg| match arg.as_str() {
                    "current_session()" => Ok(session),
                    other => Err(format!(
                        "`{command}` emits `{}` with `{other}`, which this server cannot compute",
                        e.to
                    )),
                })
                .collect::<Result<Vec<&str>, String>>()?;
            Ok(pw_materialize::Event::new(&e.to, &values))
        })
        .collect()
}

/// The graph, as `pw emit-graph` produced it. Read at build time so the server
/// cannot drift from the compiler's answer between runs.
#[cfg(test)]
const GRAPH: &str = include_str!("../../../../runtime/pw-materialize/tests/store-graph.json");

fn main() {
    let dist = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "dist".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3143);

    // What `pw build` wrote (ADR-0123): the second argument, or `build/`
    // beside the documents, where `run.sh` writes it.
    let build = std::env::args()
        .nth(2)
        .unwrap_or_else(|| format!("{dist}/build"));
    let server = match Server::from_build(dist.into(), build.into()) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("pw dev server: {e}\nrun spikes/own-renderer/run.sh, which runs `pw build`");
            std::process::exit(1);
        }
    };
    let missing = server.uncompiled_handlers();
    if !missing.is_empty() {
        eprintln!(
            "pw dev server: no compiled module for handler(s) {} under {}/handlers; \
             run spikes/own-renderer/run.sh, which runs `pw emit-handlers`",
            missing.join(", "),
            server.dist.display()
        );
        std::process::exit(1);
    }
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    println!("pw dev server on {port}");

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let server = server.clone();
        // A thread per connection. The subscription blocks, and a
        // single-threaded loop would make the first subscriber the last.
        std::thread::spawn(move || handle(&server, stream));
    }
}

fn handle(server: &Server, mut stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut request = String::new();
    if reader.read_line(&mut request).is_err() {
        return;
    }
    let mut parts = request.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));

    let mut headers = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        headers.push_str(&line);
    }
    let session = session_of(&headers);
    let fresh = !headers.contains("pw-session=");

    // The body, when the request declares one. Bounded: a command's arguments
    // are a few values, and a length nothing checks is an allocation anyone
    // can ask for.
    const MAX_BODY: usize = 64 * 1024;
    let length = headers
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    if length > MAX_BODY {
        respond(
            &mut stream,
            413,
            "text/plain; charset=utf-8",
            &session,
            fresh,
            b"request body too large",
        );
        return;
    }
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }

    let route = path.split('?').next().unwrap_or("/");
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    // The page a path names by its route, and the parameters it gives
    // (ADR-0162).
    let routed = match method {
        "GET" => server.routed(route),
        _ => None,
    };

    match (method, route) {
        // **A command the compiler built, by its component id**: what a
        // compiled handler calls (`context.command(id, args)` in
        // `dist/handlers/<identity>.mjs`). The body is the arguments, as a JSON
        // array, typed here by the component's own parameters; the guard admits
        // only components this server hosts, so an unknown id falls through to
        // the 404 below rather than to a default.
        ("POST", route)
            if route
                .strip_prefix("/command/")
                .is_some_and(|id| server.components.contains_key(id)) =>
        {
            let id = &route["/command/".len()..];
            let args = match serde_json::from_slice::<Vec<serde_json::Value>>(&body) {
                Ok(args) => args,
                Err(e) => {
                    let why =
                        serde_json::Value::String(format!("the body is not a JSON array: {e}"));
                    respond_json(
                        &mut stream,
                        400,
                        &session,
                        fresh,
                        &format!("{{\"committed\":false,\"error\":{why}}}"),
                    );
                    return;
                }
            };
            // The interaction this request belongs to, sent by the runtime
            // once per press and kept by a retry (ADR-0121).
            let interaction = headers.lines().find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.trim()
                    .eq_ignore_ascii_case("pw-interaction")
                    .then(|| v.trim().to_string())
            });
            match server.command_answer(id, &session, &args, interaction.as_deref()) {
                // A malformed request: nothing ran.
                Err(e) => {
                    let why = serde_json::Value::String(e);
                    respond_json(
                        &mut stream,
                        400,
                        &session,
                        fresh,
                        &format!("{{\"committed\":false,\"error\":{why}}}"),
                    );
                }
                Ok(answered) => {
                    if let Some(why) = &answered.why
                        && std::env::var("PW_TRACE").is_ok()
                    {
                        eprintln!("{id}: {why}");
                    }
                    // A commit says which versions it produced (ADR-0122), so a
                    // page can tell when the value it is sent includes it. A
                    // version, not a value: the value reaches the page from the
                    // resource, as every change does.
                    let mut body = serde_json::json!({ "committed": answered.committed });
                    if answered.committed {
                        body["basis"] = server.committed_basis(&session);
                    }
                    // And what the command answered, for the handler that
                    // called it (ADR-0157): `Ok`, or the declared `Err` it can
                    // tell the customer about.
                    if let Some(result) = answered.result {
                        body["result"] = result;
                    }
                    respond_json(&mut stream, 202, &session, fresh, &body.to_string());
                }
            }
        }
        ("POST", "/command/add_and_fail") => {
            let _ = server.command(
                "store.page.add_to_cart",
                &session,
                &[Val::String("espresso".into()), Val::S64(1)],
                true,
            );
            respond_json(&mut stream, 500, &session, fresh, "{\"committed\":false}");
        }
        ("POST", "/command/menu_changed") => {
            // An event nothing about the cart listens for. It reaches the
            // materializer and selects no cart entry.
            let _ = server.materializer.command::<String>(|_tx| {
                Ok(vec![pw_materialize::Event::new(
                    "Events.MenuChanged",
                    &["47"],
                )])
            });
            server.drain(&session);
            respond_json(&mut stream, 202, &session, fresh, "{}");
        }
        // **The kitchen sets a session's order** (E14, T04): a benchmark's
        // hook, as every stack in it has one. `?status=preparing` sets the
        // status's case; no status removes the order. The page reads it again
        // when it is next served.
        ("POST", "/bench/order") => {
            let status = query
                .split('&')
                .find_map(|p| p.strip_prefix("status="))
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let mut orders = server.orders.lock().expect("orders");
            match status {
                Some(s) => orders.insert(session.clone(), s),
                None => orders.remove(&session),
            };
            drop(orders);
            respond_json(&mut stream, 200, &session, fresh, "{}");
        }
        // The recommender a test sets (ADR-0148): how long it takes, how it
        // fails (`declared` or `host`), and what it recommends, `id:Name`
        // pairs. Nothing kept from before is served after: what a page reads
        // next is what the recommender now says.
        ("POST", "/bench/recommendations") => {
            let q = |k: &str| {
                query
                    .split('&')
                    .find_map(|p| p.strip_prefix(&format!("{k}=")))
                    .and_then(percent_decoded)
            };
            let mut set = Recommender::default();
            if let Some(ms) = q("delay").and_then(|v| v.parse().ok()) {
                set.delay_ms = ms;
            }
            set.fail = q("fail").filter(|f| !f.is_empty());
            if let Some(items) = q("items") {
                set.items = items
                    .split(',')
                    .filter_map(|pair| pair.split_once(':'))
                    .map(|(id, name)| (id.to_string(), name.to_string()))
                    .collect();
            }
            *server.recommender.lock().expect("recommender") = set;
            // Every query a page of this build reads: what it kept, and what
            // is still running with the recommender as it was.
            let resources: std::collections::BTreeSet<String> = server
                .plans
                .values()
                .flat_map(|plan| {
                    ["bindings", "streams"].into_iter().flat_map(move |k| {
                        plan[k]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|b| b["resource"].as_str().map(str::to_string))
                    })
                })
                .collect();
            for resource in resources {
                server.queries.invalidate(&resource);
            }
            respond_json(&mut stream, 200, &session, fresh, "{}");
        }
        // The store's staff post a notice (E14, T09): at the source, and
        // nothing is told. What a page shows of it is the `Notice` query's
        // to keep, for as long as its freshness says.
        ("POST", "/bench/notice") => {
            let text = query
                .split('&')
                .find_map(|p| p.strip_prefix("text="))
                .and_then(percent_decoded)
                .unwrap_or_default();
            *server.notice.lock().expect("notice") = text;
            respond_json(&mut stream, 200, &session, fresh, "{}");
        }
        // **A binding read again, for the key the browser asks for**
        // (ADR-0152): `?binding=..&seq=..&doc=..&key=<JSON>`. The patch set
        // comes in the session's frames; this says whether it was applied.
        ("GET", "/pw-read") => {
            let param = |name: &str| {
                query
                    .split('&')
                    .find_map(|p| p.strip_prefix(&format!("{name}=") as &str))
                    .and_then(percent_decoded)
            };
            let number = |name: &str| param(name).and_then(|v| v.parse::<u64>().ok());
            let key: Option<BTreeMap<String, serde_json::Value>> =
                param("key").and_then(|k| serde_json::from_str(&k).ok());
            let (Some(binding), Some(seq), Some(document), Some(key)) =
                (param("binding"), number("seq"), number("doc"), key)
            else {
                return respond_json(&mut stream, 400, &session, fresh, "{}");
            };
            // The browser has left when its connection is closed: a read
            // finds nothing more to read, and nothing waiting.
            let watched = stream.try_clone().ok();
            if let Some(w) = &watched {
                let _ = w.set_nonblocking(true);
            }
            let left = move || {
                watched
                    .as_ref()
                    .is_some_and(|w| matches!(w.peek(&mut [0u8; 1]), Ok(0)))
            };
            let outcome = server.read_keyed(&session, &binding, seq, document, &key, &left);
            // The watch made the connection nonblocking; the answer is
            // written as every other is.
            let _ = stream.set_nonblocking(false);
            match outcome {
                Ok(outcome) => {
                    let body = match outcome {
                        KeyOutcome::Applied => serde_json::json!({ "applied": seq }),
                        KeyOutcome::Superseded => serde_json::json!({ "superseded": seq }),
                    };
                    respond_json(&mut stream, 200, &session, fresh, &body.to_string());
                }
                Err(why) => {
                    if std::env::var("PW_TRACE").is_ok() {
                        eprintln!("pw-read {binding}: {why}");
                    }
                    respond_json(&mut stream, 400, &session, fresh, "{}");
                }
            }
        }
        // An item sold out at the source, or back in stock (charter §15.5's
        // forced stale item): `?item=cortado&available=false`. A page that
        // shows it is not told; the next add is refused by name.
        ("POST", "/bench/stock") => {
            let param = |name: &str| {
                query
                    .split('&')
                    .find_map(|p| p.strip_prefix(&format!("{name}=") as &str))
                    .and_then(percent_decoded)
            };
            let Some(item) = param("item") else {
                return respond_json(&mut stream, 400, &session, fresh, "{}");
            };
            let mut sold_out = server.sold_out.lock().expect("sold out");
            if param("available").as_deref() == Some("false") {
                sold_out.insert(item);
            } else {
                sold_out.remove(&item);
            }
            drop(sold_out);
            respond_json(&mut stream, 200, &session, fresh, "{}");
        }
        // Which of the menu's categories is slow, and how slow (E14, T07):
        // `?slow=hot&delay=1500`. Nothing is told.
        ("POST", "/bench/category") => {
            let param = |name: &str| {
                query
                    .split('&')
                    .find_map(|p| p.strip_prefix(&format!("{name}=") as &str))
                    .and_then(percent_decoded)
            };
            *server.categories.lock().expect("categories") = Categories {
                slow: param("slow"),
                delay_ms: param("delay").and_then(|d| d.parse().ok()).unwrap_or(0),
            };
            respond_json(&mut stream, 200, &session, fresh, "{}");
        }
        // The kitchen's prep time changes (E14, T02): at the source, and
        // nothing is told. A page reads it as its query's policies say.
        ("POST", "/bench/prep") => {
            match query
                .split('&')
                .find_map(|p| p.strip_prefix("minutes="))
                .and_then(|m| m.parse::<i64>().ok())
            {
                Some(minutes) => {
                    *server.prep_minutes.lock().expect("prep minutes") = minutes;
                    respond_json(&mut stream, 200, &session, fresh, "{}");
                }
                None => respond_json(&mut stream, 400, &session, fresh, "{}"),
            }
        }
        // How many times each slow source was asked (E14, T02 and T09), as
        // every stack in the benchmark reports it.
        ("GET", "/bench/calls") => {
            let calls = server.calls.lock().expect("calls");
            let count = |op: &str| calls.get(op).copied().unwrap_or(0);
            let body = serde_json::json!({
                "notice": count("store:data/notices#current"),
                "prep": count("store:data/kitchen#prep-minutes"),
                "category": count("store:data/menus#in-category"),
                "category_stopped": server
                    .category_stopped
                    .load(std::sync::atomic::Ordering::SeqCst),
            });
            respond_json(&mut stream, 200, &session, fresh, &body.to_string());
        }
        // How the estimator behaves for the request's session (E14, T10): how
        // long it takes, how it fails, and what it estimates. What was kept
        // for the session is not served after.
        ("POST", "/bench/estimate") => {
            let q = |k: &str| {
                query
                    .split('&')
                    .find_map(|p| p.strip_prefix(&format!("{k}=")))
                    .and_then(percent_decoded)
            };
            let mut set = Estimator::default();
            if let Some(ms) = q("delay").and_then(|v| v.parse().ok()) {
                set.delay_ms = ms;
            }
            set.fail = q("fail").filter(|f| !f.is_empty());
            if let Some(m) = q("minutes").and_then(|v| v.parse().ok()) {
                set.minutes = m;
            }
            server
                .estimators
                .lock()
                .expect("estimators")
                .insert(session.clone(), set);
            let own = format!("session={session}");
            server
                .queries
                .evict_where(|k| k.key == own || k.key.starts_with(&format!("{own}\u{1f}")));
            respond_json(&mut stream, 200, &session, fresh, "{}");
        }
        // E7-P's structural commands. Each mutates the keyed collection and
        // broadcasts the patch that describes the mutation. A refusal returns
        // 409 and changes nothing — including the version.
        ("POST", "/command/menu") => {
            let q = |k: &str| {
                query
                    .split('&')
                    .find_map(|p| p.strip_prefix(&format!("{k}=")))
                    .map(|v| v.replace("%20", " "))
            };
            let id = q("id").unwrap_or_default();
            let op = match q("op").as_deref() {
                Some("insert_before") | Some("insert_after") => MenuOp::Insert {
                    name: q("name").unwrap_or_else(|| id.clone()),
                    id,
                    at: q("at"),
                    before: q("op").as_deref() == Some("insert_before"),
                },
                Some("remove") => MenuOp::Remove { id },
                Some("move") => MenuOp::Move {
                    id,
                    after: q("after"),
                },
                Some("rename") => MenuOp::Rename {
                    name: q("name").unwrap_or_default(),
                    id,
                },
                _ => {
                    respond_json(&mut stream, 400, &session, fresh, "{\"error\":\"no op\"}");
                    return;
                }
            };
            match server.broadcast_menu(op) {
                Ok(()) => respond_json(&mut stream, 202, &session, fresh, "{\"committed\":true}"),
                Err(e) => respond_json(
                    &mut stream,
                    409,
                    &session,
                    fresh,
                    &format!("{{\"committed\":false,\"refused\":\"{e}\"}}"),
                ),
            }
        }
        // A patch addressing an instance that does not exist. Not reachable
        // through any command — injected, because the property under test is
        // what the BROWSER does when a server and a page disagree, and a
        // correct server never produces the disagreement.
        ("POST", "/command/menu_ghost") => {
            let entry = ResourceEntryId::derive(&menu_identity(STORE_ID), &IDENTITY);
            let version = server.menu_version(STORE_ID);
            let mut queue = server.pending.lock().expect("pending");
            for doc in documents_of(&queue, &session) {
                if let Some(waiting) = queue.get_mut(&doc) {
                    waiting.push(StreamFrame::Patch(Patch {
                        protocol: CURRENT,
                        basis: CausalBasis::of(entry.clone(), Version(version.0 + 1)),
                        target: server.menu_address(),
                        operation: PatchOp::RemoveInstance {
                            instance: pw_document::InstanceToken::from_wire("nosuchinstance"),
                        },
                    }));
                }
            }
            drop(queue);
            respond_json(&mut stream, 202, &session, fresh, "{}");
        }
        // What the materializer holds for the PUBLIC menu entry. Enough to
        // show that a second reader reads rather than regenerates, and no
        // more: the storage key never appears, because the browser has no
        // business knowing how an entry is stored.
        ("GET", "/menu-entry") => {
            let identity = menu_identity(STORE_ID);
            let entry = ResourceEntryId::derive(&identity, &IDENTITY);
            let version = server.menu_version(STORE_ID);
            respond_json(
                &mut stream,
                200,
                &session,
                fresh,
                &format!(
                    "{{\"entry\":\"{entry}\",\"version\":{},\"partition\":\"{}\"}}",
                    version.0,
                    identity.partition_text()
                ),
            );
        }
        // E7-L — the handler's code, fetched on first interaction and not
        // before.
        //
        // Keyed by handler IDENTITY, not by name: a changed implementation is
        // a different identity, so it is a different URL and no cache can
        // serve yesterday's behaviour to today's document. The name is what
        // the module DOES; the identity is which version of it this document
        // was rendered against.
        //
        // The module is the COMPILER's (E10, 2026-09-25): `pw emit-handlers`
        // compiled the handler's body to `dist/handlers/<identity>.mjs`, and
        // this route serves that file unchanged. Until then the server wrote a
        // module here itself. An identity this build's templates do not name
        // is refused before the file system is asked, so a request can only
        // ever reach a file the compiler named.
        // The page's speculation module (ADR-0122), as `pw emit-speculations`
        // wrote it. Only the one the manifest names.
        ("GET", route)
            if route.strip_prefix("/speculation/").is_some_and(|m| {
                server
                    .speculation
                    .as_ref()
                    .and_then(|s| s["module"].as_str())
                    == Some(m.split('?').next().unwrap_or(m))
            }) =>
        {
            let module = route.trim_start_matches("/speculation/");
            let module = module.split('?').next().unwrap_or(module);
            match std::fs::read(server.artifacts.join("speculations").join(module)) {
                Ok(bytes) => respond(
                    &mut stream,
                    200,
                    "text/javascript; charset=utf-8",
                    &session,
                    fresh,
                    &bytes,
                ),
                Err(_) => respond(
                    &mut stream,
                    404,
                    "text/plain; charset=utf-8",
                    &session,
                    fresh,
                    b"this speculation was not compiled",
                ),
            }
        }
        ("GET", route) if route.starts_with("/handler/") => {
            let id = route
                .trim_start_matches("/handler/")
                .trim_end_matches(".mjs");
            // The query is the browser's, not ours: a retry appends one so the
            // module map treats it as a new specifier, because a failed load
            // is cached forever otherwise. The identity is still the path.
            let id = id.split('?').next().unwrap_or(id);
            if !server.handler_identities().contains(id) {
                // An identity this build does not know. Refused rather than
                // guessed: serving *some* handler for an unknown identity is
                // how a stale document ends up running new code.
                respond(
                    &mut stream,
                    404,
                    "text/plain; charset=utf-8",
                    &session,
                    fresh,
                    b"no such handler",
                );
                return;
            }
            match std::fs::read(server.handler_module(id)) {
                Ok(module) => respond(
                    &mut stream,
                    200,
                    "text/javascript; charset=utf-8",
                    &session,
                    fresh,
                    &module,
                ),
                Err(_) => respond(
                    &mut stream,
                    404,
                    "text/plain; charset=utf-8",
                    &session,
                    fresh,
                    b"this handler was not compiled",
                ),
            }
        }
        // A page whose values are its signals and its streams (ADR-0130,
        // ADR-0148).
        ("GET", route) if route.starts_with("/page/") => {
            let path = route.trim_start_matches("/page/");
            match server.page_params(path, query) {
                Ok(params) => serve_page(server, &mut stream, &session, fresh, path, params),
                Err(why) => respond(
                    &mut stream,
                    404,
                    "text/plain; charset=utf-8",
                    &session,
                    fresh,
                    why.as_bytes(),
                ),
            }
        }
        // What this build compiled, for the browser's resume decision
        // (ADR-0132): one `identity|capture` line per handler. Served with
        // the build, so a document cached from another build cannot vouch
        // for a handler this one lacks.
        ("GET", "/pw-handlers") => {
            let table: String = server
                .handler_table()
                .into_iter()
                .map(|(identity, capture)| format!("{identity}|{capture}\n"))
                .collect();
            respond(
                &mut stream,
                200,
                "text/plain; charset=utf-8",
                &session,
                fresh,
                table.as_bytes(),
            );
        }
        // What the server did (ADR-0127, charter §10.5): how many times each
        // data-layer operation ran, and how many query values are kept.
        ("GET", "/metrics") => {
            let calls = server.calls.lock().expect("calls").clone();
            let body = serde_json::json!({
                "calls": calls,
                "kept": server.queries.stored(),
            });
            respond_json(&mut stream, 200, &session, fresh, &body.to_string());
        }
        ("GET", "/menu") => {
            let items = server.menu.lock().expect("menu");
            let ids: Vec<String> = items.iter().map(|(k, _)| format!("\"{k}\"")).collect();
            respond_json(
                &mut stream,
                200,
                &session,
                fresh,
                &format!("{{\"items\":[{}]}}", ids.join(",")),
            );
        }
        ("GET", "/stream") => stream_frames(server, &mut stream, &session, fresh, query),
        ("GET", "/StorePage.html") | ("GET", "/") => {
            // The large-menu case, for E7 gate item 10. A query parameter
            // rather than a second route: the same page, the same renderer,
            // the same runtime — only more items, which is what makes the
            // measurement about SIZE rather than about a different page.
            let items: usize = query
                .split('&')
                .find_map(|p| p.strip_prefix("items="))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            if items > 0 {
                let mut menu = server.menu.lock().expect("menu");
                if menu.len() != items {
                    *menu = (0..items)
                        .map(|i| (format!("item-{i}"), format!("Item {i}")))
                        .collect();
                    drop(menu);
                    server
                        .materializer
                        .invalidate(&server.menu_key(STORE_ID), 0);
                    server.queries.invalidate("store.page.Menu");
                }
            }
            serve_store(server, &mut stream, &session, fresh, store_params(STORE_ID));
        }
        // A command this server does not host, including the address-resolving
        // `/command/add_to_cart?instance=..` route E10 deleted. Named, rather
        // than left to the 405 below: the method is right and the command is
        // not there.
        ("POST", route) if route.starts_with("/command/") => respond(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            &session,
            fresh,
            b"no such command",
        ),
        // **A page at its route** (ADR-0162): the store at `/stores/{id}`,
        // its parameters as the address gives them, and any other page that
        // declares one.
        ("GET", _)
            if routed
                .as_ref()
                .is_some_and(|(page, _)| server.plan["page"] == *page) =>
        {
            let (_, params) = routed.expect("matched");
            serve_store(server, &mut stream, &session, fresh, params)
        }
        ("GET", _) if routed.is_some() => {
            let (page, params) = routed.expect("matched");
            serve_page(server, &mut stream, &session, fresh, &page, params)
        }
        ("GET", _) => serve_file(server, &mut stream, route, &session, fresh),
        _ => respond(&mut stream, 405, "text/plain", &session, fresh, b"method"),
    }
}

/// Long-poll: hand over whatever frames are waiting, or wait briefly for one.
///
/// The transport, and only the transport. `pw-protocol` names none of this, so
/// E7-P can replace it with a streaming fetch without a frame changing.
/// The streaming adapter: one connection, frames written as they arrive.
///
/// # Why two adapters exist
///
/// Architect ruling, 2026-08-07: a multiplexed `StreamFrame` transport, *with
/// long-poll as a fallback adapter*. Two of them is the point. A protocol with
/// exactly one transport is indistinguishable from a protocol that IS its
/// transport, and the difference only becomes visible when a second one has to
/// carry the same frames without changing them.
///
/// Both adapters deliver identical frames, in order, under the same cursor
/// discipline. What differs is only when the connection ends: this one holds
/// it open and writes batch after batch; the long poll returns after the first
/// batch and is asked again.
///
/// No `content-length`, and the body is terminated by the close. That is the
/// oldest streaming mechanism HTTP has and needs no chunked framing to be
/// written by hand — which matters here, because a bug in hand-rolled chunked
/// encoding would look exactly like a transport that drops frames.
fn stream_open(
    server: &Server,
    stream: &mut TcpStream,
    session: &str,
    fresh: bool,
    document: u64,
    since: u64,
) {
    let doc: Doc = (session.to_string(), document);
    let cookie = if fresh {
        format!("set-cookie: pw-session={session}; Path=/; SameSite=Lax\r\n")
    } else {
        String::new()
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\n\
         cache-control: no-store\r\n{cookie}connection: close\r\n\r\n"
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }

    // A page with a cursor the server has no subscriber for was forgotten
    // while idle: what it missed cannot be said, so it is told to reload.
    // So is a page that names no document (ADR-0161).
    if since > 0 && !server.pending.lock().expect("pending").contains_key(&doc) {
        let _ = stream.write_all(format!("{}\n", reload_batch(since)).as_bytes());
        return;
    }

    // What the page says it applied, once: a frame is forgotten when a page
    // acknowledges it, never because it was written (ADR-0139). Until
    // 2026-10-02 each pass below acknowledged what the pass before had
    // written, so a stream held for a page that had gone, reloaded or closed,
    // dropped what the page that replaced it was waiting for.
    server
        .pending
        .lock()
        .expect("pending")
        .entry(doc.clone())
        .or_insert_with(|| Subscriber::at(document))
        .acknowledge(since);
    let mut written = since;

    // Bounded, like the long poll: a held connection is a held thread, and
    // three engine families times six workers is eighteen of them.
    for _ in 0..80 {
        let batch = {
            let mut queue = server.pending.lock().expect("pending");
            let Some(waiting) = queue.get_mut(&doc) else {
                // Forgotten while the stream was held: the next request
                // is told to reload.
                return;
            };
            waiting.seen = std::time::Instant::now();
            let (cursor, frames) = waiting.after(written);
            if frames.is_empty() {
                None
            } else {
                let body: Vec<serde_json::Value> = frames
                    .into_iter()
                    .map(|f| serde_json::to_value(f).expect("frame"))
                    .collect();
                Some((cursor, serde_json::to_string(&body).expect("frames")))
            }
        };

        if let Some((cursor, frames)) = batch {
            // Written, but NOT acknowledged. The cursor advances here only for
            // this connection's own reads; the subscriber's copy is dropped on
            // the next request, which is the client saying it applied them.
            // A write that never arrives therefore costs nothing.
            let line = format!("{{\"cursor\":{cursor},\"frames\":{frames}}}\n");
            if stream.write_all(line.as_bytes()).is_err() || stream.flush().is_err() {
                return;
            }
            written = cursor;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn stream_frames(
    server: &Server,
    stream: &mut TcpStream,
    session: &str,
    fresh: bool,
    _query: &str,
) {
    // A bounded hold, deliberately short. Each waiting subscriber occupies a
    // connection and a thread, and three engine families times six workers is
    // eighteen pages each holding one — a longer hold made the suite flaky
    // under parallelism while every test passed alone, which is a harness
    // failure that reads exactly like a runtime failure.
    let number = |name: &str| -> Option<u64> {
        _query
            .split('&')
            .find_map(|p| p.strip_prefix(name)?.strip_prefix('='))
            .and_then(|v| v.parse().ok())
    };
    let since = number("since").unwrap_or(0);
    // The document the page is (ADR-0161): its number, which was its first
    // cursor. A page that names none is no document this server served.
    let document = number("doc").unwrap_or(0);
    let doc: Doc = (session.to_string(), document);

    // One route, two adapters. The frames are the same either way — see
    // `e2e/transport.spec.mjs`, which runs the whole subscription twice.
    if _query.split('&').any(|p| p == "mode=stream") {
        stream_open(server, stream, session, fresh, document, since);
        return;
    }

    // Forgotten while idle: see `stream_open`.
    if since > 0 && !server.pending.lock().expect("pending").contains_key(&doc) {
        respond_json(stream, 200, session, fresh, &reload_batch(since));
        return;
    }

    for _ in 0..40 {
        {
            let mut queue = server.pending.lock().expect("pending");
            let waiting = queue
                .entry(doc.clone())
                .or_insert_with(|| Subscriber::at(document));
            waiting.seen = std::time::Instant::now();
            // This request is the client's acknowledgement of everything up to
            // `since`: it applied those frames and asked for what follows.
            waiting.acknowledge(since);
            let (cursor, frames) = waiting.after(since);
            if !frames.is_empty() {
                let body: Vec<serde_json::Value> = frames
                    .into_iter()
                    .map(|f| serde_json::to_value(f).expect("frame"))
                    .collect();
                let json = format!(
                    "{{\"cursor\":{cursor},\"frames\":{}}}",
                    serde_json::to_string(&body).expect("frames")
                );
                // Left in place until the next request proves they arrived.
                respond_json(stream, 200, session, fresh, &json);
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    respond_json(
        stream,
        200,
        session,
        fresh,
        &format!("{{\"cursor\":{since},\"frames\":[]}}"),
    );
}

/// **The store's page, at the parameters `params` give it** (ADR-0162):
/// `/StorePage.html` and `/` for store 47, and `/stores/{id}` for any.
fn serve_store(
    server: &Server,
    stream: &mut TcpStream,
    session: &str,
    fresh: bool,
    params: Params,
) {
    let unavailable = |stream: &mut TcpStream, why: Unread| match why {
        // A page whose address names nothing, as the page declares it
        // (ADR-0163), is not found.
        Unread::NotFound(_) => respond(
            stream,
            404,
            "text/html; charset=utf-8",
            session,
            fresh,
            NOT_FOUND_PAGE.as_bytes(),
        ),
        // A page whose values could not be read is answered as
        // unavailable, and the server goes on (E14, T04).
        Unread::Failed(why) => respond(
            stream,
            503,
            "text/plain; charset=utf-8",
            session,
            fresh,
            format!("the store page cannot be shown: {why}").as_bytes(),
        ),
    };
    // Each stream's query, started before the document is rendered
    // (ADR-0148): the page waits for the ones it is declared to wait
    // for, and the rest fill their regions in the same response.
    let runs = match stream_runs(&server.plan, session, &params) {
        Ok(runs) => runs,
        Err(why) => return unavailable(stream, Unread::Failed(why)),
    };
    std::thread::scope(|scope| {
        let mut settling = Settling::start(scope, server, session, runs);
        let settled = settling.for_document();
        // Registering the subscriber HERE, when the document is
        // served, rather than when it first subscribes: the page
        // holds instances from this moment on, so a structural change
        // after this moment has an address in it. A registration
        // deferred to the first poll would silently drop every change
        // that raced it.
        let (rendered, cursor, entries, env) =
            match server.serve_document_settled(session, &params, &settled) {
                Ok(served) => served,
                Err(why) => return unavailable(stream, why),
            };
        let speculation = server
            .speculation
            .as_ref()
            .and_then(|m| m["module"].as_str())
            .map(|module| (format!("/speculation/{module}"), entries));
        let page = document(
            &rendered,
            &server.templates,
            &server.plan,
            cursor,
            speculation,
        );
        server.respond_streaming(
            stream,
            session,
            fresh,
            &page,
            server.store_template(),
            &env,
            &mut settling,
        );
    });
}

/// **A page that binds no query, at the parameters `params` give it**
/// (ADR-0130, ADR-0162): from `/page/<path>?..`, or its route.
fn serve_page(
    server: &Server,
    stream: &mut TcpStream,
    session: &str,
    fresh: bool,
    path: &str,
    params: Params,
) {
    let not_found = |stream: &mut TcpStream, why: String| {
        respond(
            stream,
            404,
            "text/plain; charset=utf-8",
            session,
            fresh,
            why.as_bytes(),
        );
    };
    let runs = match server
        .plans
        .get(path)
        .ok_or_else(|| format!("no page `{path}` in this build"))
        .and_then(|plan| stream_runs(plan, session, &params))
    {
        Ok(runs) => runs,
        Err(why) => return not_found(stream, why),
    };
    std::thread::scope(|scope| {
        let mut settling = Settling::start(scope, server, session, runs);
        let settled = settling.for_document();
        match server.render_page(path, &params, session, &settled) {
            Ok((page, env, template)) => server.respond_streaming(
                stream,
                session,
                fresh,
                &page,
                template,
                &env,
                &mut settling,
            ),
            Err(why) => not_found(stream, why),
        }
    });
}

/// **The parameters a path gives a route** (ADR-0162), or `None` when it is
/// not the route's: one segment each, as the compiler's link check matches
/// a link to a route (`routes::matches`), each decoded as a path is.
fn route_params(route: &str, path: &str) -> Option<Params> {
    let segments = |s: &str| -> Vec<String> {
        match s.trim_matches('/') {
            "" => Vec::new(),
            t => t.split('/').map(str::to_string).collect(),
        }
    };
    let (route, path) = (segments(route), segments(path));
    if route.len() != path.len() {
        return None;
    }
    let mut out = Params::new();
    for (r, p) in route.iter().zip(&path) {
        match r.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
            Some(name) => {
                let value = path_decoded(p).filter(|v| !v.is_empty())?;
                out.insert(name.to_string(), value);
            }
            None if r == p => {}
            None => return None,
        }
    }
    Some(out)
}

/// A path segment, decoded: `%XX` as the byte it escapes. A `+` is itself,
/// as RFC 3986 has it in a path; only a form's query makes it a space.
/// `None` for an escape that is not one, or bytes that are not UTF-8.
fn path_decoded(v: &str) -> Option<String> {
    let bytes = v.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn serve_file(server: &Server, stream: &mut TcpStream, route: &str, session: &str, fresh: bool) {
    let name = route.trim_start_matches('/');
    // No traversal: a request names a file in `dist` and nothing above it.
    if name.contains("..") || name.contains('/') {
        respond(stream, 404, "text/plain", session, fresh, b"not found");
        return;
    }
    let path = server.dist.join(name);
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("mjs") | Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    };
    match std::fs::read(&path) {
        Ok(bytes) => respond(stream, 200, mime, session, fresh, &bytes),
        Err(_) => respond(stream, 404, "text/plain", session, fresh, b"not found"),
    }
}

/// A query string's value, decoded: `%XX` as the byte it escapes, and `+`
/// as a space, as a form encodes one. `None` for an escape that is not one,
/// or bytes that are not UTF-8.
fn percent_decoded(v: &str) -> Option<String> {
    let bytes = v.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// **What a document says about its page's signals** (ADR-0130, ADR-0140):
/// each signal's first value, the parts they decide, and each block's
/// template, which the browser renders again. Empty for a page with none.
fn signal_manifest(
    plan: &serde_json::Value,
    template: &Template,
) -> (
    serde_json::Map<String, serde_json::Value>,
    serde_json::Value,
    serde_json::Map<String, serde_json::Value>,
) {
    let signals: serde_json::Map<String, serde_json::Value> = plan["signals"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            (
                s["name"].as_str().unwrap_or_default().to_string(),
                s["initial"].clone(),
            )
        })
        .collect();
    let mut blocks = serde_json::Map::new();
    for live in plan["live"].as_array().into_iter().flatten() {
        if !matches!(live["kind"].as_str(), Some("conditional" | "match")) {
            continue;
        }
        let id = live["part"].as_u64().unwrap_or_default() as u32;
        if let Some(part) = find_part(&template.chunks, id) {
            blocks.insert(
                id.to_string(),
                serde_json::to_value(part).unwrap_or_default(),
            );
        }
    }
    let live = plan["live"].clone();
    (signals, live, blocks)
}

/// Each signal's first value, set into a rendering environment (ADR-0130).
fn with_signals(mut env: Env, plan: &serde_json::Value) -> Env {
    for s in plan["signals"].as_array().into_iter().flatten() {
        let name = s["name"].as_str().unwrap_or_default();
        env = env.set(name, Value::from_wire(&s["initial"]));
    }
    env
}

/// **A page that binds no query, as a document** (ADR-0130, ADR-0148): its
/// values are its signals and its streams' parts.
///
/// Its parts manifest carries what the browser holds and renders again:
/// each signal's first value, each part a signal decides, and the template
/// of each block one decides, which the browser's copy of the renderer
/// renders when it changes.
fn signal_document(
    body: &str,
    template: &Template,
    plan: &serde_json::Value,
    templates: &[Template],
) -> String {
    let (signals, _, blocks) = signal_manifest(plan, template);
    let manifest = serde_json::json!({
        "template": template.path,
        "schema": template.schema,
        "cursor": 0,
        "parts": template.manifest(),
        "resume": resume_manifest(templates),
        "signals": signals,
        "live": plan["live"].clone(),
        "blocks": blocks,
        // Nothing on this page is a resource's, so nothing is listened for.
        "listens": false,
    });
    // No `<` in a script element's text (ADR-0097).
    let json =
        pw_render::escape::json_in_script(&serde_json::to_string(&manifest).unwrap_or_default());
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>{}</title>\n</head>\n<body>\n{body}\n\
         <script type=\"application/json\" id=\"pw-parts\">{json}</script>\n\
         {RUNTIME}\n{DOCUMENT_END}",
        pw_render::escape::text(&template.name)
    )
}

/// The part numbered `id`, wherever it is in `chunks`.
fn find_part(chunks: &[pw_render::ir::Chunk], id: u32) -> Option<&pw_render::ir::Part> {
    use pw_render::ir::Chunk;
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if p.id().is_some_and(|i| i.0 == id) {
            return Some(p);
        }
        // Through every region a part has, as the IR's own walks go: a walk
        // that names the kinds it descends into misses the next one, as this
        // missed a stream's arms (ADR-0148).
        if let Some(inner) = p.nested().into_iter().find_map(|r| find_part(r, id)) {
            return Some(inner);
        }
    }
    None
}

/// Each handler `templates` hold, by identity, with the paths it captures
/// joined (ADR-0132).
fn handler_table(templates: &[Template]) -> BTreeMap<String, String> {
    fn walk(chunks: &[pw_render::ir::Chunk], out: &mut BTreeMap<String, String>) {
        for c in chunks {
            let pw_render::ir::Chunk::Dynamic(p) = c else {
                continue;
            };
            if let pw_render::ir::Part::Event {
                handler, captures, ..
            } = p
                && !handler.is_empty()
            {
                out.insert(handler.clone(), captures.join(","));
            }
            // Every region, as `find_part` walks them. Until 2026-10-03 this
            // named the three block kinds, so a handler in a stream's arm was
            // in no table, and its Pick refused (ADR-0148).
            for region in p.nested() {
                walk(region, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    for t in templates {
        walk(&t.chunks, &mut out);
    }
    out
}

/// The document's resume manifest (ADR-0132): one base, and one entry per
/// handler, keyed by its identity, presenting its capture schema. The base
/// names no handler, so a part with no entry of its own authorises nothing.
fn resume_manifest(templates: &[Template]) -> serde_json::Value {
    let handlers: serde_json::Map<String, serde_json::Value> = handler_table(templates)
        .into_iter()
        .map(|(identity, capture)| {
            let bytes = if capture.is_empty() { "" } else { "x" };
            (
                identity.clone(),
                serde_json::json!({ "handler": identity, "capture": capture, "captures": bytes }),
            )
        })
        .collect();
    serde_json::json!({
        "scheme": "2", "abi": "1", "build": BUILD, "handler": "",
        "capture": "", "document": "cart-doc", "scope": "public",
        "captures": "", "construct": "region",
        "handlers": handlers,
    })
}

/// The document shell, with the parts manifest and the runtime.
fn document(
    body: &str,
    templates: &[Template],
    plan: &serde_json::Value,
    cursor: u64,
    speculation: Option<(String, serde_json::Value)>,
) -> String {
    let template = templates
        .iter()
        .find(|t| t.name == "StorePage")
        .expect("StorePage");
    let manifest = serde_json::json!({
        "template": template.path,
        "schema": template.schema,
        // Where this document starts listening. A page carries its own
        // position rather than starting at zero, because starting at zero
        // would replay changes this document already contains.
        "cursor": cursor,
        "parts": template.manifest(),
        // One base manifest and a per-handler override.
        //
        // Per handler, because `decide` answers about ONE handler and a page
        // with two would otherwise authorise both on the strength of whichever
        // one the page-wide manifest described. `clear_cart` captures nothing,
        // so its capture schema is the schema of nothing — which is still a
        // schema, and still has to match.
        "resume": resume_manifest(templates),
    });
    // The page's speculation module and the values it starts from (ADR-0122).
    // A private page's own session's values: this document is `cache private`.
    let mut manifest = manifest;
    // And its signals (ADR-0140): a page that reads queries holds UI state
    // too, and the browser renders what they decide, as on a page of
    // signals alone.
    let (signals, live, blocks) = signal_manifest(plan, template);
    if !signals.is_empty() {
        manifest["signals"] = serde_json::Value::Object(signals);
        manifest["live"] = live;
        manifest["blocks"] = serde_json::Value::Object(blocks);
    }
    // Each binding a signal keys (ADR-0152): the browser reads it again, for
    // the new key, when one of its signals changes, and does what its stale
    // work says.
    let keyed: Vec<serde_json::Value> = plan["bindings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["signals"].as_array().is_some_and(|s| !s.is_empty()))
        .map(|b| {
            serde_json::json!({
                "binding": b["binding"],
                "signals": b["signals"],
                "on_key_change": b["policy"]["on_key_change"],
            })
        })
        .collect();
    if !keyed.is_empty() {
        manifest["keyed"] = serde_json::Value::Array(keyed);
    }
    if let Some((module, entries)) = speculation {
        manifest["speculation"] = serde_json::Value::String(module);
        manifest["entries"] = entries;
    }
    // No `<` in a script element's text (ADR-0097).
    let json =
        pw_render::escape::json_in_script(&serde_json::to_string(&manifest).unwrap_or_default());
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>Store</title>\n</head>\n<body>\n{body}\n\
         <style>{STYLE}</style>\n\
         <script type=\"application/json\" id=\"pw-parts\">{json}</script>\n\
         {RUNTIME}\n{DOCUMENT_END}"
    )
}

/// **How a document starts its runtime** (ADR-0148): a classic script that
/// imports it, which runs while the response is still open, so a page whose
/// streamed regions have not settled is interactive. Measured on 2026-10-03
/// (`spikes/own-renderer/probes/streaming.mjs`): a deferred module starts
/// only once the whole response has arrived, in every engine, and an `async`
/// one does in WebKit.
const RUNTIME: &str = "<script>import(\"/pw-runtime.mjs\")</script>";

/// The store page's speculation manifest, if the build wrote one (ADR-0122).
fn speculation_manifest(dist: &std::path::Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(dist.join("speculations").join("store.page.StorePage.json"))
        .ok()?;
    serde_json::from_str(&text).ok()
}

fn session_of(headers: &str) -> String {
    headers
        .split("pw-session=")
        .nth(1)
        .and_then(|rest| rest.split([';', '\r', '\n']).next())
        .map(str::to_string)
        .unwrap_or_else(|| format!("s-{}", std::process::id() as u64 + rand_ish()))
}

/// A per-connection value, without a random source.
///
/// `Math.random`'s absence is deliberate elsewhere in this project; here a
/// monotonic counter is enough, because sessions only need to differ.
fn rand_ish() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::SeqCst)
}

fn respond_json(stream: &mut TcpStream, code: u16, session: &str, fresh: bool, body: &str) {
    respond(
        stream,
        code,
        "application/json; charset=utf-8",
        session,
        fresh,
        body.as_bytes(),
    );
}

/// **A status code's reason phrase** (RFC 9110 §15). A client ignores it
/// (RFC 9112 §4); a person reading the response does not. Until 2026-10-03
/// every response said `OK`, a 404 and a 503 among them.
fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        503 => "Service Unavailable",
        _ => "",
    }
}

fn respond(stream: &mut TcpStream, code: u16, mime: &str, session: &str, fresh: bool, body: &[u8]) {
    let cookie = if fresh {
        format!("set-cookie: pw-session={session}; Path=/; SameSite=Lax\r\n")
    } else {
        String::new()
    };
    let head = format!(
        "HTTP/1.1 {code} {}\r\ncontent-type: {mime}\r\ncontent-length: {}\r\n{cookie}connection: close\r\n\r\n",
        reason(code),
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
    let mut sink = Vec::new();
    let _ = stream.take(0).read_to_end(&mut sink);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The store's `app.pw`, with the cart's lines listed below the menu from
    /// a private query of their own: what T03's reference adds (ADR-0145).
    fn with_lines(app: &str) -> String {
        let query = "
// What the cart section shows of each line: its item's name, and how many.
type CartEntry = CartEntry {
    item_id: MenuItemId,
    name: String,
    quantity: Int,
}

// The name a menu gives an item.
fn name_in(menu: List<MenuItem>, item: MenuItemId) -> String !{} {
    List.fold(menu, \"\", fn(found, entry) if same_item(entry.id, item) { entry.name } else { found })
}

session query CartLines(id: StoreId, session: Session<SessionId>) -> Result<List<CartEntry>, CartError>
    freshness      0.seconds
    consistency    read_your_writes
    cache          private
    key            id, session
    invalidates_on CartChanged(session)
    concurrency    one_per_key
    on_key_change  cancel
    timeout        2.seconds
{
    let cart = Carts.current(session)?
    let menu = match Menus.for_store(id) {
        Ok(items) => items,
        Err(_) => [],
    }
    Ok(List.map(cart.lines, fn(line) CartEntry { item_id: line.item_id, name: name_in(menu, line.item_id), quantity: line.quantity.count }))
}
";
        let out = app
            .replacen("import context.{ current_session }", "import context.{ current_session }\nimport List", 1)
            .replacen("    MenuItemId, PositiveInt }", "    MenuItemId, PositiveInt, same_item }", 1)
            .replacen("\ncommand add_to_cart(", &format!("{query}\ncommand add_to_cart("), 1)
            .replacen(
                "    let cart = query Cart(current_session())\n",
                "    let cart = query Cart(current_session())\n    let lines = query CartLines(id, current_session())\n",
                1,
            )
            .replacen(
                "                <p id=\"cart-count\">{cart.line_count}</p>",
                "                <p id=\"cart-count\">{cart.line_count}</p>\n                <ul id=\"cart-lines\">\n                    {#each lines as line (line.item_id)}\n                        <li>{line.name} × {line.quantity}</li>\n                    {/each}\n                </ul>",
                1,
            );
        assert!(out.contains("CartLines(id, current_session())") && out.contains("cart-lines"));
        out
    }

    /// **That store, built by the compiler as `pw build` builds it, and
    /// served from what was written** (ADR-0145).
    fn lines_server() -> Served {
        lines_server_with(with_lines)
    }

    /// The store, its `app.pw` changed by `change`, built and served.
    fn lines_server_with(change: fn(&str) -> String) -> Served {
        served_from(change, None)
    }

    /// **The store with T04's setup**: a session's order, which the kitchen
    /// sets (E14, T04). The benchmark's own patch, applied as its harness
    /// applies it, so this and the task cannot drift apart.
    fn orders_server() -> Served {
        served_from(
            |app| app.to_string(),
            Some(include_str!(
                "../../../../benchmarks/tasks/T04-order-ready/setup/pleris.patch"
            )),
        )
    }

    /// The store's sources copied, `app.pw` changed by `change` and `patch`
    /// applied, built by the compiler as `pw build` builds them, and served
    /// from what was written.
    fn served_from(change: fn(&str) -> String, patch: Option<&str>) -> Served {
        served_from_patches(change, patch.as_slice())
    }

    /// The store, changed by `change` and then by each patch in order: a
    /// task's setup, and its reference after it. The benchmark's store
    /// (ADR-0156), which every task's patches are written against.
    fn served_from_patches(change: fn(&str) -> String, patches: &[&str]) -> Served {
        served_from_patches_in("benchmarks/baselines/pleris", change, patches)
    }

    /// [`served_from_patches`], the store's sources read from `base`, under
    /// the repository's root: the benchmark's store, or the canonical one,
    /// `examples`.
    fn served_from_patches_in(base: &str, change: fn(&str) -> String, patches: &[&str]) -> Served {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let base = root.join(base);
        let dir = tempfile::TempDir::with_prefix("pw-served-").expect("a temporary directory");
        let work = dir.path().to_path_buf();
        let examples = work.join("examples");
        let pw_files = |dir: &std::path::Path| -> Vec<std::path::PathBuf> {
            let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
                .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
                .map(|e| e.expect("entry").path())
                .filter(|p| p.extension().is_some_and(|x| x == "pw"))
                .collect();
            paths.sort();
            paths
        };
        for rel in ["lib", "store"] {
            std::fs::create_dir_all(examples.join(rel)).expect("dir");
            for p in pw_files(&base.join(rel)) {
                let to = examples.join(rel).join(p.file_name().expect("name"));
                std::fs::copy(&p, &to).expect("copy");
            }
        }
        std::fs::copy(base.join("domain.pw"), examples.join("domain.pw")).expect("copy");
        let app = examples.join("store/app.pw");
        let changed = change(&std::fs::read_to_string(&app).expect("app"));
        std::fs::write(&app, changed).expect("app");
        for (i, patch) in patches.iter().enumerate() {
            let file = work.join(format!("{i}.patch"));
            std::fs::write(&file, patch).expect("patch");
            let applied = std::process::Command::new("git")
                .arg("apply")
                .arg(&file)
                .current_dir(&examples)
                .output()
                .expect("git runs");
            assert!(
                applied.status.success(),
                "the patch applies: {}",
                String::from_utf8_lossy(&applied.stderr)
            );
        }
        let mut sources: Vec<(String, String)> = Vec::new();
        let mut read = |p: std::path::PathBuf| {
            let src = std::fs::read_to_string(&p).expect("read");
            sources.push((p.display().to_string(), src));
        };
        for p in pw_files(&root.join("packages/pw-std")) {
            read(p);
        }
        for p in pw_files(&root.join("packages/pw-platform-web")) {
            read(p);
        }
        read(examples.join("domain.pw"));
        for p in pw_files(&examples.join("lib")) {
            read(p);
        }
        for p in pw_files(&examples.join("store")) {
            read(p);
        }
        let units: Vec<pw_core::check::Unit> = sources
            .into_iter()
            .map(|(path, src)| pw_core::check::Unit {
                hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
                path,
                src,
            })
            .collect();
        let build = pw_core::build::build(&units).expect("the store builds");
        assert!(build.refusals().is_empty(), "{:?}", build.refusals());
        let out = work.join("build");
        build.write(&out).expect("the build is written");
        Served {
            server: Server::from_build(out.clone(), out).expect("served"),
            _dir: dir,
        }
    }

    /// **A build served from a directory of its own, removed when the test
    /// is done with it** (ADR-0158). The server reads handler modules from
    /// the directory on request, so the directory lives as long as it does.
    struct Served {
        server: Server,
        _dir: tempfile::TempDir,
    }

    impl std::ops::Deref for Served {
        type Target = Server;
        fn deref(&self) -> &Server {
            &self.server
        }
    }

    impl std::ops::DerefMut for Served {
        fn deref_mut(&mut self) -> &mut Server {
            &mut self.server
        }
    }

    #[test]
    fn an_order_the_kitchen_sets_is_the_order_query_s_value() {
        let s = orders_server();
        let (none, _) = s.serve_document("a");
        assert!(visible(&none).contains("No order yet."), "{none}");
        s.orders
            .lock()
            .expect("orders")
            .insert("a".into(), "preparing".into());
        let (preparing, _) = s.serve_document("a");
        assert!(
            visible(&preparing).contains("Your order is being prepared."),
            "{preparing}"
        );
        // Another session has none.
        let (other, _) = s.serve_document("b");
        assert!(visible(&other).contains("No order yet."), "{other}");
    }

    #[test]
    fn a_page_whose_values_cannot_be_read_is_answered_and_the_server_goes_on() {
        let s = orders_server();
        // A status the program's type does not have: the component's types
        // refuse what the host gives, and the query fails.
        s.orders
            .lock()
            .expect("orders")
            .insert("a".into(), "ready".into());
        let refused = s
            .serve_document_with_entries("a")
            .expect_err("a page that cannot be shown");
        assert!(refused.contains("the store page's queries"), "{refused}");
        // Nothing was left held: another session's page is served.
        let (other, _) = s.serve_document("b");
        assert!(visible(&other).contains("No order yet."), "{other}");
    }

    #[test]
    fn a_served_page_whose_values_fail_is_told_to_read_itself_again() {
        let s = orders_server();
        s.serve_document("a");
        s.orders
            .lock()
            .expect("orders")
            .insert("a".into(), "ready".into());
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("the command commits");
        let queue = s.pending.lock().expect("pending");
        assert!(
            queue[&latest(&queue, "a")]
                .frames
                .iter()
                .any(|(_, f)| matches!(
                    f,
                    StreamFrame::Recovery {
                        recovery: Recovery::Reload,
                        ..
                    }
                )),
            "{:?}",
            queue[&latest(&queue, "a")].frames
        );
    }

    /// **The store with recommendations streamed below its cart** (ADR-0148),
    /// their failed arm telling a declared error from the host's.
    fn with_recommendations(app: &str) -> String {
        app.replacen("import Menus\n", "import Menus\nimport Recommender\n", 1)
            .replacen(
                "MenuItemId, PositiveInt }",
                "MenuItemId, PositiveInt, Recommendation, RecommendationError }",
                1,
            )
            .replacen(
                "public query Store(",
                "public query Recommendations(id: StoreId) -> Result<List<Recommendation>, RecommendationError>
    freshness     0.seconds
    consistency   eventual
    cache         shared
    key           id
    concurrency   one_per_key
    on_key_change cancel
    delivery      streamed
    timeout       1.seconds
{
    Recommender.for_store(id)
}

public query Store(",
                1,
            )
            .replacen(
                "\n        </main>",
                "
            <section aria-label=\"Recommendations\">
                <stream query={Recommendations(id)}>
                    <placeholder><p>Finding recommendations</p></placeholder>
                    <ready as={items}>
                        <ul>{#each items as item (item.id)}<li>{item.name}</li>{/each}</ul>
                    </ready>
                    <failed as={why}>{#match why}{:Some(e)}<p>Declined</p>{:None}<p>Unavailable</p>{/match}</failed>
                </stream>
            </section>
        </main>",
                1,
            )
    }

    /// What one request to `s` answers, chunk by chunk, each with when it
    /// arrived: the connection served by `handle`, as a browser's is.
    fn fetched(s: &Server, path: &str) -> Vec<(std::time::Duration, String)> {
        fetched_as(s, path, None)
    }

    /// [`fetched`], the request carrying `session`'s cookie when given.
    fn fetched_as(
        s: &Server,
        path: &str,
        session: Option<&str>,
    ) -> Vec<(std::time::Duration, String)> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let at = listener.local_addr().expect("address");
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let (stream, _) = listener.accept().expect("accept");
                handle(s, stream);
            });
            let mut client = TcpStream::connect(at).expect("connect");
            let cookie = session
                .map(|id| format!("Cookie: pw-session={id}\r\n"))
                .unwrap_or_default();
            client
                .write_all(format!("GET {path} HTTP/1.1\r\nHost: t\r\n{cookie}\r\n").as_bytes())
                .expect("request");
            let started = std::time::Instant::now();
            let mut chunks = Vec::new();
            let mut buf = [0u8; 65536];
            loop {
                let n = client.read(&mut buf).expect("read");
                if n == 0 {
                    // The close, which is what tells a browser the document
                    // is complete: the last chunk, empty, when it came.
                    chunks.push((started.elapsed(), String::new()));
                    break;
                }
                chunks.push((
                    started.elapsed(),
                    String::from_utf8_lossy(&buf[..n]).into_owned(),
                ));
            }
            chunks
        })
    }

    /// When the response had first carried `text`, and everything it carried.
    fn arrived(
        chunks: &[(std::time::Duration, String)],
        text: &str,
    ) -> Option<std::time::Duration> {
        let mut so_far = String::new();
        for (at, chunk) in chunks {
            so_far.push_str(chunk);
            if so_far.contains(text) {
                return Some(*at);
            }
        }
        None
    }

    fn recommend(s: &Server, delay_ms: u64, fail: Option<&str>) {
        *s.recommender.lock().expect("recommender") = Recommender {
            delay_ms,
            fail: fail.map(str::to_string),
            ..Recommender::default()
        };
    }

    /// What the recommendations' region shows: the arm the document carries
    /// for it, in place or as its patch. Not the page: the menu has a Cold
    /// Brew too, so a page holds it whatever the region shows.
    fn region(page: &str) -> String {
        let at = page
            .find("aria-label=\"Recommendations\"")
            .expect("the region");
        let rest = &page[at..];
        match rest.find("<?start") {
            // Pending: the patch, after the document.
            Some(_) => page[page.find("<template for=").expect("the patch")..].to_string(),
            None => rest[..rest.find("</section>").expect("its end")].to_string(),
        }
    }

    #[test]
    fn a_streamed_region_follows_its_document_in_the_same_response() {
        let s = served_from(with_recommendations, None);
        recommend(&s, 400, None);
        let chunks = fetched(&s, "/StorePage.html");
        let whole: String = chunks.iter().map(|(_, c)| c.as_str()).collect();
        // The document first, its region pending, and its runtime started.
        let shell = arrived(&chunks, "<?start name=").expect("a pending region");
        let runtime = arrived(&chunks, "import(\"/pw-runtime.mjs\")").expect("the runtime");
        let filled = arrived(&chunks, "<template for=").expect("the region's arm");
        assert!(shell < std::time::Duration::from_millis(300), "{shell:?}");
        assert!(
            runtime < std::time::Duration::from_millis(300),
            "{runtime:?}"
        );
        assert!(
            filled >= std::time::Duration::from_millis(350),
            "{filled:?}"
        );
        assert!(
            !whole.contains("content-length"),
            "a streamed document's length is not known first"
        );
        // The arm, and then the document's end, and nothing after it.
        let arm = &whole[whole.find("<template for=").expect("arm")..];
        assert!(
            arm.contains("Cortado") && arm.contains("Cold Brew"),
            "{arm}"
        );
        assert!(whole.ends_with("</template></body>\n</html>\n"), "{whole}");
    }

    #[test]
    fn a_region_past_its_budget_is_given_the_host_s_failure_when_its_budget_is_spent() {
        let s = served_from(with_recommendations, None);
        // Three seconds of a one-second budget.
        recommend(&s, 3000, None);
        let chunks = fetched(&s, "/StorePage.html");
        let ended = chunks.last().expect("an answer").0;
        let whole: String = chunks.iter().map(|(_, c)| c.as_str()).collect();
        let arm = &whole[whole.find("<template for=").expect("arm")..];
        assert!(arm.contains("Unavailable"), "{arm}");
        assert!(ended < std::time::Duration::from_millis(2500), "{ended:?}");
    }

    #[test]
    fn a_declared_error_and_a_host_s_failure_are_shown_apart() {
        let s = served_from(with_recommendations, None);
        recommend(&s, 0, Some("declared"));
        let declared: String = fetched(&s, "/StorePage.html")
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        assert!(declared.contains("<p>Declined</p>"), "{declared}");
        recommend(&s, 0, Some("host"));
        let host: String = fetched(&s, "/StorePage.html")
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        assert!(host.contains("<p>Unavailable</p>"), "{host}");
    }

    /// The same, with the recommendations kept five minutes.
    fn with_kept_recommendations(app: &str) -> String {
        with_recommendations(app).replacen(
            "    freshness     0.seconds\n    consistency   eventual",
            "    freshness     5.minutes\n    consistency   eventual",
            1,
        )
    }

    #[test]
    fn a_declared_error_is_given_and_not_kept() {
        let s = served_from(with_kept_recommendations, None);
        recommend(&s, 0, Some("declared"));
        let page = |s: &Server| -> String {
            fetched(s, "/StorePage.html")
                .into_iter()
                .map(|(_, c)| c)
                .collect()
        };
        assert!(region(&page(&s)).contains("<p>Declined</p>"));
        // The recommender answers again, and the next page asks it: what it
        // declined is not served for five minutes.
        recommend(&s, 0, None);
        let answered = region(&page(&s));
        assert!(
            answered.contains("Cortado") && !answered.contains("Declined"),
            "{answered}"
        );
        // An answer is kept, as its policy says.
        recommend(&s, 0, Some("declared"));
        let kept = region(&page(&s));
        assert!(
            kept.contains("Cortado") && !kept.contains("Declined"),
            "{kept}"
        );
    }

    /// T10's store: a session's delivery estimate, which the page waits for.
    const ESTIMATE_SETUP: &str =
        include_str!("../../../../benchmarks/tasks/T10-estimate-states/setup/pleris.patch");
    /// And T10's reference after it: the estimate streamed, failure and all.
    const ESTIMATE_STREAMED: &str =
        include_str!("../../../../benchmarks/tasks/T10-estimate-states/reference/pleris.patch");

    fn estimate(s: &Server, session: &str, fail: Option<&str>, minutes: i64) {
        s.estimators.lock().expect("estimators").insert(
            session.to_string(),
            Estimator {
                delay_ms: 0,
                fail: fail.map(str::to_string),
                minutes,
            },
        );
    }

    fn page_as(s: &Server, session: &str) -> String {
        fetched_as(s, "/StorePage.html", Some(session))
            .into_iter()
            .map(|(_, c)| c)
            .collect()
    }

    #[test]
    fn a_session_s_estimate_is_its_estimator_s() {
        let s = served_from_patches(|app| app.to_string(), &[ESTIMATE_SETUP]);
        estimate(&s, "a", None, 35);
        assert!(visible(&page_as(&s, "a")).contains("Delivery in 35 min"));
        // Another session's estimator is its own.
        assert!(visible(&page_as(&s, "b")).contains("Delivery in 25 min"));
        // A page that waits for an estimate that fails is unavailable, as
        // any page whose values cannot be read is (ADR-0147): what T10 fixes.
        estimate(&s, "a", Some("down"), 35);
        assert!(page_as(&s, "a").starts_with("HTTP/1.1 503"));
    }

    #[test]
    fn a_failed_estimate_fills_its_region_and_the_page_is_served() {
        let s = served_from_patches(|app| app.to_string(), &[ESTIMATE_SETUP, ESTIMATE_STREAMED]);
        estimate(&s, "a", Some("down"), 35);
        let failed = page_as(&s, "a");
        assert!(failed.starts_with("HTTP/1.1 200"), "{failed}");
        assert!(visible(&failed).contains("Espresso"), "{failed}");
        let arm = &failed[failed.find("<template for=").expect("the region's arm")..];
        assert!(
            visible(arm).contains("Delivery estimate unavailable"),
            "{arm}"
        );
        // And a declared error is a failure too, where the arm binds none.
        estimate(&s, "b", Some("declared"), 35);
        let declared = page_as(&s, "b");
        assert!(
            visible(&declared).contains("Delivery estimate unavailable"),
            "{declared}"
        );
        estimate(&s, "c", None, 35);
        assert!(visible(&page_as(&s, "c")).contains("Delivery in 35 min"));
    }

    #[test]
    fn a_menu_changed_at_its_source_shows_once_its_value_is_read_again() {
        // A materialized fragment is kept while it shows the `Menu` query's
        // value. Until 2026-10-03 it was kept until a command invalidated it,
        // so a menu changed at its source, read again once its freshness was
        // spent, was not what a new document showed.
        let s = served_from(|app| app.to_string(), None);
        let (before, _) = s.serve_document("a");
        assert!(visible(&before).contains("Cortado"), "{before}");
        s.menu
            .lock()
            .expect("menu")
            .retain(|(id, _)| id != "cortado");
        // The query's freshness spent: its value is read again.
        s.queries.invalidate("store.page.Menu");
        let (after, _) = s.serve_document("b");
        assert!(!visible(&after).contains("Cortado"), "{after}");
        assert!(visible(&after).contains("Espresso"), "{after}");
    }

    /// T09's store: a notice board the page reads, kept five minutes.
    const NOTICE_SETUP: &str =
        include_str!("../../../../benchmarks/tasks/T09-notice-freshness/setup/pleris.patch");

    fn notice_calls(s: &Server) -> u64 {
        s.calls
            .lock()
            .expect("calls")
            .get("store:data/notices#current")
            .copied()
            .unwrap_or(0)
    }

    #[test]
    fn a_notice_is_kept_as_its_freshness_says_and_each_ask_is_counted() {
        let s = served_from_patches(|app| app.to_string(), &[NOTICE_SETUP]);
        let (first, _) = s.serve_document("a");
        assert!(visible(&first).contains("Open until 7 pm"), "{first}");
        assert_eq!(notice_calls(&s), 1);
        // Posted at the source, and nothing is told: the kept answer is
        // served, and the board is not asked again.
        *s.notice.lock().expect("notice") = "Closing early".to_string();
        let (kept, _) = s.serve_document("b");
        assert!(visible(&kept).contains("Open until 7 pm"), "{kept}");
        assert_eq!(notice_calls(&s), 1);
        // Once its freshness is spent, the board is asked, once.
        s.queries.invalidate("store.page.Notice");
        let (fresh, _) = s.serve_document("c");
        assert!(visible(&fresh).contains("Closing early"), "{fresh}");
        assert_eq!(notice_calls(&s), 2);
    }

    #[test]
    fn an_item_sold_out_since_the_page_is_refused_by_name() {
        // Charter §15.4 and §15.6 test 10, on the canonical store: the add
        // reads the item's availability inside the command, and a sold-out
        // item is refused with its typed error, which the handler is
        // answered (ADR-0157). Nothing commits, and a retried interaction is
        // given the same answer.
        let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
        s.serve_document("a");
        s.sold_out
            .lock()
            .expect("sold out")
            .insert("cortado".to_string());
        let cortado = [serde_json::json!("cortado"), serde_json::json!(1)];
        let refused = s
            .command_answer(ADD, "a", &cortado, Some("press-1"))
            .expect("a well-formed request");
        assert!(!refused.committed);
        assert_eq!(
            refused.result,
            Some(serde_json::json!({
                "$case": "err",
                "value": { "$case": "item-unavailable", "value": "cortado" }
            }))
        );
        assert_eq!(
            s.cart_value("a"),
            0,
            "the refused add left the cart as it was"
        );
        let again = s
            .command_answer(ADD, "a", &cortado, Some("press-1"))
            .expect("a well-formed request");
        assert_eq!(again, refused, "a retry is given the first answer");
        // Back in stock, the next press adds.
        s.sold_out.lock().expect("sold out").clear();
        let added = s
            .command_answer(ADD, "a", &cortado, Some("press-2"))
            .expect("a well-formed request");
        assert!(added.committed);
        // Answered `Ok`, without the cart: the page learns that from the
        // resource.
        assert_eq!(added.result, Some(serde_json::json!({ "$case": "ok" })));
        assert_eq!(s.cart_value("a"), 1);
    }

    /// T07's store: the menu by category, a binding a signal keys, its
    /// stale work `keep` (ADR-0152).
    const BROWSE_SETUP: &str =
        include_str!("../../../../benchmarks/tasks/T07-stale-category/setup/pleris.patch");
    /// T07's reference, `cancel`, and its unsafe patch, `supersede`.
    const BROWSE_CANCEL: &str =
        include_str!("../../../../benchmarks/tasks/T07-stale-category/reference/pleris.patch");
    const BROWSE_SUPERSEDE: &str =
        include_str!("../../../../benchmarks/tasks/T07-stale-category/unsafe/pleris.patch");

    /// The names the page shows of the binding `browsing`, as recorded.
    fn browsed(s: &Server, session: &str) -> Vec<String> {
        let document = latest(&s.shown.lock().expect("shown"), session).1;
        browsed_in(s, session, document)
    }

    /// [`browsed`], for one document of the session (ADR-0161).
    fn browsed_in(s: &Server, session: &str, document: u64) -> Vec<String> {
        let shown = s.shown.lock().expect("shown");
        shown[&(session.to_string(), document)].lists["browsing"]
            .iter()
            .filter_map(|item| match item {
                Value::Record(fields) => match fields.get("name") {
                    Some(Value::Text(name)) => Some(name.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    fn category(name: &str) -> BTreeMap<String, serde_json::Value> {
        BTreeMap::from([("category".to_string(), serde_json::json!(name))])
    }

    const STAYED: &(dyn Fn() -> bool + Sync) = &|| false;

    #[test]
    fn a_keyed_read_is_applied_as_one_patch_set() {
        let s = served_from_patches(|app| app.to_string(), &[BROWSE_SETUP, BROWSE_CANCEL]);
        let (page, cursor) = s.serve_document("a");
        // The document shows the signal's first key: every item.
        assert!(visible(&page).contains("Cold Brew"), "{page}");
        assert_eq!(browsed(&s, "a"), ["Espresso", "Cortado", "Cold Brew"]);
        let read = s.read_keyed("a", "browsing", 1, cursor, &category("cold"), STAYED);
        assert_eq!(read, Ok(KeyOutcome::Applied));
        assert_eq!(browsed(&s, "a"), ["Cold Brew"]);
        // One patch set, in the session's frames, after the document.
        let queue = s.pending.lock().expect("pending");
        let sets: Vec<&PatchSet> = queue[&latest(&queue, "a")]
            .frames
            .iter()
            .filter_map(|(_, f)| match f {
                StreamFrame::PatchSet(set) => Some(set),
                _ => None,
            })
            .collect();
        assert_eq!(sets.len(), 1);
        assert!(!sets[0].patches.is_empty());
        drop(queue);
        // A command re-reads the binding for the key the page shows.
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("the command commits");
        assert_eq!(browsed(&s, "a"), ["Cold Brew"]);
    }

    #[test]
    fn a_read_older_than_the_latest_is_not_applied() {
        let s = served_from_patches(|app| app.to_string(), &[BROWSE_SETUP, BROWSE_CANCEL]);
        let (_, cursor) = s.serve_document("a");
        assert_eq!(
            s.read_keyed("a", "browsing", 2, cursor, &category("cold"), STAYED),
            Ok(KeyOutcome::Applied)
        );
        // The first read, arriving after the second.
        assert_eq!(
            s.read_keyed("a", "browsing", 1, cursor, &category("hot"), STAYED),
            Ok(KeyOutcome::Superseded)
        );
        assert_eq!(browsed(&s, "a"), ["Cold Brew"]);
        // A second page of the session, as a second tab: each document is its
        // own (ADR-0161). Until 2026-10-03 it replaced the first, and a read
        // the first asked for was dropped.
        let (_, newer) = s.serve_document("a");
        // The new page shows the first key, as its browser holds it.
        assert_eq!(browsed(&s, "a"), ["Espresso", "Cortado", "Cold Brew"]);
        // A read the first page asks for is the first page's ...
        assert_eq!(
            s.read_keyed("a", "browsing", 3, cursor, &category("hot"), STAYED),
            Ok(KeyOutcome::Applied)
        );
        assert_eq!(browsed_in(&s, "a", cursor), ["Espresso", "Cortado"]);
        // ... and leaves the second as it was.
        assert_eq!(browsed(&s, "a"), ["Espresso", "Cortado", "Cold Brew"]);
        assert_eq!(
            s.read_keyed("a", "browsing", 1, newer, &category("cold"), STAYED),
            Ok(KeyOutcome::Applied)
        );
        assert_eq!(browsed(&s, "a"), ["Cold Brew"]);
        // A read for a document the server does not hold is no page's, and
        // takes nothing: no reads kept for it, no flight held.
        assert_eq!(
            s.read_keyed("a", "browsing", 9, newer + 1_000, &category("hot"), STAYED),
            Ok(KeyOutcome::Superseded)
        );
        assert!(
            !s.keyed
                .lock()
                .expect("keyed")
                .contains_key(&("a".to_string(), newer + 1_000))
        );
    }

    /// How long a keyed read took, in milliseconds, and what came of it.
    type Timed = (u128, Result<KeyOutcome, String>);

    /// Hot is read, slow, and Cold asked for 150 ms later: how long each
    /// read took, what came of each, and how many reads of a category were
    /// stopped.
    fn hot_then_cold(s: &Server) -> (Timed, Timed, u64) {
        *s.categories.lock().expect("categories") = Categories {
            slow: Some("hot".to_string()),
            delay_ms: 1_000,
        };
        let (_, cursor) = s.serve_document("a");
        let timed = |seq, key: &str| {
            let started = std::time::Instant::now();
            let outcome = s.read_keyed("a", "browsing", seq, cursor, &category(key), STAYED);
            (started.elapsed().as_millis(), outcome)
        };
        let (hot, cold) = std::thread::scope(|scope| {
            let hot = scope.spawn(|| timed(1, "hot"));
            std::thread::sleep(std::time::Duration::from_millis(150));
            let cold = scope.spawn(|| timed(2, "cold"));
            (hot.join().expect("hot"), cold.join().expect("cold"))
        });
        let stopped = s.category_stopped.load(std::sync::atomic::Ordering::SeqCst);
        (hot, cold, stopped)
    }

    #[test]
    fn under_cancel_a_newer_key_stops_the_old_keys_read() {
        let s = served_from_patches(|app| app.to_string(), &[BROWSE_SETUP, BROWSE_CANCEL]);
        let ((hot_ms, hot), (cold_ms, cold), stopped) = hot_then_cold(&s);
        assert_eq!(hot, Ok(KeyOutcome::Superseded));
        assert_eq!(cold, Ok(KeyOutcome::Applied));
        // Stopped part way, not read to its end.
        assert_eq!(stopped, 1);
        assert!(hot_ms < 700, "Hot's read took {hot_ms} ms");
        assert!(cold_ms < 500, "Cold's read took {cold_ms} ms");
        assert_eq!(browsed(&s, "a"), ["Cold Brew"]);
    }

    #[test]
    fn under_supersede_the_old_keys_read_runs_on_unshown() {
        let s = served_from_patches(|app| app.to_string(), &[BROWSE_SETUP, BROWSE_SUPERSEDE]);
        let ((hot_ms, hot), (cold_ms, cold), stopped) = hot_then_cold(&s);
        assert_eq!(hot, Ok(KeyOutcome::Superseded));
        assert_eq!(cold, Ok(KeyOutcome::Applied));
        assert_eq!(stopped, 0);
        assert!(hot_ms >= 900, "Hot's read took {hot_ms} ms");
        assert!(cold_ms < 500, "Cold's read took {cold_ms} ms");
        assert_eq!(browsed(&s, "a"), ["Cold Brew"]);
    }

    #[test]
    fn under_keep_the_new_key_is_read_after_the_old_one_ends() {
        let s = served_from_patches(|app| app.to_string(), &[BROWSE_SETUP]);
        let ((hot_ms, hot), (cold_ms, cold), stopped) = hot_then_cold(&s);
        // Hot's read ends, and is not shown: Cold was asked for meanwhile.
        assert_eq!(hot, Ok(KeyOutcome::Superseded));
        assert_eq!(cold, Ok(KeyOutcome::Applied));
        assert_eq!(stopped, 0);
        assert!(hot_ms >= 900, "Hot's read took {hot_ms} ms");
        // Cold waited for it.
        assert!(cold_ms >= 700, "Cold's read took {cold_ms} ms");
        assert_eq!(browsed(&s, "a"), ["Cold Brew"]);
    }

    #[test]
    fn a_flight_another_reader_waits_for_is_not_stopped() {
        // ADR-0152: `cancel` lets go of a read's flight, and `pw-resource`
        // stops a flight only when nobody holds it. Here session `a` reads
        // Hot, and session `b`, whose page shows Hot, reads it again for a
        // command while `a`'s read is in flight, sharing it. `a` then moves
        // to Cold: the flight goes on for `b`.
        let s = served_from_patches(|app| app.to_string(), &[BROWSE_SETUP, BROWSE_CANCEL]);
        let (_, b) = s.serve_document("b");
        assert_eq!(
            s.read_keyed("b", "browsing", 1, b, &category("hot"), STAYED),
            Ok(KeyOutcome::Applied)
        );
        *s.categories.lock().expect("categories") = Categories {
            slow: Some("hot".to_string()),
            delay_ms: 1_000,
        };
        let (_, a) = s.serve_document("a");
        std::thread::scope(|scope| {
            let hot = scope.spawn(|| s.read_keyed("a", "browsing", 1, a, &category("hot"), STAYED));
            std::thread::sleep(std::time::Duration::from_millis(100));
            let command = scope.spawn(|| s.command(ADD, "b", &add("espresso", 1), false));
            std::thread::sleep(std::time::Duration::from_millis(100));
            assert_eq!(
                s.read_keyed("a", "browsing", 2, a, &category("cold"), STAYED),
                Ok(KeyOutcome::Applied)
            );
            assert_eq!(hot.join().expect("hot"), Ok(KeyOutcome::Superseded));
            command
                .join()
                .expect("command")
                .expect("the command commits");
        });
        // Read to its end, for `b`: nothing was stopped, and `b`'s page was
        // not told to read itself again.
        assert_eq!(
            s.category_stopped.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        let queue = s.pending.lock().expect("pending");
        assert!(
            !queue[&latest(&queue, "b")]
                .frames
                .iter()
                .any(|(_, f)| matches!(f, StreamFrame::Recovery { .. })),
            "b was told to reload"
        );
        drop(queue);
        assert_eq!(browsed(&s, "b"), ["Espresso", "Cortado"]);
        assert_eq!(browsed(&s, "a"), ["Cold Brew"]);
    }

    #[test]
    fn a_browser_that_leaves_lets_go_of_its_read() {
        let s = served_from_patches(|app| app.to_string(), &[BROWSE_SETUP, BROWSE_CANCEL]);
        *s.categories.lock().expect("categories") = Categories {
            slow: Some("hot".to_string()),
            delay_ms: 1_000,
        };
        let (_, cursor) = s.serve_document("a");
        let started = std::time::Instant::now();
        let left = || started.elapsed() > std::time::Duration::from_millis(150);
        let read = s.read_keyed("a", "browsing", 1, cursor, &category("hot"), &left);
        assert_eq!(read, Ok(KeyOutcome::Superseded));
        assert!(started.elapsed() < std::time::Duration::from_millis(700));
        assert_eq!(
            s.category_stopped.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        // Nothing it read is shown.
        assert_eq!(browsed(&s, "a"), ["Espresso", "Cortado", "Cold Brew"]);
    }

    /// T02's store: the kitchen's prep time, which every page asks for.
    const PREP_SETUP: &str =
        include_str!("../../../../benchmarks/tasks/T02-request-storm/setup/pleris.patch");
    /// T02's reference: pages that ask while an ask is under way share it.
    const PREP_SHARED: &str =
        include_str!("../../../../benchmarks/tasks/T02-request-storm/reference/pleris.patch");

    fn prep_calls(s: &Server) -> u64 {
        s.calls
            .lock()
            .expect("calls")
            .get("store:data/kitchen#prep-minutes")
            .copied()
            .unwrap_or(0)
    }

    /// Eight documents served at once, each to its own session: how long
    /// the eight took, and how many times the kitchen was asked.
    fn eight_at_once(s: &Server) -> (std::time::Duration, u64) {
        let before = prep_calls(s);
        let started = std::time::Instant::now();
        std::thread::scope(|scope| {
            for i in 0..8 {
                scope.spawn(move || {
                    let (page, _) = s.serve_document(&format!("burst-{i}"));
                    assert!(visible(&page).contains("Ready in 12 min"), "{page}");
                });
            }
        });
        (started.elapsed(), prep_calls(s) - before)
    }

    #[test]
    fn pages_read_at_once_share_an_ask_their_query_says_is_one_per_key() {
        // ADR-0151: a document is read outside the subscriber table, so
        // eight pages are read at once, and share the kitchen's answer.
        let s = served_from_patches(|app| app.to_string(), &[PREP_SETUP, PREP_SHARED]);
        let (took, asked) = eight_at_once(&s);
        assert!(asked <= 2, "the kitchen was asked {asked} times");
        // About one ask's time, where one after another took eight.
        assert!(
            took < std::time::Duration::from_millis(4 * KITCHEN_MS),
            "{took:?}"
        );
    }

    #[test]
    fn pages_read_at_once_each_ask_when_their_query_says_parallel() {
        let s = served_from_patches(|app| app.to_string(), &[PREP_SETUP]);
        let (took, asked) = eight_at_once(&s);
        assert_eq!(asked, 8);
        // Asked at once, all the same.
        assert!(
            took < std::time::Duration::from_millis(4 * KITCHEN_MS),
            "{took:?}"
        );
    }

    #[test]
    fn a_change_that_reaches_a_session_while_its_page_is_read_is_not_lost() {
        // ADR-0151: a document read before a change, whose frames reached
        // the session while it was read, is not installed; it is read again.
        let s = served_from_patches(|app| app.to_string(), &[]);
        s.serve_document("a");
        let doc: Doc = (
            "a".to_string(),
            s.documents
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );
        s.params
            .lock()
            .expect("params")
            .insert(doc.clone(), store_params(STORE_ID));
        let pushed = s.subscribed(&doc);
        let before = s.render_store_document(&doc, &[]).expect("the page reads");
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("the command commits");
        let mut queue = s.pending.lock().expect("pending");
        assert!(
            s.installed(&mut queue, &doc, before, Some(pushed))
                .is_none()
        );
        drop(queue);
        // Served whole, it shows the change.
        let (page, _) = s.serve_document("a");
        let at = page.find("id=\"cart-count\"").expect("the count");
        let rest = &page[at..];
        let count = visible(&rest[rest.find('>').expect("its tag") + 1..]);
        assert!(count.starts_with('1'), "{page}");
    }

    #[test]
    fn a_command_asks_only_the_queries_its_page_shows_once() {
        // ADR-0151: the cart's count is read from the cart's binding alone.
        // A command asked every query the page reads for it, and then again
        // for the patches.
        let s = served_from_patches(|app| app.to_string(), &[PREP_SETUP]);
        s.serve_document("a");
        assert_eq!(prep_calls(&s), 1);
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("the command commits");
        // Once, for what the page shows after it.
        assert_eq!(prep_calls(&s), 2);
    }

    #[test]
    fn a_page_with_nothing_streamed_is_one_response_of_known_length() {
        let s = served_from(|app| app.to_string(), None);
        let whole: String = fetched(&s, "/StorePage.html")
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        assert!(whole.contains("content-length:"), "{whole}");
        assert!(!whole.contains("<?start"), "{whole}");
    }

    /// **A session's latest document** (ADR-0161): what a test that serves a
    /// session one page means by its subscriber.
    fn latest<V>(map: &BTreeMap<Doc, V>, session: &str) -> Doc {
        documents_of(map, session)
            .pop()
            .unwrap_or_else(|| panic!("no document for `{session}`"))
    }

    /// Each patch set queued for a session's latest document, in order.
    fn patch_sets(s: &Server, session: &str) -> Vec<PatchSet> {
        let queue = s.pending.lock().expect("pending");
        documents_of(&queue, session)
            .pop()
            .and_then(|doc| queue.get(&doc))
            .map(|w| {
                w.frames
                    .iter()
                    .filter_map(|(_, f)| match f {
                        StreamFrame::PatchSet(p) => Some(p.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// One patch, as `operation@part[:instance frames]`, for an assertion.
    fn said(t: &Targeted) -> String {
        let op = match &t.operation {
            PatchOp::ReplaceText { text } => format!("text {text:?}"),
            PatchOp::InsertBefore { instance, html } => {
                format!("insert before {} {}", instance.is_some(), visible(html))
            }
            PatchOp::InsertAfter { instance, html } => {
                format!("insert after {} {}", instance.is_some(), visible(html))
            }
            PatchOp::RemoveInstance { .. } => "remove".to_string(),
            PatchOp::MoveInstance { after, .. } => format!("move after {}", after.is_some()),
            PatchOp::ReplaceRange { html } => format!("range {:?}", visible(html)),
            other => format!("{other:?}"),
        };
        format!(
            "{op} @{}{}",
            t.target.part.0,
            if t.target.instances.is_empty() {
                ""
            } else {
                " in an instance"
            }
        )
    }

    /// Markup's text, its comments and tags dropped.
    fn visible(html: &str) -> String {
        let mut out = String::new();
        let mut depth = 0;
        for c in html.chars() {
            match c {
                '<' => depth += 1,
                '>' => depth -= 1,
                c if depth == 0 => out.push(c),
                _ => {}
            }
        }
        out.trim().to_string()
    }

    #[test]
    fn a_list_a_session_s_query_fills_is_rendered() {
        let s = lines_server();
        s.command(ADD, "a", &add("espresso", 2), false)
            .expect("runs");
        let (page, _) = s.serve_document("a");
        let page = visible(&page);
        assert!(page.contains("Espresso × 2"), "the session's lines: {page}");
        // Another session's document lists none of them.
        let (other, _) = s.serve_document("b");
        let other = visible(&other);
        assert!(!other.contains("× 2"), "another session's: {other}");
    }

    #[test]
    fn a_change_patches_every_place_it_reaches_in_one_frame() {
        let s = lines_server();
        s.serve_document("a");
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("runs");
        let sets = patch_sets(&s, "a");
        assert_eq!(sets.len(), 1, "one frame for the change: {sets:?}");
        let got: Vec<String> = sets[0].patches.iter().map(said).collect();
        // The count, and the first line, at the head of the list.
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(got[0].starts_with("text \"1\""), "{got:?}");
        assert!(
            got[1].starts_with("insert before false Espresso × 1"),
            "{got:?}"
        );
    }

    #[test]
    fn an_item_that_stays_is_set_in_place() {
        let s = lines_server();
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("runs");
        s.serve_document("a");
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("runs");
        let got: Vec<String> = patch_sets(&s, "a")[0].patches.iter().map(said).collect();
        // The count, and the line's quantity inside its instance: its nodes
        // stay, nothing is removed or inserted.
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(got[0].starts_with("text \"2\""), "{got:?}");
        assert!(
            got[1].starts_with("text \"2\"") && got[1].ends_with("in an instance"),
            "{got:?}"
        );
    }

    #[test]
    fn a_new_item_goes_after_the_one_before_it() {
        let s = lines_server();
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("runs");
        s.serve_document("a");
        s.command(ADD, "a", &add("cortado", 1), false)
            .expect("runs");
        let got: Vec<String> = patch_sets(&s, "a")[0].patches.iter().map(said).collect();
        assert!(
            got.iter()
                .any(|p| p.starts_with("insert after true Cortado × 1")),
            "{got:?}"
        );
    }

    #[test]
    fn an_item_that_leaves_is_removed() {
        let s = lines_server();
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("runs");
        s.command(ADD, "a", &add("cortado", 1), false)
            .expect("runs");
        s.serve_document("a");
        s.command(CLEAR, "a", &[], false).expect("runs");
        let got: Vec<String> = patch_sets(&s, "a")[0].patches.iter().map(said).collect();
        assert_eq!(
            got.iter().filter(|p| p.starts_with("remove")).count(),
            2,
            "{got:?}"
        );
        assert!(got[0].starts_with("text \"0\""), "{got:?}");
    }

    #[test]
    fn each_change_is_derived_from_the_last_one_sent() {
        let s = lines_server();
        s.serve_document("a");
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("runs");
        s.command(ADD, "a", &add("cortado", 1), false)
            .expect("runs");
        let sets = patch_sets(&s, "a");
        assert_eq!(sets.len(), 2, "{sets:?}");
        // The second set inserts the cortado after the espresso the first
        // one inserted, and inserts nothing else.
        let got: Vec<String> = sets[1].patches.iter().map(said).collect();
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(got[0].starts_with("text \"2\""), "{got:?}");
        assert!(
            got[1].starts_with("insert after true Cortado × 1"),
            "{got:?}"
        );
    }

    /// The store, with a line its cart decides: a block a query decides
    /// (ADR-0146).
    fn with_block(app: &str) -> String {
        let out = app.replacen(
            "                <p id=\"cart-count\">{cart.line_count}</p>",
            "                <p id=\"cart-count\">{cart.line_count}</p>\n                {#if cart.lines}<p id=\"ready\">Ready when you are.</p>{/if}",
            1,
        );
        assert!(out.contains("{#if cart.lines}"));
        out
    }

    #[test]
    fn a_block_a_query_decides_is_rendered() {
        let s = lines_server_with(with_block);
        let (empty, _) = s.serve_document("a");
        assert!(!visible(&empty).contains("Ready"), "{empty}");
        s.command(ADD, "b", &add("espresso", 1), false)
            .expect("runs");
        let (held, _) = s.serve_document("b");
        assert!(visible(&held).contains("Ready when you are."), "{held}");
    }

    #[test]
    fn a_block_whose_rendering_changed_is_rendered_again() {
        let s = lines_server_with(with_block);
        s.serve_document("a");
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("runs");
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("runs");
        s.command(CLEAR, "a", &[], false).expect("runs");
        let sets: Vec<Vec<String>> = patch_sets(&s, "a")
            .iter()
            .map(|p| p.patches.iter().map(said).collect())
            .collect();
        assert_eq!(sets.len(), 3, "{sets:?}");
        // Shown, with the first line: the count, and the block.
        assert_eq!(sets[0].len(), 2, "{sets:?}");
        assert!(
            sets[0][1].starts_with("range \"Ready when you are.\""),
            "{sets:?}"
        );
        // Still shown: the count alone.
        assert_eq!(sets[1].len(), 1, "{sets:?}");
        assert!(sets[1][0].starts_with("text \"2\""), "{sets:?}");
        // Gone, with the last line.
        assert_eq!(sets[2].len(), 2, "{sets:?}");
        assert!(sets[2][1].starts_with("range \"\""), "{sets:?}");
    }

    #[test]
    fn a_case_is_a_case() {
        // As a `{#match}` arm names it (ADR-0146): `Some`, `None`, `Ok` and
        // `Err` as Pleris writes them, and a declared case by its WIT name.
        assert_eq!(
            val_to_value(&Val::Enum("preparing".into())),
            Value::Variant {
                case: "preparing".into(),
                payload: None
            }
        );
        assert_eq!(
            val_to_value(&Val::Option(Some(Box::new(Val::S64(3))))),
            Value::Variant {
                case: "Some".into(),
                payload: Some(Box::new(Value::Int(3)))
            }
        );
        assert_eq!(
            val_to_value(&Val::Result(Err(Some(Box::new(Val::String("x".into())))))),
            Value::Variant {
                case: "Err".into(),
                payload: Some(Box::new(Value::Text("x".into())))
            }
        );
    }

    #[test]
    fn the_menu_is_no_session_s_list() {
        // A shared list is one fragment for every reader, patched once for
        // all of them (E7-P); a session's lists are the private ones.
        assert_eq!(lines_server().own_lists(), ["lines".to_string()]);
    }

    #[test]
    fn a_session_s_change_reaches_its_own_document_alone() {
        let s = lines_server();
        s.serve_document("a");
        s.serve_document("b");
        s.command(ADD, "a", &add("espresso", 1), false)
            .expect("runs");
        assert_eq!(patch_sets(&s, "a").len(), 1);
        assert!(
            patch_sets(&s, "b").is_empty(),
            "b's document changed nothing"
        );
    }

    #[test]
    fn a_list_moves_what_moved_and_replaces_what_holds_more_than_text() {
        let s = lines_server();
        let template = s.store_template().clone();
        let schema = TemplateSchemaId(template.schema.clone());
        let env = Env::new().in_domain(s.domain("a"));
        let line = |id: &str, n: i64| {
            Value::Record(
                [
                    ("item_id".to_string(), Value::Text(id.into())),
                    ("name".to_string(), Value::Text(id.to_uppercase())),
                    ("quantity".to_string(), Value::Int(n)),
                ]
                .into(),
            )
        };
        // A move keeps the nodes: no removal, no insertion.
        let moved = list_patches(
            &template,
            &schema,
            "lines",
            &[line("a", 1), line("b", 1), line("c", 1)],
            &[line("c", 1), line("a", 1), line("b", 1)],
            &env,
            &s.templates,
        )
        .expect("derived");
        let got: Vec<String> = moved.iter().map(said).collect();
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].starts_with("move after false"), "{got:?}");
        // A menu item's markup holds a button as well as text: one whose name
        // changed is rendered again where it is.
        let item = |id: &str, name: &str| {
            Value::Record(
                [
                    ("id".to_string(), Value::Text(id.into())),
                    ("name".to_string(), Value::Text(name.into())),
                ]
                .into(),
            )
        };
        let renamed = list_patches(
            &template,
            &schema,
            "menu",
            &[item("a", "A"), item("b", "B")],
            &[item("a", "A"), item("b", "Bee")],
            &env,
            &s.templates,
        )
        .expect("derived");
        let got: Vec<String> = renamed.iter().map(said).collect();
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(got[0].starts_with("remove"), "{got:?}");
        assert!(got[1].starts_with("insert after true Bee"), "{got:?}");
    }

    /// A node that grants nothing at all.
    fn barren() -> Topology {
        Topology {
            nodes: vec![Node {
                name: "barren".to_string(),
                world: "origin".to_string(),
                grants: Default::default(),
            }],
        }
    }

    fn server(topology: Topology) -> Server {
        Server::on(std::path::PathBuf::from("."), Vec::new(), topology)
    }

    const ADD: &str = "store.page.add_to_cart";
    const CLEAR: &str = "store.page.clear_cart";

    /// `add_to_cart`'s arguments, as the component takes them.
    fn add(item: &str, quantity: i64) -> [Val; 2] {
        [Val::String(item.into()), Val::S64(quantity)]
    }

    /// With the compiler's template IR, for a test whose second command
    /// regenerates the fragment the first one materialized.
    fn rendering_server() -> Server {
        let templates: Vec<Template> =
            serde_json::from_str(include_str!("../../store-ir.json")).expect("the template IR");
        Server::on(std::path::PathBuf::from("."), templates, dev_topology())
    }

    /// **The command path asks, and a refusal is a refusal.**
    ///
    /// E8's last gate item asks for the dev server's command path to go through
    /// the host. This is the half that is real: `add_to_cart` requires
    /// `database.write<Carts>` by its CONTRACT, and a node without it cannot
    /// run the command — the state does not move and no event is queued.
    ///
    /// The body is the compiled component (E10-I); the refusal comes first.
    #[test]
    fn a_command_is_refused_on_a_node_that_does_not_grant_its_capability() {
        let s = server(barren());
        let err = s
            .command(ADD, "session-1", &add("espresso", 1), false)
            .expect_err("a barren node cannot host a write");
        assert!(
            err.contains("add_to_cart") && err.contains("barren"),
            "and it says which command and which node: {err}"
        );
        assert_eq!(s.cart_value("session-1"), 0, "the state did not move");
    }

    /// The discriminating half. Without it the test above passes for a command
    /// path that refuses everything, which is not a capability system.
    #[test]
    fn the_same_command_runs_where_the_capability_exists() {
        let s = server(dev_topology());
        s.command(ADD, "session-1", &add("espresso", 1), false)
            .expect("the dev origin grants the write");
        assert_eq!(s.cart_value("session-1"), 1);
    }

    /// **The command's arguments reach the data layer through the compiled
    /// body**, and its result is the component's. `cortado` and `2` go in as
    /// the command's arguments; the session is the host's; the lines the
    /// data layer holds afterwards are what `Carts.add` was called with.
    #[test]
    fn the_compiled_command_carries_its_arguments_to_the_data_layer() {
        let s = rendering_server();
        s.command(ADD, "session-9", &add("cortado", 2), false)
            .expect("runs");
        s.command(ADD, "session-9", &add("espresso", 1), false)
            .expect("runs");
        s.command(ADD, "session-9", &add("cortado", 1), false)
            .expect("runs");
        let lines = s.carts.lock().unwrap().get("session-9").cloned().unwrap();
        assert_eq!(
            lines,
            [("cortado".to_string(), 3), ("espresso".to_string(), 1)]
        );
        assert_eq!(s.cart_value("session-9"), 4);
        assert_eq!(
            s.cart_value("another-session"),
            0,
            "one session's cart only"
        );
    }

    /// **A failed command commits nothing.** The data layer answers
    /// `cart-expired`, the COMPONENT returns it, and neither the staged write
    /// nor its event survives — ADR-0019, now decided by what the compiled
    /// command returned rather than by a closure's control flow.
    #[test]
    fn a_failing_command_commits_neither_state_nor_event() {
        let s = rendering_server();
        s.command(ADD, "session-3", &add("espresso", 1), false)
            .expect("runs");
        let err = s
            .command(ADD, "session-3", &add("cortado", 5), true)
            .expect_err("the data layer refuses");
        assert!(
            err.contains("cart-expired"),
            "the component's own result: {err}"
        );
        assert_eq!(s.cart_value("session-3"), 1, "the failed add left nothing");
    }

    /// **No Rust closure computes a command, and no Rust writes a handler.**
    /// Structural, because the failure it guards against — a body quietly
    /// reimplemented beside the component, or a handler module written here
    /// instead of compiled — would pass every behavioural test above.
    ///
    /// The needles are assembled, so this test's own text cannot match them.
    #[test]
    fn the_commands_and_handlers_are_the_compilers() {
        let src = include_str!("main.rs");
        // The test module, not the first test-only item: `Server::on` and the
        // committed-artifact loaders are test-only too (ADR-0123).
        let server_code = &src[..src
            .find(&["#[cfg(test)]\nmod ", "tests"].concat())
            .expect("the tests")];
        // Where a command's component runs: `command_answered`, which
        // `command` answers through since ADR-0157.
        let start = server_code
            .find("fn command_answered(")
            .expect("the command path");
        let body =
            &server_code[start..start + server_code[start..].find("\n    }\n").expect("its end")];
        assert!(
            body.contains("self.run(component_id, session, &host, args)"),
            "every command runs its compiled component"
        );
        assert!(
            !body.contains(&["\"Events", "."].concat()),
            "a command's events are the ones it declares (ADR-0104), never named here"
        );
        for gone in [
            ["fn add_to_", "cart("].concat(),
            ["fn clear_", "cart("].concat(),
            ["fn menu_item", "_at("].concat(),
            ["export async ", "function run"].concat(),
        ] {
            assert!(
                !server_code.contains(&gone),
                "`{gone}` is back: per-command Rust, or a handler written by the server"
            );
        }
    }

    /// **A command's arguments from a browser are typed by the component.**
    ///
    /// What `dist/handlers/<identity>.mjs` sends: `context.command(id, args)`
    /// posts the arguments as JSON, and they are converted by the parameter
    /// types the ARTIFACT declares before anything runs.
    #[test]
    fn a_browsers_arguments_are_typed_by_the_components_own_parameters() {
        let s = rendering_server();
        let ran = s
            .command_json(
                ADD,
                "session-5",
                &[serde_json::json!("cortado"), serde_json::json!(2)],
                Some("press-1"),
            )
            .expect("well-formed");
        ran.expect("and it committed");
        assert_eq!(s.cart_value("session-5"), 2);

        // Refused before anything runs, each for its own reason.
        for (args, why) in [
            (
                vec![serde_json::json!("cortado")],
                "takes 2 argument(s); 1 were sent",
            ),
            (
                vec![serde_json::json!(2), serde_json::json!(2)],
                "expected a string",
            ),
            (
                vec![serde_json::json!("cortado"), serde_json::json!(1.5)],
                "expected an integer",
            ),
            (
                vec![serde_json::json!("cortado"), serde_json::json!("2")],
                "expected an integer",
            ),
            (
                vec![serde_json::json!("cortado"), serde_json::json!(u64::MAX)],
                "is outside",
            ),
        ] {
            let err = s
                .command_json(ADD, "session-5", &args, Some("press-2"))
                .expect_err("malformed");
            assert!(err.contains(why), "{args:?}: {err}");
        }
        assert_eq!(
            s.cart_value("session-5"),
            2,
            "no refused request moved the state"
        );

        let err = s
            .command_json("store.page.nothing_declares_this", "session-5", &[], None)
            .expect_err("not hosted");
        assert!(err.contains("no compiled component"), "{err}");
    }

    /// **`clear_cart` commits its state as well as its event.** Until the
    /// command path was one path, `clear_cart` committed only the event, and
    /// the materializer's `cart:<session>` state kept the old total.
    /// The manifest `pw emit-speculations` writes for the store page.
    fn speculating_server() -> Server {
        let mut s = rendering_server();
        s.speculation = Some(serde_json::json!({
            "page": "store.page.StorePage",
            "module": "store.page.StorePage.mjs",
            "bindings": [{ "binding": "cart", "resource": "store.page.Cart", "key": ["current_session()"] }],
            "commands": ["store.page.add_to_cart"],
        }));
        s
    }

    /// **A page that speculates is sent the value it speculates on**
    /// (ADR-0122): in its document, and as an `entry_value` frame each time
    /// the entry advances, with the version a commit produced in the
    /// command's answer. A page that does not speculate is sent none of it.
    #[test]
    fn a_speculating_page_is_sent_its_carts_value() {
        let s = speculating_server();
        let (_, _, entries) = s.serve_document_with_entries("session-v").expect("served");
        assert_eq!(entries["cart"]["value"], serde_json::json!({ "lines": [] }));
        let before = entries["cart"]["version"].as_u64().expect("a version");

        s.command_json(
            ADD,
            "session-v",
            &[serde_json::json!("cortado"), serde_json::json!(2)],
            Some("press-v"),
        )
        .expect("well-formed")
        .expect("committed");
        let basis = s.committed_basis("session-v");
        let version = basis[0]["version"].as_u64().expect("a version");
        assert!(version > before, "the commit produced a newer version");
        assert_eq!(
            basis[0]["entry"], entries["cart"]["entry"],
            "of the same entry"
        );

        let queue = s.pending.lock().expect("pending");
        let values: Vec<&StreamFrame> = queue[&latest(&queue, "session-v")]
            .frames
            .iter()
            .map(|(_, f)| f)
            .filter(|f| matches!(f, StreamFrame::EntryValue { .. }))
            .collect();
        let [
            StreamFrame::EntryValue {
                version: v, value, ..
            },
        ] = values.as_slice()
        else {
            panic!("one entry_value frame, got {values:?}");
        };
        assert_eq!(v.0, version, "at the version the commit produced");
        assert_eq!(
            *value,
            serde_json::json!({ "lines": [
                { "item_id": "cortado", "quantity": 2, "unit_price": { "minor_units": 450 } }
            ] })
        );
        drop(queue);

        // The control: the same page, built without speculations.
        let plain = rendering_server();
        let (_, _, entries) = plain
            .serve_document_with_entries("session-w")
            .expect("served");
        assert_eq!(entries, serde_json::json!({}));
        plain
            .command_json(
                ADD,
                "session-w",
                &[serde_json::json!("cortado"), serde_json::json!(1)],
                Some("press-w"),
            )
            .expect("well-formed")
            .expect("committed");
        let queue = plain.pending.lock().expect("pending");
        assert!(
            !queue[&latest(&queue, "session-w")]
                .frames
                .iter()
                .any(|(_, f)| matches!(f, StreamFrame::EntryValue { .. })),
            "no value to a page that does not speculate"
        );
    }

    /// **What the page shows is its queries'** (ADR-0125). Structural, as
    /// `the_commands_and_handlers_are_the_compilers` is: a value written here
    /// in Rust would render correctly and pass every behavioural test while
    /// the compiled queries ran for nothing. The needles are assembled.
    #[test]
    fn the_pages_values_are_its_queries() {
        let src = include_str!("main.rs");
        let server_code = &src[..src
            .find(&["#[cfg(test)]\nmod ", "tests"].concat())
            .expect("the tests")];
        for gone in [
            [".set(\"store", ".name\""].concat(),
            [".set(\"cart", ".line_count\""].concat(),
            ["Value::Text(\"Blue", " Bottle\""].concat(),
        ] {
            assert!(
                !server_code.contains(&gone),
                "`{gone}` is back: a page value computed by the server, not its query"
            );
        }
        // And the render goes through the plan.
        let s = rendering_server();
        let html = s.render_store("session-q");
        assert!(
            html.contains(STORE_NAME),
            "the Store query's name is rendered"
        );
        s.command(ADD, "session-q", &add("cortado", 3), false)
            .expect("runs");
        assert_eq!(
            s.part_text("session-q", "cart.line_count").as_deref(),
            Ok("3")
        );
    }

    fn calls(s: &Server, op: &str) -> u64 {
        s.calls.lock().expect("calls").get(op).copied().unwrap_or(0)
    }

    /// **A shared query runs once for every reader, within its freshness**
    /// (ADR-0127). `Store` is shared for 30 seconds and `Menu` for five
    /// minutes; `Cart` is private and kept for no time, so each render runs it.
    #[test]
    fn a_shared_query_is_run_once_for_its_readers_and_a_private_one_for_each() {
        let s = rendering_server();
        s.render_store("reader-a");
        s.render_store("reader-b");
        assert_eq!(
            calls(&s, "store:data/stores#get"),
            1,
            "one Store for both readers"
        );
        assert_eq!(
            calls(&s, "store:data/menus#for-store"),
            1,
            "one Menu for both"
        );
        assert_eq!(
            calls(&s, "store:data/carts#current"),
            2,
            "a Cart for each reader"
        );

        // Past Store's 30 seconds and inside Menu's five minutes.
        s.query_clock.0.advance(30_001);
        s.render_store("reader-a");
        assert_eq!(calls(&s, "store:data/stores#get"), 2, "Store expired");
        assert_eq!(calls(&s, "store:data/menus#for-store"), 1, "Menu did not");
    }

    /// **A command drops exactly the entry it invalidates** (ADR-0127): the
    /// session's cart, and not another session's. Cart is kept for no time in
    /// the store, so the test keeps it for a minute to see what is dropped.
    #[test]
    fn a_command_drops_the_entry_it_invalidates_and_no_other() {
        let mut s = rendering_server();
        for b in s.plan["bindings"].as_array_mut().expect("bindings") {
            if b["binding"] == "cart" {
                b["policy"]["freshness_ms"] = serde_json::json!(60_000);
            }
        }
        s.render_store("one");
        s.render_store("two");
        assert_eq!(calls(&s, "store:data/carts#current"), 2);
        s.render_store("one");
        assert_eq!(calls(&s, "store:data/carts#current"), 2, "kept");

        s.command(ADD, "one", &add("espresso", 1), false)
            .expect("runs");
        assert!(
            s.render_store("one").contains(">1<"),
            "one's own add is shown"
        );
        assert_eq!(
            calls(&s, "store:data/carts#current"),
            3,
            "one's entry was dropped"
        );
        s.render_store("two");
        assert_eq!(calls(&s, "store:data/carts#current"), 3, "two's was not");
    }

    /// **Commands that invalidate each other's reads all commit** (ADR-0127).
    /// Each commit drops the session's cart entry while another command may
    /// be re-reading it; an invalidated read is read again rather than failed.
    /// Two presses at once failed both before this.
    #[test]
    fn concurrent_commands_on_one_session_all_commit() {
        let s = std::sync::Arc::new(rendering_server());
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let s = s.clone();
                std::thread::spawn(move || {
                    for _ in 0..5 {
                        s.command(ADD, "busy", &add("espresso", 1), false)
                            .expect("commits");
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().expect("no command failed");
        }
        assert_eq!(s.cart_value("busy"), 40);
        assert_eq!(s.part_text("busy", "cart.line_count").as_deref(), Ok("40"));
    }

    /// **A change to the menu drops the Menu query's values** (ADR-0127).
    #[test]
    fn a_menu_change_drops_the_menus_kept_values() {
        let s = rendering_server();
        s.render_store("reader");
        s.broadcast_menu(MenuOp::Rename {
            id: "espresso".into(),
            name: "Ristretto".into(),
        })
        .expect("renamed");
        let html = s.render_store("reader");
        assert!(
            html.contains("Ristretto"),
            "the page shows the changed menu: {html}"
        );
        assert_eq!(calls(&s, "store:data/menus#for-store"), 2);
    }

    /// **A private entry is keyed by its session; a shared one by its
    /// arguments alone** (ADR-0127).
    #[test]
    fn an_entry_key_is_scoped_by_session_only_when_private() {
        let shared = serde_json::json!({ "cache": "shared", "privacy": "public", "key": [0] });
        let private = serde_json::json!({ "cache": "private", "privacy": "session", "key": [0] });
        let args = [Val::String("47".into())];
        assert_eq!(
            entry_key("a", &shared, &args),
            entry_key("b", &shared, &args)
        );
        assert_ne!(
            entry_key("a", &private, &args),
            entry_key("b", &private, &args)
        );
        let none = serde_json::json!({ "cache": "shared", "privacy": "public", "key": [1] });
        assert_eq!(
            entry_key("a", &none, &args),
            None,
            "a key naming an argument not given"
        );
    }

    /// **An idempotent command runs once per interaction** (ADR-0121).
    /// `add_to_cart` declares `idempotent_by InteractionId`, and until
    /// 2026-10-02 nothing read it: a retried request added twice.
    #[test]
    fn a_retried_interaction_runs_its_command_once() {
        let s = rendering_server();
        let espresso = [serde_json::json!("espresso"), serde_json::json!(1)];
        for _ in 0..3 {
            s.command_json(ADD, "session-r", &espresso, Some("press-a"))
                .expect("well-formed")
                .expect("committed, or the first commit's outcome");
        }
        assert_eq!(s.cart_value("session-r"), 1, "three sends, one interaction");

        // Another interaction is another add, and another session's same
        // interaction id is its own.
        s.command_json(ADD, "session-r", &espresso, Some("press-b"))
            .expect("well-formed")
            .expect("committed");
        assert_eq!(s.cart_value("session-r"), 2);
        s.command_json(ADD, "session-q", &espresso, Some("press-a"))
            .expect("well-formed")
            .expect("committed");
        assert_eq!(s.cart_value("session-q"), 1);

        // Refused before anything runs: no interaction, a malformed one, and
        // one reused with other arguments.
        let err = s
            .command_json(ADD, "session-r", &espresso, None)
            .expect_err("an idempotent command needs its key");
        assert!(err.contains("carries no interaction id"), "{err}");
        let err = s
            .command_json(ADD, "session-r", &espresso, Some("a b"))
            .expect_err("malformed");
        assert!(err.contains("is not an interaction id"), "{err}");
        let other = [serde_json::json!("cortado"), serde_json::json!(1)];
        let err = s
            .command_json(ADD, "session-r", &other, Some("press-a"))
            .expect_err("the same interaction is the same request");
        assert!(err.contains("first sent with other arguments"), "{err}");
        assert_eq!(s.cart_value("session-r"), 2, "no refusal moved the state");
    }

    /// **What an idempotent command keeps is bounded** (ADR-0121, and E10
    /// gate item 3's property): per session, the last
    /// `INTERACTIONS_PER_SESSION` interactions.
    #[test]
    fn interactions_kept_are_bounded_per_session() {
        let s = rendering_server();
        let espresso = [serde_json::json!("espresso"), serde_json::json!(1)];
        let presses = INTERACTIONS_PER_SESSION + 40;
        for i in 0..presses {
            s.command_json(ADD, "session-b", &espresso, Some(&format!("p{i}")))
                .expect("well-formed")
                .expect("committed");
        }
        assert_eq!(s.cart_value("session-b"), presses as i64);
        assert_eq!(s.commands.commands_held(), INTERACTIONS_PER_SESSION);
        // The newest is still recognised; the oldest is the bound's cost.
        s.command_json(
            ADD,
            "session-b",
            &espresso,
            Some(&format!("p{}", presses - 1)),
        )
        .expect("well-formed")
        .expect("its first outcome");
        assert_eq!(s.cart_value("session-b"), presses as i64);
    }

    #[test]
    fn every_command_commits_its_state_and_its_event_together() {
        let s = rendering_server();
        s.command(ADD, "session-6", &add("espresso", 3), false)
            .expect("runs");
        assert_eq!(s.materializer.state("cart:session-6").as_deref(), Some("3"));
        s.command_json(CLEAR, "session-6", &[], Some("clear-1"))
            .expect("well-formed")
            .expect("and it committed");
        assert_eq!(s.cart_value("session-6"), 0);
        assert_eq!(
            s.materializer.state("cart:session-6").as_deref(),
            Some("0"),
            "the cleared cart's state, not the old total"
        );
    }

    /// **A command commits the events it declares, and no others**
    /// (ADR-0104). `add_to_cart` declares `emits
    /// CartChanged(current_session())`, so its cart's entry moves. Declaring
    /// none, the write commits and nothing is told: until 2026-09-26 this
    /// server committed `CartChanged` whatever the command declared.
    #[test]
    fn a_command_commits_the_events_it_declares() {
        let s = rendering_server();
        assert_eq!(
            declared_events(&s.graph, ADD, "session-7").expect("computable"),
            [pw_materialize::Event::new(
                &["Events", ".CartChanged"].concat(),
                &["session-7"]
            )],
            "exactly the one it declares"
        );
        s.drain("session-7");
        let before = s.version("session-7");
        s.command(ADD, "session-7", &add("espresso", 1), false)
            .expect("runs");
        assert_ne!(s.version("session-7"), before, "the declared event");

        let mut s = rendering_server();
        s.graph
            .edges
            .retain(|e| !(e.from == ADD && e.kind == pw_materialize::EdgeKind::Emits));
        s.drain("session-7");
        let before = s.version("session-7");
        s.command(ADD, "session-7", &add("espresso", 1), false)
            .expect("runs");
        assert_eq!(s.cart_value("session-7"), 1, "the write committed");
        assert_eq!(
            s.version("session-7"),
            before,
            "and no event reached the cart"
        );
    }

    /// **A value the server cannot compute is refused before the command
    /// runs** (ADR-0104), and nothing is written.
    #[test]
    fn an_event_value_the_server_cannot_compute_is_refused() {
        let mut s = rendering_server();
        for e in s
            .graph
            .edges
            .iter_mut()
            .filter(|e| e.from == ADD && e.kind == pw_materialize::EdgeKind::Emits)
        {
            e.key = vec!["item".to_string()];
        }
        let err = s
            .command(ADD, "session-8", &add("espresso", 1), false)
            .expect_err("`item` is not computed here");
        assert!(
            err.contains("add_to_cart") && err.contains("`item`"),
            "{err}"
        );
        assert_eq!(s.cart_value("session-8"), 0, "nothing was written");
    }

    /// **The server will not start without the compiler's handler modules.**
    /// It reads the identities from the template IR and looks for each module
    /// where `pw emit-handlers` writes it.
    #[test]
    fn a_document_whose_handlers_were_not_compiled_is_refused() {
        let s = rendering_server();
        let named = s.handler_identities();
        assert_eq!(named.len(), 2, "add_to_cart and clear_cart: {named:?}");
        assert_eq!(
            s.uncompiled_handlers(),
            named.iter().cloned().collect::<Vec<_>>(),
            "`.` has no handlers directory, so every one is missing"
        );

        let dist = std::env::temp_dir().join(format!("pw-dev-handlers-{}", std::process::id()));
        std::fs::create_dir_all(dist.join("handlers")).expect("a scratch dist");
        for id in &named {
            std::fs::write(dist.join("handlers").join(format!("{id}.mjs")), "").expect("a module");
        }
        let templates: Vec<Template> =
            serde_json::from_str(include_str!("../../store-ir.json")).expect("the template IR");
        let built = Server::on(dist.clone(), templates, dev_topology());
        assert!(built.uncompiled_handlers().is_empty());
        let _ = std::fs::remove_dir_all(&dist);
    }

    /// Each command is authorised on its own contract.
    ///
    /// Asking once for the whole program would make one command's authority
    /// every command's, which is the aggregation `contracts()` emits one
    /// contract per declaration to avoid.
    #[test]
    fn clear_cart_is_authorised_separately() {
        // The authorisation only. Running `clear_cart` to completion
        // regenerates the fragment it invalidates, which needs the compiled
        // template IR — and what this test is about is which contract decides,
        // not what the renderer does afterwards.
        let s = server(dev_topology());
        assert!(s.authorise("store.page.clear_cart").is_ok());

        // The refusal reaches the caller before any state moves, which is why
        // the barren case CAN run the whole command.
        let barren = server(barren());
        let err = barren
            .command(CLEAR, "session-1", &[], false)
            .expect_err("a barren node cannot host a write");
        assert!(err.contains("clear_cart"), "{err}");
        assert_eq!(barren.cart_value("session-1"), 0);
    }

    /// Everything the server holds per subscriber, summed: frames not yet
    /// acknowledged, and subscribers.
    fn held(s: &Server) -> (usize, usize) {
        let queue = s.pending.lock().unwrap();
        (queue.values().map(|w| w.frames.len()).sum(), queue.len())
    }

    /// **Under sustained load, what the server holds stays bounded** (E10
    /// gate item 3). Each number below was measured before the bound existed,
    /// and is in `docs/evidence/E10/load.txt`: 3,000 outbox rows after 3,000
    /// commands, and 600,000 frames for 1,000 departed visitors.
    #[test]
    fn sustained_load_leaves_the_server_bounded() {
        // One session with a subscriber that applies every batch.
        let s = rendering_server();
        s.serve_document("steady");
        for _ in 0..3_000 {
            s.command(ADD, "steady", &add("espresso", 1), false)
                .expect("runs");
            let mut queue = s.pending.lock().unwrap();
            let doc = latest(&queue, "steady");
            let w = queue.get_mut(&doc).unwrap();
            let last = w.last_seq;
            w.acknowledge(last);
        }
        let (frames, subscribers) = held(&s);
        let rows = s.materializer.retained_events();
        println!(
            "steady: 3000 commands -> frames {frames}, subscribers {subscribers}, outbox rows {rows}"
        );
        assert_eq!((frames, subscribers, rows), (0, 1, 0));
        assert_eq!(s.cart_value("steady"), 3_000, "and every command committed");

        // Visitors who load the page once and leave, then changes to the menu.
        let s = rendering_server();
        for i in 0..1_000 {
            s.serve_document(&format!("visitor-{i}"));
        }
        for i in 0..300 {
            let name = if i % 2 == 0 { "Gibraltar" } else { "Cortado" };
            s.broadcast_menu(MenuOp::Rename {
                id: "cortado".into(),
                name: name.into(),
            })
            .expect("renames");
        }
        let (frames, subscribers) = held(&s);
        println!(
            "churn: 1000 visitors, 300 menu changes -> frames {frames}, subscribers {subscribers}, \
             materialized entries {}",
            s.materializer.entry_count()
        );
        assert_eq!(subscribers, 1_000, "none idle yet");
        assert_eq!(frames, 1_000, "one reload each: every visitor fell behind");
        for w in s.pending.lock().unwrap().values() {
            assert!(w.behind);
            assert!(matches!(
                w.frames.as_slice(),
                [(
                    _,
                    StreamFrame::Recovery {
                        recovery: pw_protocol::Recovery::Reload,
                        ..
                    }
                )]
            ));
        }

        // Past the idle limit, the next change forgets them all.
        let long_ago = std::time::Instant::now() - IDLE - std::time::Duration::from_secs(1);
        for w in s.pending.lock().unwrap().values_mut() {
            w.seen = long_ago;
        }
        s.broadcast_menu(MenuOp::Rename {
            id: "cortado".into(),
            name: "Cortado".into(),
        })
        .expect("renames");
        let (frames, subscribers) = held(&s);
        let entries = s.materializer.entry_count();
        println!(
            "idle: after {}s -> frames {frames}, subscribers {subscribers}, materialized entries {entries}",
            IDLE.as_secs()
        );
        assert_eq!((frames, subscribers), (0, 0));
        assert_eq!(entries, 1, "the shared menu, and no visitor's cart");

        // A visitor who comes back is served a whole document: their cart
        // entry is regenerated from state, not lost.
        s.command(ADD, "visitor-7", &add("espresso", 2), false)
            .expect("runs");
        let (html, cursor) = s.serve_document("visitor-7");
        assert!(cursor > 0);
        assert!(html.contains(">2<"), "the returning visitor's cart: {html}");
    }

    /// **A queue that reaches its bound becomes one reload, and stays one.**
    #[test]
    fn a_subscriber_that_falls_behind_is_told_to_reload() {
        let mut w = Subscriber::default();
        let notice = |n: u64| StreamFrame::ResourceChanged {
            protocol: CURRENT,
            entry: cart_entry(&format!("session-{n}")),
            version: Version(n),
        };
        for n in 0..MAX_WAITING as u64 {
            w.push(notice(n));
        }
        assert_eq!(w.frames.len(), MAX_WAITING, "at the bound, not past it");
        assert!(!w.behind);
        w.push(notice(999));
        w.push(notice(1000));
        assert!(w.behind);
        assert_eq!(w.frames.len(), 1);
        let (cursor, frames) = w.after(0);
        assert!(matches!(
            frames.as_slice(),
            [StreamFrame::Recovery {
                recovery: pw_protocol::Recovery::Reload,
                ..
            }]
        ));
        assert_eq!(
            cursor,
            MAX_WAITING as u64 + 1,
            "the reload took the next sequence"
        );
    }

    /// **A page the server has forgotten is told to reload**, through the
    /// real route: a poll with a cursor for a session with no subscriber.
    #[test]
    fn a_forgotten_page_that_polls_is_told_to_reload() {
        let s = server(dev_topology());
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let at = listener.local_addr().expect("addr");
        let poll = |since: u64| -> String {
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    let (stream, _) = listener.accept().expect("accept");
                    handle(&s, stream);
                });
                let mut c = TcpStream::connect(at).expect("connect");
                write!(
                    c,
                    "GET /stream?since={since} HTTP/1.1\r\ncookie: pw-session=gone\r\n\r\n"
                )
                .expect("request");
                let mut out = String::new();
                c.read_to_string(&mut out).expect("response");
                out
            })
        };
        let reply = poll(7);
        println!("{}", reply.lines().last().unwrap_or_default());
        assert!(reply.contains(r#""recovery":"reload""#), "{reply}");

        // The control: a page that has never been served a document, cursor
        // zero, is subscribed rather than told to reload.
        let reply = poll(0);
        assert!(!reply.contains("recovery"), "{reply}");
        assert!(
            s.pending
                .lock()
                .unwrap()
                .contains_key(&("gone".to_string(), 0))
        );
    }

    #[test]
    fn store_commands_carry_and_enforce_their_signed_in_precondition() {
        let mut s = server(dev_topology());
        for id in [ADD, CLEAR] {
            let contract = s
                .contracts
                .iter()
                .find(|c| c.component_id == id)
                .expect("command contract");
            let requirements = &contract.exports[0]
                .component
                .as_ref()
                .expect("compiled export")
                .authorization;
            assert_eq!(requirements.len(), 1, "{id}: {requirements:?}");
            assert_eq!(requirements[0].predicate, "SignedIn");
            assert!(requirements[0].arguments.is_empty());
        }

        // The development deployment knows SignedIn, so the ordinary demo
        // principal may execute it.
        s.command(ADD, "signed-in-demo", &add("espresso", 1), false)
            .expect("SignedIn is explicitly approved");

        // A predicate the deployment does not know is denied before the
        // component or staged data layer can change state.
        let requirement = &mut s
            .contracts
            .iter_mut()
            .find(|c| c.component_id == ADD)
            .expect("command contract")
            .exports[0]
            .component
            .as_mut()
            .expect("compiled export")
            .authorization[0];
        requirement.predicate = "OwnsOrder".to_string();
        let before = s.cart_value("signed-in-demo");
        let err = s
            .command(ADD, "signed-in-demo", &add("cortado", 1), false)
            .expect_err("unknown authorization must fail closed");
        assert!(err.contains("no authorization predicate"), "{err}");
        assert_eq!(s.cart_value("signed-in-demo"), before);
    }

    /// A component the build has no contract for is refused, not run.
    ///
    /// "I have no record of this" and "this is fine" must never be the same
    /// answer — the same fail-closed direction as the artifact audit's
    /// unparseable import.
    #[test]
    fn an_unknown_component_is_refused_rather_than_defaulted() {
        let s = server(dev_topology());
        let err = s
            .authorise("store.page.nothing_declares_this")
            .expect_err("no contract");
        assert!(err.contains("no contract"), "{err}");

        // The control: a component that IS in the contracts is authorised, so
        // the refusal above is about the missing record and not about the
        // lookup being broken.
        assert!(s.authorise("store.page.add_to_cart").is_ok());
    }

    /// **A frame is forgotten when the page says it applied it** (ADR-0139).
    ///
    /// A page's stream can be held on the server after its connection is
    /// gone, by a reload or a network that dropped. Until 2026-10-02 the
    /// stream dropped each frame 25 ms after writing it, so a change written
    /// to a connection nobody read never reached the page's next request: the
    /// keyed-list suite's intermittent failure, found building ADR-0138.
    /// Since ADR-0161 a page that replaces another is another document, so
    /// this is the same document asking again.
    #[test]
    fn a_frame_a_stream_wrote_to_a_dropped_connection_still_reaches_its_page() {
        let s = rendering_server();
        let session = "dropped";
        let (_, document) = s.serve_document(session);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let mut gone = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
        let (mut held, _) = listener.accept().expect("accept");
        std::thread::scope(|scope| {
            scope.spawn(|| stream_open(&s, &mut held, session, false, document, document));
            s.command(ADD, session, &add("espresso", 1), false)
                .expect("runs");
            std::thread::sleep(std::time::Duration::from_millis(300));

            // The stream wrote the change to the connection ...
            gone.set_read_timeout(Some(std::time::Duration::from_millis(500)))
                .expect("timeout");
            let mut written = vec![0; 1 << 16];
            let n = gone.read(&mut written).unwrap_or(0);
            let written = String::from_utf8_lossy(&written[..n]).to_string();
            assert!(written.contains("\"frames\""), "{written}");

            // ... which the page never read: its next request, from where it
            // was, is still sent it.
            let mut queue = s.pending.lock().expect("pending");
            let waiting = queue
                .get_mut(&(session.to_string(), document))
                .expect("its subscriber");
            let (cursor, frames) = waiting.after(document);
            assert!(
                !frames.is_empty(),
                "the change was written to a connection that dropped, and dropped"
            );
            // Control: what a page says it applied is dropped.
            waiting.acknowledge(cursor);
            assert!(waiting.after(document).1.is_empty());
        });
    }

    /// **A path gives a route its parameters** (ADR-0162): one segment each,
    /// as the compiler's link check matches a link, decoded as a path is.
    #[test]
    fn a_path_gives_a_route_its_parameters() {
        let id = |path: &str| route_params("/stores/{id}", path).map(|p| p["id"].clone());
        assert_eq!(id("/stores/48").as_deref(), Some("48"));
        assert_eq!(id("/stores/48/").as_deref(), Some("48"));
        assert_eq!(id("/stores/flat%20white").as_deref(), Some("flat white"));
        // A `+` in a path is itself; a form's query makes it a space.
        assert_eq!(id("/stores/a+b").as_deref(), Some("a+b"));
        // Not the route: another word, another length, or no segment.
        for other in ["/shops/48", "/stores", "/stores/48/menu", "/stores//", "/"] {
            assert_eq!(id(other), None, "{other}");
        }
        // Not text: refused rather than passed through.
        assert_eq!(id("/stores/%zz"), None);
        assert_eq!(id("/stores/%FF"), None);
        // A route of words alone, and the root.
        assert_eq!(route_params("/about", "/about"), Some(Params::new()));
        assert_eq!(route_params("/", "/"), Some(Params::new()));
    }

    /// **The store at its route, and a second store at its own** (ADR-0162,
    /// charter §15.3): `/stores/{id}` names the store page, and `48` is a
    /// store with its own name and menu.
    #[test]
    fn each_store_is_served_at_its_route() {
        let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
        let (page, params) = s.routed("/stores/48").expect("the store's route");
        assert_eq!(page, "store.page.StorePage");
        assert_eq!(params, store_params("48"));
        assert_eq!(s.routed("/stores"), None);
        let (harbor, _) = s.serve_store_document("a", "48").expect("served");
        let harbor = visible(&harbor);
        assert!(harbor.contains("Harbor Coffee"), "{harbor}");
        assert!(harbor.contains("Matcha Latte"), "{harbor}");
        assert!(!harbor.contains("Blue Bottle"), "{harbor}");
        assert!(!harbor.contains("Espresso"), "{harbor}");
        // Store 47 at its route is the page `/StorePage.html` serves.
        let (blue, _) = s.serve_store_document("b", STORE_ID).expect("served");
        assert!(visible(&blue).contains("Blue Bottle"), "{blue}");
        // And store 48 again, after 47: each store's menu is its own
        // fragment, not the one rendered last.
        let (again, _) = s.serve_store_document("d", "48").expect("served");
        let again = visible(&again);
        assert!(again.contains("Matcha Latte"), "{again}");
        assert!(!again.contains("Espresso"), "{again}");
        // A store this server does not hold is not found, as the page
        // declares (ADR-0163).
        let absent = s
            .serve_store_document("c", "999")
            .expect_err("no store 999");
        assert!(matches!(absent, Unread::NotFound(_)), "{absent:?}");
    }

    /// **A store that is not there is not found** (ADR-0163): the page
    /// declares `not_found_on StoreError.NotFound`, and a store the data
    /// layer does not hold is answered 404, a page that is nothing of the
    /// store's, where any other failure is answered 503 (ADR-0147).
    #[test]
    fn a_store_that_is_not_there_is_not_found() {
        let fetched = |s: &Server, path: &str, session: &str| -> String {
            fetched_as(s, path, Some(session))
                .into_iter()
                .map(|(_, c)| c)
                .collect()
        };
        let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
        let absent = fetched(&s, "/stores/999", "a");
        // With its reason, as every response has it.
        assert!(absent.starts_with("HTTP/1.1 404 Not Found\r\n"), "{absent}");
        let page = visible(&absent);
        assert!(page.contains("Not found"), "{page}");
        for of_the_store in ["Blue Bottle", "Harbor Coffee", "Espresso", "Add"] {
            assert!(!page.contains(of_the_store), "{page}");
        }
        // And it is no document the server holds.
        assert!(
            !s.pending
                .lock()
                .expect("pending")
                .keys()
                .any(|(session, _)| session == "a"),
            "a page that was not found waits for changes"
        );
        // Control: a store the server holds is served.
        assert!(fetched(&s, "/stores/48", "b").starts_with("HTTP/1.1 200 OK\r\n"));
        // Control: the clause names the case that means absent. Naming
        // another, the store's absence is a failure like any other.
        let other = served_from_patches_in(
            "examples",
            |app| {
                app.replace(
                    "not_found_on StoreError.NotFound",
                    "not_found_on StoreError.Unavailable",
                )
            },
            &[],
        );
        let failed = fetched(&other, "/stores/999", "a");
        assert!(
            failed.starts_with("HTTP/1.1 503 Service Unavailable\r\n"),
            "{failed}"
        );
        // Control: a page that names none is answered 503 for it.
        let none = served_from_patches_in(
            "examples",
            |app| app.replace("    not_found_on StoreError.NotFound\n", ""),
            &[],
        );
        assert!(!none.plan.to_string().contains("not_found"));
        let failed = fetched(&none, "/stores/999", "a");
        assert!(
            failed.starts_with("HTTP/1.1 503 Service Unavailable\r\n"),
            "{failed}"
        );
    }

    /// **A change to one store's menu reaches that store's pages only**
    /// (§15.6 test 11, ADR-0162), and a change to the session's cart reaches
    /// both its stores' pages.
    #[test]
    fn a_change_to_one_stores_menu_reaches_that_stores_pages_only() {
        let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
        let (_, blue) = s.serve_store_document("a", STORE_ID).expect("served");
        let (_, harbor) = s.serve_store_document("a", "48").expect("served");
        let doc = |d: u64| ("a".to_string(), d);
        s.broadcast_menu(MenuOp::Rename {
            id: "espresso".to_string(),
            name: "Espresso Doppio".to_string(),
        })
        .expect("the menu changes");
        let patched = |d: u64| {
            s.pending.lock().expect("pending")[&doc(d)]
                .frames
                .iter()
                .any(|(_, f)| matches!(f, StreamFrame::Patch(_)))
        };
        assert!(
            patched(blue),
            "store 47's page was not told its menu changed"
        );
        assert!(!patched(harbor), "store 48's page was told of 47's menu");
        // The cart is the session's: an add reaches both.
        s.command(ADD, "a", &add("drip", 1), false).expect("runs");
        let sets = |d: u64| {
            s.pending.lock().expect("pending")[&doc(d)]
                .frames
                .iter()
                .filter(|(_, f)| matches!(f, StreamFrame::PatchSet(set) if !set.patches.is_empty()))
                .count()
        };
        assert_eq!((sets(blue), sets(harbor)), (1, 1));
    }

    /// **A change to store 47's menu drops store 47's kept menu, and only
    /// it** (§15.6 test 11, ADR-0164). The change is `MenuChanged(47)`, and
    /// it reaches the entries whose queries declare `invalidates_on
    /// MenuChanged(id)`, for that id. Until 2026-10-03 the server dropped
    /// every store's kept menu, by the query's name.
    #[test]
    fn a_menu_change_drops_that_stores_kept_menu_only() {
        let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
        let reads = || calls(&s, "store:data/menus#for-store");
        s.serve_store_document("a", STORE_ID).expect("served");
        s.serve_store_document("a", "48").expect("served");
        let read = reads();
        // Control: each is kept, so serving them again reads neither.
        s.serve_store_document("b", STORE_ID).expect("served");
        s.serve_store_document("b", "48").expect("served");
        assert_eq!(reads(), read);
        s.broadcast_menu(MenuOp::Rename {
            id: "espresso".to_string(),
            name: "Espresso Doppio".to_string(),
        })
        .expect("the menu changes");
        // Store 48's menu is still kept ...
        s.serve_store_document("c", "48").expect("served");
        assert_eq!(reads(), read, "store 47's change dropped store 48's menu");
        // ... and store 47's is read again, with the change.
        let (blue, _) = s.serve_store_document("c", STORE_ID).expect("served");
        assert_eq!(reads(), read + 1, "store 47's menu was not read again");
        assert!(visible(&blue).contains("Espresso Doppio"), "{blue}");
    }

    /// **A document is its own subscriber** (ADR-0161). Two tabs in one
    /// session: the first is served, a change reaches it, and before its page
    /// asks for it the second is served. The first page is still sent the
    /// change.
    #[test]
    fn a_change_waiting_for_one_tab_survives_another_tab_being_served() {
        let s = rendering_server();
        let session = "two-tabs";
        let (_, first) = s.serve_document(session);
        s.command(ADD, session, &add("espresso", 1), false)
            .expect("runs");
        let (_, _second) = s.serve_document(session);
        let queue = s.pending.lock().expect("pending");
        let (_, frames) = queue[&(session.to_string(), first)].after(first);
        assert!(!frames.is_empty(), "the first tab was never sent the add");
    }

    /// A change to the session's cart reaches every document of the session
    /// (ADR-0161), each patched against what it shows.
    #[test]
    fn a_change_reaches_every_document_of_its_session() {
        let s = rendering_server();
        let session = "both-tabs";
        let (_, first) = s.serve_document(session);
        let (_, second) = s.serve_document(session);
        s.command(ADD, session, &add("espresso", 1), false)
            .expect("runs");
        let queue = s.pending.lock().expect("pending");
        for document in [first, second] {
            let frames = &queue[&(session.to_string(), document)].frames;
            assert!(
                frames.iter().any(
                    |(_, f)| matches!(f, StreamFrame::PatchSet(set) if !set.patches.is_empty())
                ),
                "document {document} was not patched: {frames:?}"
            );
        }
        // Control: another session's page hears nothing.
        drop(queue);
        let (_, other) = s.serve_document("elsewhere");
        s.command(ADD, session, &add("espresso", 1), false)
            .expect("runs");
        assert!(
            s.pending.lock().expect("pending")[&("elsewhere".to_string(), other)]
                .frames
                .is_empty()
        );
    }

    /// **A session is forgotten with its last document** (ADR-0161): its cart
    /// entry and its interactions stay while another of its pages is open.
    #[test]
    fn a_session_is_forgotten_with_its_last_document() {
        let now = std::time::Instant::now();
        let idle = Subscriber {
            seen: now - IDLE - std::time::Duration::from_secs(1),
            ..Subscriber::at(1)
        };
        let mut queue: BTreeMap<Doc, Subscriber> = BTreeMap::new();
        queue.insert(("s".to_string(), 1), idle);
        queue.insert(("s".to_string(), 2), Subscriber::at(2));
        let (forgotten, gone) = forget_idle(&mut queue, now);
        assert_eq!(forgotten, [("s".to_string(), 1)]);
        assert!(gone.is_empty(), "{gone:?}");
        // Its last page idle too, the session goes with it.
        let later = now + IDLE + std::time::Duration::from_secs(1);
        let (forgotten, gone) = forget_idle(&mut queue, later);
        assert_eq!(forgotten, [("s".to_string(), 2)]);
        assert_eq!(gone, ["s"]);
    }

    /// The other half (ADR-0139): what a page's stream request says it
    /// applied is dropped, so a live page's queue does not grow.
    #[test]
    fn a_stream_drops_what_its_page_says_it_applied() {
        let s = rendering_server();
        let session = "applied";
        let (_, document) = s.serve_document(session);
        let doc: Doc = (session.to_string(), document);
        s.command(ADD, session, &add("espresso", 1), false)
            .expect("runs");
        let applied = s.pending.lock().expect("pending")[&doc].last_seq;
        assert!(
            s.pending.lock().expect("pending")[&doc]
                .frames
                .iter()
                .any(|(n, _)| *n <= applied),
            "the change queued frames"
        );
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let _page = TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
        let (mut held, _) = listener.accept().expect("accept");
        std::thread::scope(|scope| {
            scope.spawn(|| stream_open(&s, &mut held, session, false, document, applied));
            std::thread::sleep(std::time::Duration::from_millis(100));
            let queue = s.pending.lock().expect("pending");
            assert!(
                !queue[&doc].frames.iter().any(|(n, _)| *n <= applied),
                "what the page applied is still queued"
            );
        });
    }

    /// The parts manifest a document carries.
    fn manifest_of(html: &str) -> serde_json::Value {
        let json = html
            .split("<script type=\"application/json\" id=\"pw-parts\">")
            .nth(1)
            .and_then(|r| r.split("</script>").next())
            .expect("a manifest");
        serde_json::from_str(json).expect("JSON")
    }

    /// **A page that reads queries holds signals too** (ADR-0140). The
    /// store's document said nothing of signals until 2026-10-02, so a
    /// signal on the store's page rendered nowhere the browser could change
    /// it: T11's dialog could not be written on the store.
    #[test]
    fn the_store_s_document_carries_its_signals() {
        let template: Template = serde_json::from_value(serde_json::json!({
            "path": "store.page.StorePage",
            "name": "StorePage",
            "params": [],
            "schema": "s",
            "chunks": [
                { "chunk": "static", "value": "<main>" },
                { "chunk": "dynamic", "value": {
                    "part": "conditional", "id": 0, "value": "open",
                    "then": [{ "chunk": "static", "value": "<p id=\"shown\">open</p>" }],
                    "otherwise": []
                } },
                { "chunk": "static", "value": "</main>" }
            ]
        }))
        .expect("a template");
        let plan = serde_json::json!({
            "signals": [{ "name": "open", "initial": true }],
            "live": [{ "part": 0, "signal": "open", "path": "open", "kind": "conditional",
                       "reads": ["open"] }]
        });
        // Rendered at its first value ...
        let body =
            pw_render::render(&template, &with_signals(Env::new(), &plan), &[]).expect("renders");
        assert!(body.contains("<p id=\"shown\">open</p>"), "{body}");
        // ... and the document says what the browser holds and renders again.
        let templates = vec![template];
        let manifest = manifest_of(&document(&body, &templates, &plan, 3, None));
        assert_eq!(manifest["signals"], serde_json::json!({ "open": true }));
        assert_eq!(manifest["live"][0]["part"], 0);
        assert!(manifest["blocks"]["0"]["then"].is_array(), "{manifest}");
        // Control: a page with no signals carries none of it.
        let manifest = manifest_of(&document(
            &body,
            &templates,
            &serde_json::json!({}),
            3,
            None,
        ));
        assert!(manifest.get("signals").is_none(), "{manifest}");
    }

    /// The store's page renders a block its signal decides at the signal's
    /// first value (ADR-0140): until 2026-10-02 nothing gave the renderer a
    /// signal's value on the store's route, and the page did not render.
    #[test]
    fn the_store_renders_its_signals_at_their_first_values() {
        let mut templates: Vec<Template> =
            serde_json::from_str(include_str!("../../store-ir.json")).expect("the template IR");
        templates[0].chunks.push(
            serde_json::from_value(serde_json::json!({ "chunk": "dynamic", "value": {
                "part": "conditional", "id": 99, "value": "open",
                "then": [{ "chunk": "static", "value": "<p id=\"shown\">open</p>" }],
                "otherwise": []
            } }))
            .expect("a chunk"),
        );
        let mut s = Server::on(std::path::PathBuf::from("."), templates, dev_topology());
        // The store's own signals, and one more the added part reads.
        let with_open = |s: &mut Server, initial: bool| {
            let mut signals = s.plan["signals"].as_array().cloned().unwrap_or_default();
            signals.retain(|signal| signal["name"] != "open");
            signals.push(serde_json::json!({ "name": "open", "initial": initial }));
            s.plan["signals"] = serde_json::Value::Array(signals);
        };
        with_open(&mut s, true);
        let html = s.render_store("first-shown");
        assert!(html.contains("<p id=\"shown\">open</p>"), "{html}");
        // Control: shut at its first value, and not shown.
        with_open(&mut s, false);
        assert!(!s.render_store("first-shut").contains("id=\"shown\""));
    }

    /// **A kept answer is the answer** (ADR-0121, ADR-0157): a retried
    /// request is given exactly what the first was, including the
    /// difference between answering nothing, `null`, and answering no value.
    #[test]
    fn a_kept_answer_is_the_answer_it_kept() {
        for answered in [
            Answered {
                committed: true,
                result: Some(serde_json::json!({ "$case": "ok" })),
                why: None,
            },
            Answered {
                committed: false,
                result: Some(serde_json::json!({
                    "$case": "err",
                    "value": { "$case": "item-unavailable", "value": "cortado" }
                })),
                why: None,
            },
            // A command declaring no `Result`: answered, with nothing.
            Answered {
                committed: true,
                result: Some(serde_json::Value::Null),
                why: None,
            },
            // A command that trapped: no answer at all.
            Answered {
                committed: false,
                result: None,
                why: Some("trapped".to_string()),
            },
        ] {
            assert_eq!(Answered::from_kept(&answered.kept()), answered);
        }
    }

    /// A page's parameter, as the address carries it (ADR-0136).
    #[test]
    fn a_parameter_is_the_text_the_address_carries() {
        assert_eq!(percent_decoded("cold-brew").as_deref(), Some("cold-brew"));
        assert_eq!(
            percent_decoded("flat%20white").as_deref(),
            Some("flat white")
        );
        assert_eq!(percent_decoded("flat+white").as_deref(), Some("flat white"));
        assert_eq!(percent_decoded("%E2%82%AC").as_deref(), Some("\u{20ac}"));
        assert_eq!(percent_decoded("%3Cb%3E").as_deref(), Some("<b>"));
        // Not an escape, and not text: refused rather than passed through.
        assert_eq!(percent_decoded("%zz"), None);
        assert_eq!(percent_decoded("%4"), None);
        assert_eq!(percent_decoded("%FF"), None);
    }
}
