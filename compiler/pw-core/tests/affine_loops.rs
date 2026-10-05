//! **A path that leaves a loop's body leaves the function, and owes its
//! releases** (ADR-0211, the owner's ruling 0045-a).
//!
//! A `for` had no exits: a `return`, or a failing `?`, inside its body was not
//! a path out of the function, so a transaction it left open checked. The
//! correct program, rolling back and then returning, was refused as a
//! release inside a loop. Both are fixed: an exit inside a pass owes what it
//! has not released, and a release on a path that leaves before the pass ends
//! runs once.

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

/// What checking a command with `body` reports, as `code message`.
fn reported(body: &str) -> Vec<String> {
    let src = format!(
        "module t\n\nimport Carts\nimport Database\nimport context.{{ current_session }}\n\
         import domain.{{ CartError }}\n\n\
         command c(sessions: List<String>) -> Result<(), CartError>\n    requires SignedIn\n{{\n{body}\n}}\n"
    );
    check_sources(&program(&src))
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

#[test]
fn a_return_inside_a_loop_owes_the_release() {
    refused(
        "    let tx = Database.begin()\n    for s in sessions {\n        if s == \"\" {\n            return Ok(())\n        }\n    }\n    tx.commit()",
        "not consumed on every path",
    );
}

#[test]
fn a_failing_try_inside_a_loop_owes_the_release() {
    refused(
        "    let tx = Database.begin()\n    for s in sessions {\n        Carts.clear(current_session())?\n    }\n    tx.commit()",
        "not consumed on every path",
    );
}

#[test]
fn a_release_then_a_return_inside_a_loop_runs_once() {
    clean(
        "    let tx = Database.begin()\n    for s in sessions {\n        if s == \"\" {\n            let _rolled_back = Database.rollback(tx)\n            return Ok(())\n        }\n    }\n    tx.commit()",
    );
    // In a loop inside the loop too.
    clean(
        "    let tx = Database.begin()\n    for s in sessions {\n        for t in sessions {\n            if s == t {\n                let _rolled_back = Database.rollback(tx)\n                return Ok(())\n            }\n        }\n    }\n    tx.commit()",
    );
}

#[test]
fn a_release_on_a_path_that_goes_round_again_is_refused() {
    // Released, and the pass continues: the next pass, or the commit after
    // the loop, releases it again.
    refused(
        "    let tx = Database.begin()\n    for s in sessions {\n        if s == \"\" {\n            let _rolled_back = Database.rollback(tx)\n        }\n    }\n    tx.commit()",
        "inside a loop",
    );
    // And released then tried: the `?` that succeeds goes round again.
    refused(
        "    let tx = Database.begin()\n    for s in sessions {\n        Database.rollback(tx)?\n    }\n    tx.commit()",
        "inside a loop",
    );
}

#[test]
fn an_exit_inside_a_nested_loop_owes_the_release_too() {
    refused(
        "    let tx = Database.begin()\n    for s in sessions {\n        for t in sessions {\n            if s == t {\n                return Ok(())\n            }\n        }\n    }\n    tx.commit()",
        "not consumed on every path",
    );
}

#[test]
fn a_loop_that_leaves_nothing_open_is_unchanged() {
    clean(
        "    let tx = Database.begin()\n    for s in sessions {\n        let _length = s\n    }\n    tx.commit()",
    );
    // A transaction begun and ended inside one pass.
    clean(
        "    for s in sessions {\n        let tx = Database.begin()\n        let _committed = tx.commit()\n    }\n    Ok(())",
    );
}
