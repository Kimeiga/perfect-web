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

    /// **Whether a session's documents are versioned by an entry of its
    /// own** (the store's cart), which a commit regenerates; otherwise a
    /// commit moves the session's documents on the host's clock.
    fn session_entry(&self) -> bool {
        false
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
}
