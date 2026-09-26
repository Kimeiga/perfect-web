//! **A declared function's result carries the labels its type parameters
//! bring in** (ADR-0064).
//!
//! A declaration's label is its contract, and until 2026-09-26 that was the
//! whole answer: `List.get(tokens, 0)`, over a list of secrets, was public,
//! because `get` declares no label for its result. So was `List.map`'s
//! result, and a program's own `fn first<T>(xs: List<T>) -> Option<T>`. A
//! secret passed through any generic function came out public, and could be
//! logged or rendered. A result that mentions one of the callee's type
//! parameters now carries the labels of the arguments whose declared types
//! mention it. A result that mentions none, such as `List.length`'s `Int`,
//! keeps the declaration's contract. Each test states one case, with a
//! control that what is right is not refused.

use pw_core::check::check_sources;

fn files(dirs: &[&str]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in dirs {
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
    out
}

/// `code message` for each diagnostic on `t.pw`.
fn reported(src: &str) -> Vec<String> {
    let mut all = files(&["packages/pw-std", "packages/pw-platform-web"]);
    all.push(("t.pw".to_string(), src.to_string()));
    check_sources(&all)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A body that logs publicly, in a module that may hold a secret.
fn logs(decls: &str, body: &str) -> String {
    format!(
        "module t\n\nimport log\nimport secrets\nimport List\nimport capability.{{ Payments, Public }}\n\n{decls}\n\nfn f() -> () !{{ log<Public>, secret<Payments> }} {{\n{body}\n}}\n"
    )
}

fn refused(src: &str, code: &str) {
    let found = reported(src);
    assert!(!found.is_empty(), "nothing reported");
    assert!(found.iter().all(|d| d.starts_with(code)), "{found:#?}");
}

fn none(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{found:#?}");
}

const SECRETS: &str = "[secrets.payments()]";
const WORDS: &str = "[\"a\", \"b\"]";

#[test]
fn an_element_taken_out_of_a_list_keeps_its_label() {
    let wrong = logs(
        "",
        "    let t = List.get([secrets.payments()], 0)\n    log.public(\"with {t}\")",
    );
    refused(&wrong, "PW5006");
    none(&wrong.replace(SECRETS, WORDS));
}

#[test]
fn a_list_mapped_keeps_what_its_function_computes() {
    let wrong = logs(
        "",
        "    let lines = List.map([secrets.payments()], t => \"with {t}\")\n    log.public(\"{lines}\")",
    );
    refused(&wrong, "PW5006");
    none(&wrong.replace(SECRETS, WORDS));
    // Through the function too: a public list mapped to a secret.
    let captured = logs(
        "",
        "    let key = secrets.payments()\n    let lines = List.map([\"a\"], w => \"{w}{key}\")\n    log.public(\"{lines}\")",
    );
    refused(&captured, "PW5006");
}

#[test]
fn a_fold_keeps_what_it_accumulates() {
    let wrong = logs(
        "",
        "    let joined = List.fold([secrets.payments()], \"\", (acc, t) => \"{acc}{t}\")\n    log.public(joined)",
    );
    refused(&wrong, "PW5006");
    none(&wrong.replace(SECRETS, WORDS));
}

#[test]
fn a_programs_own_generic_function_keeps_its_arguments_labels() {
    let first = "fn first<T>(xs: List<T>) -> Option<T> { List.get(xs, 0) }";
    let wrong = logs(
        first,
        "    let t = first([secrets.payments()])\n    log.public(\"with {t}\")",
    );
    refused(&wrong, "PW5006");
    none(&wrong.replace(SECRETS, WORDS));
}

#[test]
fn a_result_that_mentions_no_parameter_keeps_its_contract() {
    // `length` returns an `Int` whatever `T` is: the declaration's label.
    none(&logs(
        "",
        "    let n = List.length([secrets.payments()])\n    log.public(\"count {n}\")",
    ));
}

#[test]
fn a_method_calls_receiver_is_its_first_argument() {
    // Typed, `tokens.get(0)` is `List.get` with `tokens` as its `items`.
    let typed = logs("", "    let tokens: List<Secret<Payments>> = [secrets.payments()]\n    let t = tokens.get(0)\n    log.public(\"with {t}\")")
        .replace("{ Payments, Public }", "{ Secret, Payments, Public }");
    refused(&typed, "PW5006");
    // Untyped, the call resolves to nothing, and carries what went into it:
    // its receiver too.
    let untyped = logs(
        "",
        "    let tokens = [secrets.payments()]\n    let t = tokens.get(0)\n    log.public(\"with {t}\")",
    );
    refused(&untyped, "PW5006");
    none(&untyped.replace(SECRETS, WORDS));
}

#[test]
fn a_piped_argument_is_matched_to_its_parameter() {
    // `words |> List.map(f)`: `f` is `map`'s second parameter, whose type
    // mentions the result's `U`.
    let wrong = logs(
        "",
        "    let key = secrets.payments()\n    let lines = [\"a\"] |> List.map(w => \"{w}{key}\")\n    log.public(\"{lines}\")",
    );
    refused(&wrong, "PW5006");
    none(&wrong.replace("{w}{key}", "{w}"));
}
