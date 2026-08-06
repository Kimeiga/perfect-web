//! E6 — the resource dependency graph (charter §14 M6 task 1).
//!
//! ISR invalidates by route and by timer. This invalidates by **what a thing
//! actually depends on**, which is a different claim and a checkable one: the
//! graph is derived from declarations, so "changing one menu item regenerates
//! store 47's menu fragment and nothing else" is a property of the program
//! rather than a configuration someone remembered to write.
//!
//! # Compile time answers a different question from run time
//!
//! A key is a value, so at compile time there is no node for "store 47". What
//! there is:
//!
//! ```text
//! compile time   MenuFragment(id) --reads--> Menu(id)
//!                MenuChanged(store) --invalidates--> MenuFragment(store)
//!
//! run time       MenuFragment(47) --reads--> Menu(47)
//! ```
//!
//! The compiler produces the **schema** — nodes per declaration, edges labelled
//! with the key expressions that connect them. The materializer instantiates it
//! per key. Saying so matters, because a graph that claimed to know which
//! *instances* exist would be claiming to know the contents of the database.
//!
//! What the schema is nonetheless enough for is the gate: an event carries its
//! parameters, an edge records which of them flows into which key position, and
//! that is what makes `MenuChanged(store_47)` narrower than "the menu changed".
//!
//! # Serializable, because inspectable is a gate item
//!
//! Charter §14 M6 gate item 6 asks for the graph to be inspectable and
//! serializable. Both, and not one dressed as the other: a diagram is not
//! serializable and a blob is not inspectable. `pw emit-graph` prints JSON, and
//! `--plain` prints the edges as text a person reads.

use serde::{Deserialize, Serialize};

use crate::hir::{Decl, DeclKind, Hir};

/// A dimension a materialization's cache key varies by (charter §14 M6 task 1).
///
/// Not the same thing as a dependency. A dependency is a *resource whose change
/// invalidates this*; a dimension *partitions* the cache so two readers who
/// should see different content never share an entry. Confusing them is how a
/// cache serves French copy to an English reader and calls it a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// The build that produced the artifact. A materialization whose key omits
    /// it serves markup from one release to code from another.
    CodeVersion,
    Locale,
    Tenant,
    /// `shared` or `private`. Charter §14 M6 gate item 4 — private data must
    /// never be written into shared materialization storage — is this
    /// dimension being present and correct.
    PrivacyPartition,
    PolicyVersion,
}

impl Dimension {
    /// The policy keyword that includes this dimension in the key.
    pub fn keyword(self) -> &'static str {
        match self {
            Dimension::CodeVersion => "code_version",
            Dimension::Locale => "locale",
            Dimension::Tenant => "tenant",
            Dimension::PrivacyPartition => "partition",
            Dimension::PolicyVersion => "policy_version",
        }
    }

    pub const ALL: [Dimension; 5] = [
        Dimension::CodeVersion,
        Dimension::Locale,
        Dimension::Tenant,
        Dimension::PrivacyPartition,
        Dimension::PolicyVersion,
    ];
}

/// What a node in the graph is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "node")]
pub enum NodeKind {
    /// A `query`, `subscription` or `resource` — something with a snapshot.
    Resource {
        /// `shared`, `private` or `none`, as written.
        partition: Option<String>,
        /// `public`, `session` or `private`.
        privacy: Option<String>,
    },
    /// A `materialize` declaration.
    Materialization {
        placement: Option<String>,
        partition: Option<String>,
        regenerate: Option<String>,
        stampede: Option<String>,
        fallback: Option<String>,
        /// Dimensions declared `included_in_key`, sorted.
        varies_by: Vec<Dimension>,
    },
    /// An `event` declaration.
    Event,
    /// A `command` — what emits events.
    Command,
    /// A `page`, which reads resources like a materialization but is not one.
    Page,
}

/// One node: a declaration, with the part of its policy the graph cares about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// `Module.Name` — the same identity `Signatures` uses, so a node can be
    /// joined against a signature without a by-name lookup.
    pub path: String,
    pub name: String,
    #[serde(flatten)]
    pub kind: NodeKind,
    /// Parameter names in declaration order, so an edge can say which
    /// parameter of an event flows into which position of a key.
    pub params: Vec<String>,
}

/// Why one node points at another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// A materialization or page reads a resource. `depends_on Menu(id)`, or a
    /// page's `let menu = query Menu(id)`.
    Reads,
    /// A materialization regenerates when this event arrives.
    /// `invalidates_on MenuChanged(id)`.
    InvalidatedBy,
    /// A command emits an event. `emits MenuChanged(id)`.
    Emits,
    /// A command invalidates a resource directly. `invalidates Cart(session)`.
    Invalidates,
}

/// One edge, with the key expression that labels it.
///
/// The label is the source text of the argument list — `MenuChanged(id)` gives
/// `["id"]`. It is what makes the edge narrow: an edge with no arguments means
/// "any change to this", and one with `id` means "only where the ids match".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    /// The arguments as written, in order.
    pub key: Vec<String>,
}

/// The whole graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Names an edge referred to that no declaration defines.
    ///
    /// Kept in the graph rather than dropped, because a dangling edge is the
    /// E6 failure that looks like success: a materialization listening for an
    /// event nobody declared simply never regenerates, and the page is not
    /// wrong, only permanently stale.
    pub dangling: Vec<Dangling>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dangling {
    pub from: String,
    pub name: String,
    pub kind: EdgeKind,
}

impl Graph {
    /// Build from the whole program.
    ///
    /// Every unit at once, because a materialization in one module depends on
    /// a query in another and a graph built per file would have an edge to
    /// nothing in exactly the case the milestone is about.
    pub fn build(hirs: &[&Hir]) -> Graph {
        let mut g = Graph::default();

        // Pass one: nodes. Every edge target must exist before any edge is
        // resolved, so a forward reference is not a dangling one.
        for hir in hirs {
            for (id, decl) in hir.all_decls() {
                let Some(kind) = node_kind(decl) else {
                    continue;
                };
                let module = hir.module_of(id).unwrap_or_default();
                g.nodes.push(Node {
                    path: qualified(module, &decl.name),
                    name: decl.name.clone(),
                    kind,
                    params: decl.params.iter().map(|p| p.name.clone()).collect(),
                });
            }
        }

        // Pass two: edges.
        for hir in hirs {
            for (id, decl) in hir.all_decls() {
                if node_kind(decl).is_none() {
                    continue;
                }
                let module = hir.module_of(id).unwrap_or_default();
                let from = qualified(module, &decl.name);

                for (policy, kind) in [
                    ("depends_on", EdgeKind::Reads),
                    ("invalidates_on", EdgeKind::InvalidatedBy),
                    ("emits", EdgeKind::Emits),
                    ("invalidates", EdgeKind::Invalidates),
                ] {
                    let Some(p) = decl.policy(policy) else {
                        continue;
                    };
                    for (name, key) in calls(&p.value) {
                        g.push_edge(&from, &name, kind, key, module);
                    }
                }

                // A page declares its dependencies by reading them:
                // `let menu = query Menu(id)`. Same edge as `depends_on`,
                // discovered differently, because a page has no policy for it
                // and requiring one would make the graph a second place to
                // state a fact the body already states.
                if decl.kind == DeclKind::Page
                    && let Some(body_id) = decl.body
                {
                    for (name, key) in queried(hir.body(body_id)) {
                        g.push_edge(&from, &name, EdgeKind::Reads, key, module);
                    }
                }
            }
        }

        g.edges.sort_by(|a, b| {
            (&a.from, &a.to, a.kind, &a.key).cmp(&(&b.from, &b.to, b.kind, &b.key))
        });
        g.edges.dedup();
        g.nodes.sort_by(|a, b| a.path.cmp(&b.path));
        g.dangling
            .sort_by(|a, b| (&a.from, &a.name, a.kind).cmp(&(&b.from, &b.name, b.kind)));
        g.dangling.dedup();
        g
    }

    fn push_edge(
        &mut self,
        from: &str,
        name: &str,
        kind: EdgeKind,
        key: Vec<String>,
        module: &str,
    ) {
        match self.resolve(name, module) {
            Some(to) => self.edges.push(Edge {
                from: from.to_string(),
                to,
                kind,
                key,
            }),
            None => self.dangling.push(Dangling {
                from: from.to_string(),
                name: name.to_string(),
                kind,
            }),
        }
    }

    /// A node by the name an edge wrote, preferring the writer's own module.
    ///
    /// No unique-name fallback. If two modules declare `Menu` and the writing
    /// module declares neither, the edge is dangling — reported — rather than
    /// resolved to whichever one happens to exist. E2C deleted that fallback
    /// from member resolution for the reason that applies here too: whether an
    /// edge exists must not depend on a global accident.
    fn resolve(&self, name: &str, module: &str) -> Option<String> {
        let own = qualified(module, name);
        if self.nodes.iter().any(|n| n.path == own) {
            return Some(own);
        }
        let mut found = None;
        for n in &self.nodes {
            if n.name == name {
                if found.is_some() {
                    return None;
                }
                found = Some(n.path.clone());
            }
        }
        found
    }

    pub fn node(&self, path: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.path == path)
    }

    /// Everything an event's arrival regenerates, transitively.
    ///
    /// The gate question in one call: `affected_by("Events.MenuChanged")` must
    /// name store 47's menu fragment and nothing belonging to another store —
    /// which at schema level means every materialization listening for that
    /// event, plus anything reading what those produce.
    pub fn affected_by(&self, event: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut queue: Vec<String> = vec![event.to_string()];
        let mut seen: Vec<String> = vec![event.to_string()];
        while let Some(cur) = queue.pop() {
            for e in &self.edges {
                let hit = match e.kind {
                    // An event points AT what it invalidates: the edge is
                    // written on the materialization, so it runs backwards.
                    EdgeKind::InvalidatedBy => e.to == cur,
                    EdgeKind::Reads => e.to == cur,
                    _ => false,
                };
                if !hit {
                    continue;
                }
                if seen.contains(&e.from) {
                    continue;
                }
                seen.push(e.from.clone());
                out.push(e.from.clone());
                queue.push(e.from.clone());
            }
        }
        out.sort();
        out
    }

    /// The dimensions this materialization's key does NOT include.
    ///
    /// Charter §14 M6 task 9 — cache-key auditing. Reported as what is absent,
    /// because that is the direction that hurts: a key with an extra dimension
    /// costs hit rate, and a key missing one serves the wrong reader.
    pub fn key_gaps(&self, path: &str) -> Vec<Dimension> {
        let Some(Node {
            kind: NodeKind::Materialization { varies_by, .. },
            ..
        }) = self.node(path)
        else {
            return Vec::new();
        };
        Dimension::ALL
            .into_iter()
            .filter(|d| !varies_by.contains(d))
            .collect()
    }
}

fn qualified(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_string()
    } else {
        format!("{module}.{name}")
    }
}

fn node_kind(decl: &Decl) -> Option<NodeKind> {
    let value = |k: &str| decl.policy(k).map(|p| p.value.trim().to_string());
    Some(match decl.kind {
        DeclKind::Query | DeclKind::Subscription | DeclKind::Resource => NodeKind::Resource {
            partition: value("cache").or_else(|| value("partition")),
            privacy: decl.visibility.clone(),
        },
        DeclKind::Materialize => NodeKind::Materialization {
            placement: value("placement"),
            partition: value("partition"),
            regenerate: value("regenerate"),
            stampede: value("stampede"),
            fallback: value("fallback"),
            varies_by: varies_by(decl),
        },
        DeclKind::Event => NodeKind::Event,
        DeclKind::Command => NodeKind::Command,
        DeclKind::Page => NodeKind::Page,
        _ => return None,
    })
}

/// The dimensions a materialization declares `included_in_key`.
///
/// `partition public` counts as including the privacy partition: naming the
/// partition IS varying by it. The others need the explicit
/// `<dimension> included_in_key`, because a policy that merely mentions a
/// locale is not the same as one that keys on it.
fn varies_by(decl: &Decl) -> Vec<Dimension> {
    let mut out: Vec<Dimension> = Dimension::ALL
        .into_iter()
        .filter(|d| {
            decl.policy(d.keyword())
                .is_some_and(|p| p.value.contains("included_in_key"))
        })
        .collect();
    if decl
        .policy("partition")
        .is_some_and(|p| !p.value.trim().is_empty())
        && !out.contains(&Dimension::PrivacyPartition)
    {
        out.push(Dimension::PrivacyPartition);
    }
    out.sort();
    out
}

/// `Store(id), Menu(id)` -> `[("Store", ["id"]), ("Menu", ["id"])]`.
///
/// Split at top level only, so `InventoryChanged(id, _item: MenuItemId)` is one
/// call with two arguments rather than two calls. A bare name with no argument
/// list is a call with no arguments: `invalidates_on Rebuild` means "any".
fn calls(value: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    let mut items: Vec<&str> = Vec::new();
    for (i, c) in value.char_indices() {
        match c {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => depth -= 1,
            ',' if depth == 0 => {
                items.push(&value[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    items.push(&value[start..]);
    for item in items {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        match item.split_once('(') {
            Some((name, rest)) => {
                let inner = rest.strip_suffix(')').unwrap_or(rest);
                let args = split_args(inner);
                out.push((name.trim().to_string(), args));
            }
            None => out.push((item.to_string(), Vec::new())),
        }
    }
    out
}

fn split_args(inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    for (i, c) in inner.char_indices() {
        match c {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(inner[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = inner[start..].trim();
    if !last.is_empty() || !out.is_empty() {
        out.push(last.to_string());
    }
    out.retain(|a| !a.is_empty());
    out
}

/// One key argument as a reader would write it.
///
/// `path_of` answers "which declaration does this name" and returns nothing for
/// a call, so `query Cart(current_session())` labelled its edge with an empty
/// key — indistinguishable from `query Cart()`, which is an edge to every cart
/// rather than to one session's. The label is a description of the key, so it
/// renders the shape rather than resolving it.
fn key_text(body: &crate::hir::Body, id: crate::hir::ExprId) -> String {
    use crate::hir::Expr;
    match body.expr(id) {
        Expr::Name(n) => n.clone(),
        Expr::Field { base, name } => format!("{}.{name}", key_text(body, *base)),
        Expr::Call { callee, args } => {
            let inner: Vec<String> = args.iter().map(|a| key_text(body, a.value)).collect();
            format!("{}({})", key_text(body, *callee), inner.join(", "))
        }
        Expr::Literal(l) => format!("{l:?}"),
        _ => "_".to_string(),
    }
}

/// `let menu = query Menu(id)` in a page body -> `("Menu", ["id"])`.
fn queried(body: &crate::hir::Body) -> Vec<(String, Vec<String>)> {
    use crate::hir::Expr;
    let mut out = Vec::new();
    for id in body.walk() {
        let Expr::Keyword {
            keyword,
            modifiers,
            args,
            ..
        } = body.expr(id)
        else {
            continue;
        };
        if !matches!(keyword.as_str(), "query" | "subscription") {
            continue;
        }
        let Some(name) = modifiers.first() else {
            continue;
        };
        let key = args.iter().map(|a| key_text(body, *a)).collect();
        out.push((name.clone(), key));
    }
    out
}
