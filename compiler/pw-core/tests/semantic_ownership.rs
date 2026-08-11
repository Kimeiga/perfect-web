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
/// `for` is here under protest: `for (i, v) in values.enumerate() { .. }`
/// lowers to `Expr::Call` with the callee `Name("for")`, which is a parse
/// defect and not an intrinsic call. It is classified rather than excluded so
/// that fixing the parse moves a row here instead of silently changing a count.
const INTRINSIC: &[&str] = &[
    "Ok",
    "Err",
    "Some",
    "None",
    "resumable",
    "for",
    "todo",
    "self",
];

/// The CSS value functions the corpus uses. A value domain, not a term
/// namespace: `translate(box.x, y)` produces a transform, and its arguments are
/// lengths.
const VALUE_DOMAIN: &[&str] = &["translate", "scale", "rotate", "repeat", "minmax", "calc"];

fn program() -> (Vec<String>, Vec<Hir>) {
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

    let hirs = srcs
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    (names, hirs)
}

/// Classify every call-shaped construct in the accepted program.
fn ownership() -> BTreeMap<Owner, BTreeSet<String>> {
    let (_, hirs) = program();
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

/// **The `Unowned` class is exactly the calls already frozen elsewhere.**
///
/// This is the result that makes `PW0024` implementable. Once every other
/// class has an owner named, what is left is not a taxonomy problem — it is a
/// handful of names that do not exist, and each gets a different repair per the
/// architect's ruling. It was four; `current_session` is repaired.
#[test]
fn nothing_call_shaped_is_unowned_except_the_known_defects() {
    let by_owner = ownership();
    let unowned = by_owner.get(&Owner::Unowned).cloned().unwrap_or_default();
    let expected: BTreeSet<String> = [
        // The last one. An ordinary application reference from a deferred
        // handler, which should become a component DEPENDENCY rather than
        // smuggle `database.write<Carts>` into rendering code — and that
        // depends on a `view` having a contract to depend from, which is the
        // architect's step 8.
        //
        // Three left on 2026-08-10, each repaired differently:
        //
        //     current_session    an import; the declaration already existed
        //     current_consumer   the NAME was wrong; `Carts.add` takes a
        //                        SessionId and the command invalidates a
        //                        session-keyed cache entry
        //     include_markdown   a new tracked build-input operation with
        //                        `placement build`
        "add_to_cart",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    assert_eq!(
        unowned, expected,
        "the set of call-shaped constructs nobody owns changed. Anything new \
         here is either a real unresolved name — in which case classify it \
         against `docs/RISK_QUEUE.md` — or a construct with meaning that this \
         file has not learned to attribute, in which case adding it to the \
         `Unowned` list would be the over-broad `PW0024` again."
    );
}

/// **Every other class is non-empty**, so the test above is not passing because
/// the classifier sorts everything into one bucket.
///
/// Without this, an `ownership()` that returned `Owner::Term` for every call
/// would satisfy the gate perfectly.
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
        Owner::Unowned,
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

/// **`for` is not an intrinsic call; the parser lowered it as one.**
///
/// `for (i, v) in values.enumerate() { .. }` produces `Expr::Call` whose callee
/// is `Name("for")`. Recorded as its own test rather than buried in the
/// `INTRINSIC` list, because the classification above is a workaround: a loop
/// is a control-flow form, and every analysis that walks calls currently sees
/// one that does not exist.
///
/// The consequence is not hypothetical — this is `Expr::Keyword`'s twin. There,
/// a call is demoted to syntax by its spelling and contributes nothing; here,
/// syntax is promoted to a call by its position and contributes a callee.
#[test]
fn the_parser_lowers_a_for_loop_as_a_call() {
    let src = "\
module m

fn f(xs: List<Int>) -> Int {
    for (i, v) in xs.enumerate() {
        v
    }
}
";
    let hir = lower_file(src, &parse_tree(src).green);
    let (_, d) = hir.all_decls().find(|(_, d)| d.name == "f").expect("f");
    let body = hir.body(d.body.expect("body"));
    let calls: Vec<String> = body
        .walk()
        .iter()
        .filter_map(|id| match body.expr(*id) {
            Expr::Call { callee, .. } => Some(path_of(body, *callee)),
            _ => None,
        })
        .collect();
    assert!(
        calls.iter().any(|c| c == "for"),
        "TODAY: the loop is a call to `for`. When the parser gains a loop form \
         this fails, and the `INTRINSIC` entry above should go with it: {calls:?}"
    );
}

/// **A `for` loop and a policy block do not bind their binders.**
///
/// `for badge in badges { badge.style.set_width(unit) }` — `badge` is not a
/// binding, because the loop is an `Expr::Call` and its "arguments" are not
/// patterns. Same for `draw(ctx) { ctx.rect(..) }`: `ctx` is bound by nothing.
///
/// So `resolve::local_bindings` — which every use-checker consults — is missing
/// two binding forms, and the names they bind look like undeclared ones. It
/// handles `{#each xs as x}` correctly, which is what says the gap is these two
/// forms rather than the function.
#[test]
fn a_loop_and_a_policy_block_do_not_bind_their_binders() {
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
        !local_bindings(body).contains("x"),
        "TODAY: the loop variable is not a binding. When the parser gains a \
         loop form this fails, and `badge.style.set_width` / `ctx.rect` move \
         from `Member` to `Local` in the gate above."
    );

    // The control: the binding form the parser DOES handle.
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
    assert!(
        local_bindings(body).contains("x"),
        "`{{#each}}` binds its name, so the gap above is these two forms and \
         not `local_bindings` itself"
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
        !by_owner[&Owner::Unowned]
            .iter()
            .any(|u| u.contains('.') && u.starts_with("self")),
        "a `self.…` call was reported as an unresolved name"
    );
}
