//! **3b.3 — `Signature` carries resolved types.**
//!
//! Mid-migration: `Signature` holds both the written strings and the resolved
//! types while consumers move across one at a time. Architect ruling,
//! 2026-08-21 — that dual state is legal on a branch and **must not merge**.
//!
//! What these assert is the half that is finished: the resolved fields are
//! populated, and they carry identities rather than spellings.

use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

fn build(sources: &[&str]) -> (Vec<Hir>, Signatures) {
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    (hirs, sigs)
}

#[test]
fn a_signature_carries_resolved_parameter_and_return_types() {
    let src = "\
module m

type Store = Store { id: Int }
type Cart = Cart { n: Int }

fn get(s: Store) -> Cart { Cart(1) }
";
    let (_h, sigs) = build(&[src]);
    let sig = sigs.by_path("m.get").expect("the signature is there");

    let param = sig.resolved_params[0]
        .as_ref()
        .and_then(|r| r.resolved())
        .expect("the parameter resolves");
    let ret = sig
        .resolved_returns
        .as_ref()
        .and_then(|r| r.resolved())
        .expect("the return resolves");

    // Identities, not spellings.
    assert!(param.def_id().is_some());
    assert!(ret.def_id().is_some());
    assert!(!param.same_as(ret), "`Store` and `Cart` are two types");
    assert_eq!(param.display_name(), "Store");
    assert_eq!(ret.display_name(), "Cart");
}

#[test]
fn two_modules_declaring_one_name_give_two_identities() {
    // The property the whole repair exists for, carried into `Signature`.
    let a = "module alpha\n\nopaque type Tag = String\n\nfn take(t: Tag) -> Int { 0 }\n";
    let b = "module beta\n\nopaque type Tag = String\n\nfn take(t: Tag) -> Int { 0 }\n";
    let (_h, sigs) = build(&[a, b]);

    let one = sigs.by_path("alpha.take").expect("alpha");
    let two = sigs.by_path("beta.take").expect("beta");

    let p = |s: &pw_core::signatures::Signature| {
        s.resolved_params[0]
            .as_ref()
            .and_then(|r| r.resolved())
            .expect("resolves")
            .clone()
    };
    let (x, y) = (p(one), p(two));

    // Same spelling in both signatures — which is why the written fields could
    // never tell them apart.
    assert_eq!(one.params[0].as_deref(), Some("Tag"));
    assert_eq!(two.params[0].as_deref(), Some("Tag"));
    assert_eq!(x.display_name(), y.display_name());

    // Two identities.
    assert!(!x.same_as(&y), "two declarations, two types");
}

/// **PINS A FINDING: no callable can be generic.**
///
/// `fn id<T>(x: T) -> T` does not parse — `PW0007`, *expected a declaration,
/// found `<`*. The grammar calls `type_params()` for `opaque type` and for
/// `effect`, and for nothing else, so `fn`, `query`, `command`, `subscription`,
/// `resource` and `task` have no type parameters at all.
///
/// Found while wiring `Signature`'s resolved fields, which thread a binder and
/// the declaration's `type_params` precisely so a bound `T` resolves to its
/// binder rather than being reported unresolved. That plumbing is correct and
/// currently exercises nothing, because `decl.type_params` is always empty for
/// a callable.
///
/// **It matters for E9-V2.** The architect's discriminator is
///
/// ```text
/// identity<T>(T); identity(Store)   → infer T = Store
/// ```
///
/// and it cannot be written. V2 — *generic calls instantiate/unify through
/// resolved types* — therefore has no surface syntax to be tested against, and
/// closing it would require either adding callable generics to the language or
/// restating the gate in terms of the generics that do exist
/// (`Secret<C>`, `Session<S>`, `List<T>`). That is a language-surface decision,
/// not a repair, so it is recorded rather than taken.
#[test]
fn a_callable_cannot_declare_a_type_parameter() {
    let src = "module m\n\nfn id<T>(x: T) -> T { x }\n";
    let parsed = parse_tree(src);
    assert!(
        !parsed.ok(),
        "PINNED: callables gained type parameters. Delete this test, wire the \
         binder through, and revisit E9-V2 — its discriminator becomes \
         writable. Errors: {:?}",
        parsed.errors
    );

    // The discriminator: generics DO exist, on the declarations that have them.
    // Without this the assertion above would also hold for a parser that
    // rejected `<` everywhere.
    let ok = parse_tree("module m\n\nopaque type Secret<C> = String\n");
    assert!(ok.ok(), "{:?}", ok.errors);

    // And the plumbing is in place for the day it changes: a signature's
    // resolved fields are built with the declaration's binder and its
    // `type_params`, which is empty for every callable today.
    let (_h, sigs) = build(&["module m\n\nfn f(a: Int) -> Int { a }\n"]);
    let sig = sigs.by_path("m.f").expect("the signature is there");
    assert!(
        sig.resolved_params[0]
            .as_ref()
            .is_some_and(|r| r.is_resolved())
    );
}

#[test]
fn a_generic_argument_survives_into_the_signature() {
    // `List<Store>` must not arrive as `List`. That is the
    // `constructor_head_only` failure, and `returns_args` existed to work
    // around it — the resolved field has no such hole to work around.
    let src = "\
module m

type Store = Store { id: Int }

fn all() -> List<Store> { todo }
";
    let (_h, sigs) = build(&[src]);
    let sig = sigs.by_path("m.all").expect("the signature is there");
    let ret = sig
        .resolved_returns
        .as_ref()
        .and_then(|r| r.resolved())
        .expect("resolves");

    assert_eq!(ret.args().len(), 1, "the argument survived");
    assert!(
        ret.args()[0].def_id().is_some(),
        "and it is resolved, not a spelling"
    );
    assert_eq!(ret.display_name(), "List<Store>");
}

#[test]
fn an_unannotated_parameter_and_an_unresolvable_one_are_different() {
    // Collapsing both to `None` would make *no annotation* and *an annotation
    // naming nothing* the same answer, which is why the field holds a
    // `TypeResolution` rather than an `Option<ResolvedType>`.
    let src = "module m\n\nfn f(a, b: Nonexistent) -> Int { 0 }\n";
    let (_h, sigs) = build(&[src]);
    let Some(sig) = sigs.by_path("m.f") else {
        // The parser may reject an unannotated parameter; if so this test has
        // nothing to measure and says so rather than passing quietly.
        panic!("m.f did not survive lowering, so this proves nothing");
    };

    assert!(sig.resolved_params[0].is_none(), "no annotation is `None`");
    let second = sig.resolved_params[1]
        .as_ref()
        .expect("an annotation is present");
    assert!(
        !second.is_resolved(),
        "and it names nothing, which is a different fact: {second:?}"
    );
}
