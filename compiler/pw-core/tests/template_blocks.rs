//! **A template block's markers are read** (ADR-0042).
//!
//! Until 2026-09-25 the HIR lowering dropped every `{:..}` marker. So
//! `{#if a}A{:else}B{/if}` passed `pw check` and `pw build`, and rendered A and
//! B together when `a` held and neither when it did not. Nothing checked that
//! a block closed with its own name, and an unknown directive was refused only
//! when rendered. These tests hold each marker to what it means: in the IR,
//! and in what `pw check` refuses.

use pw_core::check::check_sources;
use pw_core::diagnostics::Diagnostic;
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).unwrap(),
            ));
        }
    }
    out
}

const PRELUDE: &str = "module m\n\ntype Hit = Hit {\n    target: String,\n    word: String,\n}\n\n\
type Shape =\n    | Circle\n    | Square\n\n";

/// A page with these parameters and this view.
fn page(params: &str, view: &str) -> String {
    format!(
        "{PRELUDE}page P({params}) {{\n    view {{\n        <main>\n{view}\n        </main>\n    }}\n}}\n"
    )
}

fn diagnostics(src: &str) -> Vec<Diagnostic> {
    let mut files = library();
    files.push(("m.pw".to_string(), src.to_string()));
    check_sources(&files)
        .into_iter()
        .filter(|(p, _)| p == "m.pw")
        .flat_map(|(_, ds)| ds)
        .collect()
}

/// The page's IR, as `pw build` writes it.
fn chunks(src: &str) -> serde_json::Value {
    let hir: Hir = lower_file(src, &parse_tree(src).green);
    let ir = pw_core::template_ir::build(&[&hir]);
    let p = ir.iter().find(|t| t.name == "P").expect("the page");
    serde_json::to_value(&p.chunks).expect("serializes")
}

/// Every part of one kind, anywhere in the IR.
fn parts<'a>(v: &'a serde_json::Value, kind: &str, out: &mut Vec<&'a serde_json::Value>) {
    match v {
        serde_json::Value::Object(o) => {
            if o.get("part").and_then(|p| p.as_str()) == Some(kind) {
                out.push(v);
            }
            o.values().for_each(|x| parts(x, kind, out));
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| parts(x, kind, out)),
        _ => {}
    }
}

fn one<'a>(v: &'a serde_json::Value, kind: &str) -> &'a serde_json::Value {
    let mut out = Vec::new();
    parts(v, kind, &mut out);
    assert_eq!(out.len(), 1, "one `{kind}` part in {v}");
    out[0]
}

fn statics(v: &serde_json::Value) -> String {
    let mut s = String::new();
    if let Some(a) = v.as_array() {
        for c in a {
            if c["chunk"] == "static" {
                s.push_str(c["value"].as_str().unwrap());
            }
        }
    }
    s
}

/// The codes `pw check` reports for a program.
fn codes(src: &str) -> Vec<&'static str> {
    diagnostics(src).iter().map(|d| d.code).collect()
}

#[test]
fn else_is_the_other_branch_not_more_of_the_first() {
    let src = page(
        "signed_in: Bool",
        "{#if signed_in}<p>Welcome back</p>{:else}<p>Please sign in</p>{/if}",
    );
    assert!(codes(&src).is_empty(), "{:?}", diagnostics(&src));
    let ir = chunks(&src);
    let c = one(&ir, "conditional");
    assert_eq!(c["value"], "signed_in");
    assert_eq!(statics(&c["then"]), "<p>Welcome back</p>");
    assert_eq!(statics(&c["otherwise"]), "<p>Please sign in</p>");
}

#[test]
fn else_if_nests_a_conditional_in_the_otherwise_branch() {
    let src = page(
        "a: Bool, b: Bool",
        "{#if a}<p>A</p>{:else if b}<p>B</p>{:else}<p>C</p>{/if}",
    );
    assert!(codes(&src).is_empty(), "{:?}", diagnostics(&src));
    let ir = chunks(&src);
    let mut all = Vec::new();
    parts(&ir, "conditional", &mut all);
    assert_eq!(all.len(), 2);
    let outer = all.iter().find(|c| c["value"] == "a").unwrap();
    assert_eq!(statics(&outer["then"]), "<p>A</p>");
    let inner = &outer["otherwise"][0]["value"];
    assert_eq!(inner["value"], "b");
    assert_eq!(statics(&inner["then"]), "<p>B</p>");
    assert_eq!(statics(&inner["otherwise"]), "<p>C</p>");
    // Ids follow the document: the nested conditional after A's parts.
    assert!(inner["id"].as_u64() > outer["id"].as_u64());
}

#[test]
fn match_takes_an_option_apart_with_its_payload_bound() {
    let src = page(
        "found: Option<Hit>",
        "{#match found}\n{:Some(h)}<p>{h.word}</p>\n{:None}<p>Nothing</p>\n{/match}",
    );
    assert!(codes(&src).is_empty(), "{:?}", diagnostics(&src));
    let ir = chunks(&src);
    let m = one(&ir, "match");
    assert_eq!(m["value"], "found");
    let arms = m["arms"].as_array().unwrap();
    assert_eq!(arms.len(), 2);
    assert_eq!(
        (arms[0]["case"].as_str(), arms[0]["binding"].as_str()),
        (Some("Some"), Some("h"))
    );
    assert_eq!(one(&arms[0]["body"], "text")["value"], "h.word");
    assert_eq!(
        (arms[1]["case"].as_str(), arms[1].get("binding")),
        (Some("None"), None)
    );
    assert_eq!(statics(&arms[1]["body"]).trim(), "<p>Nothing</p>");
}

#[test]
fn an_attribute_with_holes_is_text_and_values_in_its_context() {
    let src = page(
        "hit: Hit",
        "<a href=\"/words/{hit.target}\" class=\"hit {hit.word}\">{hit.word}</a>",
    );
    assert!(codes(&src).is_empty(), "{:?}", diagnostics(&src));
    let ir = chunks(&src);
    let mut attrs = Vec::new();
    parts(&ir, "interpolated_attribute", &mut attrs);
    let href = attrs.iter().find(|a| a["name"] == "href").unwrap();
    assert_eq!(href["context"], "url");
    assert_eq!(
        href["segments"],
        serde_json::json!([
            {"segment": "static", "value": "/words/"},
            {"segment": "value", "value": "hit.target"},
        ])
    );
    let class = attrs.iter().find(|a| a["name"] == "class").unwrap();
    assert_eq!(class["context"], "attribute");
}

#[test]
fn a_url_that_begins_with_a_value_or_a_style_with_holes_is_refused() {
    for view in [
        "<a href=\"{hit.target}/x\">x</a>",
        "<p style=\"color: {hit.word}\">x</p>",
        "<a href=\"/{hit.word + hit.target}\">x</a>",
    ] {
        let ir = chunks(&page("hit: Hit", view));
        let mut blocked = Vec::new();
        parts(&ir, "blocked", &mut blocked);
        assert_eq!(blocked.len(), 1, "{view}: {ir}");
    }
}

#[test]
fn a_secret_in_an_attributes_hole_is_refused_as_one_in_its_value_is() {
    // R-003's page, with the secret in a hole. Until ADR-0042 the hole was
    // static text, which no privacy rule read, and the page checked clean.
    let src = "module checkout.page\n\nimport secrets\nimport capability.{ Secret, Payments }\n\n\
        page Checkout() {\n    placement origin\n\n    let key: Secret<Payments> = secrets.payments()\n\n    \
        view {\n        <a href=\"/pay/{key}\">pay</a>\n    }\n}\n";
    let whole = src.replace("href=\"/pay/{key}\"", "href={key}");
    let symbols = |s: &str| {
        diagnostics(s)
            .iter()
            .map(|d| d.symbol())
            .collect::<Vec<_>>()
    };
    assert!(
        symbols(&whole).contains(&"secret_to_browser"),
        "{:?}",
        diagnostics(&whole)
    );
    assert!(
        symbols(src).contains(&"secret_to_browser"),
        "{:?}",
        diagnostics(src)
    );
}

/// Each malformed block, the code `pw check` refuses it with, and what the
/// message says: the code alone would pass if a different rule caught the
/// case, as one did while these controls were written (ADR-0042).
#[test]
fn a_malformed_block_is_refused() {
    let cases: &[(&str, &str, &str, &str)] = &[
        ("a: Bool", "{#if a}<p>A</p>{/each}", "PW5019", "closes with"),
        (
            "a: Bool",
            "{#await a}<p>A</p>{/await}",
            "PW5019",
            "is not a template block",
        ),
        (
            "a: Bool",
            "{#if a}<p>A</p>{:else}<p>B</p>{:else}<p>C</p>{/if}",
            "PW5019",
            "one `{:else}`, last",
        ),
        (
            "a: Bool",
            "{#if a}<p>A</p>{:else}<p>B</p>{:else if a}<p>C</p>{/if}",
            "PW5019",
            "one `{:else}`, last",
        ),
        (
            "a: Bool",
            "{#if a}<p>A</p>{:Some(x)}<p>B</p>{/if}",
            "PW5019",
            "one `{:else}`, last",
        ),
        (
            "hits: List<Hit>",
            "{#each hits as h (h.target)}<p>{h.word}</p>{:else}<p>none</p>{/each}",
            "PW5019",
            "takes no",
        ),
        ("a: Bool", "<p>{:else}</p>", "PW5019", "outside any block"),
        ("a: Bool", "<p>x</p>{/if}", "PW5019", "closes no block"),
        (
            "found: Option<Hit>",
            "{#match found}<p>x</p>{:Some(h)}<p>{h.word}</p>{:None}<p>-</p>{/match}",
            "PW5019",
            "only its arms",
        ),
        (
            "found: Option<Hit>",
            "{#match found}{:Some(h)}<p>{h.word}</p>{:Some(g)}<p>-</p>{:None}<p>-</p>{/match}",
            "PW5019",
            "a second",
        ),
        (
            "found: Option<Hit>",
            "{#match found}{:else}<p>-</p>{/match}",
            "PW5019",
            "is not an arm",
        ),
        (
            "s: Shape",
            "{#match s}{:Circle}<p>c</p>{:Square}<p>s</p>{/match}",
            "PW5019",
            "its subject is",
        ),
        (
            "name: String",
            "{#match name}{:Some(x)}<p>x</p>{:None}<p>-</p>{/match}",
            "PW5019",
            "its subject is",
        ),
        (
            "found",
            "{#match found}{:Circle}<p>c</p>{/match}",
            "PW5019",
            "declared type's constructor",
        ),
        (
            "found: Option<Hit>",
            "{#match found}{:Some(h)}<p>{h.word}</p>{/match}",
            "PW0305",
            "does not cover",
        ),
        (
            "found: Option<Hit>",
            "{#match found}{:Ok(h)}<p>x</p>{:Err(e)}<p>-</p>{/match}",
            "PW0608",
            "is not a constructor of",
        ),
        (
            "found: Option<Hit>",
            "{#if found}<p>x</p>{/if}",
            "PW0600",
            "may be absent",
        ),
    ];
    for (params, view, code, says) in cases {
        let src = page(params, view);
        let ds = diagnostics(&src);
        assert!(
            ds.iter()
                .any(|d| d.code == *code && d.message.contains(says)),
            "{view}: expected {code} saying {says:?}, got {ds:?}"
        );
    }
}

#[test]
fn an_unclosed_block_is_refused() {
    let src = page("a: Bool", "{#if a}<p>A</p>");
    assert!(
        diagnostics(&src)
            .iter()
            .any(|d| d.code == "PW5019" && d.message.contains("never closed")),
        "{:?}",
        diagnostics(&src)
    );
}

#[test]
fn a_well_formed_page_has_no_diagnostic() {
    // The negative control for every refusal above: the same blocks, each
    // well formed, check clean.
    let src = page(
        "a: Bool, b: Bool, found: Option<Hit>, hits: List<Hit>",
        "{#if a}<p>A</p>{:else if b}<p>B</p>{:else}<p>C</p>{/if}\n\
         {#match found}{:Some(h)}<a href=\"/{h.target}\">{h.word}</a>{:None}<p>-</p>{/match}\n\
         {#if hits}<ul>{#each hits as h (h.target)}<li>{h.word}</li>{/each}</ul>{:else}<p>none</p>{/if}",
    );
    assert!(codes(&src).is_empty(), "{:?}", diagnostics(&src));
}

#[test]
fn an_arms_binding_has_its_payloads_type() {
    // `inner` is the payload of an `Option<Option<Hit>>`: itself an `Option`,
    // so testing it with `{#if}` is PW0600. Only a typed binding says so.
    let src = page(
        "found: Option<Option<Hit>>",
        "{#match found}{:Some(inner)}{#if inner}<p>x</p>{/if}{:None}<p>-</p>{/match}",
    );
    assert!(codes(&src).contains(&"PW0600"), "{:?}", diagnostics(&src));
    let src = page(
        "found: Option<Hit>",
        "{#match found}{:Some(h)}{#if h.word}<p>x</p>{/if}{:None}<p>-</p>{/match}",
    );
    assert!(codes(&src).is_empty(), "{:?}", diagnostics(&src));
}
