//! **Optimistic transitions, compiled and run** (ADR-0122).
//!
//! The store's `add_to_cart` declares `optimistic Cart(current_session()) as
//! cart => Carts.with_line(cart, item, quantity)`, checked since ADR-0025 and
//! executed by nothing until 2026-10-02; a line's −, + and Remove declare
//! theirs (ADR-0172). These compile the store page's speculation module and
//! run it under Node: each transition the source states, over the value the
//! server sends, and the page's own parts read over the result.

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
fn the_stores_transitions_run_and_its_parts_read_the_result() {
    let compiled = compile(&store(None)).expect("the store checks");
    // The store's page, and its cart's own page (ADR-0190): each presses a
    // command that speculates.
    let pages: Vec<&str> = compiled.iter().map(|c| c.page.as_str()).collect();
    assert_eq!(pages, ["store.page.StorePage", "store.page.CartPage"]);
    let of = |page: &str| {
        let c = compiled
            .iter()
            .find(|c| c.page == page)
            .expect("the page's module");
        match &c.module {
            Encoding::Encoded(m) => m,
            other => panic!("not compiled: {other}"),
        }
    };
    // The cart page adds nothing: it changes, removes and clears.
    assert_eq!(
        of("store.page.CartPage").commands,
        [
            "store.page.decrease_in_cart",
            "store.page.increase_in_cart",
            "store.page.remove_from_cart"
        ]
    );
    let m = of("store.page.StorePage");
    assert_eq!(m.page, "store.page.StorePage");
    assert_eq!(
        m.commands,
        [
            "store.page.add_to_cart",
            "store.page.decrease_in_cart",
            "store.page.increase_in_cart",
            "store.page.remove_from_cart"
        ],
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
const line = (item_id, name, quantity, minor_units) =>
  ({ item_id, name, quantity, unit_price: { minor_units } });
const held = m.decode.cart({ lines: [line("espresso", "Espresso", 2, 450)] });
const run = (command, value, args) => m.commands[command][0].transition(value, args);
// The cart's text at the top of the page: its count, then its subtotal.
const reads = Object.values(m.parts.cart);
if (reads.length !== 2) throw new Error(`the cart has ${reads.length} parts`);
const [count, subtotal] = reads;
// What a row of the cart's lines reads through a member (ADR-0172).
const list = m.regions.cart.find((r) => r.kind === "list");
const rows = (v) =>
  v.lines
    .map((l) => `${l.name}x${list.rows["quantity.count"](l)}=${list.rows["total.display"](l)}`)
    .join(" ");
// As the page shows an item: its store, whether it can be ordered, and its
// category too (ADR-0178, ADR-0181).
const coffee = { id: "coffee", name: "Coffee" };
const espresso = { id: "espresso", store_id: "47", name: "Espresso", description: "", price: { minor_units: 450 }, available: true, category: coffee };
const cortado = { id: "cortado", store_id: "47", name: "Cortado", description: "Short.", price: { minor_units: 375 }, available: true, category: coffee };
const steps = [held];
const step = (command, args) => steps.push(run(command, steps.at(-1), args));
step("store.page.add_to_cart", [espresso, 1]);
step("store.page.add_to_cart", [cortado, 3]);
step("store.page.increase_in_cart", ["cortado"]);
step("store.page.decrease_in_cart", ["espresso"]);
step("store.page.remove_from_cart", ["espresso"]);
const last = run("store.page.decrease_in_cart", m.decode.cart({ lines: [line("espresso", "Espresso", 1, 450)] }), ["espresso"]);
const show = (v) => JSON.stringify(v, (_, x) => (typeof x === "bigint" ? Number(x) : x));
console.log(steps.map((v) => String(count(v))).join(" "));
console.log(steps.map((v) => String(subtotal(v))).join(" "));
console.log(steps.map(rows).join(" | "));
console.log(show(held));
console.log(show(steps[2]));
console.log(show(last), String(count(last)), String(subtotal(last)));
"#,
    );
    std::fs::remove_dir_all(&dir).ok();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines[0], "2 3 6 7 6 4",
        "the page's own count over each value: add one, add three, one more, \
         one fewer, a line gone"
    );
    assert_eq!(
        lines[1], "$9.00 $13.50 $24.75 $28.50 $24.00 $15.00",
        "and its subtotal, from each line's recorded price"
    );
    assert_eq!(
        lines[2],
        "Espressox2=$9.00 | Espressox3=$13.50 | Espressox3=$13.50 Cortadox3=$11.25 | \
         Espressox3=$13.50 Cortadox4=$15.00 | Espressox2=$9.00 Cortadox4=$15.00 | \
         Cortadox4=$15.00",
        "each row's count and total, as the list region computes them"
    );
    assert_eq!(
        lines[3],
        r#"{"lines":[{"item_id":"espresso","name":"Espresso","quantity":2,"unit_price":{"minor_units":450}}]}"#,
        "the held value is not changed by a transition over it: restoring it is exact"
    );
    assert_eq!(
        lines[4],
        r#"{"lines":[{"item_id":"espresso","name":"Espresso","quantity":3,"unit_price":{"minor_units":450}},{"item_id":"cortado","name":"Cortado","quantity":3,"unit_price":{"minor_units":375}}]}"#,
        "a held item's line grows; a new item's line has its name and price"
    );
    assert_eq!(
        lines[5], r#"{"lines":[]} 0 $0.00"#,
        "one fewer of a line that holds one: the line goes"
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
    let store = compiled
        .iter()
        .find(|c| c.page == "store.page.StorePage")
        .expect("the store's module");
    match &store.module {
        Encoding::Unsupported { construct, .. } => {
            assert_eq!(*construct, "a speculated value read inside a block")
        }
        other => panic!("expected a refusal, got {other}"),
    }
}
