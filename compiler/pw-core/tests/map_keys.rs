//! **A map's key is an `Int`, a `String`, a `Bool`, or an opaque type over
//! one** (ADR-0248, ruling 0057-a), refused at check where it is not, and
//! not first when the backend builds it.
//!
//! A written `Map<Float, Int>` checked until ADR-0248, and the backend
//! refused it, so `pw check` said nothing of a program `pw build` could not
//! build. A map keyed by an opaque type, `Map<ProductId, V>`, could not be
//! written at all. Each test states one case, with its control.

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

/// What the checker reports of `app.pw`: each code, its message, and the
/// text it underlines.
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

/// A module with `decls` after its imports and two opaque types.
fn module(decls: &str) -> String {
    format!(
        "module m\n\nimport List\nimport Map\nimport Set\n\nopaque type Sku = String\n\
         opaque type Weight = Float\n\n{decls}"
    )
}

const KEYS: &str = "an `Int`, a `String`, a `Bool`, or an opaque type over one";

#[test]
fn a_written_key_with_no_order_is_refused_where_it_is_written() {
    let src = module("fn f(m: Map<Float, Int>) -> Int !{} {\n    Map.size(m)\n}\n");
    assert_eq!(
        found(&src),
        vec![(
            "PW0627".to_string(),
            format!("parameter `m` keys a map or a set by `Float`, and a key is {KEYS}"),
            "m: Map<Float, Int>".to_string()
        )]
    );
    // The control: an `Int`.
    assert_eq!(
        found(&src.replace("Map<Float, Int>", "Map<Int, Int>")),
        vec![]
    );
}

#[test]
fn a_bool_and_an_opaque_type_over_an_ordered_type_are_keys() {
    let src = module(
        "fn f(a: Map<Bool, Int>, b: Map<Sku, Int>, c: Set<Bool>) -> Int !{} {\n    \
         Map.size(a) + Map.size(b) + Set.size(c)\n}\n",
    );
    assert_eq!(found(&src), vec![]);
    // An opaque type over a `Float` is no key: its representation has no
    // order the backends share.
    let weight = src.replace("Map<Sku, Int>", "Map<Weight, Int>");
    let f = found(&weight);
    assert_eq!(
        f.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
        vec!["PW0627"],
        "{f:#?}"
    );
    assert!(f[0].1.contains("by `m.Weight`"), "{f:#?}");
}

#[test]
fn a_key_inside_another_type_is_refused_too() {
    let src = module(
        "fn f(xs: List<Set<List<Int>>>) -> Int !{} {\n    List.length(xs)\n}\n\n\
         fn g() -> Int !{} {\n    let m: Map<String, Set<Float>> = Map.empty()\n    \
         Map.size(m)\n}\n",
    );
    let f = found(&src);
    assert_eq!(
        f.iter()
            .map(|f| (f.0.as_str(), f.2.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("PW0627", "xs: List<Set<List<Int>>>"),
            ("PW0627", "let m: Map<String, Set<Float>> = Map.empty()"),
        ],
        "{f:#?}"
    );
}

#[test]
fn a_key_is_refused_once_where_it_is_written_and_not_at_each_call() {
    // `f`'s result is refused where it is written; a call to `f` is not a
    // generic map's, and says nothing more.
    let src = module(
        "fn f() -> Map<Float, Int> !{} {\n    Map.empty()\n}\n\n\
         fn g() -> Int !{} {\n    Map.size(f()) + Map.size(f())\n}\n",
    );
    let f = found(&src);
    assert_eq!(
        f.iter()
            .map(|f| (f.0.as_str(), f.2.as_str()))
            .collect::<Vec<_>>(),
        vec![("PW0627", "f")],
        "{f:#?}"
    );
}

#[test]
fn a_key_a_call_instantiates_is_refused_at_the_call() {
    let src =
        module("fn f() -> Int !{} {\n    let s = Set.from_list([1.5, 2.5])\n    Set.size(s)\n}\n");
    assert_eq!(
        found(&src),
        vec![(
            "PW0627".to_string(),
            format!("`Set.from_list`'s result keys a map or a set by `Float`, and a key is {KEYS}"),
            "Set.from_list([1.5, 2.5])".to_string()
        )]
    );
    // The control: an `Int`'s; and a generic function's own key, which each
    // call instantiates.
    assert_eq!(found(&src.replace("[1.5, 2.5]", "[1, 2]")), vec![]);
    assert_eq!(
        found(&module(
            "fn f<K, V>(m: Map<K, V>) -> Int !{} {\n    Map.size(m)\n}\n"
        )),
        vec![]
    );
}
