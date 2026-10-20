//! **A mounted resource does not build** (ADR-0075); **a stream does**
//! (ADR-0148).
//!
//! Until 2026-09-26 each lowered as a literal element. A-008's `<stream
//! query={Recommendations(id)}>` was refused for its `query` attribute, as a
//! computed value (ADR-0073), where it is a stream. A-007's `<map-container
//! resource={StoreMap} center={center} />` built: an element named
//! `map-container` with two attributes, a resource nothing mounts, and a page
//! that fails when rendered, since `center` is a record. Both are valid
//! Pleris, and check. ADR-0075 refused both at build. ADR-0148 compiles the
//! stream: its query's state, as a part. This is a template-stage guarantee;
//! A-008 still has a complete-program generated-WIT name collision, asserted
//! separately below and by the exact accepted-corpus executable-support gate.

use pw_core::check::{Unit, check_sources};

fn files(extra: &[&str]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
        for e in std::fs::read_dir(root.join(d)).unwrap_or_else(|e| panic!("{d}: {e}")) {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                paths.push(p);
            }
        }
    }
    paths.push(root.join("examples/domain.pw"));
    for f in extra {
        paths.push(root.join(f));
    }
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            (
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            )
        })
        .collect()
}

fn units(files: Vec<(String, String)>) -> Vec<Unit> {
    files
        .into_iter()
        .map(|(path, src)| {
            let hir = pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green);
            Unit { path, src, hir }
        })
        .collect()
}

fn checks_clean(files: &[(String, String)]) {
    let found: Vec<String> = check_sources(files)
        .into_iter()
        .flat_map(|(n, ds)| {
            ds.into_iter()
                .map(move |d| format!("{n}: {} {}", d.code, d.message))
        })
        .collect();
    assert!(found.is_empty(), "{found:#?}");
}

fn refused(extra: &str, why: &str) {
    let all = files(&[extra]);
    checks_clean(&all);
    match pw_core::build::build(&units(all)) {
        Ok(_) => panic!("{extra} built"),
        Err(e) => assert!(e.contains(why), "{extra}: {e}"),
    }
}

#[test]
fn a_stream_builds_as_a_part() {
    let all = files(&["examples/accepted/A-008-streamed-public-recommendations.pw"]);
    checks_clean(&all);
    let input = units(all);
    let sources: Vec<_> = input.iter().map(|u| u.src.as_str()).collect();
    let templates = pw_core::program::with_semantic_program(&input, |checked| {
        let cx = checked.context();
        pw_core::build::templates(cx.hirs, &sources, cx.sigs)
    })
    .expect("the stream example is semantically valid");
    let view = templates
        .iter()
        .find(|t| t.path == "store.recommendations.Recommendations")
        .expect("A-008's view");
    assert!(
        view.manifest()
            .iter()
            .any(|e| e.kind == "stream" && e.value == "store.recommendations.Recommendations"),
        "{:?}",
        view.manifest()
    );
}

#[test]
fn the_complete_stream_example_reports_its_exact_wit_collision() {
    let all = files(&["examples/accepted/A-008-streamed-public-recommendations.pw"]);
    checks_clean(&all);
    let input = units(all);
    let report = pw_core::program::validate(&input);
    let errors: Vec<_> = report
        .diagnostics
        .iter()
        .flat_map(|(path, ds)| ds.iter().filter(|d| d.is_error()).map(move |d| (path, d)))
        .collect();
    assert_eq!(errors.len(), 1, "{:?}", report.diagnostics);
    let (path, refusal) = errors[0];
    assert!(path.ends_with("examples/accepted/A-008-streamed-public-recommendations.pw"));
    assert_eq!(refusal.code, "PW6601");
    assert_eq!(refusal.reason, pw_core::program::Stage::Wit.reason());
    assert_eq!(
        refusal.message,
        "the generated WIT does not parse: duplicate item named `store-recommendations-recommendations-api`"
    );
    let source = &input
        .iter()
        .find(|u| &u.path == path)
        .expect("responsible unit")
        .src;
    assert!(source[refusal.primary_span.clone()].contains("query Recommendations"));
    assert!(
        report.into_build().is_err(),
        "the complete program is refused"
    );
}

#[test]
fn a_mounted_resource_does_not_build() {
    refused(
        "examples/accepted/A-007-map-widget-resource-with-cleanup.pw",
        "an element that mounts a resource is not compiled",
    );
}

/// RDFa's `resource="/x"` is an ordinary attribute: the element builds, and
/// the attribute beside it is text, related as any is (ADR-0074).
#[test]
fn a_static_resource_attribute_is_an_attribute() {
    let view = |attr: &str| {
        let mut all = files(&[]);
        all.push((
            "t.pw".to_string(),
            format!(
                "module t\n\nview V(s: String, xs: List<Int>) !{{}} {{\n    \
                 <p resource=\"/r\" {attr}>x</p>\n}}\n"
            ),
        ));
        all
    };
    let ok = view("title={s}");
    checks_clean(&ok);
    if let Err(e) = pw_core::build::build(&units(ok)) {
        panic!("{e}");
    }
    let found: Vec<String> = check_sources(&view("title={xs}"))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect();
    assert_eq!(
        found,
        vec!["PW0609 `title` is `List<Int>`, which has no text form"]
    );
}
