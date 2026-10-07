//! **A program's data, as a deployment supplies it to the host** (ADR-0218).
//!
//! The host serves any built program: its pages from their plans, its
//! queries and commands from their components and contracts, its events and
//! invalidations from the outbox. What the program's contracts import of a
//! data layer (`store:data/carts#add`, `feed:data/posts#publish`) is the
//! deployment's, and this is what the host asks of it.

use super::*;
use std::collections::BTreeSet;

/// Operations by their key, `store:data/carts#add`.
pub(crate) type Ops = BTreeMap<String, HostFn>;

pub(crate) trait DataLayer: Send + Sync {
    /// **The operations a query reads through, for one session.** Nothing a
    /// query does commits.
    fn reads(&self, session: &str, stopped: Option<Stopped>) -> Ops;

    /// **A command's writes, staged until its transaction commits.** The
    /// staging holds whatever the layer needs held from the call to the
    /// commit, and gives it up when dropped: a command that fails, or traps,
    /// commits nothing.
    fn begin<'a>(&'a self, session: &str) -> Box<dyn Staged + 'a>;

    /// **What a node grants it**, beyond the platform's session and outbox.
    fn grants(&self) -> Vec<&'static str>;

    /// **The page a document with none recorded is**, where it has one.
    fn default_page(&self) -> Option<&'static str>;

    /// **The style its pages carry in their head** (ADR-0220), where it has
    /// one: the store's menu's containment (ADR-0187).
    fn style(&self) -> &'static str {
        ""
    }

    /// **Whether a session's documents are versioned by an entry of its
    /// own** (the store's cart), which a commit regenerates; otherwise a
    /// commit moves the session's documents on the host's clock.
    fn session_entry(&self) -> bool {
        false
    }

    /// **What the database it opened provides** (ADR-0246), which the host
    /// compares with what the program's sources state before it serves. By
    /// default what the host's own database gives a resource no source holds
    /// (ADR-0207): serializable transactions, reads of the latest commit, and
    /// no feed of its changes. An in-memory layer gives that under its lock.
    fn provides(&self) -> Result<Provided, String> {
        Ok(Provided::host_database())
    }

    /// **Every operation it supplies**, read from the functions it builds,
    /// not from a list kept beside them.
    fn operations(&self) -> BTreeSet<String> {
        let mut ops: BTreeSet<String> = self.reads("", None).into_keys().collect();
        ops.extend(self.begin("").ops().into_keys());
        ops
    }
}

/// **One command's writes, staged** (ADR-0218).
pub(crate) trait Staged {
    /// The command's operations: the layer's reads, and its writes, which
    /// stage.
    fn ops(&self) -> Ops;
    /// What the commit writes in the materializer's transaction, by key, or
    /// none where nothing was written.
    fn rows(&self) -> Option<Vec<(String, String)>>;
    /// After the commit: what was staged is the layer's.
    fn publish(&mut self);

    /// **The command's writes and what it handed the outbox, committed in
    /// the layer's own transaction** (ADR-0208, ADR-0246), where the layer
    /// keeps its own outbox: events and invalidated entries, each by its
    /// declaration's path with the values the command computed.
    ///
    /// Returns what its outbox committed, read back once the transaction
    /// committed, which the host delivers. `None` where the layer keeps no
    /// outbox: the host's materializer commits the events with [`rows`].
    /// An error is a commit refused, and nothing of the command is kept.
    ///
    /// [`rows`]: Staged::rows
    fn commit(
        &mut self,
        _events: &[Handed],
        _invalidated: &[Dropped],
    ) -> Result<Option<Outboxed>, String> {
        Ok(None)
    }
}

/// One thing a command handed the outbox: its declaration's path, and the
/// values the command computed.
pub(crate) type Handed = (String, Vec<Val>);

/// **An entry a command drops** (ADR-0209, ADR-0256): its query's path, and
/// the value at each of its parameters, `None` where the command wrote `_`,
/// every value there.
pub(crate) type Dropped = (String, Vec<Option<Val>>);

/// **What a layer's outbox committed** (ADR-0246): the events, then the
/// invalidated entries, each in the order the command handed them.
pub(crate) type Outboxed = (Vec<Handed>, Vec<Dropped>);

/// **What a database provides** (ADR-0207's vocabulary, ADR-0246): the
/// isolation a command's transaction gets, what a read may promise, and
/// whether it tells what changed once committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Provided {
    /// `serializable`, `snapshot`, `read_committed` or `none`.
    pub transactions: String,
    pub reads: BTreeSet<String>,
    pub feed: bool,
}

impl Provided {
    /// What the host's own database gives (ADR-0005, ADR-0207).
    pub(crate) fn host_database() -> Provided {
        Provided {
            transactions: "serializable".to_string(),
            reads: BTreeSet::from(["strong".to_string()]),
            feed: false,
        }
    }
}

/// **A source's clauses, as `pw build` wrote them** (`sources.json`).
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct Declared {
    pub name: String,
    pub holds: Vec<String>,
    pub transactions: String,
    pub reads: Vec<String>,
    pub changes: String,
}

/// How much an isolation prevents (Berenson et al., SIGMOD 1995), in the
/// checker's order: serializable, snapshot, read committed, none.
fn isolation(word: &str) -> u8 {
    match word {
        "serializable" => 3,
        "snapshot" => 2,
        "read_committed" => 1,
        _ => 0,
    }
}

/// **Does what a database provides give what a source states?**
/// (ADR-0246): its transactions at least as isolated, each promise its reads
/// make (`strong` gives every one, and every database gives `eventual`), and
/// a feed of its changes where it states one. Each shortfall, one line.
pub(crate) fn shortfalls(declared: &Declared, provided: &Provided) -> Vec<String> {
    let mut out = Vec::new();
    if isolation(&declared.transactions) > isolation(&provided.transactions) {
        out.push(format!(
            "`{}` states `transactions {}`, and the database's are {}",
            declared.name, declared.transactions, provided.transactions
        ));
    }
    for read in &declared.reads {
        let given = read == "eventual"
            || provided.reads.contains("strong")
            || provided.reads.contains(read);
        if !given {
            out.push(format!(
                "`{}` states `reads {read}`, and the database's reads give {}",
                declared.name,
                provided
                    .reads
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    if declared.changes == "feed" && !provided.feed {
        out.push(format!(
            "`{}` states `changes feed`, and its data layer delivers no feed of its changes",
            declared.name
        ));
    }
    out
}

/// **What a layer's database must give, and each way it falls short**
/// (ADR-0246): each declared source holding a resource the layer's grants
/// name (`database.read<Post>` names `Post`), and, for a resource no source
/// holds, the host's own database's guarantees (ADR-0207).
pub(crate) fn held_to(sources: &[Declared], grants: &[&str], provided: &Provided) -> Vec<String> {
    let resources: BTreeSet<&str> = grants
        .iter()
        .filter_map(|g| {
            g.strip_prefix("database.read<")
                .or_else(|| g.strip_prefix("database.write<"))
                .and_then(|r| r.strip_suffix('>'))
        })
        .collect();
    let mut out = Vec::new();
    for source in sources
        .iter()
        .filter(|s| s.holds.iter().any(|h| resources.contains(h.as_str())))
    {
        out.extend(shortfalls(source, provided));
    }
    let unheld: Vec<&str> = resources
        .iter()
        .copied()
        .filter(|r| !sources.iter().any(|s| s.holds.iter().any(|h| h == r)))
        .collect();
    if !unheld.is_empty() {
        let host = Provided::host_database();
        let implied = Declared {
            name: format!("the host's database, holding {}", unheld.join(", ")),
            holds: unheld.iter().map(|r| r.to_string()).collect(),
            transactions: host.transactions,
            reads: host.reads.into_iter().collect(),
            changes: "none".to_string(),
        };
        out.extend(shortfalls(&implied, provided));
    }
    out
}
