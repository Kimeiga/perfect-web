//! **A build is named by what it built** (ADR-0300).
//!
//! `pw build` writes `build-id` last: an FNV-1a hash over every file it
//! wrote, by path and bytes, in the order written. The same sources build the
//! same name; a change to what the build writes, a template's, a handler's or
//! a page's plan, changes it. Each test states one part, with its control.

use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

/// The packages, then the feed's `app.pw` as `change` writes it.
fn feed(change: &dyn Fn(&str) -> String) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
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
    let src = std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("the feed");
    out.push(("app.pw".to_string(), change(&src)));
    out
}

/// The name the build of `files` writes, and the directory it wrote.
fn named(files: &[(String, String)]) -> (String, tempfile::TempDir) {
    let units: Vec<Unit> = files
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            hir: lower_file(src, &parse_tree(src).green),
            src: src.clone(),
        })
        .collect();
    let build = pw_core::build::build(&units).expect("builds");
    assert!(build.refusals().is_empty(), "{:?}", build.refusals());
    let dir = tempfile::TempDir::with_prefix("pw-build-id-").expect("a temporary directory");
    let lines = build.write(dir.path()).expect("written");
    let id = std::fs::read_to_string(dir.path().join("build-id"))
        .expect("the build names itself")
        .trim()
        .to_string();
    assert!(
        lines.iter().any(|l| l.trim() == format!("build      {id}")),
        "{lines:?}"
    );
    (id, dir)
}

#[test]
fn the_same_sources_build_the_same_name() {
    let (first, _a) = named(&feed(&|s| s.to_string()));
    let (second, _b) = named(&feed(&|s| s.to_string()));
    assert_eq!(first, second);
    assert!(
        first.len() == 17 && first.starts_with('b'),
        "b and sixteen hex digits: {first}"
    );
    assert!(first[1..].chars().all(|c| c.is_ascii_hexdigit()), "{first}");
}

#[test]
fn a_change_to_what_the_build_writes_changes_its_name() {
    let (plain, _a) = named(&feed(&|s| s.to_string()));
    // A template's text.
    let heading = "<h1>Home</h1>";
    let (template, _b) = named(&feed(&|s| {
        assert!(s.contains(heading), "the home page's heading");
        s.replacen(heading, "<h1>Your home</h1>", 1)
    }));
    assert_ne!(plain, template, "a template's text");
    // A handler's body: the composer's draft, cleared another way.
    let cleared = "draft = \"\"";
    let (handler, _c) = named(&feed(&|s| {
        assert!(s.contains(cleared), "a handler that clears the draft");
        s.replacen(cleared, "draft = \" \"", 1)
    }));
    assert_ne!(plain, handler, "a handler's body");
    assert_ne!(template, handler);
}
