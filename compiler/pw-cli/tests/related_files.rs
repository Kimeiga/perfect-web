//! **`pw check` shows a related note in its own file** (ADR-XXXX).
//!
//! W8's finding, 2026-10-12: PW5046's note on a layout declared in another
//! file was rendered against the page's file. Past the page's end it stopped
//! `pw check` with an internal error ("Annotation range is beyond the end of
//! buffer"); within it, it labelled the page's own text. The binary is run as
//! a user runs it.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A layout in `shell.pw`, its declaration past the end of `page.pw`, and a
/// page in `page.pw` shown in it, declared for anyone's eyes.
fn two_files(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("pw-related-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("root");
    let padding = "// padding padding padding padding padding padding padding padding\n".repeat(20);
    let shell = format!(
        "module shell\n\n{padding}\nsession layout Shell {{\n    view {{\n        \
         <header><p>Shell</p></header>\n        <slot />\n    }}\n}}\n"
    );
    let page = "module page\n\nimport shell.{ Shell }\n\npage P() {\n    route  \"/p\"\n    \
                layout Shell\n\n    view {\n        <title>P</title>\n        <h1>P</h1>\n    }\n}\n";
    std::fs::write(root.join("shell.pw"), shell).expect("shell.pw");
    std::fs::write(root.join("page.pw"), page).expect("page.pw");
    (root.clone(), root.join("shell.pw"), root.join("page.pw"))
}

fn pw_check(files: &[&Path]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_pw"))
        .arg("check")
        .arg("--plain")
        .args(files)
        .output()
        .expect("pw runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn a_layouts_note_is_shown_in_the_layouts_file() {
    let (root, shell, page) = two_files("layout");
    let (code, text, err) = pw_check(&[&shell, &page]);
    let _ = std::fs::remove_dir_all(&root);
    assert!(!err.contains("panicked"), "no internal error:\n{err}");
    assert_eq!(code, 1, "the page's error is reported:\n{text}\n{err}");
    assert!(text.contains("[PW5046]"), "{text}");
    // The page's line in the page's file, and the layout's in its own.
    assert!(text.contains("page.pw:7:5"), "{text}");
    assert!(text.contains("shell.pw:24:16"), "{text}");
    assert!(
        text.contains("`Shell` is declared `session` here"),
        "{text}"
    );
}
