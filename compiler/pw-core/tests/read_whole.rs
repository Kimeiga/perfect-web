//! **What lowering parses, it reports** (ADR-0237).
//!
//! The declaration grammar keeps some text as text: a clause's value, a
//! string's hole, the expression a block marker carries. Lowering parses each
//! standalone, and dropped every error those parses made, so what one could
//! not read was simply not there:
//! - `invalidates_on Liked(id) Posted(_)` listened for no post;
//! - `"sum {a b}"` rendered `a`;
//! - `{#if flag other}` was decided by `flag`.
//!
//! Each is refused now, at its place, under a code the registry has, in the
//! syntax range. Each test states one place, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::diagnostics::UNREGISTERED;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

/// The platform's packages, and `files` after them.
fn program(files: &[(&str, String)]) -> Vec<(String, String)> {
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
    out.extend(files.iter().map(|(n, s)| (n.to_string(), s.clone())));
    out
}

fn feed() -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("the feed")
}

/// The feed, with `old` written `new`.
fn feed_with(old: &str, new: &str) -> String {
    let src = feed();
    assert_eq!(src.matches(old).count(), 1, "{old}");
    src.replace(old, new)
}

/// What the checker reports of `app`: each diagnostic's code, its message and
/// its place. Every code is one the registry has.
fn reported(app: &str) -> Vec<(String, String, std::ops::Range<usize>)> {
    check_sources(&program(&[("app.pw", app.to_string())]))
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds)
        .map(|d| {
            assert_ne!(d.symbol(), UNREGISTERED, "{} is not registered", d.code);
            (
                d.code.to_string(),
                d.message.clone(),
                d.primary_span.clone(),
            )
        })
        .collect()
}

/// What the checker reports of `app`, each with the text it underlines.
fn found(app: &str) -> Vec<(String, String, String)> {
    reported(app)
        .into_iter()
        .map(|(code, message, at)| (code, message, app[at].to_string()))
        .collect()
}

fn one(code: &str, message: &str, at: &str) -> Vec<(String, String, String)> {
    vec![(code.to_string(), message.to_string(), at.to_string())]
}

/// What refused the build of `app`.
fn refused(app: String) -> String {
    let files = program(&[("app.pw", app)]);
    let units: Vec<Unit> = files
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            hir: lower_file(src, &parse_tree(src).green),
            src: src.clone(),
        })
        .collect();
    match pw_core::build::build(&units) {
        Err(e) => e,
        Ok(b) => b.refusals().join("\n"),
    }
}

/// A view in a file of its own, `markup` its body.
fn view(params: &str, markup: &str) -> String {
    format!("module m\n\nview V({params}) {{\n    <div>\n        {markup}\n    </div>\n}}\n")
}

#[test]
fn a_clauses_values_are_separated_by_commas() {
    // The control: the feed as written reports nothing.
    assert_eq!(found(&feed()), vec![]);

    // A comma missing: reported over the value after it.
    let app = feed_with(
        "invalidates_on Liked(id), Posted(_)",
        "invalidates_on Liked(id) Posted(_)",
    );
    let f = found(&app);
    assert_eq!(
        f.first(),
        Some(&(
            "PW0016".to_string(),
            "a clause's values are separated by commas".to_string(),
            "Posted(_)".to_string(),
        )),
        "{f:?}"
    );
    // And read: the clause's keys are what was meant, as rustc reads an
    // omitted separator. They were `Liked(id)` alone.
    let hir = lower_file(&app, &parse_tree(&app).green);
    let (_, thread) = hir
        .all_decls()
        .find(|(_, d)| d.name == "Thread")
        .expect("the thread's query");
    let keys: Vec<&str> = thread
        .policy("invalidates_on")
        .expect("its listeners")
        .keys
        .iter()
        .map(|k| k.name.as_str())
        .collect();
    assert_eq!(keys, ["Liked", "Posted"]);
    // Its help is a repair, rendered as help, as a file's parse errors are.
    let repairs: Vec<String> = check_sources(&program(&[("app.pw", app.clone())]))
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds)
        .flat_map(|d| d.repairs.into_iter().map(|r| r.description))
        .collect();
    assert!(
        repairs.contains(&"write a comma before this value".to_string()),
        "{repairs:?}"
    );
    // And the build is refused for it, as the checker refuses it.
    let why = refused(app);
    assert!(
        why.contains("a clause's values are separated by commas"),
        "{why}"
    );

    // What is not a value where a comma is missing is no value: the comma
    // is the error, as rustc reports it, and nothing else.
    let app = feed_with(
        "invalidates_on Liked(id), Posted(_)",
        "invalidates_on Liked(id), Posted(_) ;",
    );
    assert_eq!(
        found(&app),
        one("PW0016", "a clause's values are separated by commas", ";")
    );
}

#[test]
fn an_interfaces_clause_is_read_whole() {
    // A declaration with no body is an interface, and lowering never parsed
    // its clauses: their terms have no arena. What they do not read is
    // reported all the same.
    let src = |clause: &str| {
        format!(
            "module m\n\nevent Changed(id: String)\nevent Other(id: String)\n\n\
             query Read(id: String) -> Int\n    invalidates_on {clause}\n"
        )
    };
    assert_eq!(found(&src("Changed(id), Other(id)")), vec![]);
    assert_eq!(
        found(&src("Changed(id) Other(id)")),
        one(
            "PW0016",
            "a clause's values are separated by commas",
            "Other(id)"
        )
    );
}

#[test]
fn an_optimistic_clause_is_read_whole() {
    // Text after the transition.
    let app = feed_with(
        "as feed => pending(feed, text)\n",
        "as feed => pending(feed, text) text\n",
    );
    assert_eq!(
        found(&app),
        one("PW0016", "an optimistic clause is one transition", "text")
    );

    // No name for the entry's value: the clause's own code, at its place.
    let app = feed_with(
        "Timeline(current_session(), _) as feed => pending(feed, text)",
        "Timeline(current_session(), _) => pending(feed, text)",
    );
    let f = found(&app);
    assert_eq!(
        f.first(),
        Some(&(
            "PW0017".to_string(),
            "expected `as <name>` — an optimistic clause binds the resource's current value"
                .to_string(),
            "=>".to_string(),
        )),
        "{f:?}"
    );

    // An arrow and no transition after it.
    let app = feed_with("as feed => pending(feed, text)\n", "as feed =>\n");
    let at = app.find("as feed =>").expect("the clause") + "as feed =>".len();
    let f = reported(&app);
    assert_eq!(
        f.first(),
        Some(&(
            "PW0009".to_string(),
            "expected an expression, found the end of the clause".to_string(),
            at..at,
        )),
        "{f:?}"
    );

    // No arrow.
    let app = feed_with(
        "Timeline(current_session(), _) as feed => pending(feed, text)",
        "Timeline(current_session(), _) as feed pending(feed, text)",
    );
    let f = found(&app);
    assert_eq!(
        f.first(),
        Some(&(
            "PW0017".to_string(),
            "expected `=>` and a transition expression".to_string(),
            "pending".to_string(),
        )),
        "{f:?}"
    );
}

#[test]
fn a_hole_is_one_expression() {
    // In a string.
    let app = feed_with("\"1 {one}\"", "\"1 {one many}\"");
    assert_eq!(
        found(&app),
        one("PW0016", "a hole is one expression", "many")
    );

    // In an attribute's text.
    let app = feed_with("href=\"/post/{p.id}\"", "href=\"/post/{p.id p}\"");
    assert_eq!(found(&app), one("PW0016", "a hole is one expression", "p"));
}

#[test]
fn a_block_markers_expression_is_one_expression() {
    // A block's subject.
    let app = feed_with(
        "{#if String.length(draft) > 280}",
        "{#if String.length(draft) > 280 draft}",
    );
    assert_eq!(
        found(&app),
        one("PW0016", "a block's subject is one expression", "draft")
    );

    // A `{#match}`'s, beside a control.
    let src = |subject: &str| {
        view(
            "a: Option<Int>, b: Option<Int>",
            &format!("{{#match {subject}}}{{:Some(n)}}<p>{{n}}</p>{{:None}}<p>none</p>{{/match}}"),
        )
    };
    assert_eq!(found(&src("a")), vec![]);
    assert_eq!(
        found(&src("a b")),
        one("PW0016", "a block's subject is one expression", "b")
    );

    // A branch's condition, beside a control.
    let src = |condition: &str| {
        view(
            "a: Bool, b: Bool",
            &format!("{{#if a}}<p>a</p>{{:else if {condition}}}<p>b</p>{{/if}}"),
        )
    };
    assert_eq!(found(&src("b")), vec![]);
    assert_eq!(
        found(&src("b a")),
        one("PW0016", "a branch's condition is one expression", "a")
    );
}

#[test]
fn a_standalone_parses_error_is_reported_once_and_names_its_end() {
    // The subject's expression ends at the marker's `}`, which is not the
    // end of the file; and the error node the parse leaves is that error,
    // not text the compiler does not read (PW0015) as well.
    let app = feed_with(
        "{#if List.length(thread.replies) == 0}",
        "{#if List.length(thread.replies) ==}",
    );
    let at = app.find("==}").expect("the marker") + 2;
    let f = reported(&app);
    assert_eq!(
        f,
        vec![(
            "PW0009".to_string(),
            "expected an expression, found the end of a block's subject".to_string(),
            at..at,
        )],
        "{f:?}"
    );
}
