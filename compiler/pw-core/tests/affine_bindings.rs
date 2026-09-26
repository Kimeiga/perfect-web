//! **The affine rule follows bindings, not names** (ADR-0080).
//!
//! PW2005 counts, on every path, how many times a transaction is ended
//! (ADR-0045). Until 2026-09-26 a use of it was any name spelled like it:
//! - an outer `tx` never ended passed, where each branch ended an inner `tx`;
//! - an outer and an inner `tx`, each ended once, were refused as the outer
//!   ended twice, and so was a lambda whose own parameter is named `tx`;
//! - `let end = Database.rollback` then `end(tx)` counted nothing, so ending
//!   it that way once was refused, and twice, with `Database.rollback(tx)`
//!   after, passed.
//!
//! A local bound to a declaration is that declaration where it is called; a
//! transaction given to any other function value may be ended there, where
//! nothing counts, and is refused. Each test states one case, with a control.

use pw_core::check::check_sources;

fn program(src: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
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
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

fn command(body: &str) -> String {
    format!(
        "module t\n\nimport Carts\nimport Database\nimport context.{{ current_session }}\n\
         import domain.{{ CartError }}\n\n\
         command c(flag: Bool) -> Result<(), CartError>\n    requires SignedIn\n{{\n{body}\n}}\n"
    )
}

fn reported(body: &str) -> Vec<String> {
    check_sources(&program(&command(body)))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn refused(body: &str, says: &str) {
    let found = reported(body);
    assert_eq!(found.len(), 1, "{body}: {found:#?}");
    assert!(
        found[0].starts_with("PW2005") && found[0].contains(says),
        "{body}: {found:#?}"
    );
}

fn clean(body: &str) {
    let found = reported(body);
    assert!(found.is_empty(), "{body}: {found:#?}");
}

const INNER: &str = "        let tx = Database.begin()\n        Database.rollback(tx)";

#[test]
fn an_inner_transaction_does_not_end_an_outer_one() {
    refused(
        &format!(
            "    let tx = Database.begin()\n    if flag {{\n{INNER}\n    }} else {{\n{INNER}\n    }}\n    Ok(())"
        ),
        "is not consumed on every path",
    );
    // Each ended once.
    clean(&format!(
        "    let tx = Database.begin()\n    if flag {{\n{INNER}\n    }}\n    Database.rollback(tx)"
    ));
}

#[test]
fn a_lambdas_own_name_is_not_the_transaction() {
    clean(
        "    let tx = Database.begin()\n    let f = (tx) => Database.rollback(tx)\n    Database.rollback(tx)",
    );
    // The transaction itself, ended inside a function value, is refused.
    refused(
        "    let tx = Database.begin()\n    let f = () => Database.rollback(tx)\n    Database.rollback(tx)",
        "inside a loop or a function value",
    );
}

#[test]
fn a_local_bound_to_a_release_is_that_release() {
    clean("    let tx = Database.begin()\n    let end = Database.rollback\n    end(tx)");
    refused(
        "    let tx = Database.begin()\n    let end = Database.rollback\n    end(tx)\n    Database.rollback(tx)",
        "is released twice on one path",
    );
}

#[test]
fn a_transaction_given_to_a_function_value_is_refused() {
    refused(
        "    let tx = Database.begin()\n    let f = (t) => Database.rollback(t)\n    f(tx)",
        "is given to `f`, a function value, which may release it",
    );
    refused(
        "    let tx = Database.begin()\n    let end = if flag { Database.rollback } else { Database.commit }\n    end(tx)",
        "is given to `end`, a function value",
    );
    // A binding that may be reassigned holds whatever was assigned last.
    refused(
        "    let tx = Database.begin()\n    let mut end = Database.rollback\n    end = Database.commit\n    end(tx)",
        "is given to `end`, a function value",
    );
}
