//! **A type that contains itself** (ADR-0194): which ones a program may
//! declare, which ones cross a boundary, and as what.
//!
//! What a compiled one does is `pw-conformance/tests/recursive_types.rs`'s;
//! here is what the checker, the WIT generator, the lowering and the contract
//! say of one.

use pw_core::check::check_sources;
use pw_core::contract::contracts;
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_core::wit;
use pw_syntax::parse_tree;

/// The standard packages, then `program`.
fn sources(program: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .expect("dir")
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
    out.push(("m.pw".to_string(), program.to_string()));
    out
}

/// Each diagnostic on the program's own file, as `CODE message`.
fn diagnostics(program: &str) -> Vec<String> {
    check_sources(&sources(program))
        .into_iter()
        .filter(|(path, _)| path == "m.pw")
        .flat_map(|(_, ds)| ds)
        .map(|d| format!("{} {}", d.code, d.message))
        .collect()
}

fn refused_with(program: &str, code: &str) -> Vec<String> {
    diagnostics(program)
        .into_iter()
        .filter(|d| d.starts_with(code))
        .collect()
}

/// The program's WIT package, or why there is none.
fn package(program: &str) -> Result<String, String> {
    let srcs = sources(program);
    let hirs: Vec<Hir> = srcs
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    wit::package(&refs, &ws, &cs)
        .map(|(text, _)| text)
        .map_err(|e| e.to_string())
}

#[test]
fn a_type_no_finite_value_has_is_refused_where_it_is_declared() {
    for (what, program, name) in [
        (
            "a record that holds itself",
            "module m\n\ntype Loop = Loop { again: Loop }\n",
            "Loop",
        ),
        (
            "a sum type each of whose cases holds itself",
            "module m\n\ntype Wrap =\n    | Wrap(Wrap)\n    | Twice(Wrap, Int)\n",
            "Wrap",
        ),
        (
            "a record that holds one through a generic record",
            "module m\n\ntype Box<T> = Box { value: T }\n\ntype Boxed = Boxed { b: Box<Boxed> }\n",
            "Boxed",
        ),
        (
            "two records that hold each other",
            "module m\n\ntype A = A { b: B }\n\ntype B = B { a: A }\n",
            "A",
        ),
        (
            "an opaque type over one",
            "module m\n\nopaque type Again = Inner\n\ntype Inner = Inner { a: Again }\n",
            "Again",
        ),
    ] {
        let found = refused_with(program, "PW0624");
        assert!(
            found
                .iter()
                .any(|d| d.contains(&format!("`{name}` has no finite value"))),
            "{what}: {:?}",
            diagnostics(program)
        );
    }
}

#[test]
fn a_type_that_can_be_built_without_itself_is_not() {
    for (what, program) in [
        (
            "a list of itself, which may be empty",
            "module m\n\ntype Tree = Tree { kids: List<Tree> }\n",
        ),
        (
            "a case that does not hold it",
            "module m\n\ntype Expr =\n    | Number(Int)\n    | Add(Expr, Expr)\n",
        ),
        (
            "an option of itself, which may be none",
            "module m\n\ntype Node = Node { next: Option<Node> }\n",
        ),
        (
            "a result whose other side has a value",
            "module m\n\ntype Pair = Pair { left: Result<Pair, Int> }\n",
        ),
        (
            "a generic case that does not need its argument",
            "module m\n\ntype Maybe<T> =\n    | Nothing\n    | Just(T)\n\ntype Fine = Fine { m: Maybe<Fine> }\n",
        ),
        (
            "a map of itself",
            "module m\n\ntype Trie = Trie { children: Map<String, Trie> }\n",
        ),
    ] {
        assert!(
            refused_with(program, "PW0624").is_empty(),
            "{what}: {:?}",
            diagnostics(program)
        );
    }
}

const THREAD: &str = r#"module m

import List

type Comment = Comment { text: String, replies: List<Comment> }

type Json =
    | Null
    | Num(Float)
    | Array(List<Json>)
    | Keyed(String, List<Json>)

public query Thread() -> Comment { Comment { text: "a", replies: [] } }

public query Value() -> Json { Json.Null }
"#;

#[test]
fn a_type_that_contains_itself_crosses_as_its_nodes() {
    let text = package(THREAD).expect("generates");
    for expected in [
        "type m-comment = list<m-comment-node>;",
        "record m-comment-node {\n            text: string,\n            replies: list<u32>,\n        }",
        "type m-json = list<m-json-node>;",
        "variant m-json-node {\n            null,\n            num(f64),\n            array(list<u32>),\n            keyed(tuple<string, list<u32>>),\n        }",
        "/// `m.Comment` contains itself, so it crosses as its nodes: node 0 is the",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
    // And the text is a package `wit-parser` reads.
    let mut resolve = wit_parser::Resolve::default();
    resolve
        .push_str("m.wit", &text)
        .unwrap_or_else(|e| panic!("{e}\n{text}"));
}

#[test]
fn a_type_that_contains_itself_otherwise_crosses_no_boundary_yet() {
    for (what, types, needle) in [
        (
            "in place",
            "type Chain =\n    | End\n    | Link(Int, Chain)\n",
            "holds itself in place",
        ),
        (
            "through another declaration",
            "type A = A { bs: List<B> }\n\ntype B = B { a: A }\n",
            "holds itself as `List<",
        ),
        (
            "deeper in a field",
            "type T = T { groups: List<List<T>> }\n",
            "holds itself as `List<List<",
        ),
        (
            "holding another type that contains itself",
            "type U = U { kids: List<U> }\n\ntype T = T { kids: List<T>, other: U }\n",
            "holds `m.U`, which contains itself too",
        ),
    ] {
        let first = types
            .split("type ")
            .nth(1)
            .and_then(|t| t.split_whitespace().next())
            .expect("a name");
        let refused_name = match what {
            "holding another type that contains itself" => "T",
            "through another declaration" => "A",
            _ => first,
        };
        let program = format!(
            "module m\n\nimport List\n\n{types}\npublic query Q(x: {refused_name}) -> Int {{ 0 }}\n"
        );
        let err = package(&program).expect_err(what);
        assert!(
            err.contains("is a type that contains itself, and crosses no boundary yet")
                && err.contains(needle),
            "{what}: {err}"
        );
    }
}

#[test]
fn a_type_that_contains_itself_in_place_is_refused_by_the_backend_by_name() {
    // Never in a signature, so the world is whole; the body holds one.
    let program = "module m\n\ntype Chain =\n    | End\n    | Link(Int, Chain)\n\nfn length(c: Chain) -> Int {\n    match c {\n        End => 0,\n        Link(_, rest) => 1 + length(rest),\n    }\n}\n\npublic query Two() -> Int { length(Chain.Link(1, Chain.Link(2, Chain.End))) }\n";
    let units: Vec<pw_core::check::Unit> = sources(program)
        .into_iter()
        .map(|(path, src)| pw_core::check::Unit {
            hir: lower_file(&src, &parse_tree(&src).green),
            path,
            src,
        })
        .collect();
    let err = pw_core::backend::component::compile(&units, "m.Two")
        .map(|_| ())
        .expect_err("refused");
    assert!(
        err.contains("a type that contains itself in place") && err.contains("no list between"),
        "{err}"
    );
}

#[test]
fn an_invariant_inside_a_tree_holds_at_every_node() {
    let program = r#"module m

import List

opaque type Likes = Int where value >= 0

type Comment = Comment { text: String, likes: Likes, replies: List<Comment> }

fn fetch(id: Int) -> Comment !{ database.read<Comment> }
    host "m:data/comments#fetch"

public query Fetched(id: Int) -> Int { List.length(fetch(id).replies) }
"#;
    let srcs = sources(program);
    let hirs: Vec<Hir> = srcs
        .iter()
        .map(|(_, s)| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let fetched = cs
        .iter()
        .find(|c| c.component_id == "m.Fetched")
        .expect("the contract");
    let answer = fetched
        .imports
        .iter()
        .find(|i| i.name == "fetch")
        .expect("comments#fetch");
    // As its nodes, every node's likes: a path through all of them at once.
    assert_eq!(answer.bounded.len(), 1, "{:?}", answer.bounded);
    assert_eq!(answer.bounded[0].path, ["*", "likes"]);
    assert_eq!(answer.bounded[0].ty, "m.Likes");
}

#[test]
fn a_value_of_a_type_that_contains_itself_is_not_put_on_the_browsers_wire_yet() {
    // A handler's assignment is written out by the type's shape, which for
    // this type would never end: it is refused by name instead, and nothing
    // recurses without end. The signal's first value is data, as any is.
    let program = r#"module m

import List

type Comment = Comment { text: String, replies: List<Comment> }

page P() {
    cache private

    signal draft: Comment = Comment { text: "", replies: [] }

    view {
        <main><p>{draft.text}</p><button type="button" on:press={() => draft = Comment { text: "x", replies: [] }}>Write</button></main>
    }
}
"#;
    let units: Vec<pw_core::check::Unit> = sources(program)
        .into_iter()
        .map(|(path, src)| pw_core::check::Unit {
            hir: lower_file(&src, &parse_tree(&src).green),
            path,
            src,
        })
        .collect();
    let signals = pw_core::backend::signals::compile(&units).expect("checks");
    let page = signals
        .iter()
        .find(|c| c.page.ends_with(".P"))
        .expect("the page's signals");
    let held = page.signals.as_ref().expect("its first value is data");
    assert_eq!(held.len(), 1, "{held:?}");
    let handlers = pw_core::backend::js::compile(&units).expect("checks");
    let refused: Vec<String> = handlers
        .iter()
        .filter_map(|h| match &h.module {
            pw_core::backend::wasm::Encoding::Encoded(_) => None,
            other => Some(format!("{other:?}")),
        })
        .collect();
    assert!(
        refused.iter().any(|r| r.contains("contains itself")),
        "{refused:?}"
    );
}
