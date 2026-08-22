//! **Step 3a + 4 — a resolved type, and resolution that is recursive or nothing.**
//!
//! Architect ruling, 2026-08-21, treating them as ONE gate rather than two
//! independently valid end states:
//!
//! > There should never be a successful
//! >
//! > ```text
//! > ResolvedType { head: DefId, args: ["MenuItem"] }   // still textual
//! > ```
//!
//! and:
//!
//! > **Written syntax can explain a resolved type. It cannot compete with it.**
//!
//! What these hold is the property the whole repair rests on: **two nominal
//! types with one representation are two types.** `alpha.Tag` and `beta.Tag`
//! are both `String` underneath, and the compiler must not be able to say they
//! are the same — which it could, and did, while types were compared as
//! spellings.

use pw_core::hir::DeclaredType;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::resolved::{Primitive, TypeResolution, resolve};
use pw_syntax::parse_tree;

/// Build a workspace from sources, and resolve a written type as unit `at`
/// would see it.
fn in_program(sources: &[&str], at: usize, params: &[&str], written: &str) -> TypeResolution {
    let hirs: Vec<pw_core::hir::Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let declared = match written.split_once('<') {
        None => DeclaredType::new(written, Vec::new()),
        Some((head, rest)) => DeclaredType::new(
            head,
            rest.trim_end_matches('>')
                .split(',')
                .map(|a| a.trim().to_string())
                .collect(),
        ),
    };
    let owned: Vec<String> = params.iter().map(|p| p.to_string()).collect();
    resolve(&ws, at, &owned, &declared, 0..0)
}

const ALPHA: &str = "module alpha\n\nopaque type Tag = String\n";
const BETA: &str = "module beta\n\nopaque type Tag = String\n";
const DOMAIN: &str = "\
module domain

type MenuItem = MenuItem { id: Int }
type Store = Store { id: Int }
opaque type StoreId = String
";

// --- the frozen discriminators ----------------------------------------------

/// **Two nominal types with one representation are two types.**
///
/// The property the repair exists for, stated first. `alpha.Tag` and
/// `beta.Tag` are both `opaque type Tag = String`, so every ABI they reach is
/// identical — and they are not the same type.
///
/// > `alpha.Tag ≠ beta.Tag` even when `ABI(alpha.Tag) = ABI(beta.Tag) = string`.
/// > Representation transparency belongs to ABI lowering, not value typing.
#[test]
fn two_declarations_of_one_spelling_are_two_types() {
    let a = in_program(&[ALPHA, BETA], 0, &[], "Tag");
    let b = in_program(&[ALPHA, BETA], 1, &[], "Tag");

    let (Some(a), Some(b)) = (a.resolved(), b.resolved()) else {
        panic!("both resolve: {a:?} / {b:?}");
    };

    // The spellings agree, which is exactly why comparing spellings failed.
    assert_eq!(a.display_name(), b.display_name());
    assert_eq!(a.display_name(), "Tag");

    // The identities do not.
    assert!(
        !a.same_as(b),
        "`alpha.Tag` and `beta.Tag` are two declarations and must be two types"
    );
    assert_ne!(a.def_id(), b.def_id());

    // And the control: each is the same as itself, so this is measuring
    // identity rather than a comparison that always answers `false`.
    assert!(a.same_as(a) && b.same_as(b));
}

/// **`opaque type Tag = String` does not make a `Tag` a `String`.**
///
/// The representation is `String`; the type is not. A resolver that unwrapped
/// opacity would make the test above pass and this one fail, so both are
/// needed to pin the distinction.
#[test]
fn an_opaque_type_is_not_its_representation() {
    let tag = in_program(&[ALPHA], 0, &[], "Tag");
    let string = in_program(&[ALPHA], 0, &[], "String");

    let (Some(tag), Some(string)) = (tag.resolved(), string.resolved()) else {
        panic!("both resolve");
    };
    assert!(!tag.same_as(string));
    assert_eq!(string.as_primitive(), Some(Primitive::Str));
    assert_eq!(
        tag.as_primitive(),
        None,
        "a nominal type is not a primitive"
    );
    assert!(tag.def_id().is_some());
}

/// **A generic argument is resolved, or the whole type is not.**
///
/// The half-resolved state the ruling forbids. `List<MenuItem>` must carry a
/// resolved `MenuItem`, and `List<Stroe>` must not resolve at all — not
/// resolve-with-a-string-inside, which would look resolved at every call site.
#[test]
fn resolution_reaches_every_argument_or_reports() {
    let ok = in_program(&[DOMAIN], 0, &[], "List<MenuItem>");
    let Some(list) = ok.resolved() else {
        panic!("{ok:?}")
    };
    assert_eq!(list.args().len(), 1, "the argument survived");
    assert!(
        list.args()[0].def_id().is_some(),
        "and it is RESOLVED, not a spelling"
    );
    assert_eq!(list.args()[0].display_name(), "MenuItem");

    // A misspelt argument fails the whole type, naming the argument rather than
    // the type — `List<Stroe>` sends the reader to `List`.
    let bad = in_program(&[DOMAIN], 0, &[], "List<Stroe>");
    match bad {
        TypeResolution::Unresolved { name, written } => {
            assert_eq!(name, "Stroe");
            assert_eq!(written, "List<Stroe>");
        }
        other => panic!("must not resolve: {other:?}"),
    }
}

/// **Two generics of one constructor differ by their arguments.**
///
/// `List<MenuItem>` and `List<Store>` share a head, so a comparison that
/// stopped at the constructor would call them equal — the `constructor_head_only`
/// failure that `DeclaredType` was built to prevent, at the next layer.
#[test]
fn a_generic_differs_by_its_argument() {
    let items = in_program(&[DOMAIN], 0, &[], "List<MenuItem>");
    let stores = in_program(&[DOMAIN], 0, &[], "List<Store>");
    let again = in_program(&[DOMAIN], 0, &[], "List<MenuItem>");

    let (Some(items), Some(stores), Some(again)) =
        (items.resolved(), stores.resolved(), again.resolved())
    else {
        panic!("all three resolve");
    };
    assert!(
        !items.same_as(stores),
        "the argument is part of the identity"
    );
    assert!(
        items.same_as(again),
        "and the same argument is the same type"
    );
}

/// **A type parameter is an identity, not a missing declaration.**
///
/// `T` in `fn identity<T>(x: T) -> T` binds a variable. A resolver that did not
/// know would report every generic declaration as unresolved.
#[test]
fn a_bound_type_parameter_resolves_as_itself() {
    let t = in_program(&[DOMAIN], 0, &["T"], "T");
    let Some(t) = t.resolved() else {
        panic!("{t:?}")
    };
    assert_eq!(t.type_parameter(), Some("T"));
    assert_eq!(t.def_id(), None, "a parameter is not a declaration");

    // The discriminator: the same name with nothing binding it is unresolved,
    // so this is recognising a binder rather than accepting any capital letter.
    let free = in_program(&[DOMAIN], 0, &[], "T");
    assert!(!free.is_resolved(), "{free:?}");
}

// --- provenance, which may explain but not compete --------------------------

/// **The spelling is reachable, and is not an identity.**
///
/// A diagnostic says *expected `StoreId`* in the reader's own words. What no
/// consumer gets is a string it could compare — `same_as` is the only
/// comparison, and `written_source` returns an owned `String` precisely so
/// that using it as an identity looks like what it is.
#[test]
fn the_written_spelling_survives_as_provenance() {
    let t = in_program(&[DOMAIN], 0, &[], "List<MenuItem>");
    let Some(t) = t.resolved() else {
        panic!("{t:?}")
    };
    assert_eq!(t.display_name(), "List<MenuItem>");
    assert_eq!(t.written_source(), "List<MenuItem>");

    // And the point: two types with one spelling have one `display_name` and
    // two identities, so a consumer using the name as meaning is wrong in
    // exactly the way this module exists to make visible.
    let a = in_program(&[ALPHA, BETA], 0, &[], "Tag");
    let b = in_program(&[ALPHA, BETA], 1, &[], "Tag");
    let (Some(a), Some(b)) = (a.resolved(), b.resolved()) else {
        panic!()
    };
    assert_eq!(a.display_name(), b.display_name());
    assert!(!a.same_as(b));
}

/// A name that is neither a primitive, a parameter, nor a declaration is
/// **Unresolved** — never a placeholder that means something.
#[test]
fn an_undeclared_name_is_unresolved_rather_than_assumed() {
    for name in ["Nonexistent", "domain.Nonexistent"] {
        let got = in_program(&[DOMAIN], 0, &[], name);
        assert!(
            matches!(got, TypeResolution::Unresolved { .. }),
            "{name}: {got:?}"
        );
    }

    // The control, in the same program: a name that IS declared resolves, so
    // the assertions above are not measuring a resolver that refuses
    // everything.
    assert!(in_program(&[DOMAIN], 0, &[], "Store").is_resolved());
}

/// **Blocked is not Unresolved.**
///
/// *This name is not a type here* and *I could not run* are different facts,
/// and a consumer that cannot tell them apart reports a missing import as a
/// type error. A nested generic argument is the case this build does not model,
/// and it says so rather than resolving the outer type with a spelling inside.
#[test]
fn what_the_resolver_cannot_model_is_blocked_and_says_which() {
    let got = in_program(&[DOMAIN], 0, &[], "List<Option<Store>>");
    match got {
        TypeResolution::Blocked { why } => {
            assert!(why.contains("nests"), "{why}");
        }
        other => panic!("a nested generic is not resolvable yet: {other:?}"),
    }
}

/// **The real program, and the case that started this.**
///
/// `capability.SessionId` was the platform's and `domain.SessionId` was the
/// application's, both `opaque type SessionId = String`, and `add_to_cart`
/// handed one to something expecting the other. The application's declaration
/// is gone now — the architect's step 1 — so this asserts what should be true
/// after that repair AND that the resolver is what establishes it, rather than
/// the two having become indistinguishable again.
#[test]
fn the_platforms_session_id_is_one_type_in_the_real_program() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources: Vec<String> = Vec::new();
    for dir in ["packages/pw-platform-web", "examples/lib"] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            sources.push(std::fs::read_to_string(&p).expect("read"));
        }
    }
    sources.push(std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"));

    let hirs: Vec<pw_core::hir::Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();

    // Exactly one declaration named `SessionId` in the whole program.
    let declarers: Vec<usize> = refs
        .iter()
        .enumerate()
        .filter(|(_, h)| {
            h.all_decls()
                .any(|(_, d)| d.name == "SessionId" && d.opaque_of.is_some())
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        declarers.len(),
        1,
        "two declarations of `SessionId` would be the defect this repair \
         removed, in units {declarers:?}"
    );

    // And it resolves to that one from every unit that can see it, so the
    // agreement is caused by identity rather than by a spelling that happens to
    // match. `examples/lib` imports it; the platform declares it.
    let ws = Workspace::build(&refs);
    let declared = DeclaredType::new("SessionId", Vec::new());
    let mut seen: Vec<pw_core::resolve::DefId> = Vec::new();
    for at in 0..refs.len() {
        if let TypeResolution::Resolved(t) = resolve(&ws, at, &[], &declared, 0..0)
            && let Some(def) = t.def_id()
        {
            seen.push(def);
        }
    }
    seen.sort();
    seen.dedup();
    assert_eq!(
        seen.len(),
        1,
        "every unit that resolves `SessionId` must reach the same declaration: {seen:?}"
    );
    assert!(
        seen[0].unit == declarers[0],
        "and it is the one that declares it"
    );
}
