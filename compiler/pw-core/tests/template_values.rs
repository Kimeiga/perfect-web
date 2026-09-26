//! **A template reads each value by path** (ADR-0073): a name, or fields read
//! from one.
//!
//! The renderer looks each value up by its path among the values it is given.
//! Until 2026-09-26:
//! - a computed text hole or attribute, `{n + 1}`, built with an empty path,
//!   and `{mk().a}` with the path `.a`, so every render failed;
//! - a computed condition, `{#if !b}`, built a part the renderer refuses:
//!   `pw emit-template` refused it and `pw build` wrote it;
//! - `style:width={w}` built as an attribute named `style:width`, which a
//!   browser ignores;
//! - a loop's key was read by its last segment, so `(k.r.id)` keyed on `k.id`
//!   and `(item.id)` in a loop over `x` on `x.id`.
//!
//! A computed hole is valid Pleris, which this backend cannot render, so it
//! checks and does not build. A key read from another name is wrong in any
//! backend (PW5021). Each test states one case, with a control.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_core::template_ir::{Chunk, Part};
use pw_syntax::parse_tree;

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
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

fn reported(src: &str) -> Vec<String> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn units(src: &str) -> Vec<Unit> {
    program(src)
        .into_iter()
        .map(|(path, src)| {
            let hir = lower_file(&src, &parse_tree(&src).green);
            Unit { path, src, hir }
        })
        .collect()
}

fn view(markup: &str) -> String {
    format!(
        "module t\n\nimport List\n\ntype R = R {{ a: Int, id: Int }}\n\ntype K = K {{ id: Int, r: R }}\n\n\
         fn mk() -> R !{{}} {{ R {{ a: 1, id: 2 }} }}\n\n\
         fn same(xs: List<Int>) -> List<Int> !{{}} {{ xs }}\n\n\
         view V(xs: List<Int>, rs: List<R>, ks: List<K>, r: R, n: Int, s: String, b: Bool) !{{}} {{\n    \
         <div>\n        {markup}\n    </div>\n}}\n"
    )
}

#[test]
fn a_computed_hole_checks_and_does_not_build() {
    for (markup, why) in [
        (
            "<p>{n + 1}</p>",
            "a template hole is read by path, and this is an operation",
        ),
        (
            "<p>{mk().a}</p>",
            "a template hole is read by path, and this is a field of a computed value",
        ),
        (
            "<p title={\"lit\"}>x</p>",
            "`title` is read by path, and this is a literal",
        ),
        (
            "<button type=\"button\" disabled={!b}>x</button>",
            "`disabled` is read by path, and this is an operation",
        ),
        (
            "<a href=\"/x/{r.a + 1}\">x</a>",
            "a hole in an attribute must be a value path",
        ),
        (
            "{#if !b}<p>x</p>{/if}",
            "an `{#if}` condition must be a value path",
        ),
        (
            "{#if b}<p>x</p>{:else if n > 0}<p>y</p>{/if}",
            "an `{:else if}` condition must be a value path",
        ),
    ] {
        let src = view(markup);
        let found = reported(&src);
        assert!(found.is_empty(), "{markup}: {found:#?}");
        match pw_core::build::build(&units(&src)) {
            Ok(_) => panic!("{markup} built"),
            Err(e) => assert!(e.contains(why), "{markup}: {e}"),
        }
    }
    // The same values, read by path, build.
    for markup in [
        "<p>{r.a}</p>",
        "<p title={s}>x</p>",
        "<button type=\"button\" disabled={b}>x</button>",
        "<a href=\"/x/{r.id}\">x</a>",
        "{#if b}<p>x</p>{:else if s}<p>y</p>{/if}",
        "<ul>{#each xs as x (x)}<li>{x}</li>{/each}</ul>",
    ] {
        let src = view(markup);
        let found = reported(&src);
        assert!(found.is_empty(), "{markup}: {found:#?}");
        if let Err(e) = pw_core::build::build(&units(&src)) {
            panic!("{markup}: {e}");
        }
    }
}

#[test]
fn a_directive_other_than_on_does_not_build() {
    let src = view("<p style:width={s}>x</p>");
    assert!(reported(&src).is_empty());
    match pw_core::build::build(&units(&src)) {
        Ok(_) => panic!("`style:width` built"),
        Err(e) => assert!(
            e.contains("`style:width` is a directive, and only `on:` directives are compiled"),
            "{e}"
        ),
    }
    // An XML namespace is part of an attribute's name.
    let src = view("<p xml:lang={s}>x</p>");
    assert!(reported(&src).is_empty());
    if let Err(e) = pw_core::build::build(&units(&src)) {
        panic!("{e}");
    }
}

#[test]
fn a_loops_key_is_read_from_its_element() {
    let found = reported(&view(
        "<ul>{#each rs as x (item.id)}<li>{x.a}</li>{/each}</ul>",
    ));
    assert_eq!(
        found,
        vec!["PW5021 a loop's key is `x` or a field read from it, and this is `item.id`"]
    );
    for markup in [
        "<ul>{#each rs as x (x.id)}<li>{x.a}</li>{/each}</ul>",
        "<ul>{#each ks as k (k.r.id)}<li>{k.id}</li>{/each}</ul>",
        "<ul>{#each xs as x (x)}<li>{x}</li>{/each}</ul>",
    ] {
        let found = reported(&view(markup));
        assert!(found.is_empty(), "{markup}: {found:#?}");
    }
}

#[test]
fn a_key_is_the_path_from_the_element() {
    let key = |markup: &str| {
        let units = units(&view(markup));
        let hirs: Vec<&pw_core::hir::Hir> = units.iter().map(|u| &u.hir).collect();
        let templates = pw_core::template_ir::build(&hirs);
        let v = templates.iter().find(|t| t.path == "t.V").expect("t.V");
        fn each(chunks: &[Chunk]) -> Option<Option<String>> {
            chunks.iter().find_map(|c| match c {
                Chunk::Dynamic(Part::Each { key, .. }) => Some(key.clone()),
                Chunk::Dynamic(p) => p.nested().into_iter().find_map(each),
                Chunk::Static(_) => None,
            })
        }
        each(&v.chunks).expect("an `{#each}`")
    };
    // Nested: its own `id` is another field.
    assert_eq!(
        key("<ul>{#each ks as k (k.r.id)}<li>{k.id}</li>{/each}</ul>"),
        Some("r.id".to_string())
    );
    assert_eq!(
        key("<ul>{#each rs as x (x.id)}<li>{x.a}</li>{/each}</ul>"),
        Some("id".to_string())
    );
    // The element itself.
    assert_eq!(
        key("<ul>{#each xs as x (x)}<li>{x}</li>{/each}</ul>"),
        Some(String::new())
    );
}

/// A computed list is refused by the name check already, as a name that does
/// not resolve (PW0021), and a key read from another name is PW5021. The
/// template IR blocks both too, since it can be built from a program nothing
/// checked.
#[test]
fn the_ir_blocks_a_computed_list_and_a_foreign_key() {
    let blocked = |markup: &str| {
        let units = units(&view(markup));
        let hirs: Vec<&pw_core::hir::Hir> = units.iter().map(|u| &u.hir).collect();
        let templates = pw_core::template_ir::build(&hirs);
        let v = templates.iter().find(|t| t.path == "t.V").expect("t.V");
        v.blocked()
            .into_iter()
            .filter_map(|p| match p {
                Part::Blocked { reason, .. } => Some(reason.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        blocked("<ul>{#each same(xs) as x (x)}<li>{x}</li>{/each}</ul>"),
        vec!["`{#each}` reads its list by path, and `same(xs)` is computed".to_string()]
    );
    assert_eq!(
        blocked("<ul>{#each rs as x (item.id)}<li>{x.a}</li>{/each}</ul>"),
        vec!["a loop's key is `x` or a field read from it, and this is `item.id`".to_string()]
    );
    assert!(blocked("<ul>{#each xs as x (x)}<li>{x}</li>{/each}</ul>").is_empty());
}
