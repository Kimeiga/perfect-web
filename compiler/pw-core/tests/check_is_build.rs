//! Complete executable validation beside intentional semantic-only checking.
use pw_core::check::{Unit, check_program_sources, check_sources};
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
fn the_public_callback_uses_the_exact_source_units() {
    let input = units(&[("app.pw", "module app\nquery N() -> Int { 1 }\n")]);
    pw_core::program::with_semantic_program(&input, |checked| {
        assert!(std::ptr::eq(checked.context().hirs[0], &input[0].hir));
    })
    .unwrap();
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

#[test]
fn a_wit_failure_preserves_independent_page_refusals_and_its_own_file() {
    let inputs = units(&[
        (
            "page.pw",
            "module page\npage Home() {\n route \"/\"\n cache private\n signal n: Int = 0\n view { <title>{n}</title><button type=\"button\" on:press={() => n = n + 1}>Add</button> }\n}\n",
        ),
        (
            "collision.pw",
            "module collision\nquery Foo() -> Int { 1 }\nquery foo() -> Int { 2 }\n",
        ),
    ]);
    let report = pw_core::program::validate(&inputs);
    let page = &report.diagnostics[0].1;
    let wit = &report.diagnostics[1].1;
    assert!(
        page.iter()
            .any(|d| d.reason == "backend_page" && d.message.contains("title part")),
        "{page:?}"
    );
    let collision = wit
        .iter()
        .find(|d| d.reason == "backend_wit")
        .expect("WIT failure at its responsible file");
    assert!(inputs[1].src[collision.primary_span.clone()].contains("query foo"));
    assert!(report.into_build().is_err());
}

#[test]
fn an_unmappable_export_keeps_its_declaration_identity() {
    let inputs = units(&[
        ("other.pw", "module other\nfn one() -> Int !{} { 1 }\n"),
        (
            "wrapped.pw",
            "module m\nopaque type Wrapped<T> = List<T>\nquery Pick() -> Wrapped<Int> { todo }\n",
        ),
    ]);
    let report = pw_core::program::validate(&inputs);
    assert!(report.diagnostics[0].1.is_empty());
    let failure = report.diagnostics[1]
        .1
        .iter()
        .find(|d| d.reason == "backend_wit")
        .expect("unmappable export at its source");
    assert!(failure.message.contains("has no WIT form"), "{failure:?}");
    assert!(inputs[1].src[failure.primary_span.clone()].contains("query Pick"));
}

#[test]
fn editing_a_report_cannot_turn_a_refused_plan_into_a_validated_one() {
    let input = units(&[(
        "app.pw",
        "module m\nquery Count() -> Int { todo }\npage Home() {\n route \"/\"\n let n = query Count()\n view { <title>Home</title><p>{n}</p> }\n}\n",
    )]);
    let mut report = pw_core::program::validate(&input);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|(_, ds)| ds.iter().any(|d| d.is_error()))
    );
    report.diagnostics.clear();
    assert!(
        report.into_build().is_err(),
        "public report data cannot mint the private validated plan"
    );
}
#[test]
fn syntax_errors_never_lend_a_backend_prerequisite_proof() {
    let input = units(&[("broken.pw", "module broken\nquery Bad() -> Int { + }\n")]);
    let called = std::cell::Cell::new(false);
    let errors = pw_core::program::with_semantic_program(&input, |_| called.set(true))
        .err()
        .expect("syntax errors refuse the proof");
    assert!(!called.get());
    assert!(
        errors
            .iter()
            .flat_map(|(_, ds)| ds)
            .any(|d| d.code == "PW0009")
    );
}

#[test]
fn a_bad_signal_initial_value_is_not_hidden_by_a_page_refusal() {
    let input = units(&[(
        "both.pw",
        "module both\npage Home() {\n route \"/\"\n cache private\n signal n: Int = 1 + 2\n view { <title>{n}</title><button type=\"button\" on:press={() => n = n + 1}>Add</button> }\n}\n",
    )]);
    let report = pw_core::program::validate(&input);
    let errors = &report.diagnostics[0].1;
    assert!(
        errors
            .iter()
            .any(|d| d.reason == "backend_page" && d.message.contains("title part")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|d| d.reason == "backend_signal" && d.message.contains("operation")),
        "{errors:?}"
    );
    assert!(report.into_build().is_err());
}
