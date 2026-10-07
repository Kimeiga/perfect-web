//! **A value returned early carries its label to the caller** (ADR-0252).
//!
//! A body's label was its last statement's, joined with each `?`'s value
//! (ADR-0129): a `return` inside a branch or a loop was in neither, so a
//! helper returning a secret early was public to its callers, and
//! `log.public(g())` passed. Now each `return` the body makes counts, with
//! the conditions it returns under, and so does each `?`'s: what decides
//! which value comes back is in the value. Each test states one case, with
//! its control.

use pw_core::check::check_sources;

fn files(dir: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            (p.display().to_string(), src)
        })
        .collect()
}

/// What `app.pw` reports, each as its code and message.
fn reported(src: &str) -> Vec<String> {
    let mut program = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        program.extend(files(d));
    }
    program.push(("app.pw".to_string(), src.to_string()));
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A module with a helper reading a secret into a `String`, `G` its `g`,
/// and an `f` that logs what `g` gives back publicly.
fn logged(g: &str) -> String {
    format!(
        "module t\n\nimport log\nimport secrets\nimport List\nimport capability.{{ Payments, Public }}\n\n\
         fn token() -> String !{{ secret.read }} {{\n    let k = secrets.payments()\n    \"{{k}}\"\n}}\n\n\
         fn secretly() -> Bool !{{ secret.read }} {{\n    List.length([token()]) > 0\n}}\n\n\
         fn g() -> String !{{ secret.read }} {{\n{g}\n}}\n\n\
         fn f() -> () !{{ log<Public>, secret<Payments> }} {{\n    log.public(g())\n}}\n"
    )
}

const LOGS_THE_SECRET: &str =
    "PW5006 cannot log a `Secret<Payments>` value at privacy level `Public`";

#[test]
fn a_secret_returned_from_a_branch_is_the_callers() {
    let src = logged("    if List.length([1]) > 0 {\n        return token()\n    }\n    \"none\"");
    assert_eq!(reported(&src), vec![LOGS_THE_SECRET]);
    // The control: a public value returned early.
    assert_eq!(
        reported(&src.replace("return token()", "return \"early\"")),
        Vec::<String>::new()
    );
}

#[test]
fn a_secret_returned_from_a_loop_is_the_callers() {
    let src = logged(
        "    let tokens = [token()]\n    for t in tokens {\n        return t\n    }\n    \"none\"",
    );
    assert_eq!(reported(&src), vec![LOGS_THE_SECRET]);
    assert_eq!(
        reported(&src.replace("let tokens = [token()]", "let tokens = [\"a\"]")),
        Vec::<String>::new()
    );
}

#[test]
fn where_a_secret_decides_what_returns_the_value_carries_it() {
    // Two public values; which comes back, a secret decides.
    let src = logged("    if secretly() {\n        return \"yes\"\n    }\n    \"no\"");
    assert_eq!(reported(&src), vec![LOGS_THE_SECRET]);
    // The control: a public condition.
    assert_eq!(
        reported(&src.replace("if secretly()", "if List.length([1]) > 0")),
        Vec::<String>::new()
    );
}

#[test]
fn a_return_in_a_function_value_is_the_function_values() {
    // The lambda's `return` leaves the lambda, and `g` gives back "none".
    let src = logged("    let early = () => {\n        return token()\n    }\n    \"none\"");
    assert_eq!(reported(&src), Vec::<String>::new());
}

#[test]
fn where_a_secret_decides_a_question_mark_its_failure_carries_it() {
    // `checked()?` returns its failure only where `secretly()` holds: which
    // `Result` comes back, a secret decides.
    let src = "module t\n\nimport log\nimport secrets\nimport List\nimport capability.{ Payments, Public }\n\n\
         fn token() -> String !{ secret.read } {\n    let k = secrets.payments()\n    \"{k}\"\n}\n\n\
         fn secretly() -> Bool !{ secret.read } {\n    List.length([token()]) > 0\n}\n\n\
         fn checked() -> Result<String, String> !{} {\n    Err(\"no\")\n}\n\n\
         fn g() -> Result<String, String> !{ secret.read } {\n    if secretly() {\n        \
         let _ = checked()?\n    }\n    Ok(\"fine\")\n}\n\n\
         fn f() -> () !{ log<Public>, secret<Payments> } {\n    match g() {\n        \
         Ok(v) => log.public(v),\n        Err(e) => log.public(e),\n    }\n}\n";
    let found = reported(src);
    assert!(
        !found.is_empty() && found.iter().all(|d| d.starts_with("PW5006")),
        "{found:#?}"
    );
    // The control: a public condition.
    assert_eq!(
        reported(&src.replace("if secretly()", "if List.length([1]) > 0")),
        Vec::<String>::new()
    );
}
