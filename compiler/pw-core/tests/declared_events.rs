//! **An element handles an event the platform declares** (ADR-0093).
//!
//! Which event an `on:` attribute delivers is a platform fact, declared in
//! `events` (`events.press(event: PressEvent)`), so declaring a new event is a
//! change to that file rather than to a checker. Until 2026-09-26 an
//! attribute naming no declared event checked: `on:clik={go}` was skipped by
//! the one rule that reads the declaration, and the runtime, which listens for
//! a click whatever the name, ran it by accident. Each test states one case,
//! with a control.

use pw_core::check::check_sources;

/// The platform, and one file.
fn program(src: &str, platform: bool) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    if platform {
        for d in ["packages/pw-std", "packages/pw-platform-web"] {
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
    }
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

fn reported(src: &str, platform: bool) -> Vec<String> {
    check_sources(&program(src, platform))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn button(event: &str) -> String {
    format!(
        "module t\n\ncommand go() -> Int !{{}} {{ 1 }}\n\n\
         view Button(n: Int) !{{}} {{\n    <button on:{event}={{go}}>{{n}}</button>\n}}\n"
    )
}

#[test]
fn an_element_handles_an_event_the_platform_declares() {
    let found = reported(&button("clik"), true);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with(
            "PW5022 `on:clik` names no event the platform declares: `change`, `input`, `keydown`, `press` or `submit`"
        ),
        "{found:#?}"
    );
    for event in ["press", "submit", "input", "keydown", "change"] {
        let found = reported(&button(event), true);
        assert!(found.is_empty(), "on:{event}: {found:#?}");
    }
}

/// A program with no platform declares no events, and there is nothing to
/// hold an `on:` attribute to.
#[test]
fn a_program_without_a_platform_declares_no_events() {
    let found = reported(&button("clik"), false);
    assert!(found.is_empty(), "{found:#?}");
}
