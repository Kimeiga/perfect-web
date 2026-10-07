//! **A block's statements are separated, by `;` or a line** (ADR-0243).
//!
//! `fn f(a: Int, b: Int) -> Int !{} { a b }` was two statements, `a`
//! evaluated and dropped, and checked: its value was `b`. The grammar refuses
//! it now, and `pw check` reports what the grammar refuses first (ADR-0237).
//! What a block's readers read as one is not two: a `return` and its value,
//! a clause's head and the rest of its line, a block after what precedes it.
//! Each test states one case, with a control.

use pw_core::check::check_sources;

/// The platform's packages, the files of `with`, and `src` as `app.pw` after
/// them.
fn program(src: &str, with: &[&str]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"]
        .iter()
        .chain(with)
    {
        let at = root.join(d);
        let mut paths: Vec<std::path::PathBuf> = if at.is_dir() {
            std::fs::read_dir(&at)
                .unwrap_or_else(|e| panic!("{d}: {e}"))
                .map(|e| e.expect("entry").path())
                .filter(|p| p.extension().is_some_and(|x| x == "pw"))
                .collect()
        } else {
            vec![at]
        };
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push(("app.pw".to_string(), src.to_string()));
    out
}

/// What the checker reports of `app.pw`, checked with the files of `with`:
/// each code, the text it underlines, and the repairs it offers.
fn found_with(src: &str, with: &[&str]) -> Vec<(String, String, Vec<String>)> {
    check_sources(&program(src, with))
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds)
        .map(|d| {
            (
                d.code.to_string(),
                src[d.primary_span.clone()].to_string(),
                d.repairs.into_iter().map(|r| r.description).collect(),
            )
        })
        .collect()
}

/// What the checker reports of `app.pw`.
fn found(src: &str) -> Vec<(String, String, Vec<String>)> {
    found_with(src, &[])
}

/// Each code and the text it underlines.
fn codes(found: &[(String, String, Vec<String>)]) -> Vec<(&str, &str)> {
    found.iter().map(|f| (f.0.as_str(), f.1.as_str())).collect()
}

/// A function whose body is `body`.
fn function(body: &str) -> String {
    format!(
        "module m\n\nfn f(a: Int, b: Int) -> Int !{{}} {{\n    if a > b {{\n        \
         return a\n    }}\n    {body}\n}}\n"
    )
}

#[test]
fn two_statements_on_one_line_are_refused_at_the_second() {
    assert_eq!(
        found(&function("a b")),
        vec![(
            "PW0030".to_string(),
            "b".to_string(),
            vec!["write `;` between them, or the second on a line of its own".to_string()]
        )]
    );
    // The controls: separated by `;`, and by a line.
    assert_eq!(found(&function("a; b")), vec![]);
    assert_eq!(found(&function("a\n    b")), vec![]);
}

#[test]
fn a_return_takes_the_one_statement_after_it() {
    // The control, `function`'s own `return a`, checks; a second statement
    // after the value is refused.
    let src = function("b").replace("return a", "return a b");
    assert_eq!(codes(&found(&src)), vec![("PW0030", "b")]);
}

/// A-019 as the corpus writes it, with what it imports, and `edit` made to
/// it: a resource's clauses in its block, a head and its value, and code
/// heads with their blocks.
fn a019(edit: impl Fn(&str) -> String) -> Vec<(String, String, Vec<String>)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = std::fs::read_to_string(
        root.join("examples/accepted/A-019-intersection-observation-controls-resource-lifetime.pw"),
    )
    .expect("A-019");
    found_with(&edit(&src), &["examples/domain.pw", "examples/lib"])
}

#[test]
fn a_clause_in_a_block_is_one_clause() {
    // The control: A-019 checks.
    assert_eq!(a019(|s| s.to_string()), vec![]);
    // A `;` ends a clause's value, as it ends a statement: what follows it is
    // a statement, and its names resolve. Until ADR-0243 the names check
    // read `nothing_here` as `scope`'s value, and nothing resolved it.
    let semi = a019(|s| s.replace("scope component", "scope component; nothing_here"));
    assert_eq!(codes(&semi), vec![("PW0021", "nothing_here")]);
    // A `,` does not end it: a list is one value.
    let list = a019(|s| {
        s.replace(
            "scope component",
            "scope component\n        placement browser, edge",
        )
    });
    assert_eq!(list, vec![]);
    // After a block, the line takes nothing.
    let after = a019(|s| {
        s.replace(
            "acquire { Maps.create(self, center) }",
            "acquire { Maps.create(self, center) } center",
        )
    });
    assert_eq!(codes(&after), vec![("PW0030", "center")]);
}

#[test]
fn a_clauses_word_where_no_clause_is_is_a_statement() {
    // The grammar takes what follows a clause's head on its line for the
    // clause's value; where the word names a binding, it is no clause, and
    // the two are two statements, which the names check refuses.
    let f = |body: &str| {
        format!("module m\n\nfn f(key: Int) -> Int !{{}} {{\n    {body}\n    key\n}}\n")
    };
    let refused = found(&f("key 1"));
    assert_eq!(codes(&refused), vec![("PW0030", "1")]);
    assert_eq!(
        refused[0].2,
        vec!["write `;` between them, or the second on a line of its own".to_string()]
    );
    // The controls: separated by `;`, by `,` and by a line, and a block
    // after it, which follows any statement.
    for body in ["key; 1", "key, 1", "key\n    1", "key { 1 }"] {
        assert_eq!(found(&f(body)), vec![], "{body}");
    }
    // In a function's body, which admits no clause (ADR-0216), a clause's
    // words do not resolve, and they are two statements.
    let code = found(&function("scope application\n    a"));
    assert_eq!(
        codes(&code),
        vec![
            ("PW0021", "scope"),
            ("PW0021", "application"),
            ("PW0030", "application")
        ]
    );
}
