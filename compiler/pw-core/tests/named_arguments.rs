//! **A named argument is given to the parameter of its name** (ADR-0081).
//!
//! Until 2026-09-26 a call that named an argument related none of its
//! arguments to their parameters: a signature carried no parameter names. The
//! backend passed arguments in written order, so `g(b = 1, a = 10)` computed
//! `g(1, 10)`, and the labels carried a generic result's label from the
//! argument in the wrong place. Each test states one case, with a control;
//! `pw-conformance`'s `named_arguments.rs` runs the call.

use pw_core::check::check_sources;

fn program(src: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

fn reported(src: &str) -> Vec<String> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn with_f(body: &str) -> String {
    format!("module t\n\nfn f(n: Int, s: String) -> Int !{{}} {{ n }}\n\n{body}\n")
}

fn one(body: &str, says: &str) {
    let found = reported(&with_f(body));
    assert_eq!(found.len(), 1, "{body}: {found:#?}");
    assert!(found[0].contains(says), "{body}: {found:#?}");
}

fn clean(body: &str) {
    let found = reported(&with_f(body));
    assert!(found.is_empty(), "{body}: {found:#?}");
}

#[test]
fn a_named_argument_is_related_to_its_parameter() {
    one(
        "fn h() -> Int !{} { f(n = \"a\", s = \"b\") }",
        "PW0605 argument 1 of `t.f` is declared `Int` and this is `String`",
    );
    // Given in another order, by name.
    clean("fn h() -> Int !{} { f(s = \"b\", n = 1) }");
    clean("fn h() -> Int !{} { f(1, s = \"b\") }");
}

#[test]
fn a_named_argument_names_a_parameter_once() {
    one(
        "fn h() -> Int !{} { f(z = 1, s = \"b\") }",
        "PW0617 `t.f` has no parameter `z`",
    );
    one(
        "fn h() -> Int !{} { f(1, n = 2) }",
        "PW0617 `t.f`'s parameter `n` is given twice",
    );
    one(
        "fn h() -> Int !{} { f(s = \"b\", 1) }",
        "PW0617 a positional argument follows a named one",
    );
}

#[test]
fn a_function_values_parameters_have_no_names() {
    one(
        "fn h() -> Int !{} {\n    let g = f\n    g(n = 1, s = \"b\")\n}",
        "PW0617 `g` is a function value, whose parameters have no names",
    );
    clean("fn h() -> Int !{} {\n    let g = f\n    g(1, \"b\")\n}");
}

/// A generic result carries the label of the argument its type comes from
/// (ADR-0064), found by name.
#[test]
fn a_label_is_carried_from_the_argument_named() {
    let src = |call: &str| {
        format!(
            "module t\n\nimport log\nimport secrets\nimport capability.{{ Payments, Public }}\n\n\
             fn pick<T>(n: Int, x: T) -> T !{{}} {{ x }}\n\n\
             fn leak() -> () !{{ log<Public>, secret<Payments> }} {{\n    log.public(\"{{{call}}}\")\n}}\n"
        )
    };
    let found = reported(&src("pick(x = secrets.payments(), n = 0)"));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].starts_with("PW5006"), "{found:#?}");
    // The public argument, named, is public.
    let found = reported(&src("pick(x = \"public\", n = 0)"));
    assert!(found.is_empty(), "{found:#?}");
}
