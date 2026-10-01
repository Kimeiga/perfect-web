//! Authorization clauses are invocation preconditions, not ignored policy text.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    check_sources(&[("t.pw".to_string(), src.to_string())])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

#[test]
fn requires_accepts_deployment_predicates_over_command_parameters() {
    let src = "module t\n\ncommand Save(order: Int) -> Int !{}\n    requires SignedIn, OwnsOrder(order)\n{\n    order\n}\n";
    let found = reported(src);
    assert!(
        !found.iter().any(|d| d.starts_with("PW0335 ")),
        "{found:#?}"
    );
}

#[test]
fn requires_belongs_to_a_command() {
    let src =
        "module t\n\nfn read(order: Int) -> Int !{}\n    requires SignedIn\n{\n    order\n}\n";
    let found = reported(src);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0335 ") && d.contains("belongs to a command")),
        "{found:#?}"
    );
}

#[test]
fn requires_arguments_are_command_parameters() {
    let src = "module t\n\ncommand Save(order: Int) -> Int !{}\n    requires OwnsOrder(other)\n{\n    order\n}\n";
    let found = reported(src);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0335 ") && d.contains("not a parameter")),
        "{found:#?}"
    );
}

#[test]
fn requires_rejects_non_parameter_expressions_and_duplicates() {
    for value in ["OwnsOrder(47)", "SignedIn, SignedIn"] {
        let src = format!(
            "module t\n\ncommand Save(order: Int) -> Int !{{}}\n    requires {value}\n{{\n    order\n}}\n"
        );
        let found = reported(&src);
        assert!(
            found.iter().any(|d| d.starts_with("PW0335 ")),
            "{value}: {found:#?}"
        );
    }
}
