//! The resolved signature is the one type authority for all migrated consumers.
use pw_core::binding::{Interface, RemoteSupport, remote_support};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::{DefId, Workspace};
use pw_core::resolved::{Builtin, Primitive, StableTypeId, TypeResolution};
use pw_core::signatures::Signatures;

fn build(sources: &[&str]) -> (Vec<Hir>, Workspace, Signatures) {
    let hirs: Vec<_> = sources
        .iter()
        .map(|s| {
            let parsed = pw_syntax::parse_tree(s);
            assert!(parsed.ok(), "{:?}", parsed.errors);
            lower_file(s, &parsed.green)
        })
        .collect();
    let refs: Vec<_> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    (hirs, ws, sigs)
}

#[test]
fn source_provenance_is_not_a_second_type_comparison_or_map_key() {
    let (_, _, sigs) = build(&[
        "module app\nopaque type Item = String\nfn one(x: List<Option<Item>>) -> List<Option<Item>> { x }\nfn two(x: List<Option<Item>>) -> List<Option<Item>> { x }\n",
    ]);
    let one = sigs.by_path("app.one").unwrap().params[0]
        .as_ref()
        .unwrap()
        .resolved()
        .unwrap();
    let two = sigs.by_path("app.two").unwrap().result().unwrap();
    assert_ne!(
        one.span(),
        two.span(),
        "different occurrences must retain their spans"
    );
    assert!(one.same_as(two));
    assert_eq!(one.semantic_key(), two.semantic_key());
    assert_eq!(one.as_builtin(), Some(Builtin::List));
    assert_eq!(one.args()[0].as_builtin(), Some(Builtin::Option));
    assert!(one.args()[0].args()[0].def_id().is_some());
}

#[test]
fn missing_and_unresolved_annotations_keep_their_original_positions() {
    let (hirs, _, sigs) =
        build(&["module app\nfn f(missing, bad: List<Absent>, good: Int) -> Absent { todo }\n"]);
    let sig = sigs.by_path("app.f").unwrap();
    assert_eq!(sig.params.len(), 3);
    assert!(sig.params[0].is_none());
    assert!(
        matches!(&sig.params[1], Some(TypeResolution::Unresolved { name, written }) if name == "Absent" && written == "List<Absent>")
    );
    assert_eq!(
        sig.params[2]
            .as_ref()
            .unwrap()
            .resolved()
            .unwrap()
            .as_primitive(),
        Some(Primitive::Int)
    );
    assert!(
        matches!(&sig.returns, Some(TypeResolution::Unresolved { name, .. }) if name == "Absent")
    );
    let facts = pw_core::boundary::TypeFacts::build(&hirs.iter().collect::<Vec<_>>(), &sigs);
    let RemoteSupport::Undetermined { positions } = remote_support(&Interface::from(sig), &facts)
    else {
        panic!("an unresolved interface cannot be transferable")
    };
    assert_eq!(
        positions
            .iter()
            .map(|p| p.position.as_str())
            .collect::<Vec<_>>(),
        ["argument 0", "argument 1", "result"]
    );
}

#[test]
fn ui_declarations_have_real_signature_entries_without_becoming_term_callables() {
    let (hirs, _, sigs) = build(&["module app\npage Screen(x: Int) { view { <p>x</p> } }\n"]);
    let (id, _) = hirs[0]
        .all_decls()
        .find(|(_, d)| d.name == "Screen")
        .unwrap();
    let sig = sigs
        .by_def(DefId {
            unit: 0,
            decl: id.0,
        })
        .unwrap();
    assert_eq!(
        sig.params[0]
            .as_ref()
            .unwrap()
            .resolved()
            .unwrap()
            .as_primitive(),
        Some(Primitive::Int)
    );
    assert!(
        sigs.in_module(Some("app"), "Screen").is_none(),
        "a page is not a term callable"
    );
}

#[test]
fn two_module_principals_do_not_collapse_to_their_last_segment() {
    let capability = include_str!("../../../packages/pw-platform-web/capability.pw");
    let a = "module alpha\nimport capability.{Session}\nopaque type Principal = String\nfn f() -> Session<Principal> { todo }\n";
    let b = a.replace("alpha", "beta");
    let (_, _, sigs) = build(&[capability, a, &b]);
    let a = &sigs.by_path("alpha.f").unwrap().label;
    let b = &sigs.by_path("beta.f").unwrap().label;
    assert!(!a.is_public() && !b.is_public());
    assert!(
        !a.flows_into(b) && !b.flows_into(a),
        "two nominal principals: {a}, {b}"
    );
}

#[test]
fn stable_projection_has_one_implementation_and_is_independent_of_file_order() {
    let a = "module alpha\nopaque type Item = String\n";
    let b = "module beta\nimport alpha.{Item}\nfn f(x: Result<List<Item>, String>) -> Result<List<Item>, String> { x }\n";
    let mut identities = vec![];
    for sources in [[a, b], [b, a]] {
        let (hirs, _, sigs) = build(&sources);
        let result = sigs.by_path("beta.f").unwrap().result().unwrap();
        let identity = sigs.stable_type(result).unwrap();
        assert_eq!(
            Some(identity.clone()),
            pw_core::resolved::stable(&hirs.iter().collect::<Vec<_>>(), result)
        );
        identities.push(identity);
    }
    assert_eq!(identities[0], identities[1]);
    assert_eq!(
        identities[0],
        StableTypeId::Builtin {
            ctor: "Result".into(),
            args: vec![
                StableTypeId::Builtin {
                    ctor: "List".into(),
                    args: vec![StableTypeId::Declared {
                        path: "alpha.Item".into(),
                        args: vec![]
                    }]
                },
                StableTypeId::Primitive("String".into())
            ]
        }
    );
}

#[test]
fn type_fragment_entry_point_uses_full_consumption_and_recursive_grammar() {
    for source in ["Result<List<Option<Int>>, String>", "()", "alpha.Item"] {
        assert!(pw_syntax::parse_type(source).ok(), "{source}");
    }
    for source in [
        "",
        "List<Int",
        "Int trailing",
        "Int; fn bad() {}",
        "List<Int>>",
    ] {
        assert!(
            !pw_syntax::parse_type(source).ok(),
            "must not accept a prefix of {source:?}"
        );
    }
}
