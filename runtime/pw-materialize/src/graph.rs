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
    CodeVersion,
    Locale,
    Tenant,
    PrivacyPartition,
    PolicyVersion,
}

impl Dimension {
    /// The name this dimension has in an [`crate::EntryKey`].
    pub fn key_name(self) -> &'static str {
        match self {
            Dimension::CodeVersion => "code_version",
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
