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
use pw_protocol::{CURRENT, CausalBasis, Patch, PatchOp, ResourceEntryId, StreamFrame, Version};
use pw_render::{Env, PartId, Template, Value};
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
}

impl Default for Subscriber {
    fn default() -> Subscriber {
        Subscriber {
            last_seq: 0,
            frames: Vec::new(),
            seen: std::time::Instant::now(),
            behind: false,
        }
    }
}

/// The most frames one subscriber may have waiting.
const MAX_WAITING: usize = 256;

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

/// Forget every subscriber that has not asked for [`IDLE`], and say which.
fn forget_idle(queue: &mut BTreeMap<String, Subscriber>, now: std::time::Instant) -> Vec<String> {
    let idle: Vec<String> = queue
        .iter()
        .filter(|(_, w)| now.saturating_duration_since(w.seen) >= IDLE)
        .map(|(session, _)| session.clone())
        .collect();
    for session in &idle {
        queue.remove(session);
    }
    idle
}

impl Subscriber {
    fn push(&mut self, frame: StreamFrame) {
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
    /// Frames waiting for each session's subscriber.
    pending: Mutex<BTreeMap<String, Subscriber>>,
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

/// The menu's entry identity — PUBLIC, and carrying the same generation a
/// session-scoped entry does. Partition and compatibility are orthogonal.
fn menu_identity() -> EntryIdentity {
    EntryIdentity::new("store.page.Menu", &["47"], pw_resource::Partition::Public).generation(BUILD)
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
                "database.read<Stores>",
                "database.write<Carts>",
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

            // What the command declares it emits (ADR-0104), computed before
            // it runs: a value this server cannot compute is refused, and a
            // refused command has written nothing.
            let events = declared_events(&self.graph, component_id, session)?;
            let out = self.run(component_id, session, &host, args)?;
            if let [Val::Result(Err(e))] = out.as_slice() {
                return Err(format!("{component_id} failed: {e:?}"));
            }
            let Some(lines) = staged.lock().expect("staged").take() else {
                // Nothing was written, so there is nothing to commit.
                return Ok(());
            };
            let total: i64 = lines.iter().map(|(_, q)| q).sum();
            let emitted = events.clone();
            self.materializer.command(|tx| {
                Materializer::set_state(tx, &format!("cart:{session}"), &total.to_string());
                Ok::<_, String>(events)
            })?;
            carts.insert(session.to_string(), lines);
            self.invalidate_queries(component_id, session, &emitted);
        }

        // The materializer drains the committed event and regenerates the
        // entry it invalidates. The version moves because the RESOURCE moved.
        self.drain(session);
        Ok(())
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
    fn command_json(
        &self,
        component_id: &str,
        session: &str,
        json: &[serde_json::Value],
        interaction: Option<&str>,
    ) -> Result<Result<(), String>, String> {
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
        let Some(key_type) = &export.idempotent_by else {
            return Ok(self.command(component_id, session, &args, false));
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
                match self.command(component_id, session, &args, false) {
                    Ok(()) => "committed".to_string(),
                    Err(e) => format!("failed:{e}"),
                }
            })
            .map_err(|e| format!("interaction `{id}`'s outcome is unknown: {e:?}"))?;
        Ok(match outcome.strip_prefix("failed:") {
            Some(e) => Err(e.to_string()),
            None => Ok(()),
        })
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
        let forgotten = {
            let mut queue = self.pending.lock().expect("pending");
            forget_idle(&mut queue, std::time::Instant::now())
        };
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
    /// This server holds one store; its menu is the keyed list E7-P mutates.
    fn catalog(&self) -> BTreeMap<String, HostFn> {
        let menu = self.menu.lock().expect("menu").clone();
        let not_found = || {
            vec![Val::Result(Err(Some(Box::new(Val::Variant(
                "not-found".into(),
                None,
            )))))]
        };
        let get: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] if id == STORE_ID => {
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::Record(vec![
                    ("id".into(), Val::String(STORE_ID.into())),
                    ("name".into(), Val::String(STORE_NAME.into())),
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
            [Val::String(id)] if id == STORE_ID => {
                Ok(vec![Val::Result(Ok(Some(Box::new(Val::List(
                    menu.iter()
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
        BTreeMap::from([
            ("store:data/stores#get".to_string(), get),
            ("store:data/menus#for-store".to_string(), for_store),
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
    ) -> Result<Val, String> {
        let resource = binding["resource"].as_str().unwrap_or_default();
        let policy = &binding["policy"];
        let manifest = runtime_manifest(resource, policy);
        let mut key = entry_key(session, policy, args)
            .ok_or_else(|| format!("`{resource}`'s key names an argument it was not given"))?;
        if policy["parallel"] == true {
            // `concurrency parallel`: no shared flight, so a key of its own.
            let n = self
                .parallel
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            key.push_str(&format!("\u{1f}#{n}"));
        }
        let key = pw_resource::Key::new(resource, &key);
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
                | pw_resource::Fetched::Deduplicated(v) => return Ok(Val::clone(&v)),
                pw_resource::Fetched::Failed(e) => return Err(format!("{resource}: {e}")),
                pw_resource::Fetched::Cancelled(r) => {
                    return Err(format!("{resource}: stopped, {r:?}"));
                }
                pw_resource::Fetched::TimedOut => return Err(format!("{resource}: timed out")),
            }
        }
        self.query(resource, session, args)
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
        self.queries.fetch(manifest, key, |_attempt| {
            self.query(resource, session, args).map(Arc::new)
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
    /// the session and the catalogue, and return its `Ok` value. Nothing a
    /// query does is committed: it reads.
    fn query(&self, component_id: &str, session: &str, args: &[Val]) -> Result<Val, String> {
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
        host.extend(self.catalog());
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
        match out.into_iter().next() {
            Some(Val::Result(Ok(Some(v)))) => Ok(*v),
            Some(Val::Result(Err(e))) => Err(format!("{component_id} answered {e:?}")),
            Some(v) => Ok(v),
            None => Err(format!("{component_id} returned nothing")),
        }
    }

    /// **Each binding of the store page, by its query** (ADR-0125): the page
    /// parameter `id` is the store this server holds, and `current_session()`
    /// is the request's session.
    fn bindings(&self, session: &str) -> Result<BTreeMap<String, Val>, String> {
        let mut out = BTreeMap::new();
        for b in self.plan["bindings"].as_array().into_iter().flatten() {
            let args = b["args"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|a| match a.as_str() {
                    Some("id") => Ok(Val::String(STORE_ID.into())),
                    Some("current_session()") => Ok(Val::String(session.into())),
                    other => Err(format!("an argument this server cannot compute: {other:?}")),
                })
                .collect::<Result<Vec<Val>, String>>()?;
            out.insert(
                b["binding"].as_str().unwrap_or_default().to_string(),
                self.fetch_binding(session, b, &args)?,
            );
        }
        Ok(out)
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
        let bindings = self.bindings(session)?;
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
        let value = self
            .part_text(session, "cart.line_count")
            .unwrap_or_else(|e| panic!("the cart part could not be computed: {e}"));
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

        // One frame per change: the entry advanced, and here is the patch the
        // server DERIVED from it. Two frames rather than one, because a
        // subscription and a patch are logically separate — a server may derive
        // a patch from a change rather than must.
        let entry = cart_entry(session);
        let version = self.version(session);
        let mut queue = self.pending.lock().expect("pending");
        let waiting = queue.entry(session.to_string()).or_default();
        waiting.push(StreamFrame::ResourceChanged {
            protocol: CURRENT,
            entry: entry.clone(),
            version,
        });
        waiting.push(StreamFrame::Patch(Patch {
            protocol: CURRENT,
            basis: CausalBasis::of(entry, version),
            target: self.cart_address(),
            operation: PatchOp::ReplaceText {
                text: value.to_string(),
            },
        }));
        // And the value itself, to a page that speculates on it (ADR-0122).
        if self.speculates_on_cart().is_some() {
            drop(queue);
            let cart = self.cart_json(session);
            let mut queue = self.pending.lock().expect("pending");
            queue
                .entry(session.to_string())
                .or_default()
                .push(StreamFrame::EntryValue {
                    protocol: CURRENT,
                    entry: cart_entry(session),
                    version,
                    value: cart,
                });
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
    fn fragment_domain(&self) -> IdentityDomain {
        IdentityDomain::document("store.page.Menu(47)", Partition::Public, BUILD)
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
    fn menu_env(&self, items: &[(String, String)]) -> Env {
        Env::new()
            .set("menu", menu_value(items))
            .in_domain(self.fragment_domain())
    }

    /// The menu fragment's bytes, materialized once and reused.
    ///
    /// Regenerated only when the entry is stale, and stored in the
    /// materializer under the PUBLIC entry key. Two readers get the same bytes
    /// because they are the same bytes — the entry is read, not re-rendered —
    /// and that is what makes `cache shared` a fact about the system rather
    /// than a claim about two renders agreeing.
    fn menu_fragment(&self, items: &[(String, String)]) -> String {
        let key = self.menu_key();
        if let Some(entry) = self.materializer.entry(&key)
            && !entry.stale
        {
            return entry.body;
        }
        let (template, part) = self.menu_part();
        let env = self.menu_env(items);
        let html = pw_render::render_part(template, part, &env, &self.templates)
            .expect("the menu fragment renders");
        self.clock.advance(1);
        self.materializer
            .regenerate(&key, None, || Ok(html.clone()));
        html
    }

    /// The public entry whose version every menu patch is caused by.
    fn menu_key(&self) -> EntryKey {
        EntryKey::from_identity(&menu_identity())
    }

    fn menu_version(&self) -> Version {
        Version(
            self.materializer
                .entry(&self.menu_key())
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
        // The deployment's menu changed: the `Menu` query's kept values are
        // stale (ADR-0127), as `MenuChanged` says to the materializer.
        self.queries.invalidate("store.page.Menu");

        // The fragment is re-materialized, in its own domain, once.
        let items = self.menu.lock().expect("menu").clone();
        let (template, part) = self.menu_part();
        let env = self.menu_env(&items);
        let html = pw_render::render_part(template, part, &env, &self.templates)
            .map_err(|b| format!("{b:?}"))?;
        self.clock.advance(1);
        // Invalidated first: `regenerate` leaves a current entry alone, so
        // without this the fragment kept its first rendering and every
        // document served after a change showed the old menu, while pages
        // already open were patched (found 2026-10-02, ADR-0127).
        self.materializer.invalidate(&self.menu_key(), 0);
        self.materializer
            .regenerate(&self.menu_key(), None, || Ok(html));

        let entry = ResourceEntryId::derive(&menu_identity(), &IDENTITY);
        let version = self.menu_version();

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

        let sessions: Vec<String> = queue.keys().cloned().collect();
        if std::env::var("PW_TRACE").is_ok() {
            eprintln!("broadcast {:?} to {sessions:?}", op);
        }
        for session in sessions {
            let waiting = queue.entry(session).or_default();
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
        let (rendered, cursor, _) = self.serve_document_with_entries(session);
        (rendered, cursor)
    }

    /// The document, its cursor, and the values of the entries it speculates
    /// on (ADR-0122), all read under one hold of the subscriber table, so the
    /// values are the ones the rendered parts show.
    fn serve_document_with_entries(&self, session: &str) -> (String, u64, serde_json::Value) {
        self.drain(session);
        self.forget_idle_subscribers();
        let mut queue = self.pending.lock().expect("pending");
        let waiting = queue.entry(session.to_string()).or_default();
        waiting.frames.clear();
        waiting.behind = false;
        waiting.seen = std::time::Instant::now();
        // The document takes a sequence number, so its cursor is never zero.
        waiting.last_seq += 1;
        let cursor = waiting.last_seq;
        // Rendered while the table is held, so a change cannot land between
        // the clear and the render and be lost by it.
        (
            self.render_store(session),
            cursor,
            self.speculated_entries(session),
        )
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

    /// Where the cart's value appears.
    ///
    /// Found in the template IR by the value it reads, so the address is the
    /// compiler's answer rather than a number typed twice.
    fn cart_address(&self) -> PartAddress {
        let template = self
            .templates
            .iter()
            .find(|t| t.name == "StorePage")
            .expect("the store page is in the IR");
        let part = template
            .manifest()
            .into_iter()
            .find(|p| p.value == "cart.line_count")
            .expect("the cart part is in the manifest");
        PartAddress::new(
            &TemplateSchemaId(template.schema.clone()),
            LocalPartId(part.id.0),
        )
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

    /// **A page whose values are its signals alone** (ADR-0130), rendered at
    /// each signal's first value, as `pw build`'s plan states it. A page that
    /// reads a query is the store's route's: this one asks the server for
    /// nothing, and neither does the page it serves.
    fn render_signal_page(&self, path: &str, session: &str) -> Result<String, String> {
        let plan = self
            .plans
            .get(path)
            .ok_or_else(|| format!("no page `{path}` in this build"))?;
        if plan["bindings"].as_array().is_some_and(|b| !b.is_empty()) {
            return Err(format!(
                "`{path}` reads a query; this route renders a page's signals alone"
            ));
        }
        let template = self
            .templates
            .iter()
            .find(|t| t.path == path)
            .ok_or_else(|| format!("no template `{path}`"))?;
        let mut env = Env::new();
        for s in plan["signals"].as_array().into_iter().flatten() {
            let name = s["name"].as_str().unwrap_or_default();
            env = env.set(name, Value::from_wire(&s["initial"]));
        }
        let env = env.in_domain(
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
        Ok(signal_document(&body, template, plan, &self.templates))
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

    fn render_store(&self, session: &str) -> String {
        let template = self
            .templates
            .iter()
            .find(|t| t.name == "StorePage")
            .expect("StorePage");
        // Both bound BEFORE the chain. A `MutexGuard` produced inside a method
        // argument lives until the end of the whole STATEMENT, so locking the
        // menu inside the chain and locking it again inside `menu_fragment`
        // deadlocked a non-reentrant mutex against itself — presenting as a
        // request that simply never returned.
        //
        // Every value is a query's (ADR-0125): each binding runs its compiled
        // component, and each part reads the binding's value by the steps the
        // compiler planned. The menu's items are the `Menu` query's.
        let bindings = self
            .bindings(session)
            .unwrap_or_else(|e| panic!("the store page's queries: {e}"));
        let items = menu_items(&bindings["menu"]);
        let fragment = self.menu_fragment(&items);
        let mut env = Env::new()
            .set("menu", menu_value(&items))
            // The public fragment, EMITTED rather than rendered. Its instance
            // tokens are the fragment's own, so every reader's document
            // contains the same bytes and one patch addresses all of them.
            .materialized(self.menu_part().1, &fragment);
        for part in self.plan["parts"].as_array().into_iter().flatten() {
            let path = part["path"].as_str().unwrap_or_default();
            let value = self
                .read_part(&bindings, part)
                .unwrap_or_else(|e| panic!("part `{path}`: {e}"));
            env = env.set(path, val_to_value(&value));
        }
        let env = env
            // A page, not a materialization: its domain is the route identity
            // and its partition. The generation is carried whatever the
            // partition is — the two are orthogonal.
            .in_domain(self.domain(session));
        pw_render::render(template, &env, &self.templates).expect("the store page renders")
    }
}

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

/// The one store this server holds (ADR-0125): what `stores#get` answers.
const STORE_ID: &str = "47";
const STORE_NAME: &str = "Blue Bottle";

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
        other => Value::Text(format!("{other:?}")),
    }
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
        other => serde_json::json!(format!("{other:?}")),
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
            match server.command_json(id, &session, &args, interaction.as_deref()) {
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
                Ok(result) => {
                    if let Err(e) = &result
                        && std::env::var("PW_TRACE").is_ok()
                    {
                        eprintln!("{id}: {e}");
                    }
                    // A commit says which versions it produced (ADR-0122), so a
                    // page can tell when the value it is sent includes it. A
                    // version, not a value: the value reaches the page from the
                    // resource, as every change does.
                    let body = match result {
                        Ok(()) => serde_json::json!({
                            "committed": true,
                            "basis": server.committed_basis(&session),
                        }),
                        Err(_) => serde_json::json!({ "committed": false }),
                    };
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
            let entry = ResourceEntryId::derive(&menu_identity(), &IDENTITY);
            let version = server.menu_version();
            let mut queue = server.pending.lock().expect("pending");
            queue
                .entry(session.clone())
                .or_default()
                .push(StreamFrame::Patch(Patch {
                    protocol: CURRENT,
                    basis: CausalBasis::of(entry, Version(version.0 + 1)),
                    target: server.menu_address(),
                    operation: PatchOp::RemoveInstance {
                        instance: pw_document::InstanceToken::from_wire("nosuchinstance"),
                    },
                }));
            drop(queue);
            respond_json(&mut stream, 202, &session, fresh, "{}");
        }
        // What the materializer holds for the PUBLIC menu entry. Enough to
        // show that a second reader reads rather than regenerates, and no
        // more: the storage key never appears, because the browser has no
        // business knowing how an entry is stored.
        ("GET", "/menu-entry") => {
            let identity = menu_identity();
            let entry = ResourceEntryId::derive(&identity, &IDENTITY);
            let version = server.menu_version();
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
        // A page whose values are its signals alone (ADR-0130).
        ("GET", route) if route.starts_with("/page/") => {
            let path = route.trim_start_matches("/page/");
            match server.render_signal_page(path, &session) {
                Ok(body) => respond(
                    &mut stream,
                    200,
                    "text/html; charset=utf-8",
                    &session,
                    fresh,
                    body.as_bytes(),
                ),
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
                    server.materializer.invalidate(&server.menu_key(), 0);
                    server.queries.invalidate("store.page.Menu");
                }
            }
            // Registering the subscriber HERE, when the document is served,
            // rather than when it first subscribes: the page holds instances
            // from this moment on, so a structural change after this moment
            // has an address in it. A registration deferred to the first poll
            // would silently drop every change that raced it.
            let (rendered, cursor, entries) = server.serve_document_with_entries(&session);
            let speculation = server
                .speculation
                .as_ref()
                .and_then(|m| m["module"].as_str())
                .map(|module| (format!("/speculation/{module}"), entries));
            let body = document(&rendered, &server.templates, cursor, speculation);
            respond(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                &session,
                fresh,
                body.as_bytes(),
            );
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
    mut since: u64,
) {
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
    if since > 0
        && !server
            .pending
            .lock()
            .expect("pending")
            .contains_key(session)
    {
        let _ = stream.write_all(format!("{}\n", reload_batch(since)).as_bytes());
        return;
    }

    // Bounded, like the long poll: a held connection is a held thread, and
    // three engine families times six workers is eighteen of them.
    for _ in 0..80 {
        let batch = {
            let mut queue = server.pending.lock().expect("pending");
            let waiting = queue.entry(session.to_string()).or_default();
            waiting.seen = std::time::Instant::now();
            waiting.acknowledge(since);
            let (cursor, frames) = waiting.after(since);
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
            since = cursor;
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
    let since: u64 = _query
        .split('&')
        .find_map(|p| p.strip_prefix("since="))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    // One route, two adapters. The frames are the same either way — see
    // `e2e/transport.spec.mjs`, which runs the whole subscription twice.
    if _query.split('&').any(|p| p == "mode=stream") {
        stream_open(server, stream, session, fresh, since);
        return;
    }

    // Forgotten while idle: see `stream_open`.
    if since > 0
        && !server
            .pending
            .lock()
            .expect("pending")
            .contains_key(session)
    {
        respond_json(stream, 200, session, fresh, &reload_batch(since));
        return;
    }

    for _ in 0..40 {
        {
            let mut queue = server.pending.lock().expect("pending");
            let waiting = queue.entry(session.to_string()).or_default();
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

/// **A page whose values are its signals alone, as a document** (ADR-0130).
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
         <script type=\"module\" src=\"/pw-runtime.mjs\"></script>\n\
         </body>\n</html>\n",
        pw_render::escape::text(&template.name)
    )
}

/// The part numbered `id`, wherever it is in `chunks`.
fn find_part(chunks: &[pw_render::ir::Chunk], id: u32) -> Option<&pw_render::ir::Part> {
    use pw_render::ir::{Chunk, Part};
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        if p.id().is_some_and(|i| i.0 == id) {
            return Some(p);
        }
        let inner = match p {
            Part::Conditional {
                then, otherwise, ..
            } => find_part(then, id).or_else(|| find_part(otherwise, id)),
            Part::Match { arms, .. } => arms.iter().find_map(|a| find_part(&a.body, id)),
            Part::Each { body, .. } => find_part(body, id),
            _ => None,
        };
        if inner.is_some() {
            return inner;
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
            match p {
                pw_render::ir::Part::Event {
                    handler, captures, ..
                } if !handler.is_empty() => {
                    out.insert(handler.clone(), captures.join(","));
                }
                pw_render::ir::Part::Conditional {
                    then, otherwise, ..
                } => {
                    walk(then, out);
                    walk(otherwise, out);
                }
                pw_render::ir::Part::Each { body, .. } => walk(body, out),
                pw_render::ir::Part::Match { arms, .. } => {
                    for a in arms {
                        walk(&a.body, out);
                    }
                }
                _ => {}
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
         <script type=\"module\" src=\"/pw-runtime.mjs\"></script>\n\
         </body>\n</html>\n"
    )
}

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

fn respond(stream: &mut TcpStream, code: u16, mime: &str, session: &str, fresh: bool, body: &[u8]) {
    let cookie = if fresh {
        format!("set-cookie: pw-session={session}; Path=/; SameSite=Lax\r\n")
    } else {
        String::new()
    };
    let head = format!(
        "HTTP/1.1 {code} OK\r\ncontent-type: {mime}\r\ncontent-length: {}\r\n{cookie}connection: close\r\n\r\n",
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
        let start = server_code.find("fn command(").expect("the command path");
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
        let (_, _, entries) = s.serve_document_with_entries("session-v");
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
        let values: Vec<&StreamFrame> = queue["session-v"]
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
        let (_, _, entries) = plain.serve_document_with_entries("session-w");
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
            !queue["session-w"]
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
            let w = queue.get_mut("steady").unwrap();
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
        assert!(s.pending.lock().unwrap().contains_key("gone"));
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
}
