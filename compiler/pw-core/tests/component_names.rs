//! Declaration namespaces survive contract, graph and executable projections.
use pw_core::check::Unit;
use std::collections::BTreeSet;

fn units(src: &str) -> Vec<Unit> {
    vec![Unit {
        path: "names.pw".into(),
        src: src.into(),
        hir: pw_core::lower::lower_file(src, &pw_syntax::parse_tree(src).green),
    }]
}

#[test]
fn a_query_and_view_of_the_same_name_build_as_distinct_declarations() {
    let src = "module names\nquery Same() -> Int { 7 }\nview Same() !{} { <p>View</p> }\n";
    let inputs = units(src);
    let built = pw_core::program::build(&inputs).expect("distinct namespaces must build");
    let ids: BTreeSet<_> = built
        .contracts
        .iter()
        .map(|c| c.component_id.as_str())
        .collect();
    assert_eq!(ids, BTreeSet::from(["names.Same", "names.Same~ui"]));
    assert_eq!(built.contracts.len(), 2);
    assert_eq!(built.components.len(), 2);
    assert!(matches!(
        &built.components[0].1,
        pw_core::backend::component::Built::Component { .. }
    ));
    assert!(built.refusals().is_empty());
    let mut resolve = wit_parser::Resolve::new();
    let package = resolve
        .push_str("names.wit", &built.wit)
        .expect("generated WIT resolves");
    assert_eq!(resolve.packages[package].worlds.len(), 2);
}

#[test]
fn order_and_file_boundaries_do_not_choose_a_same_named_signature() {
    let query = "query Same(n: Int) -> Int { n + 7 }\n";
    let view = "view Same(text: String) !{} { <p>{text}</p> }\n";
    for files in [
        vec![format!("module names\n{query}{view}")],
        vec![format!("module names\n{view}{query}")],
        vec![
            format!("module data\n{query}"),
            format!("module visual\n{view}"),
        ],
        vec![
            format!("module visual\n{view}"),
            format!("module data\n{query}"),
        ],
    ] {
        let inputs: Vec<_> = files
            .iter()
            .flat_map(|src| units(src))
            .enumerate()
            .map(|(i, mut u)| {
                u.path = format!("{i}.pw");
                u
            })
            .collect();
        let built =
            pw_core::program::build(&inputs).expect("neither order nor file selects a declaration");
        let refs: Vec<_> = inputs.iter().map(|u| &u.hir).collect();
        let ws = pw_core::resolve::Workspace::build(&refs);
        let (_, worlds) = pw_core::wit::package(&refs, &ws, &built.contracts).unwrap();
        assert_eq!(worlds.len(), 2);
        let stem = if files.len() == 1 {
            "names-same"
        } else {
            "data-same"
        };
        let query_world = worlds.iter().find(|w| w.name == stem).unwrap();
        let view_world = worlds
            .iter()
            .find(|w| {
                w.name
                    == if files.len() == 1 {
                        "names-sameui"
                    } else {
                        "visual-sameui"
                    }
            })
            .unwrap();
        assert_ne!(query_world.declaration, view_world.declaration);
        assert_eq!(
            refs[query_world.declaration.unit]
                .decl(pw_core::hir::DeclId(query_world.declaration.decl))
                .kind,
            pw_core::hir::DeclKind::Query
        );
        assert_eq!(
            refs[view_world.declaration.unit]
                .decl(pw_core::hir::DeclId(view_world.declaration.decl))
                .kind,
            pw_core::hir::DeclKind::View
        );
        assert!(
            built.wit.contains("same: func(arg0: s64) -> s64;"),
            "{}",
            built.wit
        );
        assert!(
            built.wit.contains("same: func(arg0: string);"),
            "{}",
            built.wit
        );
        assert!(matches!(
            &built.components[0].1,
            pw_core::backend::component::Built::Component { audited: 1, .. }
        ));
        assert!(matches!(
            &built.components[1].1,
            pw_core::backend::component::Built::NoBody {
                kind: pw_core::hir::DeclKind::View
            }
        ));
    }
}

#[test]
fn a_page_and_its_query_keep_distinct_graph_template_and_value_identities() {
    let inputs = units(
        "module names\nquery Home() -> Int { 7 }\npage Home() {\n route \"/\"\n let n = query Home()\n view { <title>Home</title><p>{n + 1}</p> }\n}\n",
    );
    let built = pw_core::program::build(&inputs).expect("same-named page and query build");
    let nodes: BTreeSet<_> = built.graph.nodes.iter().map(|n| n.path.as_str()).collect();
    assert_eq!(nodes, BTreeSet::from(["names.Home", "names.Home~ui"]));
    assert_eq!(built.graph.nodes.len(), 2);
    assert!(
        built
            .graph
            .edges
            .iter()
            .any(|e| e.from == "names.Home~ui" && e.to == "names.Home")
    );
    assert_eq!(built.templates.len(), 1);
    assert_eq!(built.templates[0].path, "names.Home~ui");
    assert_eq!(built.pages.len(), 1);
    assert_eq!(built.pages[0].page, "names.Home~ui");
    assert!(
        built
            .contracts
            .iter()
            .any(|c| c.component_id.starts_with("names.Home~ui.derived_"))
    );
    assert!(built.refusals().is_empty());
}

#[test]
fn distinct_names_that_mangle_together_are_refused_instead_of_aliased() {
    let inputs =
        units("module names\nquery Sameui() -> Int { 7 }\nview Same() !{} { <p>View</p> }\n");
    let report = pw_core::program::validate(&inputs);
    let errors: Vec<_> = report
        .diagnostics
        .iter()
        .flat_map(|(_, ds)| ds)
        .filter(|d| d.is_error())
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].reason, "backend_wit");
    assert!(
        errors[0].message.contains("both mangle to `names-sameui`"),
        "{errors:?}"
    );
    assert!(errors[0].message.contains("names.Same~ui"));
    assert!(errors[0].message.contains("names.Sameui"));
    assert!(report.into_build().is_err());
}

#[test]
fn accepted_a008_builds_its_real_query_and_view() {
    let inputs = fixture("examples/accepted/A-008-streamed-public-recommendations.pw");
    let built = pw_core::program::build(&inputs).expect("unchanged A-008 builds");
    assert_a008(&built);
}

fn fixture(extra: &str) -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
        let mut found: Vec<_> = std::fs::read_dir(root.join(dir))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|s| s == "pw"))
            .collect();
        found.sort();
        paths.extend(found);
    }
    paths.push(root.join("examples/domain.pw"));
    let extra = root.join(extra);
    if extra.is_dir() {
        let mut files: Vec<_> = std::fs::read_dir(extra)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|s| s == "pw"))
            .collect();
        files.sort();
        paths.extend(files);
    } else {
        paths.push(extra);
    }
    let inputs: Vec<_> = paths
        .into_iter()
        .map(|path| {
            let src = std::fs::read_to_string(&path).unwrap();
            Unit {
                path: path.display().to_string(),
                hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
                src,
            }
        })
        .collect();
    inputs
}

fn assert_a008(built: &pw_core::build::Build) {
    let query = "store.recommendations.Recommendations";
    let view = "store.recommendations.Recommendations~ui";
    assert!(matches!(
        &built
            .components
            .iter()
            .find(|(id, _)| id == query)
            .unwrap()
            .1,
        pw_core::backend::component::Built::Component { audited: 2, .. }
    ));
    assert!(matches!(
        &built
            .components
            .iter()
            .find(|(id, _)| id == view)
            .unwrap()
            .1,
        pw_core::backend::component::Built::NoBody {
            kind: pw_core::hir::DeclKind::View
        }
    ));
    assert_eq!(built.templates.len(), 1);
    assert_eq!(built.templates[0].path, view);
    assert!(built.refusals().is_empty());
}

#[test]
fn store_speculations_and_handlers_name_their_ui_contracts_and_templates() {
    let built = pw_core::program::build(&fixture("examples/store")).expect("store builds");
    assert!(
        !built.speculations.is_empty(),
        "store must exercise speculation identity"
    );
    assert!(
        !built.handlers.is_empty(),
        "store must exercise handler identity"
    );
    let contract_ids: BTreeSet<_> = built
        .contracts
        .iter()
        .map(|c| c.component_id.as_str())
        .collect();
    let template_ids: BTreeSet<_> = built.templates.iter().map(|t| t.path.as_str()).collect();
    for compiled in &built.speculations {
        assert!(
            contract_ids.contains(compiled.page.as_str()),
            "{}",
            compiled.page
        );
        assert!(
            template_ids.contains(compiled.page.as_str()),
            "{}",
            compiled.page
        );
    }
    for handler in &built.handlers {
        assert!(
            contract_ids.contains(handler.declaration.as_str()),
            "{}",
            handler.declaration
        );
    }
    for page in &built.pages {
        assert!(contract_ids.contains(page.page.as_str()), "{}", page.page);
        assert!(template_ids.contains(page.page.as_str()), "{}", page.page);
    }
}

#[test]
fn view_tags_choose_the_ui_namespace_beside_same_named_queries() {
    let built = pw_core::program::build(&fixture("examples/demo/namespaces.pw"))
        .expect("a view tag resolves the UI declaration, a query call the term declaration");
    let page = built
        .templates
        .iter()
        .find(|t| t.path == "demo.namespaces.Home~ui")
        .expect("page template");
    let markup = serde_json::to_string(page).expect("template JSON");
    assert!(
        markup.contains("namespace-view") && markup.contains("strong"),
        "{markup}"
    );
    assert!(
        built
            .components
            .iter()
            .any(|(id, c)| id == "demo.namespaces.Card"
                && matches!(c, pw_core::backend::component::Built::Component { .. }))
    );
    assert!(
        built
            .components
            .iter()
            .any(|(id, c)| id == "demo.namespaces.Card~ui"
                && matches!(c, pw_core::backend::component::Built::NoBody { .. }))
    );
}

#[test]
fn semantic_review_keeps_same_named_query_policies_in_both_orders() {
    let query = "public query Same() -> Int freshness 1.seconds { 7 }\n";
    let view = "view Same() !{} { <p>View</p> }\n";
    for declarations in [format!("{query}{view}"), format!("{view}{query}")] {
        let source = format!("module names\n{declarations}");
        let before = pw_core::semantic::Model::of(&units(&source)).expect("before builds");
        let after = pw_core::semantic::Model::of(&units(&source.replace("1.seconds", "2.seconds")))
            .expect("after builds");
        let report = pw_core::semantic::diff(&before, &after);
        let changes = &report
            .sections
            .iter()
            .find(|s| s.title == "Cache and freshness")
            .unwrap()
            .changes;
        assert_eq!(changes.len(), 1, "{}", report.text());
        assert!(changes[0].contains("names.Same"), "{}", changes[0]);
        assert!(
            changes[0].contains("1.seconds") && changes[0].contains("2.seconds"),
            "{}",
            changes[0]
        );
        assert_eq!(before.declarations["names.Same"].kind, "query");
        assert_eq!(before.declarations["names.Same~ui"].kind, "view");
    }
}
#[test]
fn semantic_review_keeps_type_and_term_namespaces_without_rewriting_type_shapes() {
    let query = "public query Same() -> Int freshness 1.seconds { 7 }\n";
    let ty = "type Same = Record { value: Int }\n";
    for declarations in [format!("{query}{ty}"), format!("{ty}{query}")] {
        let source = format!("module names\n{declarations}");
        let before = pw_core::semantic::Model::of(&units(&source)).expect("before builds");
        let after = pw_core::semantic::Model::of(&units(&source.replace("1.seconds", "2.seconds")))
            .expect("after builds");
        let report = pw_core::semantic::diff(&before, &after);
        let changes = &report
            .sections
            .iter()
            .find(|s| s.title == "Cache and freshness")
            .unwrap()
            .changes;
        assert_eq!(changes.len(), 1, "{}", report.text());
        assert_eq!(before.declarations["names.Same"].kind, "query");
        assert_eq!(before.declarations["names.Same~type"].kind, "type");
        assert_eq!(
            before.types.keys().map(String::as_str).collect::<Vec<_>>(),
            ["names.Same"]
        );
    }
}

#[test]
fn same_named_query_cannot_hide_an_unbounded_view_cycle() {
    let src = "module names\nquery Loop() -> Int { 7 }\nview Loop() !{} { <div><Loop /></div> }\n";
    let diagnostics = pw_core::check::check_sources(&[("loop.pw".into(), src.into())]);
    assert!(
        diagnostics
            .iter()
            .flat_map(|(_, ds)| ds)
            .any(|d| d.code == "PW5020" && d.message.contains("contains itself with no")),
        "{diagnostics:?}"
    );
}

#[test]
fn a_same_named_event_never_replaces_the_executable_query() {
    let query = "query Same() -> Int { 7 }\n";
    let event = "event Same(value: String)\n";
    for declarations in [format!("{query}{event}"), format!("{event}{query}")] {
        let source = format!("module names\n{declarations}");
        let built = pw_core::program::build(&units(&source)).expect("both namespaces build");
        assert_eq!(built.contracts.len(), 1);
        assert_eq!(built.contracts[0].component_id, "names.Same");
        assert!(built.wit.contains("same: func() -> s64;"), "{}", built.wit);
        assert_eq!(built.components.len(), 1);
        assert!(
            matches!(
                &built.components[0].1,
                pw_core::backend::component::Built::Component { audited: 1, .. }
            ),
            "the executable query must survive an event of the same name: {:?}",
            built.components
        );
        let model = pw_core::semantic::Model::of(&units(&source)).expect("review builds");
        assert_eq!(model.declarations["names.Same"].kind, "query");
        assert_eq!(model.declarations["names.Same~event"].kind, "event");
    }
}
