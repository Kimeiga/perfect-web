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
    pub fn describe(&self) -> String {
        match self {
            Via::Direct { callee } => format!("`{callee}` performs it"),
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
fn family_of(effect: &str) -> &str {
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
    known: BTreeMap<String, BTreeSet<Effect>>,
}

impl<'a> Inference<'a> {
    pub fn new(sigs: &'a Signatures) -> Self {
        Self {
            sigs,
            known: BTreeMap::new(),
        }
    }

    /// Seed from every declared row in the program, then iterate to a fixed
    /// point over local helpers.
    pub fn run(&mut self, hirs: &[&Hir]) {
        for hir in hirs {
            for (_, d) in hir.all_decls() {
                if let Some(row) = &d.declared_effects {
                    self.known.insert(
                        d.name.clone(),
                        row.iter().map(|e| e.written.clone()).collect(),
                    );
                }
            }
        }

        // A helper that does not declare a row still has effects. Propagate
        // until nothing changes — the effect set is finite, so this terminates.
        for _ in 0..8 {
            let mut changed = false;
            for hir in hirs {
                for (_, d) in hir.all_decls() {
                    let Some(body_id) = d.body else { continue };
                    let found = self.infer(hir.body(body_id));
                    let entry = self.known.entry(d.name.clone()).or_default();
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
        let mut out = self.infer(body);
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

            let declared = types.get(&receiver).map(String::as_str);
            // The unique-member fallback applies only to a value whose type is
            // not yet known — a lambda parameter. A CAPITALISED receiver names
            // a type or module, and if it has no such member the answer is
            // "unknown", not somebody else's member.
            //
            // Without this, `Money.add` borrowed `Carts.add`'s row and reported
            // a pure calculation as writing to the database.
            let receiver_is_a_name = receiver.chars().next().is_some_and(char::is_uppercase);
            let sig = match declared {
                Some(t) => self.sigs.member(Some(t), &member),
                None if receiver_is_a_name => None,
                None => self.sigs.member(None, &member),
            };
            let Some(sig) = sig else { continue };
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
    pub fn infer(&self, body: &Body) -> Inferred {
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
            let effects: Vec<Effect> = match self.sigs.by_path(&path) {
                Some(sig) => sig.effects.clone(),
                None => self
                    .known
                    .get(path.rsplit('.').next().unwrap_or(&path))
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

    /// The effects a named declaration performs, after propagation.
    pub fn effects_of(&self, name: &str) -> Option<&BTreeSet<Effect>> {
        self.known.get(name)
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

/// Spans in a body where work does **not** happen during render.
///
/// Charter §7.5A's frame phases, as the corpus writes them:
///
/// - an event handler (`on:press={..}`) runs when the user acts;
/// - a `<stream query={..}>` region resolves after the shell is sent;
/// - `post_paint` and `frame` blocks run on a later phase.
///
/// A view containing any of these is not *doing* the work while rendering, and
/// treating it as if it were reported three correct accepted programs as
/// violations. The distinction is the point of frame phases, not an exemption
/// from them.
pub fn deferred_spans(body: &Body) -> Vec<Span> {
    use crate::hir::{AttrValue, Node};
    let mut out = Vec::new();

    for id in body.walk() {
        match body.expr(id) {
            Expr::Keyword { keyword, .. }
                if matches!(
                    keyword.as_str(),
                    "post_paint" | "frame" | "task" | "subscribe" | "resource" | "animate"
                ) =>
            {
                // The whole statement, not its `block` field: `post_paint
                // !{ .. } { .. }` writes a row between the keyword and the
                // braces, so the block is a sibling of the row rather than the
                // statement's only child.
                out.push(body.expr_span(id));
            }
            Expr::Template { roots, .. } => {
                for n in body.walk_markup(roots) {
                    let Node::Element { tag, attrs, .. } = body.node(n) else {
                        continue;
                    };
                    for a in attrs {
                        let deferred =
                            a.name.starts_with("on:") || (tag == "stream" && a.name == "query");
                        if !deferred {
                            continue;
                        }
                        if let AttrValue::Expr(e) = a.value {
                            out.push(body.expr_span(e));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
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
pub fn forbidden_in(decl: &Decl, effect: &str) -> Option<&'static str> {
    use crate::hir::DeclKind::*;
    let family = effect.split('.').next().unwrap_or(effect);
    match (decl.kind, family) {
        (View | Component | Page, "database") => {
            Some("a view renders; it cannot reach the database while doing so")
        }
        (View | Component | Page, "network") => {
            Some("a view renders; fetching during render is what streaming exists to avoid")
        }
        (View | Component | Page, "secret") => Some("a view is rendered where secrets are not"),
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

    fn program(sources: &[&str]) -> (Vec<Hir>, Signatures) {
        let owned: Vec<Hir> = sources
            .iter()
            .map(|s| lower_file(s, &parse_tree(s).green))
            .collect();
        let refs: Vec<&Hir> = owned.iter().collect();
        let ws = Workspace::build(&refs);
        let sigs = Signatures::build(&ws, &refs);
        (owned, sigs)
    }

    const LIB: &str = "module Stores\n\nfn get(id: Int) -> Int !{ database.read } { 0 }\n";

    fn effects_of(sources: &[&str], decl: &str) -> BTreeSet<Effect> {
        let (hirs, sigs) = program(sources);
        let refs: Vec<&Hir> = hirs.iter().collect();
        let mut inf = Inference::new(&sigs);
        inf.run(&refs);
        for hir in &refs {
            for (_, d) in hir.all_decls() {
                if d.name != decl {
                    continue;
                }
                let Some(b) = d.body else { continue };
                return inf.infer(hir.body(b)).effects;
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
        let (hirs, sigs) = program(&[LIB, src]);
        let refs: Vec<&Hir> = hirs.iter().collect();
        let mut inf = Inference::new(&sigs);
        inf.run(&refs);

        let hir = &refs[1];
        let (_, f) = hir.all_decls().find(|(_, d)| d.name == "f").expect("f");
        let found = inf.infer(hir.body(f.body.unwrap()));
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
        assert!(forbidden_in(&view, "database.read").is_some());
        assert!(forbidden_in(&view, "network.fetch").is_some());
        // ...but rendering effects are exactly what a view is for.
        assert!(forbidden_in(&view, "dom.mutate").is_none());

        let f = crate::hir::Decl {
            kind: DeclKind::Fn,
            ..view
        };
        assert!(
            forbidden_in(&f, "database.read").is_none(),
            "an ordinary fn may read the database if it says so"
        );
    }
}
