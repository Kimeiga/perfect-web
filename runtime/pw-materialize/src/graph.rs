//! The compiler's dependency graph, read as data.
//!
//! ADR-0018's shape, applied again: the compiler emits `pw emit-graph` and this
//! crate deserializes it. `pw-core` does not know the materializer exists, and
//! the materializer does not link the compiler. A runtime the compiler depends
//! on is the only runtime there can ever be.
//!
//! The types here mirror `pw_core::graph` by **field name**, not by sharing a
//! definition — that is what makes them a boundary rather than a coupling. The
//! cost is that a field could be renamed on one side; the round-trip test in
//! `tests/graph_contract.rs` reads a graph the real compiler produced, so a
//! rename shows up as a test failure rather than as an empty list at run time.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Locale,
    Tenant,
    PrivacyPartition,
    PolicyVersion,
}

impl Dimension {
    /// The name this dimension has in an [`crate::EntryKey`].
    pub fn key_name(self) -> &'static str {
        match self {
            Dimension::Locale => "locale",
            Dimension::Tenant => "tenant",
            Dimension::PrivacyPartition => "partition",
            Dimension::PolicyVersion => "policy_version",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Reads,
    InvalidatedBy,
    Emits,
    Invalidates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub key: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub path: String,
    pub name: String,
    pub node: String,
    #[serde(default)]
    pub params: Vec<String>,
    #[serde(default)]
    pub partition: Option<String>,
    #[serde(default)]
    pub privacy: Option<String>,
    #[serde(default)]
    pub placement: Option<String>,
    #[serde(default)]
    pub regenerate: Option<String>,
    #[serde(default)]
    pub stampede: Option<String>,
    #[serde(default)]
    pub fallback: Option<String>,
    #[serde(default)]
    pub varies_by: Vec<Dimension>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graph {
    /// The compatibility generation the compiler stamped this graph with.
    /// Hand it to `Materializer::new`; it is not a dimension a caller chooses.
    #[serde(default)]
    pub compatibility: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub dangling: Vec<serde_json::Value>,
}

impl Graph {
    pub fn from_json(s: &str) -> Result<Graph, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn node(&self, path: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.path == path)
    }

    /// **Does `listener`'s entry keyed `key` listen for `event` carrying
    /// `values`?** (ADR-0091)
    ///
    /// An `invalidated_by` edge's key is the listener's arguments as written.
    /// One that names a parameter of the listener binds that part of the
    /// entry's key: the event's value at its position must equal it. `_`
    /// binds nothing, and neither does a position the list leaves out. An
    /// event that carries no values says nothing about which entry, and
    /// reaches every one.
    ///
    /// Until 2026-09-26 the materializer asked whether each of an event's
    /// values was somewhere in the entry's key. `InventoryChanged(47, item 3)`
    /// was deferred forever, since no menu is keyed by an item, and a value at
    /// one position matched a key at another.
    pub fn listens(&self, listener: &str, event: &str, values: &[String], key: &[String]) -> bool {
        let key: Vec<Option<&str>> = key.iter().map(|k| Some(k.as_str())).collect();
        self.binds(listener, event, values, &key)
    }

    /// [`Graph::listens`], for an entry whose key may be known only in part:
    /// a position that is `None` matches any value.
    fn binds(&self, listener: &str, event: &str, values: &[String], key: &[Option<&str>]) -> bool {
        let params = self
            .node(listener)
            .map(|n| n.params.as_slice())
            .unwrap_or_default();
        self.edges
            .iter()
            .filter(|e| e.kind == EdgeKind::InvalidatedBy && e.from == listener && e.to == event)
            .any(|e| {
                values.iter().enumerate().all(|(i, value)| {
                    match e
                        .key
                        .get(i)
                        .and_then(|arg| params.iter().position(|p| p == arg))
                    {
                        Some(j) => key.get(j).is_some_and(|k| k.is_none_or(|k| k == value)),
                        None => true,
                    }
                })
            })
    }

    /// **Does `event` carrying `values` reach `node`'s entry keyed `key`?**
    /// (ADR-0102)
    ///
    /// It does when the node listens for the event and binds the entry, and
    /// when the node reads a node the event reaches, at the key its read
    /// supplies. Charter §9.4 makes a materialization a view over what it
    /// reads, and the compiler's `Graph::affected_by` follows reads the same
    /// way. Until 2026-09-26 an event reached its direct listeners only: a
    /// store's change dropped the store and left the fragment built from it.
    ///
    /// A read's key is the reader's arguments as written: `Store(id)` passes
    /// the reader's `id` as the store's first parameter. A position the read
    /// does not fill from a parameter is not known, and matches any value: an
    /// entry invalidated needlessly costs a regeneration, and one missed is a
    /// stale page.
    pub fn reaches(&self, node: &str, event: &str, values: &[String], key: &[String]) -> bool {
        let key: Vec<Option<&str>> = key.iter().map(|k| Some(k.as_str())).collect();
        self.reaches_from(node, event, values, &key, &mut Vec::new())
    }

    /// [`Graph::reaches`], along one path of reads. A node already on the
    /// path is not read again, so a cycle of reads ends.
    fn reaches_from(
        &self,
        node: &str,
        event: &str,
        values: &[String],
        key: &[Option<&str>],
        path: &mut Vec<String>,
    ) -> bool {
        if self.binds(node, event, values, key) {
            return true;
        }
        if path.iter().any(|p| p == node) {
            return false;
        }
        path.push(node.to_string());
        let params = self
            .node(node)
            .map(|n| n.params.as_slice())
            .unwrap_or_default();
        let found = self
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Reads && e.from == node)
            .any(|e| {
                let width = self
                    .node(&e.to)
                    .map_or(e.key.len(), |n| n.params.len().max(e.key.len()));
                let supplied: Vec<Option<&str>> = (0..width)
                    .map(|i| {
                        e.key
                            .get(i)
                            .and_then(|arg| params.iter().position(|p| p == arg))
                            .and_then(|j| key.get(j).copied().flatten())
                    })
                    .collect();
                self.reaches_from(&e.to, event, values, &supplied, path)
            });
        path.pop();
        found
    }

    /// Everything that declared `invalidates_on <event>`.
    ///
    /// The edge is written on the listener and points at the event, so this
    /// reads it backwards — the direction an arriving event needs.
    ///
    /// Not only fragments: a cached `query` writes `invalidates_on` too, and
    /// its entry has to drop for the same reason. Narrowing this to
    /// materializations would leave every query's cache invalidated by nothing
    /// but its own freshness timer, which is the mechanism this milestone
    /// exists to replace.
    pub fn listeners_for(&self, event: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::InvalidatedBy && e.to == event)
            .map(|e| e.from.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The listeners that are materializations.
    pub fn fragments_listening_for(&self, event: &str) -> Vec<String> {
        self.listeners_for(event)
            .into_iter()
            .filter(|p| self.node(p).is_some_and(|n| n.node == "materialization"))
            .collect()
    }

    /// Resources a fragment reads.
    pub fn dependencies_of(&self, fragment: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Reads && e.from == fragment)
            .map(|e| e.to.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The dimensions a fragment's key must separate, from its declaration.
    ///
    /// Read from the graph rather than passed in by a caller, so an entry key
    /// built by hand that omits a declared dimension is a mismatch a test can
    /// see — the failure it prevents is a cache HIT serving the wrong reader,
    /// which no invalidation test finds.
    pub fn dimensions_of(&self, fragment: &str) -> Vec<Dimension> {
        self.node(fragment)
            .map(|n| n.varies_by.clone())
            .unwrap_or_default()
    }

    /// Every materialization in the graph.
    pub fn fragments(&self) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|n| n.node == "materialization")
            .collect()
    }
}
