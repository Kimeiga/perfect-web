//! **An assignment to a field has the field's type** (ADR-0070).
//!
//! `b.value = "wrong"`, where `value` is an `Int`, passed `pw check` until
//! 2026-09-26: an assignment was related only where its target is a name.
//! The backend refuses an assignment to a field by name (ADR-0051), so the
//! program never ran; the checker said nothing about why it is wrong.

use pw_core::check::check_sources;

fn std_lib() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/pw-std");
    let mut out = Vec::new();
    for e in std::fs::read_dir(root).expect("pw-std") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "pw") {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.sort();
    out
}

fn reported(src: &str) -> Vec<String> {
    let mut all = std_lib();
    all.push(("t.pw".to_string(), src.to_string()));
    check_sources(&all)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

const PROGRAM: &str = "module t\n\ntype Box = Box { value: Int, label: String }\n\nfn f(n: Int) -> Int {\n    let mut b = Box { value: n, label: \"a\" }\n    b.value = VALUE\n    b.value\n}\n";

#[test]
fn an_assignment_to_a_field_has_the_fields_type() {
    let wrong = reported(&PROGRAM.replace("VALUE", "\"wrong\""));
    assert!(
        wrong
            .iter()
            .any(|d| d.starts_with("PW0607") && d.contains("value")),
        "{wrong:#?}"
    );
    let right = reported(&PROGRAM.replace("VALUE", "n + 1"));
    assert!(right.is_empty(), "{right:#?}");
}
