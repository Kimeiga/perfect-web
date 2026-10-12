//! **A related note is shown in its own file** (ADR-XXXX).
//!
//! A diagnostic names where the conflicting property originated, not only
//! where its rule fired (charter §16.3). Until 2026-10-12 every such note's
//! span was read in the diagnostic's own file, whatever file it was in:
//! - PW5046's note on a layout in another module labelled the page's own
//!   text, and past the page's end stopped `pw check` with an internal error
//!   (W8's finding);
//! - PW0338's note on each press that sends a command did the same for a
//!   page in another file than the command's;
//! - PW5106 left out a reader in another file, as it could not say where it
//!   was;
//! - PW0351 and PW0341 named the second declaration twice, and the first,
//!   which may be in another file, nowhere.
//!
//! A note now names its file where it is not the diagnostic's own. Each test
//! states one case, with a control in one file.

use pw_core::check::check_sources;
use pw_core::diagnostics::{Diagnostic, Related};

/// The platform's packages, the store's library, and `files` after them.
fn program(files: &[(&str, &str)]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
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
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out.extend(files.iter().map(|(n, s)| (n.to_string(), s.to_string())));
    out
}

/// The diagnostics with `code` that `file` reports, checking `files`.
fn found(files: &[(&str, &str)], file: &str, code: &str) -> Vec<Diagnostic> {
    check_sources(&program(files))
        .into_iter()
        .filter(|(n, _)| n == file)
        .flat_map(|(_, ds)| ds)
        .filter(|d| d.code == code)
        .collect()
}

/// The text a note marks, in the file it names, or in `own` where it names
/// none.
fn marked<'a>(files: &[(&'a str, &'a str)], own: &str, note: &Related) -> &'a str {
    let file = note.file.as_deref().unwrap_or(own);
    let (_, src) = files
        .iter()
        .find(|(n, _)| *n == file)
        .unwrap_or_else(|| panic!("the note names `{file}`, which was not checked"));
    src.get(note.span.clone())
        .unwrap_or_else(|| panic!("{:?} is no text of `{file}`", note.span))
}

/// Twenty lines of comment, so a layout's span lies past the end of a page's
/// file: the case that stopped `pw check`.
fn padding() -> String {
    "// padding padding padding padding padding padding padding padding\n".repeat(20)
}

/// **PW5046's note is in the layout's file** (W8's finding): a page in one
/// module shown in a layout declared in another.
#[test]
fn a_layouts_note_is_in_the_layouts_file() {
    let shell = format!(
        "module shell\n\n{}\nsession layout Shell {{\n    view {{\n        \
         <header><p>Shell</p></header>\n        <slot />\n    }}\n}}\n",
        padding()
    );
    let page = "module page\n\nimport shell.{ Shell }\n\npage P() {\n    route  \"/p\"\n    \
                layout Shell\n\n    view {\n        <title>P</title>\n        <h1>P</h1>\n    }\n}\n";
    let files = [("shell.pw", shell.as_str()), ("page.pw", page)];
    let d = found(&files, "page.pw", "PW5046");
    assert_eq!(d.len(), 1, "{d:#?}");
    let note = &d[0].related[0];
    assert_eq!(note.file.as_deref(), Some("shell.pw"), "{note:?}");
    assert_eq!(marked(&files, "page.pw", note), "Shell");
    // The control: both in one file, the note names none.
    let one = "module page\n\nsession layout Shell {\n    view {\n        <header><p>Shell</p></header>\n        \
               <slot />\n    }\n}\n\npage P() {\n    route  \"/p\"\n    layout Shell\n\n    \
               view {\n        <title>P</title>\n        <h1>P</h1>\n    }\n}\n";
    let files = [("page.pw", one)];
    let d = found(&files, "page.pw", "PW5046");
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].related[0].file, None);
    assert_eq!(marked(&files, "page.pw", &d[0].related[0]), "Shell");
}

/// **PW0338's notes are in the pages' files**: a command sent without
/// idempotency, by a page in another file.
#[test]
fn a_commands_note_is_in_the_file_of_the_page_that_sends_it() {
    let command = format!(
        "module a\n\n{}\ncommand ping() -> ()\n{{\n    ()\n}}\n",
        padding()
    );
    let page = "module b\n\nimport a.{ ping }\n\npage P() {\n    cache private\n\n    view {\n        \
                <main><button type=\"button\" on:press|refusable={ping}>Go</button></main>\n    }\n}\n";
    let files = [("a.pw", command.as_str()), ("b.pw", page)];
    let d = found(&files, "a.pw", "PW0338");
    assert_eq!(d.len(), 1, "{d:#?}");
    let note = &d[0].related[0];
    assert_eq!(note.file.as_deref(), Some("b.pw"), "{note:?}");
    assert_eq!(marked(&files, "a.pw", note), "ping");
}

/// Module `name`, holding `parts`, over the store's cart.
fn cart_module(name: &str, parts: &[String]) -> String {
    format!(
        "module {name}\n\nimport Carts\nimport context.{{ current_session }}\n\
         import domain.{{ Cart, CartError, InteractionId, MenuItemId, PositiveInt }}\n\
         import capability.{{ Session, SessionId }}\n\n{}",
        parts.join("\n")
    )
}

fn basket() -> String {
    "session query Basket(session: Session<SessionId>) -> Result<Cart, CartError>\n    \
     freshness      0.seconds\n    consistency    read_your_writes\n    \
     cache          private\n{\n    Carts.current(session)\n}\n"
        .to_string()
}

fn add() -> String {
    "command add(item: MenuItemId, quantity: PositiveInt) -> Result<Cart, CartError>\n    \
     idempotent_by InteractionId\n{\n    Carts.add(current_session(), item, quantity)\n}\n"
        .to_string()
}

/// **PW5106 names a reader in another file, in it**: until 2026-10-12 it
/// had no note at all.
#[test]
fn a_reader_in_another_file_is_named_in_it() {
    let q = cart_module("q", &[basket()]);
    let t = cart_module("t", &[add()]);
    let files = [("q.pw", q.as_str()), ("t.pw", t.as_str())];
    let d = found(&files, "t.pw", "PW5106");
    assert_eq!(d.len(), 1, "{d:#?}");
    let notes: Vec<(Option<&str>, &str)> = d[0]
        .related
        .iter()
        .map(|r| (r.file.as_deref(), marked(&files, "t.pw", r)))
        .collect();
    // The command, in its own file; the reader, in its.
    assert!(
        notes
            .iter()
            .any(|(f, text)| f.is_none() && text.starts_with("command add")),
        "{notes:#?}"
    );
    assert!(
        notes
            .iter()
            .any(|(f, text)| *f == Some("q.pw") && text.starts_with("session query Basket")),
        "{notes:#?}"
    );
    // The control: both in one file, both notes in it.
    let one = cart_module("t", &[basket(), add()]);
    let files = [("t.pw", one.as_str())];
    let d = found(&files, "t.pw", "PW5106");
    assert_eq!(d.len(), 1, "{d:#?}");
    assert!(d[0].related.iter().all(|r| r.file.is_none()), "{d:#?}");
    assert_eq!(d[0].related.len(), 2, "{d:#?}");
}

/// **PW0351 names the first declaration, where it is**.
#[test]
fn a_predicate_declared_twice_names_the_first_where_it_is() {
    let a = format!(
        "module a\n\n{}\npredicate Allowed\n    says \"Only some may.\"\n",
        padding()
    );
    let b = "module b\n\npredicate Allowed\n    says \"Only some may.\"\n";
    let files = [("a.pw", a.as_str()), ("b.pw", b)];
    let d = found(&files, "b.pw", "PW0351");
    assert_eq!(d.len(), 1, "{d:#?}");
    let note = &d[0].related[0];
    assert_eq!(note.file.as_deref(), Some("a.pw"), "{note:?}");
    assert!(
        marked(&files, "b.pw", note).starts_with("predicate Allowed"),
        "{note:?}"
    );
}

/// **PW0341 names the page first served at a route, where it is**.
#[test]
fn a_route_declared_twice_names_the_first_page_where_it_is() {
    let a = format!(
        "module a\n\n{}\npage First() {{\n    route \"/same\"\n\n    view {{\n        \
         <title>First</title>\n        <h1>First</h1>\n    }}\n}}\n",
        padding()
    );
    let b = "module b\n\npage Second() {\n    route \"/same\"\n\n    view {\n        \
             <title>Second</title>\n        <h1>Second</h1>\n    }\n}\n";
    let files = [("a.pw", a.as_str()), ("b.pw", b)];
    let d = found(&files, "b.pw", "PW0341");
    assert_eq!(d.len(), 1, "{d:#?}");
    let note = &d[0].related[0];
    assert_eq!(note.file.as_deref(), Some("a.pw"), "{note:?}");
    assert!(
        marked(&files, "b.pw", note).starts_with("page First"),
        "{note:?}"
    );
}

/// **Every note's span is text of the file it names**: over the rejected
/// corpus, each of whose programs is refused, and the reference apps, each
/// checked whole. A note read in the wrong file, as PW5046's was, falls out
/// of its text, or off a character's edge.
#[test]
fn every_note_is_text_of_its_own_file() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pw_files = |dir: &std::path::Path| -> Vec<(String, String)> {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        paths
            .into_iter()
            .map(|p| {
                let src = std::fs::read_to_string(&p).expect("read");
                (p.display().to_string(), src)
            })
            .collect()
    };
    let mut programs: Vec<Vec<(String, String)>> = pw_files(&root.join("examples/rejected"))
        .into_iter()
        .map(|file| {
            let mut p = program(&[]);
            p.push(file);
            p
        })
        .collect();
    for app in ["store", "feed"] {
        let mut p = program(&[]);
        p.extend(pw_files(&root.join("examples").join(app)));
        programs.push(p);
    }
    let mut notes = 0;
    for files in &programs {
        for (file, found) in check_sources(files) {
            for d in &found {
                for r in &d.related {
                    let named = r.file.as_deref().unwrap_or(&file);
                    let (_, src) = files
                        .iter()
                        .find(|(n, _)| n == named)
                        .unwrap_or_else(|| panic!("{} names `{named}`", d.code));
                    assert!(
                        src.get(r.span.clone()).is_some(),
                        "{} in {file}: the note {:?} is no text of {named}",
                        d.code,
                        r
                    );
                    notes += 1;
                }
            }
        }
    }
    // 80 on 2026-10-12: enough that the check reads them, not a count to keep.
    assert!(notes >= 50, "the corpus's notes were read: {notes}");
}
