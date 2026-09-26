//! **An arm no value reaches is refused** (ADR-0076).
//!
//! A match tries its arms in order, so an arm whose values the arms before it
//! already take never runs. The exhaustiveness analysis has found such arms
//! from the start (`exhaust::MatchReport::unreachable`), and until 2026-09-26
//! nothing reported one:
//! - `_ => 0` before `Circle(r) => r` checked, and the backend refused it;
//! - `Some(y)` after `Some(x)` checked, and the backend refused it;
//! - `1 => 20` after `1 => 10` checked and built, its arm dead.
//!
//! Each test states one case, with a control.

use pw_core::check::{Unit, check_sources};

fn program(src: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/pw-std");
    let mut out: Vec<(String, String)> = std::fs::read_dir(root)
        .expect("pw-std")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .map(|p| {
            (
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            )
        })
        .collect();
    out.sort();
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

const SHAPE: &str = "type Shape =\n    | Circle(Int)\n    | Empty\n\n";

fn fun(ty: &str, arms: &str) -> String {
    format!(
        "module t\n\n{SHAPE}fn f(v: {ty}) -> Int !{{}} {{\n    match v {{\n{arms}\n    }}\n}}\n"
    )
}

/// Exactly one diagnostic, the unreachable arm's.
fn unreachable(ty: &str, arms: &str) {
    let found = reported(&fun(ty, arms));
    assert_eq!(found.len(), 1, "{arms}: {found:#?}");
    assert!(
        found[0].starts_with("PW0333")
            && found[0].contains("no `")
            && found[0].contains("reaches this arm"),
        "{arms}: {found:#?}"
    );
}

fn clean(ty: &str, arms: &str) {
    let found = reported(&fun(ty, arms));
    assert!(found.is_empty(), "{arms}: {found:#?}");
}

#[test]
fn an_arm_after_a_wildcard_is_refused() {
    unreachable("Shape", "        _ => 0,\n        Shape.Circle(r) => r,");
    clean("Shape", "        Shape.Circle(r) => r,\n        _ => 0,");
}

#[test]
fn a_case_matched_twice_is_refused() {
    unreachable(
        "Option<Int>",
        "        Some(x) => x,\n        None => 0,\n        Some(y) => y,",
    );
    clean("Option<Int>", "        Some(x) => x,\n        None => 0,");
}

#[test]
fn a_literal_matched_twice_is_refused() {
    unreachable("Int", "        1 => 10,\n        1 => 20,\n        _ => 0,");
    clean("Int", "        1 => 10,\n        2 => 20,\n        _ => 0,");
}

#[test]
fn a_wildcard_after_every_case_is_refused() {
    unreachable(
        "Option<Int>",
        "        Some(x) => x,\n        None => 0,\n        _ => 1,",
    );
}

/// The diagnostic is a projection of the analysis: the audit reads the same
/// arm the developer is told about.
#[test]
fn the_analysis_says_which_arm() {
    let src = fun("Int", "        1 => 10,\n        1 => 20,\n        _ => 0,");
    let units: Vec<Unit> = program(&src)
        .into_iter()
        .map(|(path, src)| {
            let hir = pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green);
            Unit { path, src, hir }
        })
        .collect();
    let arms: Vec<String> = pw_core::check::match_analysis(&units)
        .into_iter()
        .filter(|m| m.declaration == "f")
        .flat_map(|m| m.unreachable)
        .map(|s| src[s.start..s.end].to_string())
        .collect();
    assert_eq!(arms, vec!["1".to_string()]);
}
