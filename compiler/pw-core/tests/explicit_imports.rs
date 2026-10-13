//! **A name a file imports by name is the file's, before a whole module's**
//! (ADR-0326; found by NEXT 6a's money, 2026-10-12).
//!
//! `import List` beside `import domain.{ sum }` left `sum` ambiguous between
//! `List`'s and `domain`'s: `pw check` passed the call, and the build refused
//! it ("`sum` names no declaration this program contains"). A name imported
//! by name now shadows a whole module's, as an explicit `use` shadows a glob
//! in Rust; two whole modules that both declare a name the file calls are
//! refused where the call is written (PW0022). Each test states one case,
//! with its control.

use pw_core::check::{Unit, check_sources};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::{Namespace, Resolution, Workspace};
use pw_syntax::parse_tree;

const A: &str = "module a\n\nfn sum(n: Int) -> Int !{} {\n    n + 1\n}\n";
const B: &str = "module b\n\nfn sum(n: Int) -> Int !{} {\n    n + 2\n}\n";

/// A module that imports as `imports` says and calls `call` in a query.
fn m(imports: &str, call: &str) -> String {
    format!(
        "module m\n\n{imports}\n\npublic query Q() -> Int\n    freshness 0.seconds\n{{\n    {call}\n}}\n"
    )
}

fn files(m: &str) -> Vec<(String, String)> {
    vec![
        ("a.pw".to_string(), A.to_string()),
        ("b.pw".to_string(), B.to_string()),
        ("m.pw".to_string(), m.to_string()),
    ]
}

/// Each `code message` the check reports for `m.pw`.
fn reported(m: &str) -> Vec<String> {
    check_sources(&files(m))
        .into_iter()
        .filter(|(n, _)| n == "m.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// Which module `sum` means in `m.pw`, as the workspace resolves it.
fn sum_means(m: &str) -> Resolution {
    let hirs: Vec<Hir> = files(m)
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    Workspace::build(&refs).resolve_in(2, Namespace::Term, "sum")
}

/// `m.Q` compiled to a component.
fn built(m: &str) -> Result<(), String> {
    let units: Vec<Unit> = files(m)
        .into_iter()
        .map(|(path, src)| Unit {
            hir: lower_file(&src, &parse_tree(&src).green),
            path,
            src,
        })
        .collect();
    pw_core::backend::component::compile(&units, "m.Q").map(|_| ())
}

/// **Imported by name, it is that module's**, beside a whole module that
/// declares the same name: checked clean, built, and `b`'s.
#[test]
fn a_name_imported_by_name_is_the_files_before_a_whole_modules() {
    let src = m("import a\nimport b.{ sum }", "sum(1)");
    assert!(reported(&src).is_empty(), "{:?}", reported(&src));
    assert!(
        matches!(sum_means(&src), Resolution::Imported { ref from, .. } if from == "b"),
        "{:?}",
        sum_means(&src)
    );
    built(&src).expect("it builds");
    // Control: the other way round, it is `a`'s.
    let src = m("import a.{ sum }\nimport b", "sum(1)");
    assert!(
        matches!(sum_means(&src), Resolution::Imported { ref from, .. } if from == "a"),
        "{:?}",
        sum_means(&src)
    );
}

/// **Two whole modules that both declare it leave it ambiguous**, and the
/// call is refused where it is written, naming both, with the repair.
/// Until ADR-0326 it checked clean and the build refused it.
#[test]
fn a_call_two_whole_modules_declare_is_refused_where_it_is_written() {
    let src = m("import a\nimport b", "sum(1)");
    let found = reported(&src);
    assert!(
        found.contains(
            &"PW0022 `sum` is declared by `a` and `b`, each imported whole, and the file names \
              neither"
                .to_string()
        ),
        "{found:?}"
    );
    let repairs: Vec<String> = check_sources(&files(&src))
        .into_iter()
        .filter(|(n, _)| n == "m.pw")
        .flat_map(|(_, ds)| ds)
        .filter(|d| d.code == "PW0022")
        .flat_map(|d| d.repairs.into_iter().map(|r| r.description))
        .collect();
    assert_eq!(
        repairs,
        ["write `a.sum` or `b.sum`, or import `sum` by name from the module meant"]
    );
    assert!(
        built(&src).is_err(),
        "nothing is built from a program that does not check"
    );
    // Controls: each named by its module, and a whole module alone.
    for (imports, call) in [
        ("import a\nimport b", "a.sum(1)"),
        ("import a\nimport b", "b.sum(1)"),
        ("import a", "sum(1)"),
    ] {
        let src = m(imports, call);
        assert!(
            reported(&src).is_empty(),
            "{imports} / {call}: {:?}",
            reported(&src)
        );
        built(&src).expect("it builds");
    }
}
