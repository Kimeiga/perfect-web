//! **A type that contains itself** (ADR-0194).
//!
//! A declaration may hold a value of its own type: a comment its replies, a
//! folder its folders, a JSON value its elements. Three questions are asked of
//! one, and each is answered here, once, for every reader:
//!
//! - **Does any finite value have it?** A value is built from values that
//!   exist already, so a type each of whose values holds another of itself,
//!   `type Loop = Loop { again: Loop }`, has none. [`check`] refuses it where
//!   it is declared (PW0624).
//! - **Is it held in place?** A list's elements are where the list points, so
//!   `replies: List<Comment>` has a layout by its type. `next: Option<Node>`
//!   holds a `Node` within a `Node`, which no layout by type has
//!   ([`contains_itself_in_place`]). The backend boxes such a type's values
//!   (ADR-0202): each is the address of its cell.
//! - **Which of its fields hold it?** [`self_slots`]: a type that crosses a
//!   component boundary crosses as its nodes, and each field that holds the
//!   type holds node indices there: a list of them for a `List` of it, one
//!   for the type itself, and an optional one for an `Option` of it.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::{Detector, Diagnostic, Related, Repair, Severity};
use crate::hir::{DeclKind, Hir};
use crate::resolve::DefId;
use crate::resolved::{Builtin, ResolvedType, TypeResolution};
use crate::signatures::{Signatures, TypeDecl};

/// A declaration's parts: its fields, its cases' fields, its representation.
pub(crate) fn parts(t: &TypeDecl) -> impl Iterator<Item = &TypeResolution> {
    t.record
        .iter()
        .flatten()
        .map(|(_, r)| r)
        .chain(t.representation.iter())
        .chain(t.variants.iter().flatten().flat_map(|(_, fs)| fs.iter()))
}

/// **Does a declared type hold a value of its own type**, through its
/// fields, its cases or its representation, at any depth (ADR-0059)?
pub fn contains_itself(sigs: &Signatures, def: DefId) -> bool {
    fn reaches(
        sigs: &Signatures,
        ty: &ResolvedType,
        target: DefId,
        seen: &mut BTreeSet<DefId>,
    ) -> bool {
        if let Some(d) = ty.def_id() {
            if d == target {
                return true;
            }
            if seen.insert(d)
                && let Some(t) = sigs.type_decl(d)
                && parts(t).any(|r| r.resolved().is_some_and(|x| reaches(sigs, x, target, seen)))
            {
                return true;
            }
        }
        ty.args().iter().any(|a| reaches(sigs, a, target, seen))
    }
    let Some(t) = sigs.type_decl(def) else {
        return false;
    };
    let mut seen = BTreeSet::new();
    parts(t).any(|r| {
        r.resolved()
            .is_some_and(|x| reaches(sigs, x, def, &mut seen))
    })
}

/// **Does a declared type hold a value of its own type in place**: with no
/// `List`, `Map` or `Set` between, as `Option<Node>`, `Add(Expr, Expr)` or a
/// `Pair<Node>` whose `Pair` holds its argument in a field (ADR-0194)?
pub fn contains_itself_in_place(sigs: &Signatures, def: DefId) -> bool {
    let Some(t) = sigs.type_decl(def) else {
        return false;
    };
    let mut seen = BTreeSet::new();
    parts(t).any(|r| {
        r.resolved()
            .is_some_and(|x| in_place(sigs, x, def, &mut seen))
    })
}

/// Does `ty` hold a `target` in place?
fn in_place(
    sigs: &Signatures,
    ty: &ResolvedType,
    target: DefId,
    seen: &mut BTreeSet<DefId>,
) -> bool {
    if ty.parameter_binding().is_some() || ty.as_primitive().is_some() {
        return false;
    }
    if let Some(b) = ty.as_builtin() {
        return match b {
            Builtin::Option | Builtin::Result => {
                ty.args().iter().any(|a| in_place(sigs, a, target, seen))
            }
            // A list's, a map's and a set's elements are where the value
            // points; a function value is its environment's address.
            Builtin::List | Builtin::Map | Builtin::Set | Builtin::Function => false,
        };
    }
    // `Session<T>` is its argument (ADR-0033).
    if sigs.privacy_qualifier(ty).is_some() {
        return ty.args().iter().any(|a| in_place(sigs, a, target, seen));
    }
    let Some(d) = ty.def_id() else {
        return false;
    };
    if d == target {
        return true;
    }
    if seen.insert(d)
        && let Some(t) = sigs.type_decl(d)
        && parts(t).any(|r| {
            r.resolved()
                .is_some_and(|x| in_place(sigs, x, target, seen))
        })
    {
        return true;
    }
    // An argument, where the declaration holds that parameter in place.
    ty.args().iter().enumerate().any(|(i, a)| {
        holds_parameter_in_place(sigs, d, i as u32, &mut BTreeSet::new())
            && in_place(sigs, a, target, seen)
    })
}

/// Does `def` hold its `index`th type parameter in place?
fn holds_parameter_in_place(
    sigs: &Signatures,
    def: DefId,
    index: u32,
    seen: &mut BTreeSet<(DefId, u32)>,
) -> bool {
    if !seen.insert((def, index)) {
        return false;
    }
    let Some(t) = sigs.type_decl(def) else {
        return false;
    };
    parts(t).any(|r| {
        r.resolved()
            .is_some_and(|x| parameter_in_place(sigs, x, (def, index), seen))
    })
}

fn parameter_in_place(
    sigs: &Signatures,
    ty: &ResolvedType,
    parameter: (DefId, u32),
    seen: &mut BTreeSet<(DefId, u32)>,
) -> bool {
    if let Some(key) = ty.parameter_binding() {
        return key == parameter;
    }
    if ty.as_primitive().is_some() {
        return false;
    }
    if let Some(b) = ty.as_builtin() {
        return matches!(b, Builtin::Option | Builtin::Result)
            && ty
                .args()
                .iter()
                .any(|a| parameter_in_place(sigs, a, parameter, seen));
    }
    if sigs.privacy_qualifier(ty).is_some() {
        return ty
            .args()
            .iter()
            .any(|a| parameter_in_place(sigs, a, parameter, seen));
    }
    let Some(d) = ty.def_id() else {
        return false;
    };
    ty.args().iter().enumerate().any(|(j, a)| {
        parameter_in_place(sigs, a, parameter, seen)
            && holds_parameter_in_place(sigs, d, j as u32, seen)
    })
}

/// **How a part of a type that contains itself holds it**, as a value of
/// the type crosses a component boundary as its nodes (ADR-0194, ADR-0202).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// `List<T>`: a list of node indices.
    List,
    /// `T` itself, held in place: one node index.
    Box,
    /// `Option<T>`: a node index, or none.
    Option,
}

/// The slot a part of `def` is, where its type is `def` itself, an `Option`
/// of it or a `List` of it, unapplied.
pub fn slot_of(ty: &ResolvedType, def: DefId) -> Option<Slot> {
    let is_self = |t: &ResolvedType| t.def_id() == Some(def) && t.args().is_empty();
    if is_self(ty) {
        return Some(Slot::Box);
    }
    if !ty.args().first().is_some_and(is_self) {
        return None;
    }
    match ty.as_builtin()? {
        Builtin::List => Some(Slot::List),
        Builtin::Option => Some(Slot::Option),
        _ => None,
    }
}

/// **Where a type that contains itself holds itself**, as a value of it
/// crosses a component boundary (ADR-0194, ADR-0202): each part, by its
/// position among the declaration's parts, that is the type itself, an
/// `Option` of it or a `List` of it, and how.
///
/// Refused, with why, where any other part holds the type: deeper in a field
/// (`List<List<Comment>>`, `Map<String, Comment>`, `Option<List<Comment>>`),
/// or through another declaration (`A` holding `List<B>`, `B` holding an
/// `A`); and where a part holds another type that contains itself, whose own
/// nodes a node cannot hold yet.
pub fn self_slots(sigs: &Signatures, def: DefId) -> Result<BTreeMap<usize, Slot>, String> {
    let Some(t) = sigs.type_decl(def) else {
        return Err("its declaration is not a type".to_string());
    };
    let name = sigs.path_of(def).unwrap_or_default();
    let mut slots = BTreeMap::new();
    for (i, part) in parts(t).enumerate() {
        let Some(ty) = part.resolved() else {
            continue;
        };
        if let Some(slot) = slot_of(ty, def) {
            slots.insert(i, slot);
            continue;
        }
        if mentions(sigs, ty, def, &mut BTreeSet::new()) {
            return Err(format!(
                "`{name}` holds itself as `{}`, and only a field that is it, an `Option` of \
                 it or a `List` of it crosses as node indices yet",
                ty.display_name(),
            ));
        }
        if let Some(other) = holds_recursive(sigs, ty, &mut BTreeSet::new()) {
            return Err(format!(
                "`{name}` holds `{}`, which contains itself too, and a node holds no other \
                 type's nodes yet",
                sigs.path_of(other).unwrap_or_default()
            ));
        }
    }
    Ok(slots)
}

/// Does `ty` mention `target` anywhere, through declarations too?
fn mentions(
    sigs: &Signatures,
    ty: &ResolvedType,
    target: DefId,
    seen: &mut BTreeSet<DefId>,
) -> bool {
    if let Some(d) = ty.def_id() {
        if d == target {
            return true;
        }
        if seen.insert(d)
            && let Some(t) = sigs.type_decl(d)
            && parts(t).any(|r| {
                r.resolved()
                    .is_some_and(|x| mentions(sigs, x, target, seen))
            })
        {
            return true;
        }
    }
    ty.args().iter().any(|a| mentions(sigs, a, target, seen))
}

/// The first declaration `ty` reaches that contains itself, if any.
fn holds_recursive(
    sigs: &Signatures,
    ty: &ResolvedType,
    seen: &mut BTreeSet<DefId>,
) -> Option<DefId> {
    if let Some(d) = ty.def_id()
        && seen.insert(d)
    {
        if contains_itself(sigs, d) {
            return Some(d);
        }
        if let Some(t) = sigs.type_decl(d) {
            for r in parts(t) {
                if let Some(found) = r.resolved().and_then(|x| holds_recursive(sigs, x, seen)) {
                    return Some(found);
                }
            }
        }
    }
    ty.args()
        .iter()
        .find_map(|a| holds_recursive(sigs, a, seen))
}

/// **Which declared types some finite value has**, by the least fixed point
/// of "a record whose every field has one, a case whose every field has one".
///
/// A generic declaration is asked under its arguments, each only as having a
/// value or not, so `Maybe<Loop>` has one, `Nothing`, and `Box<Loop>` has
/// none. The states are finitely many, so the iteration ends.
struct Inhabited<'a> {
    sigs: &'a Signatures,
    known: BTreeMap<(DefId, Vec<bool>), bool>,
    fresh: Vec<(DefId, Vec<bool>)>,
}

impl Inhabited<'_> {
    fn of(&mut self, def: DefId, args: Vec<bool>) -> bool {
        let key = (def, args);
        if let Some(v) = self.known.get(&key) {
            return *v;
        }
        self.known.insert(key.clone(), false);
        self.fresh.push(key);
        false
    }

    /// One evaluation of a declaration's rule, from what is known so far.
    fn rule(&mut self, def: DefId, args: &[bool]) -> bool {
        let sigs = self.sigs;
        let Some(t) = sigs.type_decl(def) else {
            return true;
        };
        if let Some(cases) = t.variants.as_ref().filter(|c| !c.is_empty()) {
            return cases
                .iter()
                .any(|(_, fields)| fields.iter().all(|f| self.part(f, def, args)));
        }
        if let Some(rep) = &t.representation {
            return self.part(rep, def, args);
        }
        t.record
            .iter()
            .flatten()
            .all(|(_, f)| self.part(f, def, args))
    }

    fn part(&mut self, r: &TypeResolution, binder: DefId, args: &[bool]) -> bool {
        // A type that does not resolve is another diagnostic's.
        match r.resolved() {
            Some(t) => self.ty(t, binder, args),
            None => true,
        }
    }

    fn ty(&mut self, t: &ResolvedType, binder: DefId, args: &[bool]) -> bool {
        if let Some((b, i)) = t.parameter_binding() {
            return b != binder || args.get(i as usize).copied().unwrap_or(true);
        }
        if t.as_primitive().is_some() {
            return true;
        }
        if let Some(b) = t.as_builtin() {
            return match b {
                // `Ok` or `Err`: either side with a value is enough.
                Builtin::Result => t.args().iter().any(|a| self.ty(a, binder, args)),
                // Empty, `None`, or a function that never returns: each has
                // a value whatever its argument has.
                Builtin::List
                | Builtin::Map
                | Builtin::Set
                | Builtin::Option
                | Builtin::Function => true,
            };
        }
        if self.sigs.privacy_qualifier(t).is_some() {
            return t.args().iter().all(|a| self.ty(a, binder, args));
        }
        let Some(d) = t.def_id() else {
            return true;
        };
        let applied: Vec<bool> = t.args().iter().map(|a| self.ty(a, binder, args)).collect();
        self.of(d, applied)
    }

    /// Iterate to the fixed point: every state recomputed from the last
    /// pass's answers, until none changes and none is new.
    fn settle(&mut self) {
        loop {
            self.fresh.clear();
            let mut changed = false;
            let states: Vec<(DefId, Vec<bool>)> = self.known.keys().cloned().collect();
            for (def, args) in states {
                if self.known[&(def, args.clone())] {
                    continue;
                }
                if self.rule(def, &args) {
                    self.known.insert((def, args), true);
                    changed = true;
                }
            }
            if !changed && self.fresh.is_empty() {
                return;
            }
        }
    }
}

/// **PW0624: a type no finite value has** (ADR-0194). Each record, sum or
/// opaque type `hir` declares, asked with each of its type parameters taken
/// to have a value.
pub fn check(hir: &Hir, sigs: &Signatures, at: usize, out: &mut Vec<Diagnostic>) {
    let mut inhabited = Inhabited {
        sigs,
        known: BTreeMap::new(),
        fresh: Vec::new(),
    };
    let declared: Vec<(DefId, &crate::hir::Decl)> = hir
        .all_decls()
        .filter(|(_, d)| matches!(d.kind, DeclKind::Type | DeclKind::Opaque))
        .map(|(id, d)| {
            (
                DefId {
                    unit: at,
                    decl: id.0,
                },
                d,
            )
        })
        .collect();
    // Each declaration, and each of its fields, asked once before the fixed
    // point, so every state the report reads is settled.
    for (def, d) in &declared {
        let all = vec![true; d.type_params.len()];
        inhabited.of(*def, all.clone());
        if let Some(fields) = sigs.type_decl(*def).and_then(|t| t.record.as_ref()) {
            for (_, r) in fields {
                inhabited.part(r, *def, &all);
            }
        }
    }
    inhabited.settle();
    for (def, d) in declared {
        let all = vec![true; d.type_params.len()];
        if inhabited.of(def, all.clone()) {
            continue;
        }
        let name = &d.name;
        let why = match sigs.type_decl(def) {
            Some(t) if t.variants.as_ref().is_some_and(|c| !c.is_empty()) => {
                "each of its cases holds a field no finite value fills"
            }
            Some(t) if t.representation.is_some() => "its representation has none",
            _ => "each one holds a field no finite value fills",
        };
        let mut related = Vec::new();
        for f in d.fields.iter().flatten() {
            let empty = sigs
                .type_decl(def)
                .and_then(|t| t.record.as_ref())
                .and_then(|fs| fs.iter().find(|(n, _)| n == &f.name))
                .is_some_and(|(_, r)| !inhabited.part(r, def, &all));
            if empty {
                related.push(Related {
                    span: f.span.clone(),
                    label: format!("`{}` has no finite value", f.name),
                });
            }
        }
        out.push(Diagnostic {
            code: crate::codes::NO_FINITE_VALUE.id,
            invariant: crate::codes::NO_FINITE_VALUE.invariant,
            reason: "a_type_no_finite_value_has",
            detector: Detector::DeclarationRule,
            severity: Severity::Error,
            message: format!("`{name}` has no finite value: {why}"),
            primary_span: d.name_span.clone(),
            related,
            explanation: Some(
                "A value is built from values that exist already, so a type each of whose \
                 values must hold another value no finite one fills has no value at all: no \
                 program could build one, and a function that returns one could only run \
                 forever. A type that contains itself needs a way to be built without itself."
                    .to_string(),
            ),
            repairs: vec![Repair {
                description: "give it a way to be built without itself: a case that does not \
                              hold it, or an `Option` or a `List` around the field that does"
                    .to_string(),
                replacement: None,
            }],
        });
    }
}
