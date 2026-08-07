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
//! # Deliberately not
//!
//! A deployment abstraction, an authentication system, a plugin host, a Wasm
//! executor, an HTTP/3 experiment, or a distributed materializer. Each of those
//! belongs to a milestone that has not started.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use pw_document::{IdentityDomain, LocalPartId, PartAddress, Partition, TemplateSchemaId};
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
#[derive(Default)]
struct Subscriber {
    /// The last sequence assigned. Sequences start at ONE, so that the
    /// initial cursor — zero, meaning "nothing acknowledged yet" — is smaller
    /// than every frame. With zero-based sequences the very first frame a
    /// subscriber ever received was numbered zero, `since=0` read it as
    /// already acknowledged, and it was never delivered. It cost a real
    /// invalidation and looked like a transport that simply had nothing to say.
    last_seq: u64,
    frames: Vec<(u64, StreamFrame)>,
}

impl Subscriber {
    fn push(&mut self, frame: StreamFrame) {
        self.last_seq += 1;
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
    /// Per-session cart state, behind the materializer's command boundary.
    carts: Mutex<BTreeMap<String, i64>>,
    /// The document's static assets.
    dist: std::path::PathBuf,
    /// Frames waiting for each session's subscriber.
    pending: Mutex<BTreeMap<String, Subscriber>>,
}

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

impl Server {
    fn new(dist: std::path::PathBuf, templates: Vec<Template>) -> Server {
        let clock = Clock::new();
        let materializer = Materializer::new(clock.clone(), BUILD);
        materializer.declare("store.page.Cart", FragmentPolicy::default());
        materializer.declare("store.page.Menu", FragmentPolicy::default());
        Server {
            templates,
            clock,
            materializer,
            carts: Mutex::new(BTreeMap::new()),
            menu: Mutex::new(default_menu()),
            dist,
            pending: Mutex::new(BTreeMap::new()),
        }
    }

    /// The storage key, derived from the same identity the wire id is.
    fn cart_key(&self, session: &str) -> EntryKey {
        EntryKey::from_identity(&cart_identity(session))
    }

    fn cart_value(&self, session: &str) -> i64 {
        *self.carts.lock().expect("carts").get(session).unwrap_or(&0)
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

    /// `add_to_cart`, through the real path.
    ///
    /// The command commits state and its event together. If `fail` is set it
    /// rolls back, and then neither the state nor the event survives — so the
    /// browser receives nothing, which is the property ADR-0019 exists for.
    fn add_to_cart(&self, session: &str, fail: bool) -> Result<(), String> {
        let next = self.cart_value(session) + 1;
        let result: Result<Vec<i64>, String> = self.materializer.command(|tx| {
            Materializer::set_state(tx, &format!("cart:{session}"), &next.to_string());
            if fail {
                return Err("the command failed after writing".to_string());
            }
            Ok(vec![pw_materialize::Event::new(
                "Events.CartChanged",
                &[session],
            )])
        });
        // Rolled back: neither the state nor the event survives, so the cart
        // is not updated and nothing is queued for the browser.
        result?;
        self.carts
            .lock()
            .expect("carts")
            .insert(session.to_string(), next);

        // The materializer drains the committed event and regenerates the
        // entry it invalidates. The version moves because the RESOURCE moved.
        self.drain(session);
        Ok(())
    }

    /// `clear_cart`, through the same path as `add_to_cart`.
    fn clear_cart(&self, session: &str) -> Result<(), String> {
        let key = session.to_string();
        self.materializer.command::<String>(|_tx| {
            self.carts.lock().expect("carts").insert(key.clone(), 0);
            Ok(vec![pw_materialize::Event::new(
                "Events.CartChanged",
                &[&key],
            )])
        })?;
        self.drain(session);
        Ok(())
    }

    /// Consume committed events and regenerate what they invalidate.
    fn drain(&self, session: &str) {
        let key = self.cart_key(session);
        let graph = self.graph();
        let invalidated = self.materializer.drain(&graph, std::slice::from_ref(&key));
        let value = self.cart_value(session);
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

        // The fragment is re-materialized, in its own domain, once.
        let items = self.menu.lock().expect("menu").clone();
        let (template, part) = self.menu_part();
        let env = self.menu_env(&items);
        let html = pw_render::render_part(template, part, &env, &self.templates)
            .map_err(|b| format!("{b:?}"))?;
        self.clock.advance(1);
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
    fn serve_document(&self, session: &str) -> (String, u64) {
        self.drain(session);
        let mut queue = self.pending.lock().expect("pending");
        let waiting = queue.entry(session.to_string()).or_default();
        waiting.frames.clear();
        let cursor = waiting.last_seq;
        // Rendered while the table is held, so a change cannot land between
        // the clear and the render and be lost by it.
        (self.render_store(session), cursor)
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

    /// Which handler an identity names, if this build has it.
    ///
    /// Answered from the compiler's template IR rather than from a list here.
    /// A list would be a second answer to "which handlers exist", and the
    /// first thing it would do is disagree.
    fn handler_named(&self, identity: &str) -> Option<String> {
        self.templates.iter().find_map(|t| {
            t.manifest().into_iter().find_map(|p| {
                (p.kind == "event" && p.value == identity && !p.name.is_empty())
                    .then(|| p.name.clone())
            })
        })
    }

    /// The graph the materializer consumes.
    fn graph(&self) -> pw_materialize::Graph {
        pw_materialize::Graph::from_json(GRAPH).expect("the committed graph parses")
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
        let items = self.menu.lock().expect("menu").clone();
        let fragment = self.menu_fragment(&items);
        let env = Env::new()
            .set("store.name", Value::Text("Blue Bottle".into()))
            .set("menu", menu_value(&items))
            // The public fragment, EMITTED rather than rendered. Its instance
            // tokens are the fragment's own, so every reader's document
            // contains the same bytes and one patch addresses all of them.
            .materialized(self.menu_part().1, &fragment)
            .set("cart.line_count", Value::Int(self.cart_value(session)))
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

/// The graph, as `pw emit-graph` produced it. Read at build time so the server
/// cannot drift from the compiler's answer between runs.
const GRAPH: &str = include_str!("../../../../runtime/pw-materialize/tests/store-graph.json");

fn main() {
    let dist = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "dist".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3143);

    let ir = std::fs::read_to_string(format!("{dist}/../store-ir.json"))
        .expect("the template IR is emitted before the server starts");
    let templates: Vec<Template> = serde_json::from_str(&ir).expect("the template IR parses");

    let server = Arc::new(Server::new(dist.into(), templates));
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

    let route = path.split('?').next().unwrap_or("/");
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");

    match (method, route) {
        ("POST", "/command/add_to_cart") => {
            let ok = server.add_to_cart(&session, false).is_ok();
            respond_json(
                &mut stream,
                202,
                &session,
                fresh,
                &format!("{{\"committed\":{ok}}}"),
            );
        }
        ("POST", "/command/clear_cart") => {
            // The second command, and the reason it exists is E7-L: two
            // handlers are what make "the exact handler was loaded" a claim
            // that can be false. It goes through the same path — commit state
            // and event together, then drain.
            let ok = server.clear_cart(&session).is_ok();
            respond_json(
                &mut stream,
                202,
                &session,
                fresh,
                &format!("{{\"committed\":{ok}}}"),
            );
        }
        ("POST", "/command/add_and_fail") => {
            let _ = server.add_to_cart(&session, true);
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
        // Generating the module here is the dev host standing in for E9's code
        // generator. What is real is the boundary: the document does not carry
        // it, the browser asks for it by identity, and the server refuses an
        // identity this build does not have.
        ("GET", route) if route.starts_with("/handler/") => {
            let id = route
                .trim_start_matches("/handler/")
                .trim_end_matches(".mjs");
            // The query is the browser's, not ours: a retry appends one so the
            // module map treats it as a new specifier, because a failed load
            // is cached forever otherwise. The identity is still the path.
            let id = id.split('?').next().unwrap_or(id);
            let Some(name) = server.handler_named(id) else {
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
            };
            // Deliberately tiny, and deliberately not a bundle. Every handler
            // is its own module, because "the exact handler was fetched" is
            // only observable if handlers are separable.
            let body = format!(
                "// handler {name}, identity {id}\n\
                 export const name = {name:?};\n\
                 export async function run() {{\n\
                 \x20 await fetch(\"/command/{name}\", {{ method: \"POST\" }});\n\
                 }}\n"
            );
            respond(
                &mut stream,
                200,
                "text/javascript; charset=utf-8",
                &session,
                fresh,
                body.as_bytes(),
            );
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
            // Registering the subscriber HERE, when the document is served,
            // rather than when it first subscribes: the page holds instances
            // from this moment on, so a structural change after this moment
            // has an address in it. A registration deferred to the first poll
            // would silently drop every change that raced it.
            let (rendered, cursor) = server.serve_document(&session);
            let body = document(&rendered, &server.templates, cursor);
            respond(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                &session,
                fresh,
                body.as_bytes(),
            );
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

    // Bounded, like the long poll: a held connection is a held thread, and
    // three engine families times six workers is eighteen of them.
    for _ in 0..80 {
        let batch = {
            let mut queue = server.pending.lock().expect("pending");
            let waiting = queue.entry(session.to_string()).or_default();
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

    for _ in 0..40 {
        {
            let mut queue = server.pending.lock().expect("pending");
            let waiting = queue.entry(session.to_string()).or_default();
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

/// The document shell, with the parts manifest and the runtime.
fn document(body: &str, templates: &[Template], cursor: u64) -> String {
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
        "resume": {
            "scheme": "2", "abi": "1", "build": BUILD, "handler": "add_to_cart",
            "capture": "cart", "document": "cart-doc", "scope": "public",
            "captures": "x", "construct": "region",
            "handlers": {
                "add_to_cart": { "handler": "add_to_cart", "capture": "cart", "captures": "x" },
                "clear_cart": { "handler": "clear_cart", "capture": "", "captures": "" },
            },
        },
    });
    let json = serde_json::to_string(&manifest)
        .unwrap_or_default()
        .replace("</script", "<\\/script");
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>Store</title>\n</head>\n<body>\n{body}\n\
         <script type=\"application/json\" id=\"pw-parts\">{json}</script>\n\
         <script type=\"module\" src=\"/pw-runtime.mjs\"></script>\n\
         </body>\n</html>\n"
    )
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
