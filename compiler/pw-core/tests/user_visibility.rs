//! **`user`, the user's visibility, importable as `session` is** (track
//! `store-accounts`, ADR-XXXX; the integrator's ruling of 2026-10-09).
//!
//! `private` is two things: the user's scope, and not importable (PW0023).
//! A user's cart read on another module's page needs the first without the
//! second. `user` is `session`'s peer: a declaration it opens is the user's
//! in every rule that reads `private` as the user's, and is imported as a
//! `session` one is. It is a visibility only at a declaration's start,
//! before the declaration's keyword, so a field, a parameter or a binding
//! named `user` is still a name. `private` is unchanged.

use pw_core::check::check_sources;

fn platform() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            let src = std::fs::read_to_string(&p).expect("read");
            out.push((p.display().to_string(), src));
        }
    }
    out
}

/// A module of saved items: `Saved`, a record only `SavedItems`, declared
/// with `visibility`, produces.
fn saved(visibility: &str) -> String {
    format!(
        "module saved\n\nimport capability.{{ User, UserId }}\n\n\
         type Saved = Saved {{ count: Int }}\n\n\
         {visibility} query SavedItems(reader: User<UserId>) -> Saved\n    \
         freshness 0.seconds\n    cache     private\n    key       reader\n{{\n    todo\n}}\n"
    )
}

/// A page of another module that reads the reader's saved items, opened with
/// `page_visibility`, and captures them into its handler; `cache private`
/// unless it is the public shell.
fn page(page_visibility: &str) -> String {
    let cache = if page_visibility.is_empty() {
        ""
    } else {
        "    cache private\n"
    };
    format!(
        "module shop\n\nimport saved.{{ SavedItems }}\nimport context.{{ current_user }}\n\n\
         {page_visibility} page Saved() {{\n    route \"/saved\"\n{cache}\n    \
         let items = query SavedItems(current_user())\n    signal kept: Int = 0\n\n    \
         view {{\n        <title>Saved</title>\n        <button type=\"button\" on:press={{resumable(captures = {{ items }}) => kept = items.count}}>Keep</button>\n    }}\n}}\n"
    )
}

/// The codes `files` report, each `code message`, outside the platform.
fn reported(files: &[(&str, String)]) -> Vec<String> {
    let mut program = platform();
    program.extend(files.iter().map(|(n, s)| (n.to_string(), s.clone())));
    let mine: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| mine.contains(&n.as_str()))
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn codes(files: &[(&str, String)]) -> Vec<String> {
    reported(files)
        .into_iter()
        .map(|d| d.split(' ').next().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn a_user_query_is_imported_and_read_on_a_user_page() {
    let found = reported(&[("saved.pw", saved("user")), ("shop.pw", page("user"))]);
    assert!(found.is_empty(), "{found:?}");
    // The control: `private` is the user's and is not imported.
    let found = codes(&[("saved.pw", saved("private")), ("shop.pw", page("user"))]);
    assert!(found.contains(&"PW0023".to_string()), "{found:?}");
}

#[test]
fn a_user_querys_record_is_the_users_and_no_other_manifest_holds_it() {
    // Into the public shell: refused, as R-063's `private` one is.
    let found = codes(&[("saved.pw", saved("user")), ("shop.pw", page(""))]);
    assert!(
        found.contains(&"PW5007".to_string()),
        "a public page: {found:?}"
    );
    // Into a session's: refused, the user's is not the session's.
    let found = codes(&[("saved.pw", saved("user")), ("shop.pw", page("session"))]);
    assert!(
        found.contains(&"PW5007".to_string()),
        "a session page: {found:?}"
    );
    // And a `user` query's record is the user's in a shared cache too, as a
    // `private` one's is (PW5001's rule, read through the same label).
    let shared = "module saved\n\nimport capability.{ User, UserId }\n\n\
         type Saved = Saved { count: Int }\n\n\
         user query SavedItems(reader: User<UserId>) -> Saved\n    \
         freshness 0.seconds\n    cache     shared\n    key       reader\n{\n    todo\n}\n";
    let found = codes(&[("saved.pw", shared.to_string())]);
    assert!(
        !found.is_empty(),
        "a user's record in a shared cache: {found:?}"
    );
    let private = shared.replace("user query", "private query");
    assert_eq!(codes(&[("saved.pw", private)]), found, "as `private` is");
}

#[test]
fn a_name_user_is_still_a_name() {
    // A record's field, a parameter, a binding and an event's parameter named
    // `user`: `user` is a visibility only before a declaration's keyword.
    let src = "module names\n\nimport capability.{ UserId }\n\n\
         type Who = Who { user: UserId, name: String }\n\n\
         event Seen(user: UserId)\n\n\
         fn named(user: UserId) -> Who !{} {\n    let who = Who { user: user, name: \"x\" }\n    who\n}\n\n\
         fn of(who: Who) -> UserId !{} {\n    let user = who.user\n    user\n}\n";
    let found = reported(&[("names.pw", src.to_string())]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn user_opens_a_declaration_of_each_kind_and_lowers_as_its_visibility() {
    for decl in [
        "user query Q(reader: User<UserId>) -> Int\n    freshness 0.seconds\n    cache private\n{\n    0\n}\n",
        "user type T = T { n: Int }\n",
        "user fn f() -> Int !{} {\n    1\n}\n",
    ] {
        let src = format!("module k\n\nimport capability.{{ User, UserId }}\n\n{decl}");
        let tree = pw_syntax::parse_tree(&src);
        let hir = pw_core::lower::lower_file(&src, &tree.green);
        let visibilities: Vec<Option<&str>> = hir
            .all_decls()
            .filter(|(_, d)| matches!(d.name.as_str(), "Q" | "T" | "f"))
            .map(|(_, d)| d.visibility.as_deref())
            .collect();
        assert_eq!(visibilities, [Some("user")], "{decl}");
    }
}
