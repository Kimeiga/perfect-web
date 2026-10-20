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
