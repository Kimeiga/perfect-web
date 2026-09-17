//! Real source -> the existing parser -> HIR -> resolved semantic identity.
//! These are type-formation tests, not ordinary call/return compatibility tests.
use pw_core::hir::{DeclaredType, Hir};
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::resolved::{self, Builtin, Primitive, TypeResolution};

fn lower(source: &str) -> Hir {
    let tree = pw_syntax::parse_tree(source);
    assert!(tree.errors.is_empty(), "parse errors: {:?}", tree.errors);
    lower_file(source, &tree.green)
}

fn parameter(source: &str) -> (Hir, DeclaredType) {
    let hir = lower(source);
    let ty = hir
        .all_decls()
        .find(|(_, d)| d.name == "probe")
        .expect("probe")
        .1
        .params[0]
        .ty
        .clone()
        .expect("annotation");
    (hir, ty)
}

fn resolve_parameter(written: &str) -> TypeResolution {
    let source =
        format!("module m\ntype Item = Item {{ id: Int }}\nfn probe(x: {written}) {{ x }}\n");
    let (hir, ty) = parameter(&source);
    let ws = Workspace::build(&[&hir]);
    resolved::resolve(&ws, 0, None, &[], &ty, 0..source.len())
}

#[test]
fn nested_arguments_resolve_from_the_real_syntax_tree() {
    let got = resolve_parameter("Result<List<Option<Item>>, String>");
    let result = got.resolved().expect("complete nested type resolves");
    assert_eq!(result.as_builtin(), Some(Builtin::Result));
    assert_eq!(result.args().len(), 2);
    let list = &result.args()[0];
    assert_eq!(list.as_builtin(), Some(Builtin::List));
    let option = &list.args()[0];
    assert_eq!(option.as_builtin(), Some(Builtin::Option));
    assert!(option.args()[0].def_id().is_some());
    assert_eq!(result.args()[1].as_primitive(), Some(Primitive::Str));
    assert_eq!(
        result.written_source(),
        "Result<List<Option<Item>>, String>"
    );
}

#[test]
fn an_unknown_nested_leaf_prevents_partial_resolution() {
    let got = resolve_parameter("Result<List<Option<Missing>>, String>");
    match got {
        TypeResolution::Unresolved { name, written } => {
            assert_eq!(name, "Missing");
            assert_eq!(written, "Result<List<Option<Missing>>, String>");
        }
        other => panic!("must name the unresolved leaf: {other:?}"),
    }
}

#[test]
fn builtin_arity_is_exact_not_merely_nonzero() {
    for good in ["List<Int>", "Option<Int>", "Result<Int, String>"] {
        assert!(resolve_parameter(good).is_resolved(), "{good}");
    }
    for bad in [
        "List",
        "List<Int, String>",
        "Option",
        "Option<Int, String>",
        "Result",
        "Result<Int>",
        "Result<Int, String, Bool>",
        "List<Result<Int>>",
        "Result<List<Int, Bool>, String>",
    ] {
        assert!(
            !resolve_parameter(bad).is_resolved(),
            "wrong arity resolved: {bad}"
        );
    }
}

#[test]
fn qualified_function_names_are_not_type_identities() {
    let lib = lower("module lib\nfn impostor() -> Int { 1 }\nopaque type Actual = String\n");
    for (name, expected) in [("lib.impostor", false), ("lib.Actual", true)] {
        let (user, ty) = parameter(&format!(
            "module user\nimport lib\nfn probe(x: {name}) {{ x }}\n"
        ));
        let ws = Workspace::build(&[&lib, &user]);
        let got = resolved::resolve(&ws, 1, None, &[], &ty, 0..0);
        assert_eq!(got.is_resolved(), expected, "{name}: {got:?}");
    }
}

#[test]
fn the_grammars_unit_spelling_resolves_as_unit() {
    for spelling in ["()", "Unit"] {
        let got = resolve_parameter(spelling);
        assert_eq!(
            got.resolved().and_then(|t| t.as_primitive()),
            Some(Primitive::Unit)
        );
    }
}

#[test]
fn parameters_fields_and_returns_preserve_the_same_recursive_shape() {
    let hir = lower(
        "module m\n\
         type Envelope = Envelope { payload: Result<List<Option<Int>>, String> }\n\
         fn probe(x: Result<List<Option<Int>>, String>) -> Result<List<Option<Int>>, String> { x }\n",
    );
    let probe = hir.all_decls().find(|(_, d)| d.name == "probe").unwrap().1;
    let envelope = hir
        .all_decls()
        .find(|(_, d)| d.name == "Envelope")
        .unwrap()
        .1;
    let parameter = probe.params[0].ty.as_ref().unwrap();
    let field = envelope.fields.as_ref().unwrap()[0].ty.as_ref().unwrap();
    let returned = probe.ret.as_ref().unwrap();
    assert_eq!(parameter, field);
    assert_eq!(parameter, returned);
    assert_eq!(returned.written(), "Result<List<Option<Int>>, String>");
    assert_eq!(returned.args()[0].args()[0].args()[0].written(), "Int");

    // Boundary interfaces project the already-resolved signature. They must
    // preserve both identity and every nested argument.
    let ws = Workspace::build(&[&hir]);
    let sigs = pw_core::signatures::Signatures::build(&ws, &[&hir]);
    let signature = sigs.by_path("m.probe").expect("signature");
    let interface = pw_core::binding::Interface::from(signature);
    let parameter = signature.params[0].as_ref().unwrap().resolved().unwrap();
    let result = interface.returns.as_ref().unwrap().resolved().unwrap();
    assert!(result.same_as(parameter));
    assert_eq!(result.written_source(), "Result<List<Option<Int>>, String>");
    assert!(
        interface.params[0]
            .as_ref()
            .unwrap()
            .resolved()
            .unwrap()
            .same_as(parameter)
    );
    assert!(signature.result().unwrap().same_as(result));
}

#[test]
fn direct_and_result_wrapped_resource_manifests_keep_full_types() {
    let hir = lower(
        "module m\n\
         query Direct() -> List<Option<Int>> { [] }\n\
         query Fallible() -> Result<List<Option<Int>>, Option<String>> { [] }\n\
         query Unannotated() { [] }\n",
    );
    let built = pw_core::manifest::build(&hir);
    assert!(built.unparsed.is_empty());
    let find = |name: &str| built.manifests.iter().find(|m| m.name == name).unwrap();
    assert_eq!(
        find("Direct").result_type.as_deref(),
        Some("List<Option<Int>>")
    );
    assert_eq!(find("Direct").error_type, None);
    assert_eq!(
        find("Fallible").result_type.as_deref(),
        Some("List<Option<Int>>")
    );
    assert_eq!(
        find("Fallible").error_type.as_deref(),
        Some("Option<String>")
    );
    assert_eq!(find("Unannotated").result_type, None);
    assert_eq!(find("Unannotated").error_type, None);
    let json = serde_json::to_string(&built.manifests).expect("manifest serializes");
    let round_trip: Vec<pw_core::manifest::Manifest> = serde_json::from_str(&json).unwrap();
    assert_eq!(round_trip, built.manifests);
}

#[test]
fn nested_nominal_identity_and_argument_order_remain_significant() {
    let a = lower("module a\nopaque type Tag = String\n");
    let b = lower("module b\nopaque type Tag = String\n");
    let user = lower(
        "module user\nimport a\nimport b\n\
         fn first(x: Result<List<a.Tag>, Option<b.Tag>>) { x }\n\
         fn again(x: Result<List<a.Tag>, Option<b.Tag>>) { x }\n\
         fn swapped(x: Result<Option<b.Tag>, List<a.Tag>>) { x }\n\
         fn wrong_leaf(x: Result<List<b.Tag>, Option<b.Tag>>) { x }\n",
    );
    let ws = Workspace::build(&[&a, &b, &user]);
    let at = |name: &str| {
        let d = user.all_decls().find(|(_, d)| d.name == name).unwrap().1;
        resolved::resolve(
            &ws,
            2,
            None,
            &[],
            d.params[0].ty.as_ref().unwrap(),
            d.params[0].span.clone(),
        )
    };
    let first = at("first");
    let first = first.resolved().unwrap();
    assert!(first.same_as(at("again").resolved().unwrap()));
    assert!(!first.same_as(at("swapped").resolved().unwrap()));
    assert!(!first.same_as(at("wrong_leaf").resolved().unwrap()));
}

#[test]
fn nested_artifact_identity_is_independent_of_source_unit_order() {
    let domain = lower("module domain\nopaque type Tag = String\n");
    let (user, written) = parameter(
        "module user\nimport domain\nfn probe(x: Result<List<domain.Tag>, String>) { x }\n",
    );
    let refs = [&domain, &user];
    let forward = resolved::resolve(&Workspace::build(&refs), 1, None, &[], &written, 0..0);
    let first = resolved::stable(&refs, forward.resolved().unwrap()).unwrap();
    let reversed = [&user, &domain];
    let backward = resolved::resolve(&Workspace::build(&reversed), 0, None, &[], &written, 0..0);
    let second = resolved::stable(&reversed, backward.resolved().unwrap()).unwrap();
    assert_eq!(first, second);
    let json = serde_json::to_string(&first).unwrap();
    assert_eq!(
        first,
        serde_json::from_str::<resolved::StableTypeId>(&json).unwrap()
    );
}

#[test]
fn qualified_type_lookup_preserves_visibility_and_namespace() {
    use pw_core::resolve::{Namespace, Resolution};
    let lib = lower("module lib\nopaque type Shared = String\nfn Shared() -> Int { 1 }\n");
    for imports in ["", "import lib\n"] {
        let (user, written) = parameter(&format!(
            "module user\n{imports}fn probe(x: Option<lib.Shared>) {{ x }}\n",
        ));
        let ws = Workspace::build(&[&lib, &user]);
        let result = resolved::resolve(&ws, 1, None, &[], &written, 0..0);
        assert_eq!(result.is_resolved(), !imports.is_empty());
        if !imports.is_empty() {
            let type_id = ws.resolve_path_in(1, Namespace::Type, "lib.Shared");
            let term_id = ws.resolve_path_in(1, Namespace::Term, "lib.Shared");
            let (
                Resolution::Imported { def: type_id, .. },
                Resolution::Imported { def: term_id, .. },
            ) = (type_id, term_id)
            else {
                panic!("both namespaces resolve");
            };
            assert_ne!(type_id, term_id);
            assert_eq!(result.resolved().unwrap().args()[0].def_id(), Some(type_id));
            assert_eq!(
                ws.resolve_path(1, "lib.Shared"),
                ws.resolve_path_in(1, Namespace::Type, "lib.Shared")
            );
        }
    }
}

#[test]
fn resolving_a_written_type_does_not_discard_its_diagnostic_provenance() {
    let source = "module m\nfn probe(x: Result<List<Int>, Option<String>>) { x }\n";
    let (hir, written) = parameter(source);
    let span = 21..57;
    let got = resolved::resolve(
        &Workspace::build(&[&hir]),
        0,
        None,
        &[],
        &written,
        span.clone(),
    );
    let got = got.resolved().unwrap();
    assert_eq!(got.span(), span);
    assert_eq!(got.written_source(), written.written());
}
