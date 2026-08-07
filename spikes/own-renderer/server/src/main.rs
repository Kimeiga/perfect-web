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
use pw_render::{Env, Template, Value};
use pw_resource::{DevelopmentIdentityKey, EntryIdentity};

/// The build this server serves. One value, used as the compatibility
/// generation everywhere it is needed, so nothing derives a second one.
const BUILD: &str = "B1";

/// The deployment's PRF key, from a provider rather than a literal.
const IDENTITY: DevelopmentIdentityKey = DevelopmentIdentityKey;

struct Server {
    /// The templates the compiler emitted, deserialized once.
    templates: Vec<Template>,
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
    pending: Mutex<BTreeMap<String, Vec<StreamFrame>>>,
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

fn cart_entry(session: &str) -> ResourceEntryId {
    ResourceEntryId::derive(&cart_identity(session), &IDENTITY)
}

impl Server {
    fn new(dist: std::path::PathBuf, templates: Vec<Template>) -> Server {
        let clock = Clock::new();
        let materializer = Materializer::new(clock.clone(), BUILD);
        materializer.declare("store.page.Cart", FragmentPolicy::default());
        Server {
            templates,
            clock,
            materializer,
            carts: Mutex::new(BTreeMap::new()),
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
        let env = Env::new()
            .set("store.name", Value::Text("Blue Bottle".into()))
            .set("menu", menu())
            .set("cart.line_count", Value::Int(self.cart_value(session)))
            // A page, not a materialization: its domain is the route identity
            // and its partition. The generation is carried whatever the
            // partition is — the two are orthogonal.
            .in_domain(
                IdentityDomain::document(
                    "StorePage(47)",
                    Partition::Session {
                        id: session.to_string(),
                    },
                    BUILD,
                )
                .keyed("own-renderer-spike-key"),
            );
        pw_render::render(template, &env, &self.templates).expect("the store page renders")
    }
}

fn menu() -> Value {
    Value::List(
        [
            ("espresso", "Espresso"),
            ("cortado", "Cortado"),
            ("cold-brew", "Cold Brew"),
        ]
        .into_iter()
        .map(|(id, name)| {
            let mut f = BTreeMap::new();
            f.insert("id".to_string(), Value::Text(id.into()));
            f.insert("name".to_string(), Value::Text(name.into()));
            Value::Record(f)
        })
        .collect(),
    )
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
        ("GET", "/stream") => stream_frames(server, &mut stream, &session, fresh, query),
        ("GET", "/StorePage.html") | ("GET", "/") => {
            server.drain(&session);
            let body = document(&server.render_store(&session), &server.templates);
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
    for _ in 0..40 {
        {
            let mut queue = server.pending.lock().expect("pending");
            if let Some(frames) = queue.get_mut(session)
                && !frames.is_empty()
            {
                let body: Vec<serde_json::Value> = frames
                    .drain(..)
                    .map(|f| serde_json::to_value(&f).expect("frame"))
                    .collect();
                let json = serde_json::to_string(&body).expect("frames");
                respond_json(stream, 200, session, fresh, &json);
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    respond_json(stream, 200, session, fresh, "[]");
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
fn document(body: &str, templates: &[Template]) -> String {
    let template = templates
        .iter()
        .find(|t| t.name == "StorePage")
        .expect("StorePage");
    let manifest = serde_json::json!({
        "template": template.path,
        "schema": template.schema,
        "parts": template.manifest(),
        "resume": {
            "scheme": "2", "abi": "1", "build": BUILD, "handler": "add_to_cart",
            "capture": "cart", "document": "cart-doc", "scope": "public",
            "captures": "x", "construct": "region",
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
