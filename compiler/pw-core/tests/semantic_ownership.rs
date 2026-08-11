//! **Every call-shaped thing the author wrote has exactly one semantic owner.**
//!
//! Architect ruling, 2026-08-10:
//!
//! > You already have provenance for semantic conclusions. I would add an
//! > earlier completeness statement: **every semantic use in accepted code has
//! > an owner.**
//! >
//! > ```text
//! > call-shaped source
//! >         ↓
//! > exactly one of
//! > TermCall          → DefId
//! > Constructor       → DefId
//! > PolicyApply       → PolicyOpId
//! > IntrinsicSyntax   → intrinsic kind
//! > CSS/value apply   → value-domain identity
//! > ```
//! >
//! > Never: call-shaped thing → nobody owns it → analyses see nothing.
//! >
//! > Provenance answers: *why did the compiler conclude this?* Semantic
//! > ownership answers: *did the compiler ever assign meaning to what the
//! > author wrote?* The second hole is what you just found.
//!
//! # What the corpus contains
//!
//! 49 call-shaped constructs resolve to a declaration. About thirty-five do
//! not, and they are **not one problem**:
//!
//! ```text
//! Ok, Err                        language constructors, no DefId exists
//! CartError.ItemUnavailable      a declared union's constructor
//! merge_by_field, content_address a policy operator (src/policy.rs)
//! User(consumer)                 a privacy label constructor
//! translate, scale, repeat       CSS value functions
//! self.style.set_width           a member of a value with no resolved type
//! release(h), draw(ctx)          a policy head whose value is a BLOCK
//! for (i, v) in ..               language syntax the parser lowered as a call
//! current_session, add_to_cart   NOBODY
//! ```
//!
//! Only the last line is a defect in the program. Every line above it is a
//! defect in the *compiler's* vocabulary — a construct with real meaning that
//! no owner claims — and a `PW0024` that could not tell them apart reported all
//! of them. It was written, measured at sixteen corpus errors, narrowed to
//! eleven, and reverted twice rather than landed over-broad.
//!
//! # The gate
//!
//! This file classifies each one and freezes the classification. It is the
//! thing that makes `PW0024` implementable: the diagnostic's subject is the
//! `Unowned` class, and the class is small and named.

use std::collections::{BTreeMap, BTreeSet};

use pw_core::hir::{DeclKind, Expr, Hir};
use pw_core::infer::path_of;
use pw_core::lower::lower_file;
use pw_core::resolve::{Resolution, Workspace, local_bindings};
use pw_syntax::parse_tree;

/// Who claims a call-shaped construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    /// Resolves to a declaration. The healthy case.
    Term,
    /// A parameter or a `let` in scope: `handle.close()`.
    Local,
    /// A declared type's constructor: `CartError.ItemUnavailable(item)`.
    Constructor,
    /// Language syntax the grammar lowers into call shape: `Ok(x)`,
    /// `resumable(captures = ..)`, and — see the test — `for`.
    Intrinsic,
    /// A policy operator or head, owned by `src/policy.rs`.
    Policy,
    /// A function of the CSS value domain: `translate(x, y)`.
    ValueDomain,
    /// A member of a value whose type did not resolve: `self.style.set_width`.
    /// Distinct from `Unowned` because the missing thing is the RECEIVER's
    /// type, not the name.
    Member,
    /// **Nobody.** Every analysis contributes nothing, and nothing says so.
    Unowned,
}

/// Constructors and forms the language provides rather than a module.
///
/// `for` was here under protest until 2026-08-10 — `for (i, v) in xs { .. }`
/// lowered to `Expr::Call` with the callee `Name("for")` — and is gone because
/// the parse was repaired rather than grandfathered. Architect ruling: *syntax
/// must remain syntax; terms must remain terms.*
///
/// The list is the compiler's own, so a spelling cannot be exempt here and
/// reportable there.
const INTRINSIC: &[&str] = pw_core::resolve::INTRINSIC_CALLS;

/// The CSS value functions the corpus uses. A value domain, not a term
/// namespace: `translate(box.x, y)` produces a transform, and its arguments are
/// lengths.
const VALUE_DOMAIN: &[&str] = &["translate", "scale", "rotate", "repeat", "minmax", "calc"];

/// The same program with extra sources appended, so the gate can be run against
/// a deliberately broken one.
fn program_plus(extra: &[&str]) -> (Vec<String>, Vec<Hir>) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut names = Vec::new();
    let mut srcs = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/accepted",
        "examples/store",
    ] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            names.push(p.strip_prefix(&root).unwrap().to_string_lossy().to_string());
            srcs.push(std::fs::read_to_string(&p).expect("read"));
        }
    }
    names.push("examples/domain.pw".to_string());
    srcs.push(std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"));
    for (i, e) in extra.iter().enumerate() {
        names.push(format!("<synthetic {i}>"));
        srcs.push((*e).to_string());
    }

    let hirs = srcs
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    (names, hirs)
}

/// Classify every call-shaped construct in the accepted program.
fn ownership() -> BTreeMap<Owner, BTreeSet<String>> {
    ownership_of(&[])
}

fn ownership_of(extra: &[&str]) -> BTreeMap<Owner, BTreeSet<String>> {
    let (_, hirs) = program_plus(extra);
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);

    // Every type the program declares, so a constructor can be told from a
    // module member. `CartError.ItemUnavailable` and `Stores.get` are the same
    // shape and different facts.
    let types: BTreeSet<String> = refs
        .iter()
        .flat_map(|h| h.all_decls().collect::<Vec<_>>())
        .filter(|(_, d)| matches!(d.kind, DeclKind::Type | DeclKind::Opaque))
        .map(|(_, d)| d.name.clone())
        .collect();

    let mut out: BTreeMap<Owner, BTreeSet<String>> = BTreeMap::new();
    for (unit, hir) in refs.iter().enumerate() {
        for (_, d) in hir.all_decls() {
            let Some(bid) = d.body else { continue };
            let body = hir.body(bid);
            let locals = local_bindings(body);
            let params: BTreeSet<String> = d.params.iter().map(|p| p.name.clone()).collect();

            for id in body.walk() {
                let Expr::Call { callee, .. } = body.expr(id) else {
                    continue;
                };
                let path = path_of(body, *callee);
                if path.is_empty() {
                    continue;
                }
                let head = path.split('.').next().unwrap_or_default();
                let last = path.rsplit('.').next().unwrap_or_default();

                let owner = if !matches!(ws.resolve_path(unit, &path), Resolution::Unresolved) {
                    Owner::Term
                } else if INTRINSIC.contains(&path.as_str()) {
                    Owner::Intrinsic
                } else if types.contains(head) && path.contains('.') {
                    Owner::Constructor
                } else if pw_core::policy::domain_of(last).is_some()
                    || pw_core::policy::domain_of(head).is_some()
                    || is_policy_operator(last)
                    || is_label_constructor(last)
                {
                    Owner::Policy
                } else if VALUE_DOMAIN.contains(&last) {
                    Owner::ValueDomain
                } else if locals.contains(head) || params.contains(head) || head == "self" {
                    if path.contains('.') {
                        Owner::Member
                    } else {
                        Owner::Local
                    }
                } else if path.contains('.') && !ws.sees_module(unit, head) {
                    // A dotted path whose head names no module and no type is a
                    // member access on a value. What is missing is the
                    // RECEIVER's type, not the name — and `unresolved_uses`
                    // already reports the case where the head does look like a
                    // module. Two of these are here because the parser never
                    // bound the receiver: see
                    // `a_loop_and_a_policy_block_do_not_bind_their_binders`.
                    Owner::Member
                } else {
                    Owner::Unowned
                };
                out.entry(owner).or_default().insert(path);
            }
        }
    }
    out
}

/// Any operator under any policy head. Contextual resolution happens where the
/// head is known; here the question is only whether `src/policy.rs` claims the
/// spelling at all.
fn is_policy_operator(name: &str) -> bool {
    ["retry", "reconnect", "conflict", "identity"]
        .iter()
        .any(|h| pw_core::policy::operator(h, name).is_some())
}

/// `privacy User(consumer)` — the constructors of the privacy lattice.
fn is_label_constructor(name: &str) -> bool {
    matches!(name, "Session" | "User" | "Organization" | "Device")
}

// --- the gate ----------------------------------------------------------------

/// **The `Unowned` class is empty.**
///
/// Every call-shaped construct in the accepted program now has exactly one
/// semantic owner. It was four:
///
/// ```text
/// current_session    an import; `context.pw` had declared it since E2C
/// current_consumer   the NAME was wrong; `Carts.add` takes a SessionId
/// include_markdown   a new tracked build-input operation, `placement build`
/// add_to_cart        an import, plus `view` becoming a component kind so the
///                    dependency had a contract to live in
/// ```
///
/// This is the precondition `PW0024` was missing. A diagnostic whose subject is
/// "a call nobody owns" can be landed against a corpus where that set is empty
/// — the two earlier attempts reported sixteen and then eleven, because every
/// class below was in the residue.
#[test]
fn nothing_call_shaped_is_unowned() {
    let by_owner = ownership();
    let unowned = by_owner.get(&Owner::Unowned).cloned().unwrap_or_default();
    assert!(
        unowned.is_empty(),
        "a call-shaped construct has no semantic owner: {unowned:?}\n\n\
         Either it is a real unresolved name — classify it against \
         `docs/RISK_QUEUE.md` and repair it by what it IS — or it is a \
         construct with meaning this file has not learned to attribute, in \
         which case reporting it would be the over-broad `PW0024` again."
    );
}

/// **The negative control**, without which the test above passes for a
/// classifier that never returns `Unowned`.
///
/// `docs/RISK_QUEUE.md`'s admissibility rule: *a checker result is not
/// admissible evidence until its instrument has a negative control proving it
/// can detect the corresponding failure.* An empty set is exactly the result
/// that needs one.
#[test]
fn the_gate_detects_a_call_that_nobody_owns() {
    // The shape of all four repaired defects: a bare call, to a name that is
    // not a local, not a parameter, not an intrinsic, not a policy operator and
    // not a value-domain function.
    let broken = "\
module probe.broken

view Button(label: String) !{} {
    <button on:press={resumable() => vanished(label)}>{label}</button>
}
";
    let by_owner = ownership_of(&[broken]);
    let unowned = by_owner.get(&Owner::Unowned).cloned().unwrap_or_default();
    assert_eq!(
        unowned,
        BTreeSet::from(["vanished".to_string()]),
        "the gate did not detect a call to a name that does not exist"
    );

    // And the discriminating half: the same program with the call resolved is
    // clean, so the gate is reacting to the resolution and not to the file.
    let fixed = "\
module probe.fixed

fn vanished(s: String) -> String { s }

view Button(label: String) !{} {
    <button on:press={resumable() => vanished(label)}>{label}</button>
}
";
    let by_owner = ownership_of(&[fixed]);
    assert!(
        by_owner
            .get(&Owner::Unowned)
            .cloned()
            .unwrap_or_default()
            .is_empty(),
        "declaring the function did not silence the gate"
    );
}

/// **Every owner class actually claims something**, so the gate is not passing
/// because the classifier sorts everything into one bucket.
///
/// `Unowned` is excluded deliberately — it is empty, which is the result, and
/// `the_gate_detects_a_call_that_nobody_owns` is what proves the class is still
/// reachable.
#[test]
fn every_owner_class_actually_claims_something() {
    let by_owner = ownership();
    for owner in [
        Owner::Term,
        Owner::Constructor,
        Owner::Intrinsic,
        Owner::Policy,
        Owner::ValueDomain,
        Owner::Member,
    ] {
        let n = by_owner.get(&owner).map(|s| s.len()).unwrap_or(0);
        assert!(
            n > 0,
            "no call in the accepted corpus is owned by {owner:?}, so the gate \
             cannot tell that class from an empty one: {:?}",
            by_owner
                .iter()
                .map(|(k, v)| (k, v.len()))
                .collect::<Vec<_>>()
        );
    }

    // And the healthy majority is genuinely the majority — a gate that
    // classified four calls and left ninety unexamined would also pass above.
    let term = by_owner[&Owner::Term].len();
    assert!(
        term >= 30,
        "only {term} calls resolve to a declaration, which is too few for this \
         corpus — the classifier is probably failing to resolve rather than \
         the program failing to declare"
    );
}

/// **A `for` loop is syntax, and it binds. A policy block's binder still does
/// not.**
///
/// The loop was `Expr::Call { callee: Name("for") }` until 2026-08-10, with two
/// consequences: every analysis that walks calls saw a call to something no
/// program declares, and the loop variable was introduced by nothing — so
/// `for badge in badges { badge.style.set_width(u) }` reported `badge` as an
/// undeclared name.
///
/// The policy-block half remains. `draw(ctx) { ctx.rect(..) }` binds `ctx` by
/// nothing, because `draw` is a policy head whose value is a `Domain::Body` and
/// the grammar still reads it as a call followed by a block. That is the
/// `PolicyExpr` split's work, and this test is what will say when it lands.
#[test]
fn a_for_loop_is_syntax_that_binds_and_a_policy_block_is_not_yet() {
    use pw_core::hir::Expr;

    let src = "\
module m

fn f(xs: List<Int>) -> Int {
    for x in xs {
        x
    }
}
";
    let hir = lower_file(src, &parse_tree(src).green);
    let (_, d) = hir.all_decls().find(|(_, d)| d.name == "f").expect("f");
    let body = hir.body(d.body.expect("body"));
    assert!(
        body.walk()
            .iter()
            .any(|id| matches!(body.expr(*id), Expr::For { .. })),
        "the loop is a loop, not a call"
    );
    assert!(
        !body.walk().iter().any(|id| matches!(
            body.expr(*id),
            Expr::Call { callee, .. } if path_of(body, *callee) == "for"
        )),
        "and nothing calls `for`"
    );
    assert!(
        local_bindings(body).contains("x"),
        "and the loop variable is a binding"
    );

    // The tuple form binds BOTH names, which is what says the binder is a
    // pattern rather than a single identifier the parser special-cased.
    let tuple = "\
module m

fn f(xs: List<Int>) -> Int {
    for (i, v) in xs.enumerate() {
        v
    }
}
";
    let hir = lower_file(tuple, &parse_tree(tuple).green);
    let (_, d) = hir.all_decls().find(|(_, d)| d.name == "f").expect("f");
    let body = hir.body(d.body.expect("body"));
    let names = local_bindings(body);
    assert!(names.contains("i") && names.contains("v"), "{names:?}");

    // The control that was already correct, so the repair is attributable to
    // the loop and not to `local_bindings`.
    let each = "\
module m

view V(xs: List<Int>) !{} {
    {#each xs as x (x)}
        <p>{x}</p>
    {/each}
}
";
    let hir = lower_file(each, &parse_tree(each).green);
    let (_, d) = hir.all_decls().find(|(_, d)| d.name == "V").expect("V");
    let body = hir.body(d.body.expect("body"));
    assert!(local_bindings(body).contains("x"));

    // And the half the repair has not reached.
    let policy = "\
module m

paint P(w: Float) !{ paint.custom } {
    inputs w

    draw(ctx) {
        ctx.rect(w)
    }
}
";
    let hir = lower_file(policy, &parse_tree(policy).green);
    let (_, d) = hir.all_decls().find(|(_, d)| d.name == "P").expect("P");
    let body = hir.body(d.body.expect("body"));
    assert!(
        !local_bindings(body).contains("ctx"),
        "TODAY: a policy block's binder is bound by nothing. When the \
         `PolicyExpr` split reaches `Domain::Body`, this fails and `ctx.rect` \
         moves from `Member` to `Local` in the gate above."
    );
}

/// **A member call on an unresolved receiver is its own class.**
///
/// `self.style.set_width(w)` is unowned for a different reason than
/// `current_session()`: the name might be perfectly good, and what is missing
/// is the type of `self`. Reporting it as an unresolved NAME would be a wrong
/// diagnostic, which is why `PW0024`'s subject cannot be "every call that does
/// not resolve".
#[test]
fn a_member_of_an_unresolved_receiver_is_not_an_unresolved_name() {
    let by_owner = ownership();
    let members = &by_owner[&Owner::Member];
    assert!(
        members.iter().any(|m| m.starts_with("self.")),
        "expected the frame-phase fixtures' `self.…` calls here: {members:?}"
    );
    assert!(
        !by_owner
            .get(&Owner::Unowned)
            .cloned()
            .unwrap_or_default()
            .iter()
            .any(|u| u.contains('.') && u.starts_with("self")),
        "a `self.…` call was reported as an unresolved name"
    );
}
