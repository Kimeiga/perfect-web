//! **Whether an interface edge can be bound remotely.**
//!
//! The other policy over `boundary.rs`. `resume.rs` asks whether a captured
//! value may be written into a manifest that ships with the document; this asks
//! whether a call may be made across a process boundary.
//!
//! Architect ruling, 2026-08-07:
//!
//! > Remote capability is a property of an **interface edge**, not a component:
//! > `get(StoreId) -> Result<Store, StoreError>` is probably remotable and
//! > `with_transaction(fn(OpenTransaction) -> T)` is not, in the same
//! > component. Arguments, results AND errors are all checked, in both
//! > directions.
//!
//! So the unit here is one exported declaration's signature, not a contract.
//! A component with a remotable query and a non-remotable transaction helper is
//! an ordinary component, and a per-component answer would have to be the
//! conjunction — making the whole thing local because one function holds a
//! handle.
//!
//! # Why `BindingSupport` is a struct
//!
//! > `BindingMode` as an enum is rejected […] because `Either` is just both,
//! > and an enum forecloses "remote ✓ only through a host-mediated handle
//! > proxy".
//!
//! ```rust,ignore
//! struct BindingSupport { local: LocalSupport, remote: RemoteSupport }
//! ```
//!
//! Local and remote are two questions, and an enum makes them one. `Either` is
//! not a third mode; it is both answers being yes.
//!
//! # What this does NOT decide
//!
//! Whether an edge SHOULD be remote, and whether the two ends may be
//! co-located. Both are the planner's, from placement — `runtime/pw-host`'s
//! `plan.rs`. This says only what the signature permits, which is a compiler
//! fact and travels in the contract.

use serde::{Deserialize, Serialize};

use crate::boundary::{
    Blocked, Boundary, BoundaryContext, Crossing, Direction, TypeFacts, Violation, can_cross,
};
use crate::privacy::Label;
use crate::signatures::Signature;

/// **What binding modes an interface edge supports.**
///
/// Two independent answers, never one enum. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BindingSupport {
    pub local: LocalSupport,
    pub remote: RemoteSupport,
}

/// What a contract written before this field existed means.
///
/// The permissive answer, deliberately and narrowly: a host that had no field
/// to read already treated every edge as bindable either way, so this changes
/// nothing for an old artifact. It is not a fallback for an edge the compiler
/// declined to analyse — that is `RemoteSupport::Undetermined`, which is a
/// value the compiler writes on purpose.
impl Default for BindingSupport {
    fn default() -> Self {
        BindingSupport {
            local: LocalSupport::Direct,
            remote: RemoteSupport::Transferable,
        }
    }
}

/// Whether the edge can be a direct call in one address space.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalSupport {
    /// A direct call: no encoding, no copy, no schema.
    ///
    /// Always, from the signature's point of view. Nothing about a type can
    /// forbid passing it to a function in the same address space — that is what
    /// makes `with_transaction` legal at all. Whether the two ends may actually
    /// share a node is placement's question, and the planner composes this with
    /// that rather than this pre-empting it.
    Direct,
}

/// Whether the edge can cross a process boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RemoteSupport {
    /// Every value in the signature can cross, in both directions.
    Transferable,
    /// Something in the signature cannot, and here is each one.
    Refused { positions: Vec<Untransferable> },
    /// The analysis has no basis to decide — a position whose type this build
    /// could not determine.
    ///
    /// **Not a no and not a yes.** A host that read this as "not remotable"
    /// would silently force co-location for a program with an unrelated typing
    /// gap; one that read it as remotable would encode a guess.
    Undetermined { positions: Vec<Untransferable> },
}

impl RemoteSupport {
    pub fn is_transferable(&self) -> bool {
        matches!(self, RemoteSupport::Transferable)
    }
}

/// One position in a signature that cannot cross, and why.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Untransferable {
    /// `argument 0`, `result`, `error`. Named rather than indexed, because a
    /// host reporting "position 2" of a signature it cannot see is unactionable.
    pub position: String,
    /// The type at that position, where this build knows it.
    pub ty: Option<String>,
    /// What is wrong with it, in one phrase.
    pub reason: String,
}

/// **The positions of one interface edge**, as its declaration writes them.
///
/// Built from a `Decl` rather than from `Signatures`, and that is deliberate:
/// `Signatures` covers `fn`, `query`, `command`, `subscription`, `resource` and
/// `task`, and a `page`, `view` or `component` is not among them. Reading it
/// meant every page export fell through a lookup to the permissive default and
/// came out `Transferable` — the right answer for the store demo, arrived at by
/// a mechanism with nothing to do with its types, which is the shape
/// `docs/RISK_QUEUE.md` calls coincidental correctness. Found by probing the
/// emitted contracts rather than by a test, because every test passed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Interface {
    /// Each parameter's declared type, `None` where it carries no annotation.
    ///
    /// `None` is undetermined and not absent: the parameter exists and
    /// something crosses at that position, and this build cannot say what.
    pub params: Vec<Option<String>>,
    /// The declared return type's head, or `None` when nothing is returned.
    ///
    /// **Absent, not undetermined.** `page StorePage(id: StoreId)` returns no
    /// value, so no value crosses outbound and there is nothing to decide.
    /// Collapsing this into the parameter case would mark every page
    /// undetermined and every `fn` with no `->` too.
    pub returns: Option<String>,
    /// `Result<Store, StoreError>` gives `["Store", "StoreError"]`.
    pub returns_args: Vec<String>,
}

impl Interface {
    /// From a declaration — the form every component kind has.
    ///
    /// The parameter type is reassembled from the head and its arguments, which
    /// the HIR carries separately. Reading only `p.ty` gave `List` for
    /// `List<MenuItem>` — which emitted `list` with nothing in it, and, worse,
    /// looked up `List` rather than `MenuItem` when asking whether a position
    /// carries a resource. `wit-parser` caught the first; the second was
    /// invisible.
    pub fn of(decl: &crate::hir::Decl) -> Interface {
        Interface {
            params: decl
                .params
                .iter()
                .map(|p| {
                    p.ty.as_ref()
                        .map(|head| crate::wit::written(head, &p.ty_args))
                })
                .collect(),
            returns: decl.ret.clone(),
            returns_args: decl.ret_args.clone(),
        }
    }
}

impl From<&Signature> for Interface {
    fn from(sig: &Signature) -> Interface {
        Interface {
            params: sig.params.clone(),
            returns: sig.returns.clone(),
            returns_args: sig.returns_args.clone(),
        }
    }
}

/// **Can this interface be called across a process boundary?**
///
/// Every position, in both directions. An interface that cannot RETURN a value
/// across a boundary cannot be called across it either, and a rule that checked
/// only arguments would call `with_transaction(..) -> OpenTransaction`
/// remotable.
///
/// The label passed for each position is `Public`: an interface says what TYPES
/// cross, and which values flow through it is the caller's dataflow, not the
/// edge's. A type's own scope still counts — `TransferProfile::produced_scope`
/// carries it — so `Cart` is session-scoped here even though no particular
/// value is in evidence.
///
/// **The destination is `None`, and that is the point.** Architect ruling,
/// 2026-08-08:
///
/// > World alone is not enough to establish privacy. Both `Session<A>` and
/// > `Session<B>` may be permitted to exist in `Browser`, `Edge` or `Origin`.
/// > But `Session<A> → Session<B>` must still be forbidden. […] If the planner
/// > cannot establish the destination privacy scope, the transfer should be
/// > Blocked, not assumed valid.
///
/// A build has no deployment in evidence, so it cannot say where a remote edge
/// lands. A signature carrying a restricted type therefore comes back
/// [`RemoteSupport::Undetermined`] rather than `Transferable` — the honest
/// answer, and one a host that CAN establish the far side may narrow.
pub fn remote_support(sig: &Interface, facts: &TypeFacts) -> RemoteSupport {
    let mut refused: Vec<Untransferable> = Vec::new();
    let mut undetermined: Vec<Untransferable> = Vec::new();

    let mut consider = |position: String, ty: Option<&str>, direction: Direction| {
        let profile = facts.profile(ty);
        let verdict = can_cross(
            &profile,
            &BoundaryContext {
                boundary: Boundary::RemoteCall,
                direction,
                label: Label::public(),
                // Unknowable at build time. See the doc comment above.
                destination: None,
            },
        );
        match verdict {
            Crossing::Proven => {}
            Crossing::Violation(Violation::Resource { ty, producer }) => {
                refused.push(Untransferable {
                    position,
                    ty: Some(ty),
                    reason: format!(
                        "`{producer}` acquires it as a resource, so the value IS the \
                         thing held open and cannot be reconstructed elsewhere"
                    ),
                })
            }
            Crossing::Violation(Violation::Private { restriction }) => {
                refused.push(Untransferable {
                    position,
                    ty: ty.map(str::to_string),
                    reason: format!("it carries a {restriction} restriction"),
                })
            }
            Crossing::Blocked(Blocked::UndeterminedSchema) => undetermined.push(Untransferable {
                position,
                ty: None,
                reason: "this build could not determine the type, so it has no \
                             wire schema"
                    .to_string(),
            }),
            // Restricted, and a build cannot say where the far end is. Not a
            // refusal — the edge may be perfectly fine between two nodes of one
            // scope — and not a yes, which is what assuming would make it.
            Crossing::Blocked(Blocked::UnknownDestination { carries }) => {
                undetermined.push(Untransferable {
                    position,
                    ty: ty.map(str::to_string),
                    reason: format!(
                        "it carries {} and this build cannot establish the \
                         destination's privacy scope",
                        carries
                            .iter()
                            .map(|r| r.to_string())
                            .collect::<Vec<_>>()
                            .join(" and ")
                    ),
                })
            }
        }
    };

    for (i, p) in sig.params.iter().enumerate() {
        consider(format!("argument {i}"), p.as_deref(), Direction::Inbound);
    }

    // The result and the error are separate positions, because they are
    // separate types and a signature can be remotable in one and not the other:
    // `Result<StoreId, OpenTransaction>` returns a key on success and a handle
    // on failure, and only checking the success side would call it remotable.
    match sig.returns.as_deref() {
        // Nothing is returned, so nothing crosses outbound. A `page` is the
        // ordinary case; a `command` with no result is another. NOT the
        // undetermined case — there is no position here to be undetermined
        // about, and treating it as one marked every page in the corpus as an
        // edge the compiler could not decide.
        None => {}
        // `Result<A, E>` and `Option<A>` are carriers. What crosses is what
        // they carry, so the head is not itself a position — checking it would
        // ask whether `Result` is transferable, which is not a question.
        Some("Result") | Some("Option") | Some("List") => {
            let names = ["result", "error"];
            for (i, arg) in sig.returns_args.iter().enumerate() {
                consider(
                    names.get(i).copied().unwrap_or("result").to_string(),
                    Some(arg),
                    Direction::Outbound,
                );
            }
            // A carrier with no arguments recorded says nothing about what it
            // carries, which IS undetermined: `Result` alone does not name a
            // type, and something does cross at this position.
            if sig.returns_args.is_empty() {
                consider("result".to_string(), None, Direction::Outbound);
            }
        }
        other => consider("result".to_string(), other, Direction::Outbound),
    }

    if !refused.is_empty() {
        return RemoteSupport::Refused { positions: refused };
    }
    if !undetermined.is_empty() {
        return RemoteSupport::Undetermined {
            positions: undetermined,
        };
    }
    RemoteSupport::Transferable
}

/// The binding support of one exported declaration.
pub fn binding_support(decl: &crate::hir::Decl, facts: &TypeFacts) -> BindingSupport {
    BindingSupport {
        local: LocalSupport::Direct,
        remote: remote_support(&Interface::of(decl), facts),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower_file;
    use crate::resolve::Workspace;
    use crate::signatures::Signatures;
    use pw_syntax::parse_tree;

    const PROGRAM: &str = "\
module shop

type Store = Store { id: Int, name: String }
type StoreError = StoreError { why: String }
opaque type StoreId = String
opaque type OpenTransaction = String
type Cart = Cart { line_count: Int }
type CartError = CartError { why: String }
opaque type SessionId = String

fn get(id: StoreId) -> Result<Store, StoreError> { todo }

fn begin() -> OpenTransaction !{ resource.acquire<OpenTransaction> } { todo }

fn commit(t: OpenTransaction) -> Result<Store, StoreError> { todo }

fn recover() -> Result<StoreId, OpenTransaction> { todo }

session query Basket(s: SessionId) -> Result<Cart, CartError>
    cache private
{
    todo
}

fn read_basket(s: SessionId) -> Result<Cart, CartError> { todo }

page Landing(id: StoreId) {
    placement origin
    view { <p>hello</p> }
}
";

    /// The support of one DECLARATION, which is the path `contracts()` takes.
    ///
    /// It went through `Signatures` and looked up by `DefId`, which covers
    /// `fn`, `query`, `command`, `subscription`, `resource` and `task` — and
    /// not `page`, `view` or `component`. Every page export therefore missed
    /// the lookup and took the permissive default. `a_page_returns_nothing_..`
    /// is what would have caught it.
    fn support_in(src: &str, name: &str) -> RemoteSupport {
        let hir = lower_file(src, &parse_tree(src).green);
        let hirs = vec![&hir];
        let ws = Workspace::build(&hirs);
        let sigs = Signatures::build(&ws, &hirs);
        let facts = TypeFacts::build(&hirs, &sigs);
        let decl = hir
            .all_decls()
            .find(|(_, d)| d.name == name)
            .map(|(_, d)| d)
            .unwrap_or_else(|| panic!("no declaration named {name}"));
        remote_support(&Interface::of(decl), &facts)
    }

    fn support_of(name: &str) -> RemoteSupport {
        support_in(PROGRAM, name)
    }

    #[test]
    fn an_ordinary_query_is_remotable() {
        // The control. If every signature were refused, every assertion below
        // would pass for a reason unrelated to what it names.
        assert_eq!(support_of("get"), RemoteSupport::Transferable);
    }

    #[test]
    fn a_resource_in_an_argument_refuses_the_edge() {
        // `commit(t: OpenTransaction)`. The handle names something held open
        // here; encoding it produces a well-formed value on the other side that
        // refers to nothing.
        let RemoteSupport::Refused { positions } = support_of("commit") else {
            panic!("{:?}", support_of("commit"));
        };
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].position, "argument 0");
        assert_eq!(positions[0].ty.as_deref(), Some("OpenTransaction"));
    }

    #[test]
    fn a_resource_in_a_result_refuses_it_too() {
        // The direction a rule written for arguments alone would miss.
        // `begin() -> OpenTransaction` takes nothing at all.
        let RemoteSupport::Refused { positions } = support_of("begin") else {
            panic!("{:?}", support_of("begin"));
        };
        assert_eq!(positions[0].position, "result");
    }

    #[test]
    fn the_error_side_is_a_position_of_its_own() {
        // `recover() -> Result<StoreId, OpenTransaction>` succeeds with a key
        // and fails with a handle. A check that looked only at the success side
        // would call this remotable, and the failure path is exactly when a
        // caller most needs the value to mean something.
        let RemoteSupport::Refused { positions } = support_of("recover") else {
            panic!("{:?}", support_of("recover"));
        };
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].position, "error");
        assert_eq!(positions[0].ty.as_deref(), Some("OpenTransaction"));
    }

    #[test]
    fn a_session_scoped_type_is_undetermined_rather_than_remotable() {
        // **This asserted `Transferable` until 2026-08-08.** Architect ruling:
        //
        // > World alone is not enough to establish privacy. Both `Session<A>`
        // > and `Session<B>` may be permitted to exist in `Browser`, `Edge` or
        // > `Origin`. But `Session<A> → Session<B>` must still be forbidden.
        // > […] If the planner cannot establish the destination privacy scope,
        // > the transfer should be Blocked, not assumed valid.
        //
        // The old reasoning was that placement already decides privacy at a
        // remote boundary. It does not: placement decides which WORLD, and two
        // different sessions live in the same world. A build has no deployment
        // in evidence, so the honest answer is that it cannot yet tell.
        let support = support_of("read_basket");
        let RemoteSupport::Undetermined { positions } = &support else {
            panic!("{support:?}");
        };
        assert_eq!(positions[0].position, "result");
        assert!(
            positions[0].reason.contains("Session"),
            "and it names the restriction it carries: {}",
            positions[0].reason
        );
    }

    #[test]
    fn an_undetermined_position_is_neither_refused_nor_transferable() {
        // The third answer. A host reading it as "not remotable" would force
        // co-location for an unrelated typing gap; reading it as remotable
        // would encode a guess.
        assert!(matches!(
            support_in("module m\n\nfn f(x) -> Int { 0 }\n", "f"),
            RemoteSupport::Undetermined { .. }
        ));
    }

    #[test]
    fn a_page_returns_nothing_and_that_is_not_undetermined() {
        // **The distinction a probe of the emitted contracts found, not a
        // test.** `page Landing(id: StoreId)` declares no return type, and
        // "nothing crosses outbound" is a different fact from "this build could
        // not determine what crosses". Reading `returns: None` as undetermined
        // would mark every page in the corpus undecidable.
        //
        // The other half of the same defect: this used to be looked up through
        // `Signatures`, which has no entry for a `page` at all, so the answer
        // came from `unwrap_or_default()` rather than from `StoreId`.
        assert_eq!(support_of("Landing"), RemoteSupport::Transferable);

        // ...and the parameter is genuinely read, so `Transferable` is a
        // verdict about `StoreId` and not the shape of an empty analysis.
        assert!(matches!(
            support_in(
                "module m\n\nopaque type T = String\n\nfn f() -> T !{ resource.acquire<T> } { todo }\n\npage P(t: T) {\n    view { <p>x</p> }\n}\n",
                "P"
            ),
            RemoteSupport::Refused { .. }
        ));
    }

    #[test]
    fn local_is_always_direct_and_remote_is_not() {
        // The reason `BindingSupport` is a struct. `commit` is callable in one
        // address space and not across a boundary, and an enum would have to
        // choose one word for both facts.
        let hir = lower_file(PROGRAM, &parse_tree(PROGRAM).green);
        let hirs = vec![&hir];
        let ws = Workspace::build(&hirs);
        let sigs = Signatures::build(&ws, &hirs);
        let facts = TypeFacts::build(&hirs, &sigs);
        let decl = hir
            .all_decls()
            .find(|(_, d)| d.name == "commit")
            .map(|(_, d)| d)
            .expect("a declaration");
        let support = binding_support(decl, &facts);
        assert_eq!(support.local, LocalSupport::Direct);
        assert!(!support.remote.is_transferable());
    }
}
