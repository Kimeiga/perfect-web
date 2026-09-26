//! **E10: resumable handlers compile to JavaScript modules** (`backend::js`).
//!
//! Charter §14 M10 task 2 puts "generated modern JavaScript modules for
//! handlers" first. What a module reads of its captures is exactly what the
//! template IR tells the renderer to serialize, and a module and its event
//! part name one handler.
//!
//! Since 2026-09-25 a handler's body computes (ADR-0058): it is lowered as a
//! query's is, and its commands are awaited in order. Each module here is run
//! under Node against a context that records the commands it sends, so what
//! is held is what the browser would send, not the text that sends it.
//! Everything outside the supported set is refused by name.

use std::collections::BTreeMap;

use pw_core::backend::js::{self, HandlerModule};
use pw_core::backend::wasm::Encoding;
use pw_core::check::Unit;
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

fn store_units() -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        paths.extend(ps);
    }
    paths.push(root.join("examples/domain.pw"));
    paths
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            Unit {
                path: p.display().to_string(),
                hir: lower_file(&src, &parse_tree(&src).green),
                src,
            }
        })
        .collect()
}

/// Every compiled module, by the behaviour's name; a refusal fails the test.
fn modules(units: &[Unit]) -> BTreeMap<String, HandlerModule> {
    js::compile(units)
        .expect("the program checks")
        .into_iter()
        .map(|c| match c.module {
            Encoding::Encoded(m) => (m.name.clone(), m),
            other => panic!("a handler in `{}` was not compiled: {other}", c.declaration),
        })
        .collect()
}

/// `(name, handler identity, capture paths)` for every event part in the
/// template IR, built with the identities the resume artifacts derived, as
/// `pw emit-template` builds it, and read back from its JSON, as the runtime
/// and the renderer read it.
fn event_parts(units: &[Unit]) -> Vec<(String, String, Vec<String>)> {
    let hirs: Vec<&Hir> = units.iter().map(|u| &u.hir).collect();
    let ws = Workspace::build(&hirs);
    let sigs = Signatures::build(&ws, &hirs);
    let mut handlers = pw_core::template_ir::Handlers::new();
    for u in units {
        for (decl, lambda, m, _) in pw_core::resume_artifacts::located(
            &u.src,
            &u.hir,
            &sigs,
            pw_core::resume_artifacts::BUILD,
        ) {
            handlers.insert((decl, lambda), m.handler);
        }
    }
    let ir = serde_json::to_value(pw_core::template_ir::build_with(&hirs, &handlers))
        .expect("the IR serializes");
    let mut out = Vec::new();
    let mut stack = vec![&ir];
    while let Some(v) = stack.pop() {
        match v {
            serde_json::Value::Object(o) => {
                if o.contains_key("event") && o.contains_key("handler") {
                    let text = |k: &str| o[k].as_str().unwrap_or_default().to_string();
                    let captures = o
                        .get("captures")
                        .and_then(|c| c.as_array())
                        .map(|c| c.iter().map(|p| p.as_str().unwrap().to_string()).collect())
                        .unwrap_or_default();
                    out.push((text("name"), text("handler"), captures));
                }
                stack.extend(o.values());
            }
            serde_json::Value::Array(a) => stack.extend(a),
            _ => {}
        }
    }
    out.sort();
    out
}

// --- the store ------------------------------------------------------------

/// **What a module sends.** It runs under Node with `captures` as the
/// document would give them, and a context that records each command and
/// answers `{"committed": true}`. `sent` is each command, in order; `trap` is
/// the message it stopped with, if it stopped.
fn run(m: &HandlerModule, captures: &str) -> serde_json::Value {
    run_refusing(m, captures, "")
}

/// [`run`], with the context refusing the command `refuse`, as the browser
/// runtime's does on a non-2xx answer: its promise rejects.
fn run_refusing(m: &HandlerModule, captures: &str, refuse: &str) -> serde_json::Value {
    let dir =
        std::env::temp_dir().join(format!("pw-handler-{}-{}", std::process::id(), m.identity));
    std::fs::create_dir_all(&dir).expect("temp");
    std::fs::write(dir.join("handler.mjs"), &m.source).expect("write");
    std::fs::write(
        dir.join("run.mjs"),
        "import { run } from \"./handler.mjs\";\n\
         const sent = [];\n\
         const context = {\n  captures: JSON.parse(process.argv[2]),\n  \
         command: async (id, args) => {\n    sent.push([id, args]);\n    \
         await new Promise((r) => setTimeout(r, 1));\n    \
         if (id === process.argv[3]) throw new Error(\"refused\");\n    \
         return { committed: true };\n  },\n};\n\
         try {\n  await run(context);\n  console.log(JSON.stringify({ sent }));\n} catch (e) {\n  \
         console.log(JSON.stringify({ sent, trap: String(e.message) }));\n}\n",
    )
    .expect("write");
    let out = std::process::Command::new("node")
        .arg("run.mjs")
        .arg(captures)
        .arg(refuse)
        .current_dir(&dir)
        .output()
        .expect("node runs: a handler's module is tested under Node");
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("one line of JSON")
}

fn sent(v: &serde_json::Value) -> &serde_json::Value {
    &v["sent"]
}

#[test]
fn the_stores_handlers_compile_to_the_calls_their_bodies_make() {
    let modules = modules(&store_units());
    assert_eq!(
        modules.keys().collect::<Vec<_>>(),
        ["add_to_cart", "clear_cart"]
    );

    // on:press={resumable(captures = { item }) => add_to_cart(item.id, PositiveInt(1))}
    let add = &modules["add_to_cart"];
    println!("{}", add.source);
    assert_eq!(add.commands, ["store.page.add_to_cart"]);
    assert!(
        add.source.ends_with(&format!(
            "export const name = \"add_to_cart\";\n\
             export const handler = \"{id}\";\n\
             export async function run(context) {{\n\
             \x20 const v0 = context.captures[\"item\"][\"id\"];\n\
             \x20 const v1 = 1n;\n\
             \x20 const v2 = v1;\n\
             \x20 const v3 = await context.command(\"store.page.add_to_cart\", [v0, exact(v2)]);\n\
             \x20 return v3;\n\
             }}\n",
            id = add.identity
        )),
        "the captured item's id, and PositiveInt(1) as its representation"
    );
    assert_eq!(
        sent(&run(add, r#"{"item":{"id":"espresso"}}"#)),
        &serde_json::json!([["store.page.add_to_cart", ["espresso", 1]]])
    );

    // on:press={resumable() => clear_cart()}
    let clear = &modules["clear_cart"];
    println!("{}", clear.source);
    assert_eq!(clear.commands, ["store.page.clear_cart"]);
    assert_eq!(
        sent(&run(clear, "{}")),
        &serde_json::json!([["store.page.clear_cart", []]])
    );
}

/// **A module and its event part are one handler.** The identity the runtime
/// fetches by, the name its manifest is looked up by, and the capture paths
/// the renderer serializes all agree with what the module was compiled as and
/// what it reads.
#[test]
fn a_module_and_its_event_part_name_one_handler() {
    let units = store_units();
    let modules = modules(&units);
    let parts = event_parts(&units);
    assert_eq!(parts.len(), 2, "{parts:?}");
    for (name, identity, captures) in &parts {
        let m = &modules[name];
        assert_eq!(&m.identity, identity, "{name}: one identity");
        for path in captures {
            let read: String = path.split('.').map(|s| format!("[\"{s}\"]")).collect();
            assert!(
                m.source.contains(&format!("context.captures{read}")),
                "{name} reads `{path}`, which the document carries"
            );
        }
    }
    let captures: BTreeMap<&str, &Vec<String>> =
        parts.iter().map(|(n, _, c)| (n.as_str(), c)).collect();
    assert_eq!(
        captures["add_to_cart"],
        &["item.id"],
        "the path read, not the whole item"
    );
    assert!(
        captures["clear_cart"].is_empty(),
        "reads nothing it captured"
    );
}

/// **Every command a handler calls is a component the contracts name.** The
/// handler calls by `contract::component_id`, and so do the contracts; a
/// second derivation that disagreed would send a press to a component no host
/// has a contract for.
#[test]
fn a_handler_calls_its_commands_by_the_contracts_own_ids() {
    let units = store_units();
    let hirs: Vec<&Hir> = units.iter().map(|u| &u.hir).collect();
    let ws = Workspace::build(&hirs);
    let sigs = Signatures::build(&ws, &hirs);
    let ids: Vec<String> = pw_core::contract::contracts(&hirs, &sigs, &ws)
        .into_iter()
        .map(|c| c.component_id)
        .collect();
    for m in modules(&units).values() {
        for c in &m.commands {
            assert!(ids.contains(c), "`{c}` has no contract");
        }
    }
}

#[test]
fn a_program_that_does_not_check_compiles_no_handlers() {
    // The store without the standard packages it imports: it does not check,
    // so nothing is compiled, although every handler in it would be.
    let units: Vec<Unit> = store_units()
        .into_iter()
        .filter(|u| !u.path.contains("packages/"))
        .collect();
    let err = js::compile(&units).expect_err("does not check");
    assert!(err.contains("does not check"), "{err}");
}

// --- what a handler computes, and the refusals --------------------------------

const SHOP: &str = "\
module shop.ui

opaque type Qty = Int
opaque type Sku = String
type Item = Item { id: Sku, name: String, stock: Int }
type Pack = Pack { n: Int }

command Buy(id: Sku, qty: Qty) -> Int { 0 }
command Rename(id: Sku, name: String) -> Int { 0 }
command Ship(p: Pack) -> Int { 0 }
query Look(id: Sku) -> Int { 0 }
fn helper(x: Int) -> Int { x }

page Shop(items: List<Item>, sku: Sku) {
    view {
        <ul>
            {#each items as item (item.id)}
                <li>
                    <button on:press={resumable(captures = { item }) => HANDLER}>Go</button>
                </li>
            {/each}
        </ul>
    }
}
";

/// What the document carries for `SHOP`'s item.
const ITEM: &str = r#"{"item":{"id":"a","name":"nut","stock":5}}"#;

/// The one handler in `SHOP` with `HANDLER` replaced, compiled without the
/// check gate: these programs are about the backend, and what it compiles or
/// refuses must hold whatever produced the program.
fn shop(handler: &str) -> Encoding<HandlerModule> {
    let src = SHOP.replace("HANDLER", handler);
    let hir = lower_file(&src, &parse_tree(&src).green);
    let hirs = vec![&hir];
    let ws = Workspace::build(&hirs);
    let sigs = Signatures::build(&ws, &hirs);
    let mut all = js::handlers(&hirs, &[src.as_str()], &ws, &sigs);
    assert_eq!(all.len(), 1, "one handler in `{handler}`");
    all.remove(0).module
}

fn compiled(handler: &str) -> HandlerModule {
    match shop(handler) {
        Encoding::Encoded(m) => m,
        other => panic!("`{handler}` did not compile: {other}"),
    }
}

/// What the one handler in `SHOP` sends, run on `ITEM`.
fn sends(handler: &str) -> serde_json::Value {
    let m = compiled(handler);
    println!("{}", m.source);
    run(&m, ITEM)
}

fn refused(handler: &str, construct: &str, reason: &str) {
    match shop(handler) {
        Encoding::Unsupported {
            construct: c,
            reason: r,
        } => {
            println!("refused `{handler}`: {c}: {r}");
            assert_eq!(c, construct, "`{handler}`: {r}");
            assert!(r.contains(reason), "`{handler}`: {r}");
        }
        other => panic!("`{handler}` must be refused as {construct}, got {other:?}"),
    }
}

#[test]
fn an_opaque_constructor_sends_its_representation() {
    assert_eq!(
        sent(&sends("Buy(item.id, Qty(2))")),
        &serde_json::json!([["shop.ui.Buy", ["a", 2]]])
    );
}

#[test]
fn each_path_read_is_read_where_the_document_carries_it() {
    let m = compiled("Rename(item.id, item.name)");
    assert!(
        m.source.contains(r#"context.captures["item"]["id"]"#),
        "{}",
        m.source
    );
    assert!(
        m.source.contains(r#"context.captures["item"]["name"]"#),
        "{}",
        m.source
    );
    assert_eq!(
        sent(&run(&m, ITEM)),
        &serde_json::json!([["shop.ui.Rename", ["a", "nut"]]])
    );
    // A string literal sends its VALUE. Its token keeps the quotes, and a
    // module that sent the token would send `"new name"` with the quotes in it.
    assert_eq!(
        sent(&sends("Rename(item.id, \"new name\")")),
        &serde_json::json!([["shop.ui.Rename", ["a", "new name"]]])
    );
}

#[test]
fn a_string_with_escapes_sends_its_value() {
    // The decoded value (ADR-0049): the quotes are the value's, a line feed
    // is one, `\{` and `\}` are braces, and `\u{41}` is `A`.
    assert_eq!(
        sent(&sends("Rename(item.id, \"new \\\"name\\\"\\n\")")),
        &serde_json::json!([["shop.ui.Rename", ["a", "new \"name\"\n"]]])
    );
    assert_eq!(
        sent(&sends("Rename(item.id, \"\\{x\\} \\u{41}\")")),
        &serde_json::json!([["shop.ui.Rename", ["a", "{x} A"]]])
    );
}

/// **A handler computes what it sends** (ADR-0058). Refused until
/// 2026-09-25: an operator, and a call to the program's own function.
#[test]
fn a_handler_computes_what_it_sends() {
    assert_eq!(
        sent(&sends("Buy(item.id, Qty(1 + 1))")),
        &serde_json::json!([["shop.ui.Buy", ["a", 2]]])
    );
    // The captured `Int` is read as a `BigInt`, and sent as a number.
    assert_eq!(
        sent(&sends("Buy(item.id, Qty(helper(item.stock) * 2))")),
        &serde_json::json!([["shop.ui.Buy", ["a", 10]]])
    );
    assert_eq!(
        sent(&sends("Rename(item.id, \"{item.name}: {item.stock}\")")),
        &serde_json::json!([["shop.ui.Rename", ["a", "nut: 5"]]])
    );
}

#[test]
fn a_handler_branches_between_commands() {
    let m = compiled(
        "if item.stock > 3 { Buy(item.id, Qty(item.stock - 3)) } else { Rename(item.id, \"low\") }",
    );
    assert_eq!(m.commands, ["shop.ui.Buy", "shop.ui.Rename"]);
    assert_eq!(
        sent(&run(&m, ITEM)),
        &serde_json::json!([["shop.ui.Buy", ["a", 2]]])
    );
    assert_eq!(
        sent(&run(&m, r#"{"item":{"id":"b","name":"x","stock":2}}"#)),
        &serde_json::json!([["shop.ui.Rename", ["b", "low"]]])
    );
}

#[test]
fn a_handler_calls_several_commands_in_order() {
    assert_eq!(
        sent(&sends(
            "for q in [1, 2, 3] { Buy(item.id, Qty(q * item.stock)) }"
        )),
        &serde_json::json!([
            ["shop.ui.Buy", ["a", 5]],
            ["shop.ui.Buy", ["a", 10]],
            ["shop.ui.Buy", ["a", 15]]
        ])
    );
}

#[test]
fn an_int_a_javascript_number_cannot_carry_stops_the_handler_before_it_sends() {
    // 2^53 is exact; one past it is not, and nothing is sent.
    assert_eq!(
        sent(&sends("Buy(item.id, Qty(9007199254740992))")),
        &serde_json::json!([["shop.ui.Buy", ["a", 9007199254740992u64]]])
    );
    for handler in [
        "Buy(item.id, Qty(9007199254740993))",
        "Buy(item.id, Qty(item.stock * 9007199254740992))",
    ] {
        let out = sends(handler);
        assert_eq!(sent(&out), &serde_json::json!([]), "{handler}");
        assert!(
            out["trap"]
                .as_str()
                .unwrap_or_default()
                .contains("cannot carry exactly"),
            "{handler}: {out}"
        );
    }
    // An `Int` overflow traps as it does everywhere (ADR-0039).
    let out = sends("Buy(item.id, Qty(9223372036854775807 + item.stock))");
    assert_eq!(sent(&out), &serde_json::json!([]));
    assert!(
        out["trap"]
            .as_str()
            .unwrap_or_default()
            .contains("overflow"),
        "{out}"
    );
}

#[test]
fn a_handler_that_calls_no_command_is_refused() {
    refused("helper(1)", "a handler that calls no command", "calls none");
    refused(
        "Look(item.id)",
        "a query called from a handler",
        "answers on the server",
    );
}

#[test]
fn a_value_the_handler_did_not_capture_is_not_read() {
    // `sku` is the page's parameter. The document does not carry it for this
    // handler, so the handler would read nothing.
    assert!(
        !matches!(shop("Buy(sku, Qty(1))"), Encoding::Encoded(_)),
        "a value no capture carries compiled"
    );
}

#[test]
fn a_command_parameter_a_browser_cannot_send_is_refused() {
    refused(
        "Ship(Pack { n: 1 })",
        "a command parameter a browser cannot send",
        "`Pack`",
    );
}

#[test]
fn a_command_inside_a_function_is_refused() {
    // Awaited inside a function value, a command would not be awaited by the
    // handler; and a function value's code does not receive the captures.
    refused(
        "{\n    let buy: fn(Sku, Int) -> Int = (id, q) => Buy(id, Qty(q))\n    buy(item.id, 1)\n}",
        "a command called inside a function",
        "`Buy`",
    );
    refused(
        "{\n    let stock: fn(Int) -> Int = q => q * item.stock\n    Buy(item.id, Qty(stock(1)))\n}",
        "a captured value read inside a function value",
        "`item`",
    );
}

#[test]
fn a_refused_command_stops_the_handler_before_the_next() {
    // Each command is awaited: the second is not sent when the first is
    // refused, and the handler fails where the runtime marks it.
    let m = compiled("{\n    Buy(item.id, Qty(1))\n    Rename(item.id, item.name)\n}");
    assert_eq!(
        sent(&run(&m, ITEM)),
        &serde_json::json!([["shop.ui.Buy", ["a", 1]], ["shop.ui.Rename", ["a", "nut"]]])
    );
    let out = run_refusing(&m, ITEM, "shop.ui.Buy");
    assert_eq!(sent(&out), &serde_json::json!([["shop.ui.Buy", ["a", 1]]]));
    assert_eq!(out["trap"], "refused");
}
