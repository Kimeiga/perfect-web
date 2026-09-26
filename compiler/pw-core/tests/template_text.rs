//! **What a template renders has a text form** (ADR-0074).
//!
//! The renderer writes a `String`, an `Int` or a `Bool` as text, and refuses
//! anything else when it renders. Until 2026-09-26 each of these passed
//! `pw check` and failed only when rendered:
//! - a list, a record, a function or a `Float` in a text hole, an attribute or
//!   a URL's hole;
//! - an `Option` there, and in `{:else if}`, which ADR-0071 left to PW0600
//!   and PW0600 did not read;
//! - a loop keyed on a record, or on a field its element does not have;
//! - an `{#each}` over a field its value does not have;
//! - a boolean attribute given a case of a sum type, which has no truth.
//!
//! Each test states one case, with a control.

use pw_core::check::check_sources;

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

fn view(markup: &str) -> String {
    format!(
        "module t\n\ntype R = R {{ a: Int, id: Int }}\n\ntype K = K {{ id: Int, r: R }}\n\n\
         type Shape =\n    | Circle(Int)\n    | Empty\n\n\
         opaque type Code = String\n\n\
         fn go() -> () !{{}} {{ todo }}\n\n\
         view V(xs: List<Int>, rs: List<R>, ks: List<K>, r: R, f: Float, o: Option<Int>, \
         shape: Shape, code: Code, n: Int, s: String, b: Bool) !{{}} {{\n    \
         <div>\n        {markup}\n    </div>\n}}\n"
    )
}

/// Exactly one diagnostic, with this code, naming this.
fn one(markup: &str, code: &str, says: &str) {
    let found = reported(&view(markup));
    assert_eq!(found.len(), 1, "{markup}: {found:#?}");
    assert!(found[0].starts_with(code), "{markup}: {found:#?}");
    assert!(found[0].contains(says), "{markup}: {found:#?}");
}

fn clean(markup: &str) {
    let found = reported(&view(markup));
    assert!(found.is_empty(), "{markup}: {found:#?}");
}

#[test]
fn a_hole_renders_a_value_with_a_text_form() {
    for (markup, says) in [
        ("<p>{xs}</p>", "`List<Int>`"),
        ("<p>{r}</p>", "`t.R`"),
        ("<p>{go}</p>", "which has no text form"),
        ("<p>{f}</p>", "`Float`"),
        ("<p title={xs}>x</p>", "`List<Int>`"),
        ("<a href=\"/x/{r}\">x</a>", "`t.R`"),
    ] {
        one(markup, "PW0609", says);
    }
    for markup in [
        "<p>{s}</p>",
        "<p>{n}</p>",
        "<p>{b}</p>",
        "<p>{r.a}</p>",
        // An opaque type is written as its representation.
        "<p>{code}</p>",
        "<p title={s}>x</p>",
        "<a href=\"/x/{r.id}\">x</a>",
    ] {
        clean(markup);
    }
}

#[test]
fn a_value_that_may_be_absent_is_taken_apart() {
    for markup in [
        "<p>{o}</p>",
        "<p title={o}>x</p>",
        "<a href=\"/x/{o}\">x</a>",
        // ADR-0071 left this to PW0600, which read only `{#if}`.
        "{#if b}<p>x</p>{:else if o}<p>y</p>{/if}",
    ] {
        one(markup, "PW0600", "may be absent");
    }
    clean("{#match o}{:Some(v)}<p>{v}</p>{:None}<p>none</p>{/match}");
}

#[test]
fn a_boolean_attribute_has_a_truth() {
    one(
        "<button type=\"button\" disabled={shape}>x</button>",
        "PW0609",
        "taken apart with `{#match}`",
    );
    one(
        "<button type=\"button\" disabled={o}>x</button>",
        "PW0600",
        "may be absent",
    );
    clean("<button type=\"button\" disabled={b}>x</button>");
    clean("<button type=\"button\" disabled={n}>x</button>");
}

#[test]
fn a_loops_list_and_key_are_fields_it_has() {
    one(
        "<ul>{#each r.itemz as x (x)}<li>{x}</li>{/each}</ul>",
        "PW0610",
        "has no member `itemz`",
    );
    one(
        "<ul>{#each rs as x (x.missing)}<li>{x.a}</li>{/each}</ul>",
        "PW0610",
        "has no member `missing`",
    );
    one(
        "<ul>{#each rs as x (x)}<li>{x.a}</li>{/each}</ul>",
        "PW0609",
        "`t.R`",
    );
    one(
        "<ul>{#each ks as k (k.r)}<li>{k.id}</li>{/each}</ul>",
        "PW0609",
        "`t.R`",
    );
    for markup in [
        "<ul>{#each rs as x (x.id)}<li>{x.a}</li>{/each}</ul>",
        "<ul>{#each ks as k (k.r.id)}<li>{k.id}</li>{/each}</ul>",
        "<ul>{#each xs as x (x)}<li>{x}</li>{/each}</ul>",
    ] {
        clean(markup);
    }
}

/// Not written as text: a stream's query, the name its part binds, and a
/// mounted resource's arguments. No relation of this ADR's reads them, in
/// the accepted programs that write them (A-007, A-008).
#[test]
fn a_stream_and_a_mounted_resource_are_not_text() {
    use pw_core::check::Unit;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
        for e in std::fs::read_dir(root.join(d)).expect("dir") {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                files.push(p);
            }
        }
    }
    files.push(root.join("examples/domain.pw"));
    for f in [
        "examples/accepted/A-007-map-widget-resource-with-cleanup.pw",
        "examples/accepted/A-008-streamed-public-recommendations.pw",
    ] {
        files.push(root.join(f));
    }
    files.sort();
    let units: Vec<Unit> = files
        .iter()
        .map(|p| {
            let src = std::fs::read_to_string(p).expect("read");
            let hir = pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green);
            Unit {
                path: p.display().to_string(),
                src,
                hir,
            }
        })
        .collect();
    let read: Vec<String> = pw_core::values::analysis(&units)
        .into_iter()
        .filter(|(p, _)| p.contains("/accepted/"))
        .flat_map(|(_, rs)| rs)
        // By name, so the test builds where the kinds do not exist yet.
        .filter(|r| matches!(format!("{:?}", r.kind).as_str(), "Text" | "Absent"))
        .map(|r| r.target)
        .collect();
    for target in ["query", "as", "resource", "center"] {
        assert!(!read.iter().any(|t| t == target), "{target}: {read:?}");
    }
    // The control: the streamed list's items are written, and read.
    assert!(read.iter().any(|t| t == "{..}"), "{read:?}");
}
