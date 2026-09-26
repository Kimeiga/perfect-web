//! **A call carries what it is given** (ADR-0085).
//!
//! A declared call's result is labelled by the declaration's label, joined
//! with the label of each argument given to a parameter whose declared type
//! states no label of its own. A parameter declared `Secret<Payments>`
//! receives the secret by contract, and the declaration says what comes out.
//! Until 2026-09-26 only an argument that brought in a type parameter the
//! result mentions was carried (ADR-0064), so a secret logged publicly
//! passed `pw check` once it went through `String.trim`, `String.to_upper`,
//! `String.join` or `String.slice`, and an element a secret index chose was
//! public. Each test states one case, with a control.

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

fn reported(decls: &str, body: &str) -> Vec<String> {
    let src = format!(
        "module t\n\nimport log\nimport secrets\nimport List\nimport String\n\
         import capability.{{ Secret, Payments, Public }}\n\n{decls}\n\n\
         fn f() -> () !{{ log<Public>, secret<Payments> }} {{\n{body}\n}}\n"
    );
    check_sources(&program(&src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn leaks(decls: &str, body: &str) {
    let found = reported(decls, body);
    assert_eq!(found.len(), 1, "{body}: {found:#?}");
    assert!(
        found[0].starts_with("PW5006 cannot log a `Secret<Payments>`"),
        "{body}: {found:#?}"
    );
}

fn clean(decls: &str, body: &str) {
    let found = reported(decls, body);
    assert!(found.is_empty(), "{body}: {found:#?}");
}

const SECRET: &str = "\"with {secrets.payments()}\"";
const WORDS: &str = "\"with words\"";

#[test]
fn a_string_function_carries_what_it_is_given() {
    for call in [
        "String.trim(@)",
        "String.to_upper(@)",
        "String.slice(@, 0, 3)",
        "String.join([@], \",\")",
    ] {
        let secret = format!("    log.public({})", call.replace('@', SECRET));
        leaks("", &secret);
        clean("", &format!("    log.public({})", call.replace('@', WORDS)));
    }
}

/// A parameter declared with a label receives the value by contract: what
/// comes out is what the declaration says, as `Payments.capture`'s receipt is.
#[test]
fn a_parameter_that_states_a_label_keeps_its_contract() {
    let used = "fn used(key: Secret<Payments>) -> String !{} { \"done\" }";
    clean(used, "    log.public(used(secrets.payments()))");
    // Piped to it, the same.
    clean(used, "    log.public(secrets.payments() |> used())");
    // A plain parameter carries it.
    let echoed = "fn echoed(text: String) -> String !{} { text }";
    leaks(echoed, &format!("    log.public(echoed({SECRET}))"));
    clean(echoed, &format!("    log.public(echoed({WORDS}))"));
}

/// An element chosen by a secret index carries the index's label: where the
/// element is says what the secret is. KNOWN_LIMITATIONS recorded it as the
/// list's alone.
#[test]
fn an_element_a_secret_chooses_carries_its_label() {
    let body = |index: &str| {
        format!(
            "    let n = String.length({index})\n    match List.get([\"a\", \"b\"], n) {{\n        Some(w) => log.public(w),\n        None => (),\n    }}"
        )
    };
    leaks("", &body(SECRET));
    clean("", &body(WORDS));
}
