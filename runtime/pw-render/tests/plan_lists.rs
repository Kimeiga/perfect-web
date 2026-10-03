//! **A list a session's query fills renders empty where no session asked**
//! (ADR-0145). `pw-render --plan` renders a page for no session, as the
//! store's static document is rendered. A list from a private binding that
//! the values do not give is empty there, as the cart's count is 0. A list
//! from a shared binding is the same for everybody, and must be given.

use std::io::Write;
use std::process::{Command, Stdio};

/// A page that lists `list`, bound by a query cached `cache`.
fn render(list: &str, cache: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!(
        "pw-plan-lists-{}-{list}-{cache}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let plan = serde_json::json!({
        "page": "t.P",
        "params": [],
        "bindings": [{
            "binding": list,
            "resource": "t.Q",
            "args": [],
            "policy": { "cache": cache, "privacy": "session" },
        }],
        "parts": [],
        "collections": [list],
    });
    std::fs::write(dir.join("plan.json"), plan.to_string()).expect("plan");
    std::fs::write(dir.join("values.json"), "{}").expect("values");
    let template = serde_json::json!([{
        "path": "t.P",
        "name": "P",
        "schema": "s",
        "params": [],
        "chunks": [
            { "chunk": "static", "value": "<ul>" },
            { "chunk": "dynamic", "value": {
                "part": "each", "id": 0, "collection": list, "binding": "line",
                "key": "id",
                "body": [{ "chunk": "static", "value": "<li>a line</li>" }],
            }},
            { "chunk": "static", "value": "</ul>" },
        ],
    }]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_pw-render"))
        .args(["--out", dir.join("out").to_str().unwrap()])
        .args(["--values", dir.join("values.json").to_str().unwrap()])
        .args(["--plan", dir.join("plan.json").to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("pw-render runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(template.to_string().as_bytes())
        .expect("the IR");
    let out = child.wait_with_output().expect("finishes");
    let page = std::fs::read_to_string(dir.join("out/P.html")).unwrap_or_default();
    (
        out.status.success(),
        format!("{page}{}", String::from_utf8_lossy(&out.stderr)),
    )
}

#[test]
fn a_private_list_the_values_do_not_give_is_empty() {
    let (ok, page) = render("lines", "private");
    assert!(ok, "{page}");
    assert!(page.contains("<ul>") && !page.contains("a line"), "{page}");
}

#[test]
fn a_shared_list_must_be_given() {
    let (ok, said) = render("menu", "shared");
    assert!(
        !ok,
        "a list everybody shares is not empty by default: {said}"
    );
    assert!(said.contains("no value for `menu`"), "{said}");
}

/// A page whose block a query decides, the query cached `cache`, rendered
/// for no session with no values.
fn render_block(cache: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!("pw-plan-block-{}-{cache}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let plan = serde_json::json!({
        "page": "t.P",
        "params": [],
        "bindings": [{
            "binding": "order",
            "resource": "t.Order",
            "args": [],
            "policy": { "cache": cache, "privacy": "session" },
        }],
        "parts": [],
        "collections": [],
        "blocks": [1],
    });
    std::fs::write(dir.join("plan.json"), plan.to_string()).expect("plan");
    std::fs::write(dir.join("values.json"), "{}").expect("values");
    let template = serde_json::json!([{
        "path": "t.P",
        "name": "P",
        "schema": "s",
        "params": [],
        "chunks": [
            { "chunk": "static", "value": "<section>" },
            { "chunk": "dynamic", "value": {
                "part": "match", "id": 1, "value": "order",
                "arms": [
                    { "case": "Some", "binding": "status",
                      "body": [{ "chunk": "static", "value": "<p>an order</p>" }] },
                    { "case": "None",
                      "body": [{ "chunk": "static", "value": "<p>no order</p>" }] },
                ],
            }},
            { "chunk": "static", "value": "</section>" },
        ],
    }]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_pw-render"))
        .args(["--out", dir.join("out").to_str().unwrap()])
        .args(["--values", dir.join("values.json").to_str().unwrap()])
        .args(["--plan", dir.join("plan.json").to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("pw-render runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(template.to_string().as_bytes())
        .expect("the IR");
    let out = child.wait_with_output().expect("finishes");
    let page = std::fs::read_to_string(dir.join("out/P.html")).unwrap_or_default();
    (
        out.status.success(),
        format!("{page}{}", String::from_utf8_lossy(&out.stderr)),
    )
}

#[test]
fn a_block_a_private_query_decides_renders_nothing_for_no_session() {
    // ADR-0147: this render is no session's, and a session's order is no
    // one's here; neither arm is shown.
    let (ok, page) = render_block("private");
    assert!(ok, "{page}");
    assert!(page.contains("<section></section>"), "{page}");
    // Control: a block a shared query decides must be given its value.
    let (ok, said) = render_block("shared");
    assert!(!ok, "{said}");
    assert!(said.contains("no value for `order`"), "{said}");
}

/// A page `plan` and `chunks` describe, rendered for no session with no
/// values: whether it rendered, and the page or why not.
fn render_page(name: &str, plan: serde_json::Value, chunks: serde_json::Value) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!("pw-plan-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("plan.json"), plan.to_string()).expect("plan");
    std::fs::write(dir.join("values.json"), "{}").expect("values");
    let template = serde_json::json!([{
        "path": "t.P", "name": "P", "schema": "s", "params": [], "chunks": chunks,
    }]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_pw-render"))
        .args(["--out", dir.join("out").to_str().unwrap()])
        .args(["--values", dir.join("values.json").to_str().unwrap()])
        .args(["--plan", dir.join("plan.json").to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("pw-render runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(template.to_string().as_bytes())
        .expect("the IR");
    let out = child.wait_with_output().expect("finishes");
    let page = std::fs::read_to_string(dir.join("out/P.html")).unwrap_or_default();
    (
        out.status.success(),
        format!("{page}{}", String::from_utf8_lossy(&out.stderr)),
    )
}

/// `<p>Delivery in {estimate.minutes} min</p>`, the estimate cached `cache`.
fn estimate_text(cache: &str) -> (bool, String) {
    render_page(
        &format!("text-{cache}"),
        serde_json::json!({
            "page": "t.P", "params": [],
            "bindings": [{
                "binding": "estimate", "resource": "t.Estimate", "args": [],
                "policy": { "cache": cache, "privacy": "session" },
            }],
            "parts": [{
                "part": 0, "path": "estimate.minutes", "binding": "estimate",
                "steps": [{ "field": "minutes" }],
            }],
            "collections": [],
        }),
        serde_json::json!([
            { "chunk": "static", "value": "<p>Delivery in " },
            { "chunk": "dynamic", "value": {
                "part": "text", "id": 0, "value": "estimate.minutes", "context": "text",
            }},
            { "chunk": "static", "value": " min</p>" },
        ]),
    )
}

#[test]
fn a_text_part_a_private_query_decides_is_empty_for_no_session() {
    // T10's store shows a session's estimate as text, and its build failed
    // here: this render is no session's, and shows nothing of a session's.
    let (ok, page) = estimate_text("private");
    assert!(ok, "{page}");
    assert!(
        page.contains("<p>Delivery in <!--pw:s0--><!--pw:e0--> min</p>"),
        "{page}"
    );
    // Control: one a shared query decides must be given its value.
    let (ok, said) = estimate_text("shared");
    assert!(!ok, "{said}");
    assert!(said.contains("no value for `estimate.minutes`"), "{said}");
}

/// A page with one `<stream>`, `streamed` or waited for, and nothing given.
fn stream_page(streamed: bool) -> (bool, String) {
    render_page(
        &format!("stream-{streamed}"),
        serde_json::json!({
            "page": "t.P", "params": [], "bindings": [], "parts": [], "collections": [],
            "streams": [{
                "part": 0, "resource": "t.Recs", "args": [],
                "policy": { "cache": "shared", "privacy": "public" },
                "streamed": streamed,
            }],
        }),
        serde_json::json!([
            { "chunk": "static", "value": "<section>" },
            { "chunk": "dynamic", "value": {
                "part": "stream", "id": 0, "query": "t.Recs", "args": [], "streamed": streamed,
                "placeholder": [{ "chunk": "static", "value": "<p>Finding</p>" }],
                "ready": { "binding": "items", "body": [{ "chunk": "static", "value": "<ul></ul>" }] },
                "failed": { "body": [{ "chunk": "static", "value": "<p>None</p>" }] },
            }},
            { "chunk": "static", "value": "</section>" },
        ]),
    )
}

#[test]
fn a_streamed_region_is_pending_and_one_waited_for_is_nothing_for_no_session() {
    // ADR-0148: as a document is before its query settles.
    let (ok, page) = stream_page(true);
    assert!(ok, "{page}");
    assert!(
        page.contains(
            "<section><!--pw:s0--><?start name=\"pw-0\"><p>Finding</p><?end><!--pw:e0--></section>"
        ),
        "{page}"
    );
    // A region the page waits for has no placeholder, and this render ran
    // its query for no request: it shows nothing, as a private block does.
    let (ok, page) = stream_page(false);
    assert!(ok, "{page}");
    assert!(page.contains("<section></section>"), "{page}");
}
