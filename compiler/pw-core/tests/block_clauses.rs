//! **A clause written in a block is judged by its domain** (ADR-0247).
//!
//! The names check reads a clause a block writes as one, `scope component`
//! in a resource's block (ADR-0047), and nothing judged what it held: `scope
//! bogus` checked, as did `respects bogus` in an `animate` block,
//! `intrinsic_height bogus` in a `subtree`'s, and `captures bogus` in a
//! `handler_policy`. The table that judges a policy heading a declaration
//! (PW0335, ADR-0089) judges each now, and a length, which nothing judged
//! anywhere, is a CSS length. Each test edits a program of the accepted
//! corpus, and the program as the corpus writes it is the control.

use pw_core::check::check_sources;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `.pw` files of `dir`, by name.
fn files(dir: &str) -> Vec<(String, String)> {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root().join(dir))
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
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
}

/// What the checker reports of the accepted program whose name begins with
/// `id`, edited by `edit`, checked with the platform, the domain, the
/// library and the rest of the accepted corpus: each code, its message and
/// the text it underlines.
fn found(id: &str, edit: impl Fn(&str) -> String) -> Vec<(String, String, String)> {
    let mut program = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
        program.extend(files(d));
    }
    program.push((
        "examples/domain.pw".to_string(),
        std::fs::read_to_string(root().join("examples/domain.pw")).expect("domain"),
    ));
    let mut target = None;
    for (name, src) in files("examples/accepted") {
        let file = std::path::Path::new(&name)
            .file_name()
            .expect("name")
            .to_string_lossy()
            .to_string();
        if file.starts_with(id) {
            target = Some((name, edit(&src)));
        } else {
            program.push((name, src));
        }
    }
    let (name, src) = target.unwrap_or_else(|| panic!("no accepted program {id}"));
    program.push((name.clone(), src.clone()));
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| *n == name)
        .flat_map(|(_, ds)| ds)
        .map(|d| {
            (
                d.code.to_string(),
                d.message.clone(),
                src[d.primary_span.clone()].to_string(),
            )
        })
        .collect()
}

/// The notes PW0335 relates to `id`'s program as `edit` makes it.
fn related(id: &str, edit: impl Fn(&str) -> String) -> Vec<String> {
    let mut program = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
        program.extend(files(d));
    }
    program.push((
        "examples/domain.pw".to_string(),
        std::fs::read_to_string(root().join("examples/domain.pw")).expect("domain"),
    ));
    let mut name = String::new();
    for (n, src) in files("examples/accepted") {
        let file = std::path::Path::new(&n)
            .file_name()
            .expect("name")
            .to_string_lossy()
            .to_string();
        if file.starts_with(id) {
            name = n.clone();
            program.push((n, edit(&src)));
        } else {
            program.push((n, src));
        }
    }
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| *n == name)
        .flat_map(|(_, ds)| ds)
        .filter(|d| d.code == "PW0335")
        .flat_map(|d| d.related.into_iter().map(|r| r.label))
        .collect()
}

fn one(code: &str, message: &str, at: &str) -> Vec<(String, String, String)> {
    vec![(code.to_string(), message.to_string(), at.to_string())]
}

#[test]
fn a_clause_in_a_resources_block_is_judged() {
    // A-019's resource writes `scope component` in its block.
    assert_eq!(found("A-019", |s| s.to_string()), vec![]);
    let edit = |to: &'static str| move |s: &str| s.replace("scope component", to);
    assert_eq!(
        found("A-019", edit("scope bogus")),
        one(
            "PW0335",
            "`scope bogus`: `bogus` is not one of `component`, `page`, `session` or `application`",
            "scope bogus"
        )
    );
    // Said of the clause in its declaration's body, not of a policy heading
    // it.
    assert_eq!(
        related("A-019", edit("scope bogus")),
        vec!["a clause in `LazyMap`".to_string()]
    );
    // A clause's value is all its line holds: one word where the domain
    // takes one. Its layout is not its value's.
    assert_eq!(
        found("A-019", edit("scope component page")),
        one(
            "PW0335",
            "`scope component page`: `component page` is not one of `component`, `page`, `session` or `application`",
            "scope component page"
        )
    );
    assert_eq!(
        found("A-019", edit("scope component   page")),
        one(
            "PW0335",
            "`scope component page`: `component page` is not one of `component`, `page`, `session` or `application`",
            "scope component   page"
        )
    );
    // A list is judged whole: each world of a placement.
    assert_eq!(
        found(
            "A-019",
            edit("scope component\n        placement browser, edge")
        ),
        vec![]
    );
    assert_eq!(
        found(
            "A-019",
            edit("scope component\n        placement browser, nowhere")
        )
        .into_iter()
        .map(|f| (f.0, f.2))
        .collect::<Vec<_>>(),
        vec![(
            "PW0335".to_string(),
            "placement browser, nowhere".to_string()
        )]
    );
}

#[test]
fn a_handler_policys_clauses_are_judged() {
    // A-014's `handler_policy { .. }`.
    assert_eq!(found("A-014", |s| s.to_string()), vec![]);
    for (from, to, why) in [
        (
            "captures serializable_only",
            "captures bogus",
            "`bogus` is not one of `serializable_only`",
        ),
        (
            "load on_first_interaction",
            "load bogus",
            "`bogus` is not one of `on_first_interaction` or `eager`",
        ),
        (
            "on_version_mismatch safe_refetch",
            "on_version_mismatch bogus",
            "`bogus` is not one of `safe_refetch` or `refuse`",
        ),
    ] {
        assert_eq!(
            found("A-014", |s| s.replace(from, to)),
            one("PW0335", &format!("`{to}`: {why}"), to),
            "{to}"
        );
    }
    // An operator's arguments, as a header's are.
    let f = found("A-014", |s| {
        s.replace(
            "identity content_address(code, captures)",
            "identity content_address(bogus)",
        )
    });
    assert_eq!(
        f.iter()
            .map(|f| (f.0.as_str(), f.2.as_str()))
            .collect::<Vec<_>>(),
        vec![("PW0335", "identity content_address(bogus)")],
        "{f:#?}"
    );
}

#[test]
fn an_animations_preference_is_judged() {
    // A-020's `animate` block yields to `prefers_reduced_motion`.
    assert_eq!(found("A-020", |s| s.to_string()), vec![]);
    assert_eq!(
        found("A-020", |s| s
            .replace("respects prefers_reduced_motion", "respects bogus")),
        one(
            "PW0335",
            "`respects bogus`: `bogus` is not one of `prefers_reduced_motion`",
            "respects bogus"
        )
    );
}

#[test]
fn a_subtrees_height_is_a_css_length() {
    // A-023's `subtree independent { intrinsic_height 24.px }`.
    assert_eq!(found("A-023", |s| s.to_string()), vec![]);
    for to in ["intrinsic_height 1.5.rem", "intrinsic_height 10.dvh"] {
        assert_eq!(
            found("A-023", |s| s.replace("intrinsic_height 24.px", to)),
            vec![],
            "{to}"
        );
    }
    for (to, value) in [
        ("intrinsic_height bogus", "bogus"),
        ("intrinsic_height 24.pz", "24.pz"),
        ("intrinsic_height 24", "24"),
    ] {
        assert_eq!(
            found("A-023", |s| s.replace("intrinsic_height 24.px", to)),
            one(
                "PW0335",
                &format!("`{to}`: `{value}` is not a length: a count and a CSS unit, `24.px`"),
                to
            ),
            "{to}"
        );
    }
}

#[test]
fn a_length_is_a_count_and_a_css_unit() {
    use pw_core::policy::length;
    for ok in ["24.px", "0.px", "1.5.rem", "100.vh", "3.Q", "2.svmin"] {
        assert!(length(ok), "{ok}");
    }
    for bad in [
        "bogus", "24", "24.", ".px", "24.pz", "-1.px", "1..px", "1.5.", "x.px", "24.PX",
    ] {
        assert!(!length(bad), "{bad}");
    }
}
