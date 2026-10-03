//! **Optimistic transitions, compiled and run** (ADR-0122).
//!
//! The store's `add_to_cart` declares `optimistic Cart(current_session()) as
//! cart => Carts.with_line(cart, item, quantity)`, checked since ADR-0025 and
//! executed by nothing until 2026-10-02. These compile the store page's
//! speculation module and run it under Node: the transition the source
//! states, over the value the server sends, and the page's own part read
//! over the result.

use pw_conformance::units;
use pw_core::backend::speculation::compile;
use pw_core::backend::wasm::Encoding;

fn store(page_override: Option<(&str, &str)>) -> Vec<pw_core::check::Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files: Vec<(String, String)> = Vec::new();
    let mut add = |dir: &str| {
        let mut paths: Vec<_> = std::fs::read_dir(root.join(dir))
            .expect(dir)
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            files.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    };
    add("examples/lib");
    add("examples/store");
    files.push((
        "examples/domain.pw".into(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"),
    ));
    if let Some((from, to)) = page_override {
        for (path, src) in &mut files {
            if path.ends_with("store/app.pw") {
                assert!(src.contains(from), "the override's anchor is in app.pw");
                *src = src.replace(from, to);
            }
        }
    }
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    units(&refs)
}

fn node(dir: &std::path::Path, script: &str) -> String {
    std::fs::write(dir.join("try.mjs"), script).expect("write");
    let out = std::process::Command::new("node")
        .arg("try.mjs")
        .current_dir(dir)
        .output()
        .expect("node runs: speculation modules are tested under Node");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf-8")
}

#[test]
fn the_stores_transition_runs_and_its_part_reads_the_result() {
    let compiled = compile(&store(None)).expect("the store checks");
    assert_eq!(compiled.len(), 1, "one page speculates");
    let Encoding::Encoded(m) = &compiled[0].module else {
        panic!("not compiled: {}", compiled[0].module);
    };
    assert_eq!(m.page, "store.page.StorePage");
    assert_eq!(
        m.commands,
        ["store.page.add_to_cart"],
        "clear_cart declares no optimistic clause, so it has no speculation"
    );
    assert_eq!(m.bindings.len(), 1);
    assert_eq!(m.bindings[0].binding, "cart");
    assert_eq!(m.bindings[0].resource, "store.page.Cart");
    assert_eq!(m.bindings[0].key, ["current_session()"]);

    let temp = tempfile::TempDir::with_prefix("pw-speculation-").expect("a temporary directory");
    let dir = temp.path().to_path_buf();
    std::fs::write(dir.join("m.mjs"), &m.source).expect("write");
    let out = node(
        &dir,
        r#"
import * as m from "./m.mjs";
const held = m.decode.cart({ lines: [{ item_id: "espresso", quantity: 2, unit_price: { minor_units: 450 } }] });
const [s] = m.commands["store.page.add_to_cart"];
// The cart's one part, the count, whatever number the page gives it.
const reads = Object.values(m.parts.cart);
if (reads.length !== 1) throw new Error(`the cart has ${reads.length} parts`);
const [read] = reads;
const more = s.transition(held, ["espresso", 1]);
const other = s.transition(more, ["cortado", 3]);
const show = (v) => JSON.stringify(v, (_, x) => (typeof x === "bigint" ? Number(x) : x));
console.log(String(read(held)), String(read(more)), String(read(other)));
console.log(show(held));
console.log(show(other));
"#,
    );
    std::fs::remove_dir_all(&dir).ok();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "2 3 6", "the page's own count over each value");
    assert_eq!(
        lines[1],
        r#"{"lines":[{"item_id":"espresso","quantity":2,"unit_price":{"minor_units":450}}]}"#,
        "the held value is not changed by a transition over it: restoring it is exact"
    );
    assert_eq!(
        lines[2],
        r#"{"lines":[{"item_id":"espresso","quantity":3,"unit_price":{"minor_units":450}},{"item_id":"cortado","quantity":3,"unit_price":{"minor_units":0}}]}"#,
        "a held item's line grows; a new item is a new, unpriced line"
    );
}

#[test]
fn a_speculated_value_read_inside_a_block_is_refused_by_name() {
    // The count, read again inside each menu item: an address with a frame,
    // which this module does not compute. Refused, not silently left stale.
    let compiled = compile(&store(Some((
        "<span>{item.name}</span>",
        "<span>{item.name}</span><span>{cart.line_count}</span>",
    ))))
    .expect("the store still checks");
    match &compiled[0].module {
        Encoding::Unsupported { construct, .. } => {
            assert_eq!(*construct, "a speculated value read inside a block")
        }
        other => panic!("expected a refusal, got {other}"),
    }
}
