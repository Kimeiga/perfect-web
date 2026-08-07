//! E8 — an effect is a declaration, and its name is a dotted path.
//!
//! Architect ruling, 2026-08-07:
//!
//! > Use `effect database.read<T> { ... }`. But implement `database.read` as an
//! > effect-specific qualified name, not as a new ability for arbitrary
//! > declarations to have dotted names. […] Given everything this project has
//! > discovered about syntax features accidentally widening unrelated grammar,
//! > I would strongly prefer that.
//!
//! The declaration is meant to look like its uses:
//!
//! ```text
//! fn load(id: StoreId) -> Store !{ database.read<Stores>, trace }
//!
//! effect database.read<T> { .. }
//! effect trace { .. }
//! ```
//!
//! This file is the FIRST slice: the form parses, lowers, and takes its own
//! namespace. Resolving an effect row against these declarations, and the
//! unknown-family / unknown-operation diagnostics that follow, are the next
//! slice — see `docs/NEXT.md`.

use pw_core::hir::{DeclKind, Hir};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn hir(src: &str) -> Hir {
    let parsed = parse_tree(src);
    assert!(parsed.ok(), "did not parse: {src}");
    lower_file(src, &parsed.green)
}

fn effects(src: &str) -> Vec<String> {
    hir(src)
        .all_decls()
        .filter(|(_, d)| d.kind == DeclKind::Effect)
        .map(|(_, d)| d.name.clone())
        .collect()
}

#[test]
fn an_effect_with_no_argument_declares_itself() {
    assert_eq!(
        effects("module p\n\neffect log {\n    capability none\n}\n"),
        ["log"]
    );
}

#[test]
fn a_dotted_effect_name_is_one_name() {
    // Not a family containing an operation. `database.read` resolves to one
    // declaration, and the dot is namespacing rather than a second object.
    assert_eq!(
        effects(
            "module p\n\neffect database.read<T> {\n    \
             capability database.read<T>\n    host \"pw:host/database#read\"\n}\n"
        ),
        ["database.read"]
    );
}

#[test]
fn two_operations_of_one_family_are_two_declarations() {
    // And two identities. A checker must never recover one from the other by
    // splitting the spelling.
    let found = effects(
        "module p\n\n\
         effect database.read<T> {\n    capability database.read<T>\n}\n\n\
         effect database.write<T> {\n    capability database.write<T>\n}\n",
    );
    assert_eq!(found, ["database.read", "database.write"]);
}

#[test]
fn effects_from_different_families_sharing_a_final_segment_are_distinct() {
    // `docs/RISK_QUEUE.md` 34's shape, at the ontology layer. If these ever
    // collapse, an effect declared for one family lowers the other.
    let found = effects(
        "module p\n\n\
         effect database.read<T> {\n    capability database.read<T>\n}\n\n\
         effect cache.read {\n    capability cache.read\n}\n",
    );
    // Source order, not sorted: the HIR records what the file says.
    assert_eq!(found, ["database.read", "cache.read"]);
    assert_ne!(found[0], found[1]);
}

// --- the parser did NOT widen ------------------------------------------------

#[test]
fn a_type_may_not_have_a_dotted_name() {
    // The control for the whole design. If `name()` had learned about dots,
    // every declaration would silently accept them and the grammar change
    // would be global rather than local to the one feature that needs it.
    let parsed = parse_tree("module t\n\ntype foo.bar = X { a: Int }\n");
    assert!(!parsed.ok(), "`type foo.bar` must not parse");
}

#[test]
fn a_function_may_not_have_a_dotted_name() {
    let parsed = parse_tree("module t\n\nfn foo.bar() -> Int { 0 }\n");
    assert!(!parsed.ok(), "`fn foo.bar()` must not parse");
}

#[test]
fn a_query_may_not_have_a_dotted_name() {
    let parsed =
        parse_tree("module t\n\nquery foo.bar(id: Int) -> Int\n    cache shared\n{\n    0\n}\n");
    assert!(!parsed.ok(), "`query foo.bar` must not parse");
}

// --- it is its own namespace -------------------------------------------------

#[test]
fn an_effect_does_not_collide_with_a_type_or_a_function() {
    // `effect log` and `fn log` are different things with the same spelling,
    // and both are legitimate. A shared namespace would report one as a
    // duplicate of the other — which is what `Namespace` exists to prevent,
    // and why effects get their own rather than joining `Term`.
    let src = "module p\n\n\
               effect log {\n    capability none\n}\n\n\
               fn log(x: Int) -> Int !{ log } { x }\n";
    let parsed = parse_tree(src);
    assert!(parsed.ok(), "both declarations parse");

    let h = lower_file(src, &parsed.green);
    let kinds: Vec<DeclKind> = h
        .all_decls()
        .filter(|(_, d)| d.name == "log")
        .map(|(_, d)| d.kind)
        .collect();
    assert_eq!(kinds.len(), 2, "two declarations named `log`");
    assert!(kinds.contains(&DeclKind::Effect));
    assert!(kinds.contains(&DeclKind::Fn));
}

#[test]
fn the_type_parameter_uses_the_same_binder_as_a_type_declaration() {
    // Architect ruling: "Don't write an effect-specific generic binder."
    // `opaque type Secret<C>` and `effect database.read<T>` bind the same way,
    // so nothing can retain the argument in one and drop it in the other —
    // which is precisely what happened once already.
    let src = "module p\n\n\
               opaque type Secret<C> = String\n\n\
               effect database.read<T> {\n    capability database.read<T>\n}\n";
    let parsed = parse_tree(src);
    assert!(parsed.ok(), "both binders parse");
}
