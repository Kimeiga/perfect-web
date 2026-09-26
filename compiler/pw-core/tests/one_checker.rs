//! **`pw build` checks what `pw check` checks** (ADR-0090).
//!
//! The declaration rules (`rules::check`: a retry without an idempotency
//! key, an unbounded retry, a stale session read, a keyed query with no
//! stale-work policy, ..) ran beside `check_sources` in the command, and
//! nowhere else. `pw build` checks through `check_units`, so until 2026-09-26
//! it compiled R-015's `retry forever` query into a component and built
//! R-014, each of which `pw check` refuses. One checker runs them now, for
//! the command, the build and every test that asks `check_sources`.

use pw_core::build::build;
use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

/// The library every corpus program is checked against, and one file.
fn program(extra: &[&str]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        paths.extend(ps);
    }
    paths.push(root.join("examples/domain.pw"));
    paths.extend(extra.iter().map(|e| root.join(e)));
    paths
        .iter()
        .map(|p| {
            (
                p.display().to_string(),
                std::fs::read_to_string(p).expect("read"),
            )
        })
        .collect()
}

fn units(files: &[(String, String)]) -> Vec<Unit> {
    files
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            hir: lower_file(src, &parse_tree(src).green),
            src: src.clone(),
        })
        .collect()
}

/// What `check_sources` reports for the one file `name` names.
fn codes(files: &[(String, String)], name: &str) -> Vec<&'static str> {
    check_sources(files)
        .into_iter()
        .filter(|(p, _)| p.ends_with(name))
        .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code))
        .collect()
}

const R014: &str = "examples/rejected/R-014-retry-of-non-idempotent-command.pw";
const R015: &str = "examples/rejected/R-015-unbounded-retry.pw";

#[test]
fn the_build_refuses_what_check_refuses() {
    for (fixture, code) in [(R014, "[PW0312]"), (R015, "[PW0313]")] {
        let files = program(&[fixture]);
        match build(&units(&files)) {
            Ok(_) => panic!("{fixture} built, and `pw check` refuses it"),
            Err(e) => assert!(e.contains(code), "{fixture}: {e}"),
        }
    }
}

/// The control: a program that checks builds, the store.
#[test]
fn a_program_that_checks_builds() {
    let files = program(&["examples/store/app.pw", "examples/store/menu.pw"]);
    if let Err(e) = build(&units(&files)) {
        panic!("the store does not build: {e}");
    }
}

#[test]
fn check_sources_runs_the_declaration_rules() {
    assert_eq!(
        codes(
            &program(&[R014]),
            "R-014-retry-of-non-idempotent-command.pw"
        ),
        ["PW0312"]
    );
    assert_eq!(
        codes(&program(&[R015]), "R-015-unbounded-retry.pw"),
        ["PW0313"]
    );
}
