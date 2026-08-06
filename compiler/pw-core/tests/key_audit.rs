//! Charter §14 M6 task 9 — cache-key auditing.
//!
//! Landed as an E6 follow-up on the architect's ruling of 2026-08-06: the
//! computation existed and `pw explain` did not surface it, and E6 had just
//! demonstrated how dangerous unread policy information is.
//!
//! # Advice, not a diagnostic
//!
//! Two readers sharing a cache entry is sometimes exactly what a cache is for,
//! so the compiler cannot tell a deliberate omission from a mistake. What it
//! can do is say which dimensions the key separates and which a dependency
//! separates that this key does not.
//!
//! The one omission that is never deliberate — the build that produced the
//! artifact — is not reported at all, because it is injected. `PW5102` used to
//! ask the author for it and was retired.

use pw_core::graph::{Dimension, Graph};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_syntax::parse_tree;

fn graph(sources: &[&str]) -> Graph {
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| {
            let p = parse_tree(s);
            assert!(p.ok(), "{s:?} does not parse: {:?}", p.errors);
            lower_file(s, &p.green)
        })
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    Graph::build(&refs, &ws)
}

/// A resource whose entries are separated by `key`, and a fragment reading it.
fn program(resource_key: &str, fragment_args: &str, fragment_dims: &str) -> Vec<String> {
    vec![
        format!(
            "module world\n\n\
             public query Recommendations(store: Int, experiment: Int) -> Int\n    \
                 freshness   5.minutes\n    \
                 consistency snapshot\n    \
                 cache       shared\n    \
                 key         {resource_key}\n\
             {{\n    0\n}}\n"
        ),
        format!(
            "module page\n\n\
             import world.{{ Recommendations }}\n\n\
             materialize Strip(store: Int) {{\n    \
                 placement      edge\n    \
                 partition      public\n    \
                 depends_on     Recommendations({fragment_args})\n    \
                 regenerate     on_invalidation\n\
             {fragment_dims}\
             }}\n"
        ),
    ]
}

fn audit(sources: &[String]) -> pw_core::graph::KeyAudit {
    let refs: Vec<&str> = sources.iter().map(String::as_str).collect();
    graph(&refs)
        .key_audit("page.Strip")
        .expect("the fragment is in the graph")
}

#[test]
fn a_dependency_keyed_by_more_than_the_fragment_is_reported() {
    // The architect's example. `Recommendations` separates its entries by an
    // experiment assignment; the fragment passes only the store, so ONE
    // document serves every arm of the experiment.
    let a = audit(&program("store, experiment", "store", ""));
    assert_eq!(a.gaps.len(), 1, "{:?}", a.gaps);
    assert_eq!(a.gaps[0].resource, "Recommendations");
    assert_eq!(a.gaps[0].component, "experiment");
    assert!(
        a.gaps[0].why.contains("one document serves every value"),
        "{}",
        a.gaps[0].why
    );
}

#[test]
fn a_dependency_the_fragment_fully_supplies_is_not_reported() {
    // The control. Without it the rule above is satisfied by reporting every
    // dependency of every fragment.
    let a = audit(&program("store, experiment", "store, experiment", ""));
    assert!(a.gaps.is_empty(), "{:?}", a.gaps);
}

#[test]
fn a_dimension_the_fragment_declares_closes_the_gap() {
    // The other way to cover a component: vary by it rather than pass it.
    let a = audit(&program(
        "store, locale",
        "store",
        "    locale included_in_key\n",
    ));
    assert!(a.gaps.is_empty(), "{:?}", a.gaps);

    // And removing the declaration re-opens it, so the assertion above is
    // about the declaration rather than about the word `locale`.
    let without = audit(&program("store, locale", "store", ""));
    assert_eq!(without.gaps.len(), 1, "{:?}", without.gaps);
    assert_eq!(without.gaps[0].component, "locale");
}

#[test]
fn the_audit_reports_what_the_key_does_include() {
    // A gap report that never says what IS covered reads as a list of
    // complaints, and a reader cannot tell an audited key from an unaudited
    // one.
    let a = audit(&program(
        "store",
        "store",
        "    locale included_in_key\n    tenant included_in_key\n",
    ));
    assert_eq!(a.logical_key, vec!["store".to_string()]);
    assert_eq!(a.partition.as_deref(), Some("public"));

    let included: Vec<Dimension> = a
        .application
        .iter()
        .filter(|(_, present)| *present)
        .map(|(d, _)| *d)
        .collect();
    assert!(included.contains(&Dimension::Locale));
    assert!(included.contains(&Dimension::Tenant));
    assert!(!included.contains(&Dimension::PolicyVersion));
}

#[test]
fn the_compatibility_generation_is_not_an_application_dimension() {
    // `PW5102`'s retirement, checked where a reader would notice it coming
    // back: the audit must never ask an author for the build generation,
    // because the platform injects it.
    let a = audit(&program("store", "store", ""));
    for (d, _) in &a.application {
        assert_ne!(
            d.keyword(),
            "code_version",
            "the generation is injected; asking for it is what was retired"
        );
    }
    // And the privacy partition is reported as automatic rather than as
    // something to declare.
    assert_eq!(a.partition.as_deref(), Some("public"));
}

#[test]
fn a_declaration_that_is_not_a_materialization_has_no_audit() {
    let g = graph(&[
        "module m\n\npublic query Q(id: Int) -> Int\n    cache shared\n    key id\n{\n    0\n}\n",
    ]);
    assert!(g.key_audit("m.Q").is_none());
}
