//! **A values file states a case** (ADR-0193): `{"$case": "Some", "value":
//! ...}`, the wire form a page's values travel in, so a page that matches on
//! an option renders from one. Until 2026-10-04 `pw-render` read every object
//! as a record.

/// `pw-render`'s body for a template matching `order`, given `values`.
fn rendered(values: serde_json::Value) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let temp = tempfile::TempDir::with_prefix("pw-cases-").expect("a temporary directory");
    let out = temp.path().join("out");
    let given = temp.path().join("values.json");
    std::fs::write(&given, values.to_string()).expect("the values");
    let arm = |case: &str, binding: Option<&str>, body: serde_json::Value| {
        let mut arm = serde_json::json!({ "case": case, "body": body });
        if let Some(b) = binding {
            arm["binding"] = serde_json::json!(b);
        }
        arm
    };
    let text = |t: &str| serde_json::json!([{ "chunk": "static", "value": t }]);
    let template = serde_json::json!([{
        "path": "t.P", "name": "P", "schema": "s", "params": [],
        "chunks": [{ "chunk": "dynamic", "value": {
            "part": "match", "id": 0, "value": "order",
            "arms": [
                arm("Some", Some("status"), serde_json::json!([{ "chunk": "dynamic", "value": {
                    "part": "match", "id": 1, "value": "status",
                    "arms": [arm("placed", None, text("<p>Placed</p>")), arm("preparing", None, text("<p>Preparing</p>"))],
                }}])),
                arm("None", None, text("<p>No order</p>")),
            ],
        }}],
    }]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_pw-render"))
        .args([
            "--out",
            out.to_str().expect("a path"),
            "--values",
            given.to_str().expect("a path"),
        ])
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
    let done = child.wait_with_output().expect("finishes");
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
    std::fs::read_to_string(out.join("P.html")).expect("written")
}

#[test]
fn a_values_file_states_a_case_and_its_payload() {
    let none = rendered(serde_json::json!({ "order": { "$case": "None" } }));
    assert!(none.contains("<p>No order</p>"), "{none}");
    let preparing = rendered(serde_json::json!({
        "order": { "$case": "Some", "value": { "$case": "preparing" } },
    }));
    assert!(preparing.contains("<p>Preparing</p>"), "{preparing}");
    assert!(!preparing.contains("No order"), "{preparing}");
}
