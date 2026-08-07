//! E2D — source effect inference.
//!
//! Pulled forward out of E9 by architect ruling, 2026-08-06, because it is the
//! single highest-leverage missing capability: roughly fifteen rejected corpus
//! programs need it, and so does the headline P0 claim.
//!
//! # What it does
//!
//! Given a body, work out which effects it actually performs, and compare that
//! against the row it declares. `RQ-2` measured that Koka propagates effects
//! through unannotated higher-order code; this is `pw` owning the same property
//! over its own source, with its own spans.
//!
//! ```text
//! view StorePage(id: StoreId) !{} {      declared: {}
//!     let store = Stores.get(id)         Stores.get : !{ database.read }
//!     ...                                inferred: { database.read }
//! }                                      -> the row is not satisfied
//! ```
//!
//! # Where the effects come from
//!
//! [`crate::signatures::Signatures`], and nowhere else. There is no table of
//! function names here — E2C's deletion gate forbids it, and the reason is that
//! a table can describe a function that does not exist.
//!
//! # The three shapes that matter
//!
//! `docs/RISK_QUEUE.md` RQ-2 names them, and the corpus tests each:
//!
//! - **direct** — the body calls the effectful thing itself;
//! - **helper-hidden** — the body calls a local helper that does;
//! - **callback-hidden** — the body passes a lambda to a generic function, and
//!   the effect is inside the lambda.
//!
//! The third is the one an approximation gets wrong, which is why the body
//! grammar parses lambdas properly rather than skipping them.

use std::collections::{BTreeMap, BTreeSet};

use crate::hir::{Body, Decl, Expr, ExprId, Hir, Span};
use crate::resolve::{DefId, Resolution, Workspace};
use crate::signatures::Signatures;

/// An effect, as written in a row: `database.read`, `layout.measure`.
pub type Effect = String;

/// Why a body has an effect. The material for a diagnostic that explains the
/// chain rather than announcing a mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub effect: Effect,
    /// The call that introduced it.
    pub span: Span,
    /// How it got here.
    pub via: Via,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Via {
    /// The body calls it directly.
    Direct { callee: String },
    /// Through a helper declared in the same program.
    Helper { helper: String, callee: String },
    /// Inside a lambda handed to another function.
    Callback { passed_to: String, callee: String },
}

impl Via {
    /// The thing that actually performs the effect, whatever route it took to
    /// get here. A diagnostic about an escape hatch has to name the hatch.
    pub fn callee(&self) -> &str {
        match self {
            Via::Direct { callee } | Via::Helper { callee, .. } | Via::Callback { callee, .. } => {
                callee
            }
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Via::Direct { callee } => format!("`{callee}` performs it"),
            // A helper that IS the callee is a direct call that arrived by the
            // helper route. "`raw_html` calls `raw_html`" reads as a bug in the
            // compiler rather than a fact about the program.
            Via::Helper { helper, callee } if helper == callee => {
                format!("`{callee}` performs it")
            }
            Via::Helper { helper, callee } => {
                format!("`{helper}` calls `{callee}`, which performs it")
            }
            Via::Callback { passed_to, callee } => {
                format!("the callback passed to `{passed_to}` calls `{callee}`")
            }
        }
    }
}

/// What a body was found to do.
#[derive(Debug, Clone, Default)]
pub struct Inferred {
    pub effects: BTreeSet<Effect>,
    pub sources: Vec<Source>,
}

impl Inferred {
    /// Effects performed but not declared. Empty means the row is honest.
    ///
    /// An **open row** — one the author did not write at all — is not checked:
    /// `None` means "not stated", and inferring a row for a declaration that
    /// never claimed one would reject working programs.
    pub fn undeclared(&self, declared: Option<&[String]>) -> Vec<&Source> {
        let Some(declared) = declared else {
            return Vec::new();
        };
        let declared: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
        let mut seen = BTreeSet::new();
        self.sources
            .iter()
            .filter(|s| !covered(&declared, &s.effect))
            .filter(|s| seen.insert(s.effect.clone()))
            .collect()
    }
}

/// Does a declared row cover an inferred effect?
///
/// `database` covers `database.read`: declaring a family accepts its members.
/// The reverse is not true — declaring `database.read` does not permit
/// `database.write`, which is what makes a query that writes detectable.
pub fn row_covers(declared: &[String], effect: &str) -> bool {
    let set: BTreeSet<&str> = declared.iter().map(String::as_str).collect();
    covered(&set, effect)
}

fn covered(declared: &BTreeSet<&str>, effect: &str) -> bool {
    if declared.contains(effect) {
        return true;
    }
    let want = family_of(effect);
    declared
        .iter()
        .any(|d| family_of(d) == want && is_broader(d, effect))
}

/// An effect's family: `style.mutate<LayoutAffect>` and `style.mutate` are both
/// `style`.
///
/// Type arguments are stripped **before** splitting on `.`, because
/// `secret<Payments>` has no dot at all — comparing families without stripping
/// made a row declaring `secret<Payments>` fail to cover `secret.read`, and two
/// corpus fixtures were reported for an effect they had declared.
pub fn family_of(effect: &str) -> &str {
    let base = effect.split('<').next().unwrap_or(effect);
    base.split('.').next().unwrap_or(base)
}

/// Is `declared` at least as permissive as `effect`?
///
/// A family covers its members: declaring `database` accepts `database.read`.
/// A member does not cover a sibling — declaring `database.read` must not
/// permit `database.write`, which is what makes a query that writes detectable.
fn is_broader(declared: &str, effect: &str) -> bool {
    let d = declared.split('<').next().unwrap_or(declared);
    let e = effect.split('<').next().unwrap_or(effect);
    d == e || !d.contains('.')
}

/// Infer the effects of every declaration in a program.
///
/// Two passes over the call graph, so a helper defined after its caller still
/// contributes. Deeper chains converge by iterating until nothing changes,
/// bounded because the set of effects is finite.
pub struct Inference<'a> {
    sigs: &'a Signatures,
    /// Declaration name → the effects its body performs.
    /// Inferred facts, keyed by RESOLVED IDENTITY.
    ///
    /// `docs/RISK_QUEUE.md` 34: this was keyed by the bare declaration name, so
    /// `Resources.Cart` and `store.page.Cart` shared one entry and one of them
    /// silently received facts belonging to the other. A query whose body is
    /// `todo` was reported as requiring `database.read`, which is exactly what
    /// a cart query *should* require — the wrongness was invisible in the
    /// output because the borrowed fact was plausible for its new owner.
    ///
    /// Architect ruling, 2026-08-07:
    ///
    /// > It should be `DefId → inferred facts` […] never `"Cart" → inferred
    /// > facts`. This is not "implementing the permanent E9 type system early".
    /// > It is repairing the current compiler so every existing analysis
    /// > consumes the name resolution machinery E2B already established.
    known: BTreeMap<DefId, BTreeSet<Effect>>,
    // NOTE: `known` keyed by a `String` would be RISK_QUEUE 34 again. See
    // `name-keyed-allow.txt` and `tests/name_keyed_maps.rs`.
    /// Last-segment spellings that name exactly ONE declaration in the whole
    /// program. `None` means several do, and therefore no answer.
    ///
    /// The last resort for a MEMBER call whose receiver's module is not
    /// imported — `el.getBoundingClientRect()` in a file that imports only
    /// `List`. Uniqueness is what makes it safe: the collision RISK_QUEUE 34
    /// is about (`Resources.Cart` and `store.page.Cart`) resolves to `None`
    /// here and contributes nothing, where the old rule silently picked one.
    unique_names: BTreeMap<String, Option<DefId>>,
    /// Which unit each `Hir` in the last `run` was, so a declaration can be
    /// given the same `DefId` the workspace gave it.
    workspace: &'a Workspace,
}

impl<'a> Inference<'a> {
    pub fn new(sigs: &'a Signatures, workspace: &'a Workspace) -> Self {
        Self {
            sigs,
            known: BTreeMap::new(),
            unique_names: BTreeMap::new(),
            workspace,
        }
    }

    /// The identity of a declaration, as the workspace assigns it.
    ///
    /// Built the same way `Signatures::build` builds its `by_def` key, because
    /// two constructions of one identity is how this defect gets reintroduced.
    fn def_of(&self, unit: usize, id: crate::hir::DeclId) -> DefId {
        DefId { unit, decl: id.0 }
    }

    /// Seed from every declared row in the program, then iterate to a fixed
    /// point over local helpers.
    pub fn run(&mut self, hirs: &[&Hir]) {
        for (unit, hir) in hirs.iter().enumerate() {
            for (id, d) in hir.all_decls() {
                // Built once, counting duplicates: a second declaration with
                // the same spelling turns the entry into `None` rather than
                // overwriting it.
                let def = self.def_of(unit, id);
                self.unique_names
                    .entry(d.name.clone())
                    .and_modify(|slot| {
                        if *slot != Some(def) {
                            *slot = None;
                        }
                    })
                    .or_insert(Some(def));
                if let Some(row) = &d.declared_effects {
                    self.known.insert(
                        self.def_of(unit, id),
                        row.iter().map(|e| e.written.clone()).collect(),
                    );
                }
            }
        }

        // A helper that does not declare a row still has effects. Propagate
        // until nothing changes — the effect set is finite, so this terminates.
        for _ in 0..8 {
            let mut changed = false;
            for (unit, hir) in hirs.iter().enumerate() {
                for (id, d) in hir.all_decls() {
                    let Some(body_id) = d.body else { continue };
                    let found = self.infer_at(unit, hir.body(body_id));
                    let entry = self.known.entry(self.def_of(unit, id)).or_default();
                    let before = entry.len();
                    entry.extend(found.effects);
                    if entry.len() != before {
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// What one body performs, given the types its declaration makes known.
    pub fn infer_in(&self, body: &Body, types: &BTreeMap<String, String>) -> Inferred {
        self.infer_in_at(usize::MAX, body, types)
    }

    /// `infer_in`, resolving unqualified calls within `unit`.
    pub fn infer_in_at(
        &self,
        unit: usize,
        body: &Body,
        types: &BTreeMap<String, String>,
    ) -> Inferred {
        let mut out = self.infer_at(unit, body);
        self.member_effects(body, types, &mut out);
        out
    }

    /// Effects reached through a member: `anchor.offsetWidth`,
    /// `el.getBoundingClientRect()`.
    ///
    /// Charter §7.5A bans reading geometry directly from ordinary code. That ban
    /// lives in the accessors' declared rows (`packages/pw-platform-web/browser.pw`),
    /// not in a list of property names here — which is what E2C's deletion gate
    /// requires and what lets a new accessor be added without touching a checker.
    fn member_effects(&self, body: &Body, types: &BTreeMap<String, String>, out: &mut Inferred) {
        for id in body.walk() {
            // Both shapes: a property read, and a method call on a value.
            let (receiver, member, span) = match body.expr(id) {
                Expr::Field { base, name } => {
                    (receiver_name(body, *base), name.clone(), body.expr_span(id))
                }
                Expr::Call { callee, .. } => match body.expr(*callee) {
                    Expr::Field { base, name } => {
                        (receiver_name(body, *base), name.clone(), body.expr_span(id))
                    }
                    _ => continue,
                },
                _ => continue,
            };
            let Some(receiver) = receiver else { continue };

            // A module path is not a member access — `Stores.get` is already
            // handled by the call walk, and counting it twice would report the
            // same effect from two places.
            if self.sigs.by_path(&format!("{receiver}.{member}")).is_some() {
                continue;
            }

            // By the receiver's TYPE, or not at all. The by-name fallback that
            // used to sit here is gone: it made whether an effect was seen
            // depend on no other type declaring a member with the same
            // spelling, and it had already borrowed `Carts.add`'s row for
            // `Money.add` once, reporting a pure calculation as writing to the
            // database. A receiver whose type this program does not state is a
            // receiver whose members are unknown.
            let Some(declared) = types.get(&receiver).map(String::as_str) else {
                continue;
            };
            let Some(sig) = self.sigs.member_of(declared, &member) else {
                continue;
            };
            for e in &sig.effects {
                out.effects.insert(e.clone());
                out.sources.push(Source {
                    effect: e.clone(),
                    span: span.clone(),
                    via: Via::Direct {
                        callee: format!("{receiver}.{member}"),
                    },
                });
            }
        }
    }

    /// What one body performs, with a reason for each effect.
    /// What one body performs, EXCLUDING the given subtrees.
    ///
    /// E8-0 needs this. A page that renders `on:press={.. => add_to_cart(..)}`
    /// contains a call to a command that writes the database — but the page
    /// does not perform that write. The handler does, when someone presses the
    /// button, and E7-L already makes the handler a separately loaded unit
    /// with its own identity.
    ///
    /// Without the exclusion the page's contract requires `database.write`,
    /// and a host granting it would give the render path authority it never
    /// uses. That is the exact over-granting the capability model exists to
    /// prevent, arriving through the front door.
    pub fn infer_excluding(&self, unit: usize, body: &Body, exclude: &[ExprId]) -> Inferred {
        let mut out = self.infer_at(unit, body);
        if exclude.is_empty() {
            return out;
        }

        let inside = |span: &Span| {
            exclude.iter().any(|root| {
                let outer = body.expr_span(*root);
                span.start >= outer.start && span.end <= outer.end
            })
        };

        // SUBTRACTED, not rebuilt.
        //
        // An effect is dropped only when it has at least one source and every
        // one of them lies inside an excluded subtree. Rebuilding the set from
        // the surviving sources instead was wrong in the direction that
        // matters: an effect carried without a source vanished, and a query
        // that reads the database reported needing no authority at all.
        //
        // Keeping a sourceless effect is the conservative direction. An
        // over-stated capability is refused work; an under-stated one is
        // authority nobody approved.
        let mut dropped: BTreeSet<String> = BTreeSet::new();
        for effect in &out.effects {
            let mut seen = false;
            let mut all_inside = true;
            for s in out.sources.iter().filter(|s| &s.effect == effect) {
                seen = true;
                if !inside(&s.span) {
                    all_inside = false;
                }
            }
            if seen && all_inside {
                dropped.insert(effect.clone());
            }
        }
        out.effects.retain(|e| !dropped.contains(e));
        out.sources.retain(|s| !inside(&s.span));
        out
    }

    /// `infer`, without a unit to resolve names in.
    ///
    /// Kept for callers that have a body and no workspace position. It can
    /// still resolve a call through `Signatures::by_path`, which is a
    /// fully-qualified lookup; what it cannot do is find a SIBLING's inferred
    /// row, because "which sibling" is a question only a unit can answer.
    pub fn infer(&self, body: &Body) -> Inferred {
        self.infer_at(usize::MAX, body)
    }

    /// What one body performs, resolving unqualified calls within `unit`.
    pub fn infer_at(&self, unit: usize, body: &Body) -> Inferred {
        let mut out = Inferred::default();
        // Lambdas are visited through their enclosing call, so the reason can
        // say *which* function the callback was handed to.
        let mut inside_callback: BTreeMap<ExprId, String> = BTreeMap::new();
        for id in body.walk() {
            if let Expr::Call { callee, args } = body.expr(id) {
                let name = path_of(body, *callee);
                for a in args {
                    if matches!(body.expr(a.value), Expr::Lambda { .. }) {
                        inside_callback.insert(a.value, name.clone());
                    }
                }
            }
        }

        for id in body.walk() {
            // A frame-phase keyword carries its own effect (see
            // `intrinsic_effect`): `measure { .. }` reads geometry by
            // definition, whatever it calls inside.
            if let Some((phase, e)) = match body.expr(id) {
                Expr::Keyword { keyword, .. } => {
                    intrinsic_effect(keyword).map(|e| (keyword.clone(), e))
                }
                _ => None,
            } {
                {
                    out.effects.insert(e.to_string());
                    out.sources.push(Source {
                        effect: e.to_string(),
                        span: body.expr_span(id),
                        via: Via::Direct {
                            callee: format!("{phase} {{ .. }}"),
                        },
                    });
                }
            }

            let Expr::Call { callee, .. } = body.expr(id) else {
                continue;
            };
            let path = path_of(body, *callee);
            if path.is_empty() {
                continue;
            }

            // A signature first — that is E2C's single source of truth — then a
            // local declaration's own inferred effects.
            //
            // Resolved, not spelled. `Cart` in one module and `Cart` in another
            // are two declarations, and matching on the last segment of a path
            // gave one of them the other's effects — `RISK_QUEUE` 34.
            let effects: Vec<Effect> = match self.sigs.by_path(&path) {
                Some(sig) => sig.effects.clone(),
                None => self
                    .resolved(unit, &path)
                    .and_then(|def| self.known.get(&def))
                    .map(|s| s.iter().cloned().collect())
                    .unwrap_or_default(),
            };
            if effects.is_empty() {
                continue;
            }

            let short = path.rsplit('.').next().unwrap_or(&path).to_string();
            let via = match enclosing_callback(body, id, &inside_callback) {
                Some(passed_to) => Via::Callback {
                    passed_to,
                    callee: path.clone(),
                },
                None if self.sigs.by_path(&path).is_none() => Via::Helper {
                    helper: short,
                    callee: path.clone(),
                },
                None => Via::Direct {
                    callee: path.clone(),
                },
            };

            for e in effects {
                out.effects.insert(e.clone());
                out.sources.push(Source {
                    effect: e,
                    span: body.expr_span(id),
                    via: via.clone(),
                });
            }
        }
        out
    }

    /// Which declaration a path names, from inside `unit`.
    ///
    /// `usize::MAX` means "no unit", which resolves nothing — the honest
    /// answer for a caller that did not say where it was standing. Falling back
    /// to a name match would reintroduce exactly the defect this replaces.
    fn resolved(&self, unit: usize, path: &str) -> Option<DefId> {
        if unit == usize::MAX {
            return None;
        }
        if let Resolution::Local(def) | Resolution::Imported { def, .. } =
            self.workspace.resolve_path(unit, path)
        {
            return Some(def);
        }
        // A MEMBER call: `it.style.set_padding`, whose head is a value rather
        // than a module, so `resolve_path` cannot see it. Its last segment is
        // matched against the declarations THIS UNIT CAN SEE — its own module
        // and the modules it imports — and accepted only when exactly one
        // matches.
        //
        // This replaces the old fallback, which matched the last segment
        // against every declaration in the program by name. That is
        // `docs/RISK_QUEUE.md` 34: `Resources.Cart` and `store.page.Cart`
        // collided, and one silently received the other's effects.
        //
        // Scoped-and-unique keeps what the old fallback was load-bearing for —
        // R-035 depends on `set_padding`'s row reaching a call written as a
        // member — while making the cross-module borrow impossible: an
        // unrelated module is either not imported, or it produces two matches
        // and the answer is none.
        let last = path.rsplit('.').next()?;
        let module = self.workspace.module_of(unit)?;
        let visible = std::iter::once(module.name.as_str())
            .chain(module.imports.iter().map(|i| i.module.as_str()));

        let mut found: Option<DefId> = None;
        for m in visible {
            let candidate = match self.workspace.resolve_path(unit, &format!("{m}.{last}")) {
                Resolution::Local(def) | Resolution::Imported { def, .. } => def,
                _ => continue,
            };
            match found {
                // Ambiguous: two visible declarations share the spelling, so
                // there is no answer rather than an arbitrary one.
                Some(seen) if seen != candidate => return None,
                _ => found = Some(candidate),
            }
        }
        if found.is_some() {
            return found;
        }

        // Last resort: a spelling that names exactly ONE declaration in the
        // whole program. `R-037` needs it — `el.getBoundingClientRect()` in a
        // file that imports only `List`, so the platform module holding that
        // declaration is not visible by any rule this function can apply.
        //
        // Uniqueness is the whole safety argument. The old rule took the last
        // segment and picked whatever matched; this one refuses when more than
        // one does, so `RISK_QUEUE` 34's two `Cart`s contribute nothing instead
        // of contributing each other's effects.
        //
        // It stays until platform packages have a declared prelude status, at
        // which point the scoped branch above covers this case honestly.
        self.unique_names.get(last).copied().flatten()
    }

    /// **What this definition actually requires at run time.**
    ///
    /// Architect ruling, 2026-08-07, and the reason no caller may decide this
    /// for itself:
    ///
    /// ```text
    /// DeclaredEffects       what this interface permits/promises
    /// InferredEffects       what this implementation actually performs
    /// RequiredCapabilities  what this compiled artifact currently needs
    ///
    /// local code      RequiredCapabilities <- InferredEffects
    /// bodyless code   RequiredCapabilities <- DeclaredEffects
    /// ```
    ///
    /// So for
    ///
    /// ```text
    /// fn foo() !{ database.read, trace } { trace("hello") }
    /// ```
    ///
    /// the host must not grant database access merely because the annotation
    /// permitted it. The declaration still CONSTRAINS: a later implementation
    /// that adds `database.write` is outside its contract and the checker says
    /// so — that check lives with the row rules and is unaffected by this.
    ///
    /// A definition with no body is an interface or separately compiled code.
    /// Its declared row is a promise, and a promise is all there is.
    pub fn effective_effects(&self, unit: usize, hir: &Hir, id: crate::hir::DeclId) -> Vec<String> {
        let decl = hir.decl(id);
        match decl.body {
            Some(body) => {
                let mut out: Vec<String> = self
                    .infer_at(unit, hir.body(body))
                    .effects
                    .into_iter()
                    .collect();
                out.sort();
                out.dedup();
                out
            }
            None => decl
                .declared_effects
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|e| e.written.clone())
                .collect(),
        }
    }

    /// `effective_effects`, minus what only happens inside `exclude`.
    ///
    /// E8-0 needs this for a page whose handler lambdas are separately loaded
    /// units — see `infer_excluding`. The declared-versus-inferred rule is the
    /// same one; only the scope narrows.
    pub fn effective_effects_excluding(
        &self,
        unit: usize,
        hir: &Hir,
        id: crate::hir::DeclId,
        exclude: &[ExprId],
    ) -> Vec<String> {
        let decl = hir.decl(id);
        match decl.body {
            Some(body) => {
                let mut out: Vec<String> = self
                    .infer_excluding(unit, hir.body(body), exclude)
                    .effects
                    .into_iter()
                    .collect();
                out.sort();
                out.dedup();
                out
            }
            None => self.effective_effects(unit, hir, id),
        }
    }

    /// The effects a declaration performs, after propagation.
    pub fn effects_of_def(&self, def: DefId) -> Option<&BTreeSet<Effect>> {
        self.known.get(&def)
    }

    /// The effects of the declaration `path` names, from inside `unit`.
    ///
    /// There is deliberately no bare-name form. A caller that has only a
    /// spelling does not have enough information to be answered, and the
    /// previous version answered anyway.
    pub fn effects_of(&self, unit: usize, path: &str) -> Option<&BTreeSet<Effect>> {
        self.resolved(unit, path).and_then(|d| self.known.get(&d))
    }
}

/// The function a call sits inside the callback of, if any.
fn enclosing_callback(
    body: &Body,
    call: ExprId,
    callbacks: &BTreeMap<ExprId, String>,
) -> Option<String> {
    // A call is inside a callback when one of the lambdas handed to another
    // function contains it. Compared by span, because HIR has no parent links —
    // the check is containment, which is exactly what "inside" means.
    let span = body.expr_span(call);
    callbacks
        .iter()
        .filter(|(lambda, _)| {
            let l = body.expr_span(**lambda);
            l.start <= span.start && span.end <= l.end && l != span
        })
        .min_by_key(|(lambda, _)| {
            let l = body.expr_span(**lambda);
            l.end - l.start
        })
        .map(|(_, to)| to.clone())
}

/// The name a member access is reached through: `anchor` in `anchor.offsetWidth`.
fn receiver_name(body: &Body, base: ExprId) -> Option<String> {
    match body.expr(base) {
        Expr::Name(n) => Some(n.clone()),
        // `el.getBoundingClientRect().width` — the receiver of `.width` is the
        // call, whose own effects were already counted. Not a member access to
        // attribute again.
        _ => None,
    }
}

/// A callee's dotted path as written.
fn path_of(body: &Body, id: ExprId) -> String {
    match body.expr(id) {
        Expr::Name(n) => n.clone(),
        Expr::Field { base, name } => {
            let base = path_of(body, *base);
            if base.is_empty() {
                name.clone()
            } else {
                format!("{base}.{name}")
            }
        }
        _ => String::new(),
    }
}

/// The minimal set of effects the **language** knows, as distinct from what a
/// library declares.
///
/// E2C's deletion gate forbids a checker from knowing what `secrets.payments`
/// does — that is a library fact and belongs in a signature. A frame phase is
/// not a library fact: `measure { .. }` is charter §7.5A syntax whose whole
/// meaning is *this is the phase where geometry may be read*. A language that
/// did not know that would need a library function to explain its own keyword.
///
/// The architect's bound on this list: "keep it tiny and explicit". It is four
/// entries, and each is a phase keyword the grammar already reserves.
pub fn intrinsic_effect(keyword: &str) -> Option<&'static str> {
    Some(match keyword {
        "measure" => "layout.measure",
        "mutate" => "style.mutate",
        "post_paint" => "paint.post",
        "animate" => "animation.composite",
        _ => return None,
    })
}

/// The frame phase a span sits inside, innermost first.
///
/// Charter §7.5A gives the frame a shape: measure, then mutate, then paint,
/// then post-paint. Which phase code is in decides what it may do — and that is
/// an ordering question, not a question of *which* effects exist. E2D's
/// inference answers the second; this answers the first.
pub fn phase_at(body: &Body, span: &Span) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for id in body.walk() {
        let Expr::Keyword { keyword, .. } = body.expr(id) else {
            continue;
        };
        if !matches!(
            keyword.as_str(),
            "measure" | "mutate" | "post_paint" | "animate" | "frame" | "draw" | "subtree"
        ) {
            continue;
        }
        let k = body.expr_span(id);
        if k.start <= span.start && span.end <= k.end && k != *span {
            let width = k.end - k.start;
            if best.as_ref().is_none_or(|(w, _)| width < *w) {
                best = Some((width, keyword.clone()));
            }
        }
    }
    best.map(|(_, k)| k)
}

/// May an effect happen in this frame phase?
///
/// Each entry is charter §7.5A stating what a phase is *for*. A phase that
/// permitted everything would not be a phase.
pub fn forbidden_in_phase(phase: &str, effect: &str) -> Option<&'static str> {
    let family = effect.split('.').next().unwrap_or(effect);
    match (phase, family) {
        // The measure phase reads. A write inside it invalidates the very
        // geometry the phase exists to read consistently.
        ("measure", "style") | ("measure", "dom") => Some(
            "the measure phase reads geometry; writing inside it invalidates what \
                  the rest of the phase is about to read",
        ),
        // After paint, the frame is already on screen. Measuring forces the
        // browser to lay out again for a frame nobody will see.
        ("post_paint", "layout") => Some(
            "the frame is already presented; measuring now forces a second layout \
                  for a frame nobody will see",
        ),
        // A painter draws. Touching the document from inside one re-enters
        // layout from a phase that runs after it.
        ("draw", "dom") | ("draw", "style") | ("draw", "layout") => Some(
            "a painter draws; reaching the document from inside one re-enters \
                  layout from a phase that runs after it",
        ),
        // A compositor animation runs off the main thread. An animated property
        // that invalidates layout drags it back on.
        ("animate", "layout") => Some(
            "a compositor animation runs without layout; animating a property that \
                  invalidates it forces a layout every frame",
        ),
        _ => None,
    }
}

/// Declarations whose bodies must be pure regardless of what they declare.
///
/// Charter §7.5A and §8.5: a `view` renders; it does not fetch. The row it
/// writes is a claim about itself, and `!{}` on a view that reads the database
/// is the corpus's very first rejected case.
/// Why a declaration's output is reused rather than recomputed per reader.
///
/// Charter §7.9 and §9.4. Two different declarations reach the same place: a
/// `placement build` page is generated once and shipped as a file, and a
/// `partition public` materialization is computed once and served from one
/// cache entry to every reader. In both, output that varies with *when* it ran
/// is not a function of its declared inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reuse {
    /// Recomputed for its reader; nondeterminism is that reader's own.
    PerReader,
    /// Generated once at build time.
    Build,
    /// One cache entry serves every reader.
    SharedPartition,
}

/// Effects that make an output depend on *when* it ran.
///
/// The wall clock is the corpus's case. It is `clock.wall` and not the whole
/// `clock` family on purpose: a monotonic read measures a duration and can be
/// deterministic in the sense that matters here, while a wall read cannot.
pub fn nondeterministic(effect: &str) -> bool {
    matches!(family_of(effect), "random") || effect.starts_with("clock.wall")
}

pub fn forbidden_in(
    decl: &Decl,
    reuse: Reuse,
    world: Option<crate::placement::World>,
    effect: &str,
) -> Option<&'static str> {
    use crate::hir::DeclKind::*;
    let family = family_of(effect);

    // Charter §7.9, §9.4. An artifact computed once and reused is only correct
    // if it is a function of its declared inputs. `clock.wall` makes it a
    // function of when it ran as well, so two readers of one cache entry — or
    // one reader of a file generated last Tuesday — get an answer that was
    // never true for them.
    if reuse != Reuse::PerReader && nondeterministic(effect) {
        return Some(match reuse {
            Reuse::Build => {
                "static generation requires a deterministic effect row: the output is \
                 generated once and shipped as a file, so anything that varies with \
                 when it ran is baked in"
            }
            _ => {
                "a shared materialized fragment must be a pure function of its declared \
                 dependencies: one cache entry serves every reader, so a value that \
                 varies with when it was computed is served to readers it was never \
                 true for"
            }
        });
    }

    // A painter is identified by what it declares, not by a declaration kind:
    // `paint.custom` in the row *is* the statement "this runs inside the paint
    // pipeline". Charter §7.5A gives it inputs precisely so it can be replayed
    // and cached, which is only sound if it is a pure function of them.
    if decl
        .declared_effects
        .as_ref()
        .is_some_and(|r| r.iter().any(|e| e.path == "paint.custom"))
        && matches!(family, "dom" | "style" | "layout" | "database" | "network")
    {
        return Some(
            "a painter must be a pure function of its declared inputs, so that the \
             result can be replayed and cached — `paint.custom` may not use effect \
             `dom.mutate`, nor any other reach outside the inputs it names. A painter \
             that touches the document re-enters the pipeline that called it",
        );
    }

    match (decl.kind, family) {
        (View | Component | Page, "database") => {
            Some("a view renders; it cannot reach the database while doing so")
        }
        (View | Component | Page, "network") => {
            Some("a view renders; fetching during render is what streaming exists to avoid")
        }
        // A view is *usually* rendered where secrets are not — but a
        // declaration that names a world where they ARE may read one while
        // rendering. R-003 is an origin-placed page, and reporting it here
        // said "a page may not do this" about something a page may do; the
        // defect is that the secret then reaches the MARKUP, which is
        // `secret_to_browser` and is reported separately.
        (View | Component | Page, "secret") if !world.is_some_and(|w| w.grants("secret")) => {
            Some("a view is rendered where secrets are not")
        }
        // Charter §7.5A. Reading geometry while rendering forces a synchronous
        // reflow, which is the cost the frame phases exist to make visible.
        // Measuring belongs in a `measure` phase, not in the render itself.
        (View | Component | Page, "layout") => {
            Some("reading geometry during render forces a synchronous reflow")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower_file;
    use crate::resolve::Workspace;
    use pw_syntax::parse_tree;

    fn program(sources: &[&str]) -> (Vec<Hir>, Signatures, Workspace) {
        let owned: Vec<Hir> = sources
            .iter()
            .map(|s| lower_file(s, &parse_tree(s).green))
            .collect();
        let refs: Vec<&Hir> = owned.iter().collect();
        let ws = Workspace::build(&refs);
        let sigs = Signatures::build(&ws, &refs);
        (owned, sigs, ws)
    }

    const LIB: &str = "module Stores\n\nfn get(id: Int) -> Int !{ database.read } { 0 }\n";

    fn effects_of(sources: &[&str], decl: &str) -> BTreeSet<Effect> {
        let (hirs, sigs, ws) = program(sources);
        let refs: Vec<&Hir> = hirs.iter().collect();
        let mut inf = Inference::new(&sigs, &ws);
        inf.run(&refs);
        for (unit, hir) in refs.iter().enumerate() {
            for (_, d) in hir.all_decls() {
                if d.name != decl {
                    continue;
                }
                let Some(b) = d.body else { continue };
                return inf.infer_at(unit, hir.body(b)).effects;
            }
        }
        BTreeSet::new()
    }

    #[test]
    fn a_direct_call_contributes_its_effect() {
        let e = effects_of(
            &[
                LIB,
                "module page\n\nimport Stores\n\nfn f() -> Int !{} { Stores.get(1) }\n",
            ],
            "f",
        );
        assert!(e.contains("database.read"), "{e:?}");
    }

    #[test]
    fn an_effect_hidden_behind_a_helper_still_propagates() {
        // RQ-2's second shape. A checker that only looked at direct calls would
        // report `f` as pure, which is the failure the corpus exists to catch.
        let src = "module page\n\nimport Stores\n\n\
                   fn helper(id: Int) -> Int !{} { Stores.get(id) }\n\
                   fn f() -> Int !{} { helper(1) }\n";
        let e = effects_of(&[LIB, src], "f");
        assert!(
            e.contains("database.read"),
            "the effect must reach `f` through `helper`: {e:?}"
        );
    }

    #[test]
    fn an_effect_inside_a_callback_still_propagates() {
        // RQ-2's third shape, and the one the architect called load-bearing: the
        // callee knows nothing about the effect it ends up performing.
        let src = "module page\n\nimport Stores\nimport List\n\n\
                   fn f(ids: List<Int>) -> Int !{} { List.map(ids, id => Stores.get(id)) }\n";
        let lib2 = "module List\n\nfn map(xs: Int, f: Int) -> Int !{} { 0 }\n";
        let e = effects_of(&[LIB, lib2, src], "f");
        assert!(
            e.contains("database.read"),
            "the effect must escape the callback: {e:?}"
        );
    }

    #[test]
    fn a_pure_body_infers_nothing() {
        // The control. If everything came back effectful the tests above would
        // pass while measuring nothing.
        let src = "module page\n\nfn f() -> Int !{} { 1 + 2 }\n";
        assert!(effects_of(&[src], "f").is_empty());
    }

    #[test]
    fn a_declared_family_covers_its_members_but_not_the_reverse() {
        let mut declared = BTreeSet::new();
        declared.insert("database");
        assert!(covered(&declared, "database.read"));
        assert!(covered(&declared, "database.write"));

        let mut narrow = BTreeSet::new();
        narrow.insert("database.read");
        assert!(covered(&narrow, "database.read"));
        assert!(
            !covered(&narrow, "database.write"),
            "declaring a read must not permit a write"
        );
    }

    #[test]
    fn an_undeclared_effect_is_reported_with_its_chain() {
        let src = "module page\n\nimport Stores\n\n\
                   fn helper(id: Int) -> Int !{} { Stores.get(id) }\n\
                   fn f() -> Int !{} { helper(1) }\n";
        let (hirs, sigs, ws) = program(&[LIB, src]);
        let refs: Vec<&Hir> = hirs.iter().collect();
        let mut inf = Inference::new(&sigs, &ws);
        inf.run(&refs);

        // Unit 1 — `helper` is a SIBLING in that unit, and finding it is the
        // point of the test. `infer` without a unit resolves no siblings, by
        // design: a caller that does not say where it stands cannot be told
        // which `helper` it means.
        let hir = &refs[1];
        let (_, f) = hir.all_decls().find(|(_, d)| d.name == "f").expect("f");
        let found = inf.infer_at(1, hir.body(f.body.unwrap()));
        let missing = found.undeclared(Some(&[]));
        assert_eq!(missing.len(), 1, "{missing:?}");
        assert_eq!(missing[0].effect, "database.read");
        assert!(
            missing[0].via.describe().contains("helper"),
            "the reason must name the chain, not just the effect: {}",
            missing[0].via.describe()
        );
    }

    #[test]
    fn a_row_that_is_not_written_is_not_checked() {
        // An open row means "not stated". Inferring one for a declaration that
        // never claimed anything would reject working programs, and E2D's job
        // is to check claims, not to invent them.
        let found = Inferred {
            effects: BTreeSet::from(["database.read".to_string()]),
            sources: vec![Source {
                effect: "database.read".into(),
                span: 0..1,
                via: Via::Direct {
                    callee: "Stores.get".into(),
                },
            }],
        };
        assert!(found.undeclared(None).is_empty());
        assert_eq!(found.undeclared(Some(&[])).len(), 1);
    }

    #[test]
    fn a_view_may_not_reach_the_database_whatever_it_declares() {
        use crate::hir::DeclKind;
        let view = crate::hir::Decl {
            name: "V".into(),
            name_span: 0..0,
            kind: DeclKind::View,
            params: vec![],
            ret: None,
            ret_args: vec![],
            variants: None,
            fields: None,
            opaque_of: None,
            policies: vec![],
            imports: vec![],
            visibility: None,
            declared_effects: Some(vec![]),
            body: None,
            children: vec![],
        };
        assert!(forbidden_in(&view, Reuse::PerReader, None, "database.read").is_some());
        assert!(forbidden_in(&view, Reuse::PerReader, None, "network.fetch").is_some());
        // ...but rendering effects are exactly what a view is for.
        assert!(forbidden_in(&view, Reuse::PerReader, None, "dom.mutate").is_none());

        // A view placed where secrets live may read one while rendering. What
        // it may not do is put it in the markup, which is a different
        // invariant with a different code. R-003 was reported for both, and
        // "a page may not do this" was false about the one it may.
        assert!(
            forbidden_in(&view, Reuse::PerReader, None, "secret.read").is_some(),
            "a view with no declared world is rendered where secrets are not"
        );
        assert!(
            forbidden_in(
                &view,
                Reuse::PerReader,
                Some(crate::placement::World::Origin),
                "secret.read"
            )
            .is_none(),
            "an origin-placed view renders where secrets ARE"
        );
        assert!(
            forbidden_in(
                &view,
                Reuse::PerReader,
                Some(crate::placement::World::Browser),
                "secret.read"
            )
            .is_some(),
            "a browser-placed view still may not"
        );

        let f = crate::hir::Decl {
            kind: DeclKind::Fn,
            ..view
        };
        assert!(
            forbidden_in(&f, Reuse::PerReader, None, "database.read").is_none(),
            "an ordinary fn may read the database if it says so"
        );

        // (see below for the delegation control)

        // Reuse, not the declaration kind, is what makes a clock read wrong.
        // The same page is fine when it is rendered for its reader.
        let page = crate::hir::Decl {
            kind: DeclKind::Page,
            ..f
        };
        assert!(
            forbidden_in(&page, Reuse::PerReader, None, "clock.wall").is_none(),
            "a page rendered per request may read the clock"
        );
        assert!(forbidden_in(&page, Reuse::Build, None, "clock.wall").is_some());
        assert!(forbidden_in(&page, Reuse::SharedPartition, None, "clock.wall").is_some());

        // And the distinction inside the clock family is load-bearing: a
        // monotonic read measures a duration and does not make the output
        // depend on when it ran. Without this, the rule would be "a build-time
        // page may not use the clock", which is a different and wronger claim.
        assert!(
            forbidden_in(&page, Reuse::Build, None, "clock.read").is_none(),
            "a duration measurement does not make a build artifact irreproducible"
        );
    }
}
