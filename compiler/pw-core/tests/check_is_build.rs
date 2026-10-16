//! Complete executable validation beside intentional semantic-only checking.
use pw_core::{
    backend::lower::{Checked, Context},
    check::{Unit, check_program_sources, check_sources},
};
fn units(files: &[(&str, &str)]) -> Vec<Unit> {
    files
        .iter()
        .map(|(path, src)| Unit {
            path: (*path).into(),
            src: (*src).into(),
            hir: pw_core::lower::lower_file(src, &pw_syntax::parse_tree(src).green),
        })
        .collect()
}
fn files(src: &str) -> Vec<(String, String)> {
    vec![("app.pw".into(), src.into())]
}
#[test]
fn semantic_library_checks_do_not_promise_executable_support() {
    let src = "module m\nquery Count() -> Int { todo }\npage Home() {\n route \"/\"\n let n = query Count()\n view { <title>Home</title><p>{n}</p> }\n}\n";
    assert!(check_sources(&files(src))[0].1.is_empty());
    let checked = check_program_sources(&files(src));
    let diagnostic = &checked[0].1[0];
    assert_eq!(diagnostic.code, "PW6601");
    assert_eq!(diagnostic.reason, "backend_placeholder");
    assert!(src[diagnostic.primary_span.clone()].contains("page Home"));
    assert!(diagnostic.explanation.is_some());
    assert!(!diagnostic.repairs.is_empty());
    let error = pw_core::program::build(&units(&[("app.pw", src)])).unwrap_err();
    assert!(
        error.contains("PW6601") && error.contains(&diagnostic.message),
        "{error}"
    );
}
#[test]
fn an_unread_placeholder_remains_supported() {
    let src = "module m\nquery Count() -> Int { todo }\n";
    let build = pw_core::program::build(&units(&[("app.pw", src)])).unwrap();
    assert_eq!(build.placeholders(), ["m.Count"]);
    assert!(build.refusals().is_empty());
}
#[test]
fn semantic_errors_short_circuit_support_diagnostics() {
    let src = "module m\nquery Count() -> Int { \"wrong\" }\n";
    let semantic = check_sources(&files(src));
    let executable = check_program_sources(&files(src));
    assert!(!semantic[0].1.is_empty());
    assert_eq!(semantic[0].1, executable[0].1);
    assert!(executable[0].1.iter().all(|d| d.code != "PW6601"));
}
#[test]
fn a_proof_cannot_be_reused_for_other_hir_of_equal_length() {
    let a = units(&[("a.pw", "module a\nquery N() -> Int { 1 }\n")]);
    let b = units(&[("b.pw", "module b\nquery N() -> Int { 2 }\n")]);
    let hirs: Vec<_> = b.iter().map(|u| &u.hir).collect();
    let ws = pw_core::resolve::Workspace::build(&hirs);
    let sigs = pw_core::signatures::Signatures::build(&ws, &hirs);
    let contracts = pw_core::contract::contracts(&hirs, &sigs, &ws);
    let make = || {
        Checked::of(
            &a,
            Context {
                hirs: &hirs,
                ws: &ws,
                sigs: &sigs,
                contracts: &contracts,
            },
        )
    };
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(make)).is_err());
    assert!(
        Checked::of(
            &b,
            Context {
                hirs: &hirs,
                ws: &ws,
                sigs: &sigs,
                contracts: &contracts
            }
        )
        .is_ok()
    );
}
#[test]
fn a_partial_inspection_plan_cannot_be_written() {
    let src = "module m\nquery Count() -> Int { todo }\npage Home() {\n route \"/\"\n let n = query Count()\n view { <title>Home</title><p>{n}</p> }\n}\n";
    let partial = pw_core::build::build(&units(&[("app.pw", src)])).unwrap();
    assert!(!partial.refusals().is_empty());
    let path = std::env::temp_dir().join(format!("pw-partial-plan-{}", std::process::id()));
    assert!(!path.exists());
    assert!(partial.write(&path).is_err());
    assert!(!path.exists());
}
