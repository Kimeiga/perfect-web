//! E8 — what an effect **is**, as a declaration rather than a spelling.
//!
//! Effect ontology slice 2. Slice 1 gave `effect database.read<T> { .. }` a
//! grammar, a `DeclKind` and a namespace. This resolves a written effect
//! against those declarations, and it is the first production consumer of
//! ADR-0022's semantic provenance.
//!
//! Architect ruling, 2026-08-07:
//!
//! > So slice 2 becomes both: completion of the effect ontology; and the first
//! > production implementation of semantic provenance.
//!
//! # The pipeline
//!
//! ```text
//! database.read<Stores>
//!   → EffectPath(["database","read"]) + TypeArg("Stores")
//!   → EffectDefId + TypeDefId
//!   → EffectInstance { effect, args }
//! ```
//!
//! Everything downstream consumes [`EffectInstance`]. **No checker splits
//! `"database.read"` at the dot** — the dot is read exactly twice, both times
//! here: once when a declaration's name is turned into an [`EffectPath`], and
//! once when a written row entry is. After that there is a resolved identity,
//! and an identity does not need re-parsing.
//!
//! # Why a family is derived and never invented
//!
//! `!{ database }` is a legitimate row: charter §7.5A's rule is that declaring
//! a family accepts its members, so a broad claim is broader than any one
//! operation. But `database` is not itself an effect anybody performs.
//!
//! So a family EXISTS because operations were declared into it. There is no
//! `effect database` and no list of valid family spellings in a checker —
//! `databse.read` is unknown because nothing declared anything under `databse`,
//! which is a fact about the program rather than about a table someone
//! maintained. That is the property `docs/RISK_QUEUE.md` 37 cost us when
//! `LayoutAffect` named nothing in five files.
//!
//! # What this module does NOT do
//!
//! It does not decide whether an effect is permitted, where it may run, or what
//! authority it needs. Those are `effects.rs`, `placement.rs` and `contract.rs`,
//! and each of them is a policy over this answer. This module answers only
//! *which declaration did you write*.

use std::collections::{BTreeMap, BTreeSet};

use crate::hir::{Decl, DeclKind, EffectRef, Hir, Span};
use crate::placement::{ALL_WORLDS, World};
use crate::provenance::{Evidence, FactId, FactKind, Route};
use crate::resolve::{DefId, Namespace, Resolution, Workspace};

/// A declared effect's identity.
///
/// A newtype over [`DefId`] rather than a bare one, so an effect cannot be
/// passed where a type or a function is expected. `docs/RISK_QUEUE.md` 34 is
/// what one identity standing in for another costs, and the compiler now has
/// enough different `DefId`s in flight that the distinction is worth a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectDefId(pub DefId);

/// An effect's name, in segments.
///
/// Built at the boundary — from a declaration's name, or from a row entry's
/// written path — and never again. A checker holding one of these has already
/// been told what the segments are and has no reason to look at a dot.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectPath(Vec<String>);

impl EffectPath {
    /// Read a written path. **The only place the dot is interpreted.**
    pub fn parse(written: &str) -> EffectPath {
        EffectPath(written.split('.').map(str::to_string).collect())
    }

    /// The first segment: `database` in `database.read`.
    ///
    /// A family is the first segment and nothing else. `a.b.c` has family `a`,
    /// which is a decision rather than an accident: the corpus's rule is that
    /// declaring a family accepts its members, and nesting families would make
    /// "which family covers this" a search instead of a lookup.
    pub fn family(&self) -> &str {
        self.0.first().map(String::as_str).unwrap_or_default()
    }

    /// Everything after the family: `read` in `database.read`. Empty when the
    /// effect names only a family, as `log` and `trace` do.
    pub fn operation(&self) -> String {
        self.0[1..].join(".")
    }

    pub fn segments(&self) -> &[String] {
        &self.0
    }

    /// The canonical spelling. Round-trips with [`EffectPath::parse`].
    pub fn text(&self) -> String {
        self.0.join(".")
    }
}

/// What one type argument turned out to name.
///
/// Four answers, not two, because the difference between them is the whole
/// value of asking. Modules count, and the reason is in `capability.rs`: the
/// corpus writes `database.read<Stores>` and `Stores` is a MODULE — the domain
/// being read. A first version of the capability-argument rule accepted only
/// types and reported two working corpus files as defective.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeArgument {
    /// A `type` or `opaque type`, resolved through the workspace from where
    /// the row was written. The only one carrying an identity.
    Type { written: String, def: DefId },
    /// A module this unit can see. An external contract, exactly as an opaque
    /// type is, and one with no `DefId` because a module is not a declaration.
    Module { written: String },
    /// The program declares it and this unit does not import it.
    ///
    /// **Accepted, and recorded as the compromise it is.** `PW5200` has always
    /// judged an argument against every declaration in the program, and
    /// `examples/generality/value_exceeds_sink_level/branch-join.pw` writes
    /// `secret<Payments>` while importing neither `capability` nor `Payments`.
    /// Tightening this to full visibility would newly reject a working corpus
    /// file, which is a decision with a diff and not a side effect of building
    /// the ontology. Named so that decision is visible rather than absent —
    /// assumption A-009's ambient union, surviving in exactly one place with a
    /// name on it.
    Unscoped { written: String },
    /// Nothing in this program declares it. Kept rather than dropped — the
    /// authority the program asked for is not erased by being unresolvable,
    /// and `PW5200` is what makes the mistake visible where it can be repaired.
    Unresolved { written: String },
}

impl TypeArgument {
    pub fn written(&self) -> &str {
        match self {
            TypeArgument::Type { written, .. }
            | TypeArgument::Module { written }
            | TypeArgument::Unscoped { written }
            | TypeArgument::Unresolved { written } => written,
        }
    }

    /// Does it name something the program declares? The question `PW5200` asks.
    pub fn is_resolved(&self) -> bool {
        !matches!(self, TypeArgument::Unresolved { .. })
    }

    /// Was it reached from where it was written, rather than from the program
    /// at large?
    pub fn is_in_scope(&self) -> bool {
        matches!(
            self,
            TypeArgument::Type { .. } | TypeArgument::Module { .. }
        )
    }
}

/// **A resolved effect.** What every analysis downstream consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectInstance {
    pub effect: EffectDefId,
    pub args: Vec<TypeArgument>,
    /// The row entry this came from, for a diagnostic that underlines it.
    pub span: Span,
}

/// What a row entry resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// One declared operation: `database.read<Stores>`.
    Operation(EffectInstance),
    /// A whole family: `!{ database }`, which covers every operation declared
    /// under it. Not an effect anything performs — a claim about a row.
    Family { name: String, span: Span },
}

/// Why a written effect resolved to nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectError {
    /// Nothing is declared under this first segment. `databse.read`.
    UnknownFamily {
        family: String,
        /// The nearest declared family, when one is close enough to be a typo.
        nearest: Option<String>,
        span: Span,
    },
    /// The family exists; this operation is not one of its members.
    /// `database.reed`.
    UnknownOperation {
        family: String,
        operation: String,
        /// Every operation the family DOES declare, so the message can list
        /// them rather than only reject.
        declared: Vec<String>,
        nearest: Option<String>,
        span: Span,
    },
    /// The effect exists and binds a different number of type parameters.
    /// `database.read` with none, `database.read<A, B>` with two.
    WrongArity {
        effect: String,
        expected: usize,
        found: usize,
        span: Span,
    },
}

impl EffectError {
    pub fn span(&self) -> &Span {
        match self {
            EffectError::UnknownFamily { span, .. }
            | EffectError::UnknownOperation { span, .. }
            | EffectError::WrongArity { span, .. } => span,
        }
    }
}

/// One `effect` declaration, as the program wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectDecl {
    pub def: EffectDefId,
    pub path: EffectPath,
    /// How many type parameters it binds. `effect log` binds none;
    /// `effect database.read<T>` binds one.
    pub arity: usize,
    /// The `capability` clause, as written. `None` for `capability none` — an
    /// effect that needs no host authority, which is a real and common case
    /// and not a missing declaration.
    pub capability: Option<String>,
    /// The `host` clause: `pw:host/database#read`.
    ///
    /// Where `interface_for` will read from instead of formatting
    /// `pw:host/{family}` — see `docs/NEXT.md`. `None` means this effect names
    /// no host interface, which is what `capability none` implies and what a
    /// purely local effect wants.
    pub host: Option<String>,
    /// The `placement` clause: where this effect is *meaningful*, as distinct
    /// from where it is *authorised*.
    ///
    /// Architect ruling, 2026-08-07:
    ///
    /// > If an effect has `capability none` we still need to know that
    /// > `layout.measure` is browser-only. […] For browser-semantic effects
    /// > with no capability, the declaration itself supplies the placement
    /// > restriction.
    ///
    /// This is the fact `World::worlds_for` holds today as a hard-coded table,
    /// and the reason its `dom`/`style`/`layout` entries were read as "needs a
    /// host capability" when they only ever meant "only means anything in a
    /// browser". Empty means unconstrained — an effect that can happen
    /// anywhere, which is the right default because a checker must not reject
    /// what it has not been taught.
    pub placement: Vec<World>,
    pub span: Span,
}

/// **Every effect the program declares.**
#[derive(Debug, Clone, Default)]
pub struct Ontology {
    by_name: BTreeMap<String, EffectDecl>,
    /// First segment → the operations declared under it, in sorted order.
    ///
    /// Derived from the declarations, never written down. A family exists
    /// because something was declared into it.
    families: BTreeMap<String, BTreeSet<String>>,
    /// What a type argument may name — declared types, opaque types, modules.
    ///
    /// A SET, not a name→declaration map. The set answers "does the program
    /// declare this at all", which is `PW5200`'s question; *which* declaration
    /// is asked of the workspace from the unit that wrote the row, because a
    /// name→`DefId` map here would be `docs/RISK_QUEUE.md` 34 rebuilt.
    argument_names: BTreeSet<String>,
    /// Effect names declared more than once in the checked set.
    ///
    /// Detected rather than absorbed. `by_name` keeps one declaration per
    /// spelling by design — the effect vocabulary is one flat namespace, which
    /// is what makes `database.read` mean the same thing in every row — so a
    /// second declaration of one name is a conflict somebody has to resolve
    /// and not a map entry that quietly wins.
    conflicts: BTreeMap<String, Vec<EffectDefId>>,
}

impl Ontology {
    /// Collect every `effect` declaration in the program.
    pub fn build(hirs: &[&Hir]) -> Ontology {
        let mut out = Ontology {
            argument_names: crate::capability::capability_argument_names(hirs),
            ..Ontology::default()
        };

        for (unit, hir) in hirs.iter().enumerate() {
            for (id, decl) in hir.all_decls() {
                if decl.kind != DeclKind::Effect || decl.name.is_empty() {
                    continue;
                }
                let def = EffectDefId(DefId { unit, decl: id.0 });
                let path = EffectPath::parse(&decl.name);
                out.families
                    .entry(path.family().to_string())
                    .or_default()
                    .insert(path.operation());
                if let Some(first) = out.by_name.get(&decl.name) {
                    out.conflicts
                        .entry(decl.name.clone())
                        .or_insert_with(|| vec![first.def])
                        .push(def);
                    continue;
                }
                out.by_name.insert(
                    decl.name.clone(),
                    EffectDecl {
                        def,
                        path,
                        arity: decl.type_params.len(),
                        capability: capability_clause(decl),
                        host: host_clause(decl),
                        placement: placement_clause(decl),
                        span: hir.decl_span(id),
                    },
                );
            }
        }
        out
    }

    /// Effect names two declarations claim. Empty in a well-formed program.
    pub fn conflicts(&self) -> &BTreeMap<String, Vec<EffectDefId>> {
        &self.conflicts
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    pub fn declarations(&self) -> impl Iterator<Item = &EffectDecl> {
        self.by_name.values()
    }

    pub fn get(&self, name: &str) -> Option<&EffectDecl> {
        self.by_name.get(name)
    }

    /// The declaration an instance names.
    pub fn declaration(&self, of: EffectDefId) -> Option<&EffectDecl> {
        self.by_name.values().find(|d| d.def == of)
    }

    /// Is `name` a family something was declared into?
    pub fn has_family(&self, name: &str) -> bool {
        self.families.contains_key(name)
    }

    /// **Resolve one written row entry.**
    ///
    /// The whole pipeline, in one place, recording an edge per transition.
    ///
    /// `unit` is where the row was written. It decides visibility: an effect
    /// declared in a module this file does not import is not in scope, exactly
    /// as a type is not. `usize::MAX` means "no unit", and then only the
    /// ontology's own membership answers — which is what a caller holding a
    /// row and no workspace position honestly has.
    pub fn resolve(
        &self,
        ws: &Workspace,
        unit: usize,
        entry: &EffectRef,
        evidence: &mut Evidence,
    ) -> Result<Resolved, EffectError> {
        let path = EffectPath::parse(&entry.path);

        // A bare family: `!{ database }`. Legitimate, and not an operation.
        if path.operation().is_empty() && !self.by_name.contains_key(&entry.path) {
            if self.families.contains_key(path.family()) {
                return Ok(Resolved::Family {
                    name: path.family().to_string(),
                    span: entry.span.clone(),
                });
            }
            return Err(EffectError::UnknownFamily {
                family: path.family().to_string(),
                nearest: nearest(path.family(), self.families.keys().cloned()),
                span: entry.span.clone(),
            });
        }

        let Some(decl) = self.by_name.get(&entry.path) else {
            // Which HALF is wrong decides the message. The family is checked
            // first because "no such family" and "that family has no such
            // operation" send a reader to two different places, and a rule
            // that said only "unknown effect" would send them to neither.
            let Some(operations) = self.families.get(path.family()) else {
                return Err(EffectError::UnknownFamily {
                    family: path.family().to_string(),
                    nearest: nearest(path.family(), self.families.keys().cloned()),
                    span: entry.span.clone(),
                });
            };
            let operation = path.operation();
            return Err(EffectError::UnknownOperation {
                nearest: nearest(&operation, operations.iter().cloned()),
                family: path.family().to_string(),
                operation,
                declared: operations.iter().cloned().collect(),
                span: entry.span.clone(),
            });
        };

        // Visibility, if the caller said where it stands. An effect declared
        // in a module this file neither owns nor imports is out of scope, and
        // reporting it as unknown is the same answer resolution gives a type.
        //
        // BEFORE the fact is recorded, and the order is load-bearing. Recorded
        // first, a resolution that then failed would leave an edge behind
        // saying it succeeded — and every `must contain` assertion written
        // against this graph would be satisfied by a compiler that resolved
        // nothing. ADR-0022: provenance RECORDS what the analysis concluded,
        // and until this line the analysis has concluded nothing.
        if unit != usize::MAX && !self.visible(ws, unit, &entry.path, decl) {
            return Err(EffectError::UnknownFamily {
                family: path.family().to_string(),
                nearest: nearest(path.family(), self.families.keys().cloned()),
                span: entry.span.clone(),
            });
        }

        // How the name was reached. `Qualified` because an effect name is a
        // path against declarations, resolved whole — never by matching a
        // final segment, which is the route ADR-0022 exists to forbid.
        let resolved = evidence.record(
            FactKind::ResolvedEffect {
                path: entry.path.clone(),
                to: decl.def,
                via: Route::Qualified,
            },
            entry.span.clone(),
            vec![],
        );

        if entry.args.len() != decl.arity {
            return Err(EffectError::WrongArity {
                effect: entry.path.clone(),
                expected: decl.arity,
                found: entry.args.len(),
                span: entry.span.clone(),
            });
        }

        let args = entry
            .args
            .iter()
            .map(|a| self.argument(ws, unit, a, &entry.span, resolved, evidence))
            .collect();

        Ok(Resolved::Operation(EffectInstance {
            effect: decl.def,
            args,
            span: entry.span.clone(),
        }))
    }

    /// One type argument, resolved and recorded.
    ///
    /// Through the WORKSPACE first, from the unit that wrote the row, so a
    /// resolved argument carries the identity of the declaration this file can
    /// actually see. The program-wide set is consulted only to distinguish
    /// [`TypeArgument::Unscoped`] from [`TypeArgument::Unresolved`], which is
    /// a distinction and not a fallback: neither answer becomes a `DefId`.
    fn argument(
        &self,
        ws: &Workspace,
        unit: usize,
        written: &str,
        span: &Span,
        cause: FactId,
        evidence: &mut Evidence,
    ) -> TypeArgument {
        let in_scope = (unit != usize::MAX)
            .then(|| match ws.resolve_in(unit, Namespace::Type, written) {
                Resolution::Local(def) | Resolution::Imported { def, .. } => {
                    Some(TypeArgument::Type {
                        written: written.to_string(),
                        def,
                    })
                }
                _ if ws.sees_module(unit, written) => Some(TypeArgument::Module {
                    written: written.to_string(),
                }),
                _ => None,
            })
            .flatten();

        let arg = match in_scope {
            Some(a) => a,
            None if self.argument_names.contains(written) => TypeArgument::Unscoped {
                written: written.to_string(),
            },
            None => TypeArgument::Unresolved {
                written: written.to_string(),
            },
        };
        // Recorded either way. That an argument did NOT resolve is a fact
        // about the program, and a graph that only held successes could not
        // distinguish "checked and found" from "never looked".
        evidence.record(
            FactKind::ResolvedTypeArgument {
                written: written.to_string(),
                resolved: arg.is_resolved(),
            },
            span.clone(),
            vec![cause],
        );
        arg
    }

    /// Can `unit` see this effect declaration?
    ///
    /// Through the workspace, in the effect namespace. Not by membership in
    /// the ontology: the ontology holds every effect in the checked set, and
    /// "this program declares it somewhere" is the ambient union assumption
    /// A-009 removed.
    fn visible(&self, ws: &Workspace, unit: usize, name: &str, decl: &EffectDecl) -> bool {
        // The effect's own module always sees it, and so does a module that
        // imports it by name or wholesale.
        if let Resolution::Local(def) | Resolution::Imported { def, .. } =
            ws.resolve_in(unit, Namespace::Effect, name)
        {
            return EffectDefId(def) == decl.def;
        }
        // A workspace with no module for this unit answers nothing — a
        // synthetic body, or a caller checking one file in isolation. The
        // ontology's own membership is then the honest answer.
        ws.module_of(unit).is_none()
    }
}

/// `capability database.read<T>` → `Some("database.read<T>")`;
/// `capability none` → `None`.
///
/// `none` is a real answer, not a missing one. Charter §7.5A's `log` and
/// `trace` perform an effect and require no host authority, and an ontology
/// that could not say so would force every effect to invent a capability.
fn capability_clause(decl: &Decl) -> Option<String> {
    let value = decl.policy("capability")?.value.trim();
    (value != "none" && !value.is_empty()).then(|| value.to_string())
}

/// `host "pw:host/database#read"` → `Some("pw:host/database#read")`.
///
/// A string literal in the source, because a WIT interface name is not an
/// expression — see `docs/NEXT.md`. The quotes are the literal's, not part of
/// the name.
fn host_clause(decl: &Decl) -> Option<String> {
    let value = decl.policy("host")?.value.trim();
    let value = value.trim_matches('"');
    (!value.is_empty()).then(|| value.to_string())
}

/// `placement browser` → `[Browser]`; `placement browser, edge, origin` →
/// three.
///
/// A LIST, because `network.fetch` is meaningful anywhere with a request
/// context and only build time has none. An enum would have forced either a
/// fourth world named "anywhere with a request" or a table somewhere else
/// saying which set each keyword abbreviates, and the second is what
/// `worlds_for` already is.
///
/// A word that names no world is dropped rather than guessed at. The
/// diagnostic for it belongs with the other `PW52xx` unknown-name rules and
/// is step 5; silently defaulting to "everywhere" here would make a typo into
/// a widening, which is the wrong direction for a placement constraint.
fn placement_clause(decl: &Decl) -> Vec<World> {
    let Some(p) = decl.policy("placement") else {
        return Vec::new();
    };
    p.value
        .split(',')
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .filter_map(|w| ALL_WORLDS.iter().copied().find(|world| world.name() == w))
        .collect()
}

/// The closest declared spelling, when one is close enough to be a typo.
///
/// The same bound `capability.rs` uses, and for the same reason: a suggestion
/// that is merely the alphabetically first candidate sends the reader to an
/// unrelated declaration, which is worse than no suggestion.
fn nearest(written: &str, candidates: impl Iterator<Item = String>) -> Option<String> {
    let budget = (written.len() / 3).max(1);
    candidates
        .map(|c| (distance(written, &c), c))
        .filter(|(d, _)| *d <= budget && *d > 0)
        .min_by_key(|(d, c)| (*d, c.len()))
        .map(|(_, c)| c)
}

fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut row = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            row[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1);
        }
        std::mem::swap(&mut prev, &mut row);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_read_once_and_then_asked() {
        // The property the module exists for. After `parse`, the segments are
        // answers rather than something to re-derive.
        let p = EffectPath::parse("database.read");
        assert_eq!(p.family(), "database");
        assert_eq!(p.operation(), "read");
        assert_eq!(p.text(), "database.read");

        // A family-only effect has no operation, and that is not an error.
        let log = EffectPath::parse("log");
        assert_eq!(log.family(), "log");
        assert_eq!(log.operation(), "");
    }

    #[test]
    fn capability_none_is_an_answer_and_not_a_missing_clause() {
        let with = Decl {
            policies: vec![crate::hir::Policy {
                name: "capability".into(),
                value: "database.read<T>".into(),
                span: 0..0,
            }],
            ..bare()
        };
        assert_eq!(
            capability_clause(&with).as_deref(),
            Some("database.read<T>")
        );

        let without = Decl {
            policies: vec![crate::hir::Policy {
                name: "capability".into(),
                value: "none".into(),
                span: 0..0,
            }],
            ..bare()
        };
        assert_eq!(capability_clause(&without), None);
        // Which is NOT the same as never having written the clause — but both
        // mean "no host authority", so they agree here on purpose.
        assert_eq!(capability_clause(&bare()), None);
    }

    #[test]
    fn a_host_clause_loses_its_quotes_and_nothing_else() {
        let d = Decl {
            policies: vec![crate::hir::Policy {
                name: "host".into(),
                value: "\"pw:host/database#read\"".into(),
                span: 0..0,
            }],
            ..bare()
        };
        assert_eq!(host_clause(&d).as_deref(), Some("pw:host/database#read"));
    }

    fn bare() -> Decl {
        Decl {
            name: "database.read".into(),
            name_span: 0..0,
            kind: DeclKind::Effect,
            params: vec![],
            ret: None,
            ret_args: vec![],
            variants: None,
            fields: None,
            opaque_of: None,
            type_params: vec![],
            policies: vec![],
            imports: vec![],
            visibility: None,
            declared_effects: None,
            body: None,
            children: vec![],
        }
    }
}
