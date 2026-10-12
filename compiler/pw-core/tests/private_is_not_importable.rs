//! **`private` says a declaration is not importable, and nothing more**
//! (ADR-XXXX; NEXT 3a, W8's finding of 2026-10-09).
//!
//! It said two things: the user's scope (the boundary, the resume manifest,
//! the checker's labels) and not importable (PW0023). `user` says the first
//! alone (ADR-0318). A module's own helper that said `private` to stay its
//! own was the user's data wherever it was read. Now `private` is not
//! importable, alone; a declaration that holds data says whose with `user`,
//! `session` or `public`, and `private` on one is refused by name (PW0029).
//! Each test states one case, with its control.

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

/// Each `code message` `files` report, outside the platform.
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

/// The repair each `code` diagnostic of `files` gives, outside the platform.
fn repairs(files: &[(&str, String)], code: &str) -> Vec<String> {
    let mut program = platform();
    program.extend(files.iter().map(|(n, s)| (n.to_string(), s.clone())));
    let mine: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| mine.contains(&n.as_str()))
        .flat_map(|(_, ds)| ds)
        .filter(|d| d.code == code)
        .flat_map(|d| d.repairs.into_iter().map(|r| r.description))
        .collect()
}

/// A store's public page, which shows its name through `helper`, declared
/// with `visibility` in the page's own module.
fn shop(visibility: &str) -> String {
    format!(
        "module shop\n\n\
         {visibility} fn said(name: String) -> String !{{}} {{\n    name\n}}\n\n\
         public query Name() -> String\n    freshness 60.seconds\n    cache     shared\n{{\n    \"Blue Bottle\"\n}}\n\n\
         page Home() {{\n    route \"/\"\n    cache shared\n\n    let name = query Name()\n\n    \
         view {{\n        <title>Stores</title>\n        <h1>{{said(name)}}</h1>\n    }}\n}}\n"
    )
}

/// **A module's own helper is not the user's data.** A public page shows
/// what its `private` function computes from public data, and nothing is
/// refused. Until ADR-XXXX the function was the user's (`label_of`), and so
/// was what it computed.
#[test]
fn a_private_function_is_its_modules_and_not_the_users() {
    let found = reported(&[("shop.pw", shop("private"))]);
    assert!(found.is_empty(), "{found:?}");
    // Control: the same with no visibility.
    let found = reported(&[("shop.pw", shop(""))]);
    assert!(found.is_empty(), "{found:?}");
}

/// **And another module does not import it** (PW0023, as before), and is
/// told to drop `private`, not to make it public: `public` says whose data
/// a declaration holds.
#[test]
fn a_private_declaration_is_not_imported() {
    let other = "module other\n\nimport shop.{ said }\n\nfn twice(n: String) -> String !{} {\n    said(n)\n}\n";
    let found = reported(&[
        ("shop.pw", shop("private")),
        ("other.pw", other.to_string()),
    ]);
    assert!(found.iter().any(|d| d.starts_with("PW0023")), "{found:?}");
    assert_eq!(
        repairs(
            &[
                ("shop.pw", shop("private")),
                ("other.pw", other.to_string())
            ],
            "PW0023"
        ),
        ["drop `private` from `said` in `shop`, or stop importing it"]
    );
    // Control: with no visibility, it is.
    let found = reported(&[("shop.pw", shop("")), ("other.pw", other.to_string())]);
    assert!(found.is_empty(), "{found:?}");
}

/// **`private` on a declaration that holds data is refused, by name**, so a
/// program written when `private` meant the user's is not read as everyone's
/// data: a query, a page, a layout, a command and a materialization.
#[test]
fn private_on_a_declaration_that_holds_data_is_refused() {
    for (decl, kind, name) in [
        (
            "private query Mine(reader: User<UserId>) -> Int\n    freshness 0.seconds\n    cache private\n{\n    0\n}\n",
            "query",
            "Mine",
        ),
        (
            "private page Mine() {\n    route \"/mine\"\n    cache private\n\n    view {\n        <title>Mine</title>\n    }\n}\n",
            "page",
            "Mine",
        ),
        (
            "private layout Shell {\n    view {\n        <slot />\n    }\n}\n",
            "layout",
            "Shell",
        ),
        (
            "private command forget() -> Int\n{\n    0\n}\n",
            "command",
            "forget",
        ),
    ] {
        let src = format!("module k\n\nimport capability.{{ User, UserId }}\n\n{decl}");
        let found = reported(&[("k.pw", src)]);
        let expected = format!(
            "PW0029 the {kind} `{name}` is `private`, which says it is not importable, and \
             nothing of whose data it holds"
        );
        assert!(found.contains(&expected), "{decl}: {found:?}");
    }
    let src = "module k\n\nprivate query Mine() -> Int\n    freshness 0.seconds\n{\n    0\n}\n";
    assert_eq!(
        repairs(&[("k.pw", src.to_string())], "PW0029"),
        [
            "write `user` for the user's, `session` for the session's, or `public` for everyone's; \
          each is importable"
        ]
    );
    // Control: `user`, `session` and `public` each say whose.
    for visibility in ["user", "session", "public"] {
        let src = format!(
            "module k\n\nimport capability.{{ User, UserId }}\n\n\
             {visibility} query Mine(reader: User<UserId>) -> Int\n    freshness 0.seconds\n    cache private\n{{\n    0\n}}\n"
        );
        let found = reported(&[("k.pw", src)]);
        assert!(
            !found.iter().any(|d| d.starts_with("PW0029")),
            "{visibility}: {found:?}"
        );
    }
}
