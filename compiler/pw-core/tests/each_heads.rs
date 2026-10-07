//! **An `{#each}`'s head is read once, by the grammar** (ADR-0242).
//!
//! `{#each menu as item (item.id)}` was kept as text, and five places split
//! it at ` as ` and `(`, each its own reading: name resolution, the template
//! IR and its key checks, lexical scope, the names check and Marko. So:
//! - `{#each xs ys as x (x)}` checked, its list `xs ys`;
//! - an unclosed key, `(x`, checked;
//! - `{#each xs}` was refused for a key and a name it lacks, never for the
//!   `as` it lacks;
//! - a list computed by a call, `same(xs)`, held a `(`, so a keyless loop
//!   over it was taken to have a key (PW5011).
//!
//! The grammar parses the head now, and each reader reads its parts. Each
//! test states one case, with a control.

use pw_core::check::check_sources;

/// The platform's packages, and `src` as `app.pw` after them.
fn program(src: &str) -> Vec<(String, String)> {
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
    out.push(("app.pw".to_string(), src.to_string()));
    out
}

/// What the checker reports of `app.pw`: each code, its message and the text
/// it underlines.
fn found(src: &str) -> Vec<(String, String, String)> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
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

fn one(code: &str, message: &str, at: &str) -> Vec<(String, String, String)> {
    vec![(code.to_string(), message.to_string(), at.to_string())]
}

/// A view whose list is `head`'s.
fn view(head: &str) -> String {
    format!(
        "module m\n\nfn same(xs: List<Int>) -> List<Int> !{{}} {{\n    xs\n}}\n\n\
         view V(xs: List<Int>, ys: List<Int>) {{\n    <ul>\n        \
         {{#each {head}}}\n            <li>{{x}}</li>\n        {{/each}}\n    </ul>\n}}\n"
    )
}

#[test]
fn a_head_is_its_list_as_its_name_and_its_key() {
    // The control.
    assert_eq!(found(&view("xs as x (x)")), vec![]);
    // Two words for a list: the one error, and `x` is bound all the same.
    assert_eq!(
        found(&view("xs ys as x (x)")),
        one(
            "PW0019",
            "expected `as` and a name for each row, found an identifier",
            "ys"
        )
    );
    // An unclosed key.
    let unclosed = view("xs as x (x");
    let f = found(&unclosed);
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(
        (f[0].0.as_str(), f[0].1.as_str()),
        (
            "PW0001",
            "expected `)` to close the key, found the end of the `{#each}`"
        )
    );
    // A second name, an index, stands between the name and the key: the one
    // error, and the key read, so the list is not refused for a key it has.
    assert_eq!(
        found(&view("xs as x, i (x)")),
        one(
            "PW0016",
            "an `{#each}` is its list, `as` a name for each row, and a key in parentheses",
            ", i"
        )
    );
}

#[test]
fn a_head_with_no_name_says_so() {
    // `as` and a name it lacks, said; then what reads the name it would bind.
    let f = found(&view("xs"));
    assert_eq!(f.first().map(|f| f.0.as_str()), Some("PW0019"), "{f:#?}");
    assert!(
        f[0].1
            .starts_with("expected `as` and a name for each row, found the end"),
        "{f:#?}"
    );
}

#[test]
fn a_computed_list_needs_its_key_too() {
    // A `(` in the list was taken for a key.
    assert_eq!(
        found(&view("same(xs) as x")).first().map(|f| f.0.clone()),
        Some("PW5011".to_string())
    );
    // The control: keyed.
    assert!(
        !found(&view("same(xs) as x (x)"))
            .iter()
            .any(|f| f.0 == "PW5011")
    );
}

#[test]
fn a_loops_key_is_refused_at_the_key() {
    assert_eq!(
        found(&view("xs as x (y)")),
        one(
            "PW5021",
            "a loop's key is `x` or a field read from it, and this is `y`",
            "y"
        )
    );
}
