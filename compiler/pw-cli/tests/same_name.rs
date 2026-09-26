//! **`pw check` reports each file, whatever it is called** (ADR-0106).
//!
//! Two files may share a name: `store/app.pw` and `kiokun/app.pw`. Until
//! 2026-09-26 `pw check` kept each file's diagnostics in a map keyed by its
//! name alone, so the last file's replaced the first's: `pw check a/app.pw
//! b/app.pw` printed "no diagnostics" and exited 0 with a type error in
//! `a/app.pw`. The binary is run as a user runs it.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Two directories, each with an `app.pw`: `a`'s has an error, `b`'s none.
fn two_apps(tag: &str, first_bad: bool) -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("pw-same-name-{tag}-{}", std::process::id()));
    let (a, b) = (root.join("a"), root.join("b"));
    std::fs::create_dir_all(&a).expect("a");
    std::fs::create_dir_all(&b).expect("b");
    let bad = "module a.app\n\nfn f() -> Int !{} { \"not an int\" }\n";
    let good = "module b.app\n\nfn g() -> Int !{} { 1 }\n";
    std::fs::write(a.join("app.pw"), if first_bad { bad } else { good }).expect("a/app.pw");
    std::fs::write(b.join("app.pw"), if first_bad { good } else { bad }).expect("b/app.pw");
    (root, a.join("app.pw"), b.join("app.pw"))
}

fn pw_check(files: &[&Path]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_pw"))
        .arg("check")
        .arg("--plain")
        .args(files)
        .output()
        .expect("pw runs");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    (out.status.code().unwrap_or(-1), text)
}

#[test]
fn a_file_is_reported_though_another_has_its_name() {
    for first_bad in [true, false] {
        let (root, a, b) = two_apps(if first_bad { "first" } else { "second" }, first_bad);
        let (code, text) = pw_check(&[&a, &b]);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(code, 1, "the error is reported:\n{text}");
        assert!(text.contains("1 error(s)"), "{text}");
        assert_eq!(
            text.matches("[PW0606]").count(),
            1,
            "once, for its own file:\n{text}"
        );
        // Named by its path, since its name is not its own.
        let bad = if first_bad { &a } else { &b };
        assert!(
            text.contains(&format!("--> {}:", bad.display())),
            "the file is told apart by its path:\n{text}"
        );
    }
    // The control: a file with a name of its own is named by it, as before.
    let (root, a, _) = two_apps("alone", true);
    let (code, text) = pw_check(&[&a]);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("--> app.pw:"), "{text}");
}

#[test]
fn each_file_keeps_its_own_answers_under_one_name() {
    // One file given twice is two units: the second also declares the
    // module twice. Each keeps its own diagnostics, by its place, so the
    // first reports one error and the second two. By name, both had the
    // second's.
    let (root, a, _) = two_apps("twice", true);
    let (code, text) = pw_check(&[&a, &a]);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(code, 1, "{text}");
    assert_eq!(text.matches("[PW0606]").count(), 2, "{text}");
    assert_eq!(text.matches("[PW0024]").count(), 1, "{text}");
    assert!(text.contains("3 error(s)"), "{text}");
}
