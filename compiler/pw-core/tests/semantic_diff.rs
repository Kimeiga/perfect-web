//! **`pw diff`: what a change means, for review** (ADR-0149, charter §19.2).
//!
//! The store, before and after each benchmark task's reference patch, as a
//! reviewer would be shown it: the section each change belongs to, one line
//! each, and `none` where a section has nothing.

use pw_core::check::Unit;
use pw_core::semantic::{Model, diff};

/// The store, with `patches` applied in order to a copy of its sources.
fn store(patches: &[&str]) -> Vec<Unit> {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let work = std::env::temp_dir().join(format!(
        "pw-semantic-{}-{}",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&work);
    let pw = |dir: &std::path::Path| -> Vec<std::path::PathBuf> {
        let mut out: Vec<_> = std::fs::read_dir(dir)
            .map(|d| d.map(|e| e.expect("entry").path()).collect())
            .unwrap_or_default();
        out.retain(|p: &std::path::PathBuf| p.extension().is_some_and(|x| x == "pw"));
        out.sort();
        out
    };
    for rel in ["lib", "store"] {
        std::fs::create_dir_all(work.join(rel)).expect("dir");
        for p in pw(&root.join("benchmarks/baselines/pleris").join(rel)) {
            std::fs::copy(&p, work.join(rel).join(p.file_name().expect("name"))).expect("copy");
        }
    }
    std::fs::copy(
        root.join("benchmarks/baselines/pleris/domain.pw"),
        work.join("domain.pw"),
    )
    .expect("copy");
    for (i, patch) in patches.iter().enumerate() {
        let file = work.join(format!("{i}.patch"));
        std::fs::write(&file, patch).expect("patch");
        let applied = std::process::Command::new("git")
            .args(["apply", file.to_str().expect("path")])
            .current_dir(&work)
            .output()
            .expect("git runs");
        assert!(
            applied.status.success(),
            "{}",
            String::from_utf8_lossy(&applied.stderr)
        );
    }
    let mut paths = Vec::new();
    for p in ["packages/pw-std", "packages/pw-platform-web"] {
        paths.extend(pw(&root.join(p)));
    }
    paths.push(work.join("domain.pw"));
    paths.extend(pw(&work.join("lib")));
    paths.extend(pw(&work.join("store")));
    paths
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            Unit {
                hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
                path: p.display().to_string(),
                src,
            }
        })
        .collect()
}

macro_rules! task {
    ($task:literal, $part:literal) => {
        include_str!(concat!(
            "../../../benchmarks/tasks/",
            $task,
            "/",
            $part,
            "/pleris.patch"
        ))
    };
}

/// The report's lines in one section.
fn section<'r>(report: &'r pw_core::semantic::Report, title: &str) -> &'r [String] {
    &report
        .sections
        .iter()
        .find(|s| s.title == title)
        .unwrap_or_else(|| panic!("no section `{title}`"))
        .changes
}

#[test]
fn the_same_program_changes_nothing() {
    let a = Model::of(&store(&[])).expect("the store checks");
    let b = Model::of(&store(&[])).expect("the store checks");
    let report = diff(&a, &b);
    assert!(report.is_empty(), "{}", report.text());
    // Every section is still there, saying `none`: read as checked.
    assert_eq!(
        report.text().matches("- none").count(),
        report.sections.len()
    );
}

#[test]
fn a_new_case_is_a_domain_change() {
    let setup = task!("T04-order-ready", "setup");
    let before = Model::of(&store(&[setup])).expect("T04's setup checks");
    let after = Model::of(&store(&[setup, task!("T04-order-ready", "reference")]))
        .expect("T04's reference checks");
    let report = diff(&before, &after);
    assert_eq!(
        section(&report, "Domain changes"),
        ["added `Ready` to `domain.OrderStatus`"]
    );
    // A new case, shown by the page: nothing else a reviewer must read.
    assert!(section(&report, "Effects").is_empty(), "{}", report.text());
    assert!(
        section(&report, "Capabilities").is_empty(),
        "{}",
        report.text()
    );
    assert!(
        section(&report, "Cache and freshness").is_empty(),
        "{}",
        report.text()
    );
}

#[test]
fn a_streamed_region_is_a_page_change() {
    let setup = task!("T05-stream-recommendations", "setup");
    let before = Model::of(&store(&[setup])).expect("T05's setup checks");
    let after = Model::of(&store(&[
        setup,
        task!("T05-stream-recommendations", "reference"),
    ]))
    .expect("T05's reference checks");
    let report = diff(&before, &after);
    assert_eq!(
        section(&report, "Pages"),
        ["`store.page.StorePage` streams `store.page.Recommendations(id), streamed`"]
    );
}

#[test]
fn a_new_query_brings_its_effects_capabilities_and_policies() {
    let before = Model::of(&store(&[])).expect("the store checks");
    let after = Model::of(&store(&[task!("T05-stream-recommendations", "setup")]))
        .expect("T05's setup checks");
    let report = diff(&before, &after);
    let caps = section(&report, "Capabilities");
    assert!(
        caps.iter()
            .any(|c| c == "the node must now grant `network.fetch`"),
        "{caps:?}"
    );
    let cache = section(&report, "Cache and freshness");
    assert!(
        cache
            .iter()
            .any(|c| c == "`store.page.Recommendations` (new): `delivery streamed`"),
        "{cache:?}"
    );
    assert!(
        section(&report, "Privacy changes")
            .iter()
            .any(|c| c == "`store.page.Recommendations` (new) is `public`"),
        "{}",
        report.text()
    );
}

#[test]
fn a_freshness_change_is_a_cache_change() {
    let setup = task!("T09-notice-freshness", "setup");
    let before = Model::of(&store(&[setup])).expect("T09's setup checks");
    let after = Model::of(&store(&[setup, task!("T09-notice-freshness", "reference")]))
        .expect("T09's reference checks");
    let report = diff(&before, &after);
    assert_eq!(
        section(&report, "Cache and freshness"),
        ["`store.page.Notice`: `freshness` is `5.seconds`, was `5.minutes`"]
    );
}

#[test]
fn a_program_that_does_not_check_has_no_model() {
    let refused = Model::of(&store(&[
        task!("T05-stream-recommendations", "setup"),
        task!("T05-stream-recommendations", "unsafe"),
    ]));
    let why = refused.expect_err("T05's unsafe patch does not check");
    // Its errors, each by the file it is in: what the build's own refusal
    // does not say, which is why `pw diff` checks before it builds.
    assert!(why.contains("store/app.pw: PW5400"), "{why}");
}

#[test]
fn a_comment_or_a_reflow_changes_nothing() {
    let mut units = store(&[]);
    let app = units
        .iter_mut()
        .find(|u| u.path.ends_with("store/app.pw"))
        .expect("app.pw");
    let src = app.src.replacen(
        "module store.page\n",
        "module store.page\n\n// A comment, and a blank line.\n",
        1,
    );
    *app = Unit {
        hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
        path: app.path.clone(),
        src,
    };
    let report = diff(
        &Model::of(&store(&[])).expect("the store checks"),
        &Model::of(&units).expect("the commented store checks"),
    );
    assert!(report.is_empty(), "{}", report.text());
}
