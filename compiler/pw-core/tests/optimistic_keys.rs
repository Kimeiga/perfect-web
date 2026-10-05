//! **An optimistic clause's target may leave a key unnamed** (ADR-0222,
//! ruling 0105-a).
//!
//! The feed's timeline is `Timeline(current_session(), shown)`: its length is
//! the page's, a signal "Load more" grows. A post speculates on it whatever
//! that length is, `Timeline(current_session(), _)`, and the page's binding
//! is matched by the key it names. Until ADR-0222 `_` was a name, and did not
//! resolve.
//!
//! Each test states one part, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn sources(change: &dyn Fn(&str) -> String) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/feed",
    ] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            let src = std::fs::read_to_string(&p).expect("read");
            let src = if d == "examples/feed" {
                change(&src)
            } else {
                src
            };
            out.push((p.display().to_string(), src));
        }
    }
    out
}

fn reported(change: &dyn Fn(&str) -> String) -> Vec<String> {
    check_sources(&sources(change))
        .into_iter()
        .filter(|(n, _)| n.ends_with("feed/app.pw"))
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn build(change: &dyn Fn(&str) -> String) -> pw_core::build::Build {
    let units: Vec<Unit> = sources(change)
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    pw_core::build::build(&units).expect("builds")
}

const TARGET: &str = "optimistic    Timeline(current_session(), _) as feed => pending(feed, text)";

#[test]
fn a_target_may_leave_a_key_unnamed() {
    let same = |s: &str| s.to_string();
    assert!(
        same(&sources(&same).last().expect("the feed").1).contains(TARGET),
        "the feed states it"
    );
    assert_eq!(reported(&same), Vec::<String>::new());
    let b = build(&same);
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let home = b
        .speculations
        .iter()
        .find(|s| s.page == "feed.app.Home")
        .expect("the home page speculates");
    let pw_core::backend::wasm::Encoding::Encoded(module) = &home.module else {
        panic!("{:?}", home.module);
    };
    let bindings: Vec<(String, String, Vec<String>)> = module
        .bindings
        .iter()
        .map(|b| (b.binding.clone(), b.resource.clone(), b.key.clone()))
        .collect();
    assert_eq!(
        bindings,
        [(
            "feed".to_string(),
            "feed.app.Timeline".to_string(),
            vec!["current_session()".to_string(), "shown".to_string()]
        )],
        "the binding by the key it names"
    );
    assert_eq!(module.commands, ["feed.app.post"]);
}

#[test]
fn a_key_left_unnamed_is_a_targets_alone() {
    // In the transition, `_` is a name, and names nothing.
    let in_transition = |s: &str| s.replace("pending(feed, text)", "pending(_, text)");
    let found = reported(&in_transition);
    assert!(
        found.iter().any(|d| d == "PW0021 `_` does not resolve"),
        "{found:#?}"
    );
    // Inside a key, as a call's argument, likewise.
    let nested = |s: &str| s.replace("Timeline(current_session(), _)", "Timeline(author(_), _)");
    let found = reported(&nested);
    assert!(
        found.iter().any(|d| d == "PW0021 `_` does not resolve"),
        "{found:#?}"
    );
    // A key the page does not show the entry by is still no match: the
    // speculation would be shown on no binding.
    let unmatched = |s: &str| {
        s.replace(
            "Timeline(current_session(), _)",
            "Timeline(current_session(), 3)",
        )
    };
    assert_eq!(reported(&unmatched), Vec::<String>::new());
    let refusals = build(&unmatched).refusals();
    assert!(
        refusals
            .iter()
            .any(|r| r.contains("reads by no binding whose key resolves to the same")),
        "{refusals:?}"
    );
}
