//! **A dependency-graph clause belongs to a declaration that can mean it**
//! (ADR-0092).
//!
//! ADR-0007 made invalidation explicit: a command emits typed events and
//! invalidates the entries it changes, and a resource or a materialization
//! listens for events and depends on resources. Until 2026-09-26 a clause
//! checked on any declaration. A query that `emits`, a command that listens,
//! a `fn` that emits and a page that invalidates each checked, and nothing
//! read what they said. Each test states one case, with a control.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn one(src: &str, says: &str) {
    let found = reported(src);
    assert_eq!(found.len(), 1, "{src}\n{found:#?}");
    assert!(found[0].contains(says), "{src}\n{found:#?}");
}

fn clean(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{src}\n{found:#?}");
}

const DECLS: &str = "module t\n\nevent Moved(to: Int)\n\nquery Box(id: Int) -> Int !{} { id }\n\n";

/// A declaration written `head(params)`, with one clause, and a body.
fn with(head: &str, clause: &str) -> String {
    format!("{DECLS}{head} -> Int !{{}}\n    {clause}\n{{\n    0\n}}\n")
}

#[test]
fn a_command_emits_and_invalidates() {
    clean(&with("command shift(by: Int)", "emits Moved(by)"));
    clean(&with("command shift(by: Int)", "invalidates Box(by)"));
    one(
        &with("query Other(by: Int)", "emits Moved(by)"),
        "PW5105 `Other` is a query, and `emits` belongs to a command",
    );
    one(
        &with("query Other(by: Int)", "invalidates Box(by)"),
        "PW5105 `Other` is a query, and `invalidates` belongs to a command",
    );
    one(
        &with("fn helper(by: Int)", "emits Moved(by)"),
        "PW5105 `helper` is a function, and `emits` belongs to a command",
    );
}

#[test]
fn a_resource_listens() {
    clean(&with("query Other(by: Int)", "invalidates_on Moved(by)"));
    one(
        &with("command shift(by: Int)", "invalidates_on Moved(by)"),
        "PW5105 `shift` is a command, and `invalidates_on` belongs to a resource or a materialization",
    );
}

#[test]
fn a_materialization_depends() {
    clean(&format!(
        "{DECLS}materialize Panel(id: Int) {{\n    placement origin\n    partition private\n    \
         depends_on Box(id)\n    invalidates_on Moved(id)\n    regenerate on_invalidation\n}}\n"
    ));
    one(
        &with("command shift(by: Int)", "depends_on Box(by)"),
        "PW5105 `shift` is a command, and `depends_on` belongs to a materialization",
    );
}
