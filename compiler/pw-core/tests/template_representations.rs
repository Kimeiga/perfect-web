//! **An opaque value's representation is read in a template** (ADR-0232).
//!
//! `{p.id.value}` reads the representation of an opaque `PostId`, which at
//! run time is the same value. Until 2026-10-06 the template read `.value`
//! as a field of the text: the page checked and built, and every request
//! for it was answered 503, `MissingValue { path: "p.id.value" }`. The
//! template reads it by the opaque value's own path now, wherever it reads a
//! value by its path. A record's field named `value` is still a field.
//!
//! Each test states one part, with controls.

use pw_core::check::{Unit, check_sources};
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

fn reported(files: &[(String, String)]) -> Vec<String> {
    check_sources(files)
        .into_iter()
        .filter(|(n, _)| !n.contains("packages/"))
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// The build, or what refused it.
fn build(files: &[(String, String)]) -> Result<pw_core::build::Build, String> {
    let units: Vec<Unit> = files
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            hir: lower_file(src, &parse_tree(src).green),
            src: src.clone(),
        })
        .collect();
    let b = pw_core::build::build(&units)?;
    match b.refusals() {
        refused if refused.is_empty() => Ok(b),
        refused => Err(refused.join("\n")),
    }
}

/// The feed, its `app.pw` changed by each `(anchor, after)`: `after` written
/// after the anchor, which is there once.
fn feed(changes: &[(&str, &str)]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut src = std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("the feed");
    for (anchor, after) in changes {
        assert_eq!(src.matches(anchor).count(), 1, "{anchor}");
        src = src.replace(anchor, &format!("{anchor}{after}"));
    }
    program(&[("app.pw", src)])
}

/// Every path the template `name` reads a value by: a text part's, an
/// attribute's, a block's subject, a loop's list, an instance's argument.
fn paths(b: &pw_core::build::Build, name: &str) -> Vec<String> {
    let t = b
        .templates
        .iter()
        .find(|t| t.path == name)
        .unwrap_or_else(|| panic!("no template {name}"));
    let mut out = Vec::new();
    let mut stack = vec![serde_json::to_value(t).expect("serializes")];
    while let Some(v) = stack.pop() {
        match v {
            serde_json::Value::Object(o) => {
                for (k, v) in o {
                    match (k.as_str(), v) {
                        ("path" | "value" | "list", serde_json::Value::String(s)) => out.push(s),
                        (_, v) => stack.push(v),
                    }
                }
            }
            serde_json::Value::Array(a) => stack.extend(a),
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

const ROW: &str = "                        <p>{p.text}</p>\n";
const THREAD: &str = "            <Replies post={thread} />\n";
const VIEW: &str = "        <p>{post.text}</p>\n";

#[test]
fn a_representation_is_read_by_the_opaque_values_path() {
    let files = feed(&[
        (
            ROW,
            "                        <p class=\"pid\" title=\"{p.id.value}\">\
             <a href=\"/post/{p.id.value}\">{p.id.value}</a></p>\n",
        ),
        (
            THREAD,
            "            <p id=\"which\">{id.value} {thread.id.value}</p>\n            \
             {#match List.get(thread.replies, 0)}{:Some(r)}<p>{r.id.value}</p>{:None}<p>none</p>{/match}\n",
        ),
        (VIEW, "        <p class=\"vid\">{post.id.value}</p>\n"),
    ]);
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files).unwrap_or_else(|e| panic!("{e}"));
    // In a row, a whole attribute, an attribute's text and a text part; a
    // page's parameter, a query's value, a `{#match}` arm's binding under a
    // computed subject, and a view's parameter.
    for (template, read) in [
        ("feed.app.Home", "p.id"),
        ("feed.app.PostPage", "id"),
        ("feed.app.PostPage", "thread.id"),
        ("feed.app.PostPage", "r.id"),
        ("feed.app.Replies", "post.id"),
    ] {
        let read_by = paths(&b, template);
        assert!(
            read_by.contains(&read.to_string()),
            "{template}: {read_by:?}"
        );
        assert!(
            read_by.iter().all(|p| !p.ends_with(".value")),
            "{template}: {read_by:?}"
        );
    }
    // And the page's plan reads the thread's by the same path.
    let thread = b
        .pages
        .iter()
        .find(|p| p.page == "feed.app.PostPage")
        .and_then(|p| p.plan.as_ref().ok())
        .expect("the thread page's plan");
    assert!(
        thread.parts.iter().any(|p| p.path == "thread.id"),
        "{:?}",
        thread.parts
    );
}

/// A record's field, an opaque type over another, and an opaque value
/// read whole.
fn values_page(markup: &str) -> Vec<(String, String)> {
    program(&[(
        "t.pw",
        format!(
            "module t\n\n\
             opaque type Inner = String\n\n\
             opaque type Outer = Inner\n\n\
             type Pair = Pair {{ key: String, value: String, outer: Outer }}\n\n\
             public query Got(key: String) -> Pair !{{}} {{\n    \
             Pair {{ key: key, value: \"v\", outer: Outer(Inner(\"o\")) }}\n}}\n\n\
             page P(key: String) {{\n    route \"/p/{{key}}\"\n    cache private\n\n    \
             let pair = query Got(key)\n\n    \
             view {{\n        <title>P</title>\n        <main>\n            {markup}\n        \
             </main>\n    }}\n}}\n"
        ),
    )])
}

#[test]
fn a_records_field_named_value_is_a_field() {
    let files = values_page(
        "<p id=\"field\">{pair.value}</p>\n            \
         <p id=\"nested\">{pair.outer.value.value}</p>\n            \
         <p id=\"whole\">{pair.outer}</p>",
    );
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files).unwrap_or_else(|e| panic!("{e}"));
    let read_by = paths(&b, "t.P");
    // The record's field is read as a field; an opaque type over another
    // opaque type is read by its own path, each `.value` its representation.
    assert!(read_by.contains(&"pair.value".to_string()), "{read_by:?}");
    assert!(read_by.contains(&"pair.outer".to_string()), "{read_by:?}");
    assert!(
        !read_by.iter().any(|p| p.starts_with("pair.outer.")),
        "{read_by:?}"
    );
}

/// A tally a press speculates on, its count an opaque type, shown through a
/// view given the tally whole and at the top of the page.
const TALLY: &str = "module t

import context.{ current_session }
import capability.{ Session, SessionId }

opaque type Count = Int
opaque type InteractionId = String

type Tally = Tally { n: Count, label: String }

event Bumped(n: Int)

fn tally(s: Session<SessionId>) -> Tally !{ database.read<Tally> }
    host \"t:data/tally#read\"

fn bump_tally(s: Session<SessionId>) -> Tally !{ database.write<Tally> }
    host \"t:data/tally#bump\"

session query Tallied(session: Session<SessionId>) -> Tally
    freshness      0.seconds
    consistency    read_your_writes
    cache          private
    key            session
    invalidates_on Bumped(_)
    concurrency    one_per_key
    on_key_change  cancel
    timeout        2.seconds
{
    tally(session)
}

command bump() -> Tally
    requires      SignedIn
    idempotent_by InteractionId
    emits         Bumped(1)
    optimistic    Tallied(current_session()) as t => bumped(t)
{
    bump_tally(current_session())
}

fn bumped(t: Tally) -> Tally !{} {
    Tally { n: Count(t.n.value + 1), label: t.label }
}

view Shown(tally: Tally) {
    <p id=\"shown\">{tally.n.value}</p>
}

session page P() {
    route     \"/\"
    placement origin
    cache     private

    let t = query Tallied(current_session())

    view {
        <title>P</title>
        <main>
            <Shown tally={t} />
            <p id=\"direct\">{t.n.value}</p>
            <button type=\"button\" on:press|refusable={() => bump()}>Bump</button>
        </main>
    }
}
";

#[test]
fn a_speculated_value_read_through_a_view_reads_its_representation() {
    // The speculation module compiles the view's part from the tally given
    // whole, as the template reads it: by `t.n`, which the view reads as
    // `tally.n`. Read as a field, the two paths differed past their first
    // name, and the page's speculation was refused.
    let files = program(&[("t.pw", TALLY.to_string())]);
    assert_eq!(reported(&files), Vec::<String>::new());
    let b = build(&files).unwrap_or_else(|e| panic!("{e}"));
    assert!(paths(&b, "t.P").contains(&"t.n".to_string()));
    assert!(
        b.speculations.iter().any(|s| s.page == "t.P"),
        "the page's speculation"
    );
}
