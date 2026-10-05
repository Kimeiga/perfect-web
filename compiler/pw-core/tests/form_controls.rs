//! **A form control's value is written where HTML reads it** (ADR-0221).
//!
//! A `<textarea>` has no `value` attribute: HTML reads its value from its
//! text. `bind:value={draft}` was lowered to `value={draft}` and written as
//! an attribute, and a first value showed nothing until the runtime set it,
//! and nothing with scripts off. A `<select>` has none either: its value is
//! the `<option>` it marks `selected`. Found by the feed (ADR-0220).
//!
//! Each test states one part, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn library() -> Vec<(String, String)> {
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
    out
}

/// A page of signals whose view holds `markup`: `note` starts as `note`
/// says, and `fixed` is a binding of the body.
fn page(note: &str, markup: &str) -> String {
    format!(
        "module t\n\npage P() {{\n    cache private\n\n    signal note: String = {note}\n    \
         signal name: String = \"Ada\"\n    let fixed = \"x\"\n\n    \
         view {{\n        <main>{markup}<p>{{note}} {{name}}</p></main>\n    }}\n}}\n"
    )
}

fn reported(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn unit(src: &str) -> Unit {
    Unit {
        path: "t.pw".into(),
        hir: lower_file(src, &parse_tree(src).green),
        src: src.to_string(),
    }
}

/// The page, rendered at its signals' first values, as a host writes it.
fn rendered(src: &str) -> String {
    let u = unit(src);
    let templates: Vec<pw_render::Template> = pw_core::template_ir::build(&[&u.hir])
        .into_iter()
        .map(|t| serde_json::from_value(serde_json::to_value(t).expect("writes")).expect("reads"))
        .collect();
    let page = templates
        .iter()
        .find(|t| t.path == "t.P")
        .expect("the page");
    let compiled = pw_core::backend::signals::compile(&[u]).expect("checks");
    let first = compiled[0].signals.as_ref().expect("data");
    let mut env = pw_render::Env::new();
    for s in first {
        env = env.set(
            &s.name,
            pw_render::Value::from_wire(&s.initial).expect("a value"),
        );
    }
    pw_render::render(page, &env, &templates).expect("renders")
}

/// The start tag of the first `<{tag}>`, and the text up to its end tag.
fn control(html: &str, tag: &str) -> (String, String) {
    let at = html.find(&format!("<{tag}")).expect("the control");
    let rest = &html[at..];
    let open = rest.find('>').expect("its start tag ends");
    let close = rest.find(&format!("</{tag}>")).expect("its end tag");
    (rest[..=open].to_string(), rest[open + 1..close].to_string())
}

const NOTE: &str = "<textarea aria-label=\"Note\" bind:value={note}></textarea>";

#[test]
fn a_textareas_value_is_written_as_its_text() {
    let src = page("\"First line\"", NOTE);
    assert_eq!(reported(&src), Vec::<String>::new());
    let (start, text) = control(&rendered(&src), "textarea");
    assert!(!start.contains("value="), "{start}");
    assert_eq!(text, "First line");
    // Escaped as text, so nothing in the value ends the element.
    let src = page("\"</textarea><b>x</b>\"", NOTE);
    let (_, text) = control(&rendered(&src), "textarea");
    assert_eq!(text, "&lt;/textarea&gt;&lt;b&gt;x&lt;/b&gt;");
    // The parser drops the newline after the start tag, so a value that
    // starts with one is written with one more.
    let src = page("\"\\nsecond line\"", NOTE);
    let (_, text) = control(&rendered(&src), "textarea");
    assert_eq!(text, "\n\nsecond line");
    // Written as text in the source, the same.
    let src = page(
        "\"\"",
        "<textarea aria-label=\"Note\" value=\"Dear Ada & Grace\"></textarea>",
    );
    assert_eq!(reported(&src), Vec::<String>::new());
    let (start, text) = control(&rendered(&src), "textarea");
    assert!(!start.contains("value="), "{start}");
    assert_eq!(text, "Dear Ada &amp; Grace");
    // Control: an `<input>`'s value is its attribute.
    let src = page(
        "\"\"",
        "<input type=\"text\" aria-label=\"Name\" bind:value={name} />",
    );
    let html = rendered(&src);
    let at = html.find("<input").expect("the input");
    assert!(html[at..].starts_with("<input") && html[at..].contains("value=\"Ada\""));
}

#[test]
fn the_browser_sets_a_textareas_value_in_place() {
    let src = page("\"First line\"", NOTE);
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.clone()));
    let units: Vec<Unit> = sources
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    let b = pw_core::build::build(&units).expect("builds");
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let plan = b
        .pages
        .iter()
        .find(|p| p.page == "t.P")
        .expect("the page")
        .plan
        .clone()
        .expect("planned");
    let live: Vec<(String, String)> = plan
        .live
        .iter()
        .filter(|l| l.kind == "attribute")
        .map(|l| (l.attribute.clone(), l.signal.clone()))
        .collect();
    assert_eq!(live, [("value".to_string(), "note".to_string())]);
}

#[test]
fn a_value_html_does_not_read_is_refused_where_it_is_written() {
    for (markup, refused) in [
        (
            "<select aria-label=\"Who\" bind:value={name}><option value=\"Ada\">Ada</option></select>",
            "PW5036 `bind:value` on a `<select>` is written as an attribute HTML does not read",
        ),
        (
            "<select aria-label=\"Who\" value=\"Ada\"><option value=\"Ada\">Ada</option></select>",
            "PW5036 `value` on a `<select>` is written as an attribute HTML does not read",
        ),
        (
            "<textarea aria-label=\"Note\" bind:value={note}>Hello</textarea>",
            "PW5036 a `<textarea>` has a value and text",
        ),
        (
            "<textarea aria-label=\"Note\">{name}</textarea>",
            "PW5036 a `<textarea>`'s text holds a value",
        ),
        (
            "<textarea aria-label=\"Note\" value={fixed}></textarea>",
            "PW5036 `value` on a `<textarea>` is a value no change sets again",
        ),
        (
            "<textarea aria-label=\"Note\" value=\"Dear {name}\"></textarea>",
            "PW5036 `value` on a `<textarea>` is a value no change sets again",
        ),
    ] {
        let found = reported(&page("\"\"", markup));
        assert!(
            found.iter().any(|d| d.starts_with(refused)),
            "{markup}: {found:#?}"
        );
        assert_eq!(
            found.iter().filter(|d| d.starts_with("PW5036")).count(),
            1,
            "{markup}: one fault, said once: {found:#?}"
        );
    }
    // Controls: a signal, text, a value written as text, an option marked
    // `selected`, and an input's value.
    for markup in [
        NOTE,
        "<textarea aria-label=\"Note\">Hello</textarea>",
        "<textarea aria-label=\"Note\" value=\"Hello\"></textarea>",
        "<select aria-label=\"Who\"><option value=\"Ada\" selected>Ada</option></select>",
        "<input type=\"text\" aria-label=\"Name\" bind:value={name} />",
    ] {
        assert_eq!(
            reported(&page("\"\"", markup)),
            Vec::<String>::new(),
            "{markup}"
        );
    }
}

/// **The template writes none of what PW5036 refuses** (ADR-0221): each is
/// a refusal in the IR too, so a build that skipped the check would not
/// write a value where HTML does not read it.
#[test]
fn the_template_refuses_what_the_check_refuses() {
    for markup in [
        "<select aria-label=\"Who\" bind:value={name}><option value=\"Ada\">Ada</option></select>",
        "<textarea aria-label=\"Note\" bind:value={note}>Hello</textarea>",
        "<textarea aria-label=\"Note\">{name}</textarea>",
        "<textarea aria-label=\"Note\" value=\"Dear {name}\"></textarea>",
    ] {
        let src = page("\"\"", markup);
        let u = unit(&src);
        let t = pw_core::template_ir::build(&[&u.hir])
            .into_iter()
            .find(|t| t.path == "t.P")
            .expect("the page");
        let blocked: Vec<String> = t
            .chunks
            .iter()
            .filter_map(|c| match c {
                pw_core::template_ir::Chunk::Dynamic(pw_core::template_ir::Part::Blocked {
                    reason,
                    ..
                }) => Some(reason.clone()),
                _ => None,
            })
            .collect();
        assert!(
            blocked.iter().any(|r| r.ends_with("(PW5036)")),
            "{markup}: {blocked:?}"
        );
    }
}
