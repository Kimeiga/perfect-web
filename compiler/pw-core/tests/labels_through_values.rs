//! **A function value carries the label of what it makes** (ADR-0079).
//!
//! A declaration named as a value is labelled by what calling it makes, as a
//! lambda is by its body (ADR-0064), and a call through a value carries its
//! callee's label. Until 2026-09-26 a name that meant a declaration was
//! public, and so was a call through a value, whatever its callee held. So a
//! `Secret<Payments>` logged publicly passed `pw check` through:
//! - `let f = secrets.payments` then `f()`;
//! - `let f = payments`, imported by name, then `f()`;
//! - a record field holding `secrets.payments`, called through.
//!
//! A helper whose declared result is the secret was already refused: its
//! signature carries the label. Each test states one case, with a control.

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

fn module(decls: &str) -> String {
    format!(
        "module t\n\nimport log\nimport secrets\nimport secrets.{{ payments }}\n\
         import capability.{{ Payments, Public, Secret }}\n\n\
         fn greeting() -> String !{{}} {{ \"hello\" }}\n\n{decls}\n"
    )
}

fn leak(body: &str) -> String {
    module(&format!(
        "type Box = Box {{ f: fn() -> Secret<Payments> }}\n\n\
         type Words = Words {{ f: fn() -> String }}\n\n\
         fn leak() -> () !{{ log<Public>, secret<Payments> }} {{\n{body}\n}}"
    ))
}

fn refused(body: &str) {
    let found = reported(&leak(body));
    assert_eq!(found.len(), 1, "{body}: {found:#?}");
    assert!(
        found[0].starts_with("PW5006 cannot log a `Secret<Payments>`"),
        "{body}: {found:#?}"
    );
}

fn clean(body: &str) {
    let found = reported(&leak(body));
    assert!(found.is_empty(), "{body}: {found:#?}");
}

#[test]
fn a_secret_made_through_a_function_value_is_labelled() {
    refused("    let f = secrets.payments\n    log.public(\"{f()}\")");
    refused("    let f = payments\n    log.public(\"{f()}\")");
    refused("    let b = Box { f: secrets.payments }\n    log.public(\"{b.f()}\")");
    // The same routes, holding a function that makes a public value.
    clean("    let f = greeting\n    log.public(\"{f()}\")");
    clean("    let w = Words { f: greeting }\n    log.public(\"{w.f()}\")");
}

/// A binding in scope is its own value, whatever declaration shares its name.
#[test]
fn a_binding_named_like_a_secrets_source_is_its_value() {
    clean("    let payments = \"none\"\n    log.public(\"{payments}\")");
}
