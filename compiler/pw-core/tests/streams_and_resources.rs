//! **A stream and a mounted resource do not build** (ADR-0075).
//!
//! Neither is compiled by this renderer. Until 2026-09-26 each lowered as a
//! literal element. A-008's `<stream query={Recommendations(id)}>` was refused
//! for its `query` attribute, as a computed value (ADR-0073), where it is a
//! stream. A-007's `<map-container resource={StoreMap} center={center} />`
//! built: an element named `map-container` with two attributes, a resource
//! nothing mounts, and a page that fails when rendered, since `center` is a
//! record. Both are valid Pleris, and check.

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
fn a_stream_does_not_build() {
    refused(
        "examples/accepted/A-008-streamed-public-recommendations.pw",
        "a `<stream>` is not compiled by this renderer",
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
