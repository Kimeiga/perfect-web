//! **Every name means one binding** (ADR-0063).
//!
//! Until 2026-09-26 three analyses each kept one environment per body, keyed
//! by name:
//! - the value relations, where a name bound at two sites was unknown
//!   wherever it was used, so a type error in any use of it passed
//!   `pw check`;
//! - the declared-type environment, where every use of a name had its last
//!   binding's type, so a handler's capture was typed by another binding of
//!   its name;
//! - the privacy labels, where every use of a name had its first binding's
//!   label.
//!
//! And a `for` loop's name, and a template `{#each}` block's, had no type in
//! the value relations and no label, so a secret iterated over was public.
//! Each test states one of those, with a control that what is right is not
//! refused.

use pw_core::check::check_sources;

fn files(dirs: &[&str]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in dirs {
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

/// `code message` for each diagnostic on `t.pw`, checked with the standard
/// library and the web platform's.
fn reported(src: &str) -> Vec<String> {
    let mut all = files(&["packages/pw-std", "packages/pw-platform-web"]);
    all.push(("t.pw".to_string(), src.to_string()));
    check_sources(&all)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn program(decls: &str) -> String {
    format!("module t\n\nimport List\nimport String\n\n{decls}\n")
}

/// What `src` is refused for: at least one diagnostic, each of them `code`.
fn one(src: &str, code: &str) -> String {
    let found = reported(src);
    assert!(!found.is_empty(), "nothing reported");
    assert!(found.iter().all(|d| d.starts_with(code)), "{found:#?}");
    found.join("\n")
}

fn none(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{found:#?}");
}

// --- the value relations ---------------------------------------------------

#[test]
fn two_lambdas_binding_one_name_are_typed_apart() {
    let wrong = program(
        "fn f(xs: List<Int>, ys: List<String>) -> Int {\n    let a = List.map(xs, s => s + 1)\n    let b = List.map(ys, s => s * 2)\n    List.length(a) + List.length(b)\n}",
    );
    let d = one(&wrong, "PW0609");
    assert!(
        d.contains("`*` must be `Int or Float`, and this is `String`"),
        "{d}"
    );
    // The same `s`, a `String` each time it is used as one.
    none(&wrong.replace("s => s * 2", "s => \"{s}!\""));
}

#[test]
fn two_branches_binding_one_name_are_typed_apart() {
    // And neither outlives its block: the `k` after the `if` is the first.
    let wrong = program(
        "fn f(n: Int) -> Int {\n    let k = n\n    let v = if n > 0 {\n        let k = 1\n        k\n    } else {\n        let k = \"one\"\n        k * 2\n    }\n    v + k\n}",
    );
    one(&wrong, "PW0609");
    none(&wrong.replace("k * 2", "String.length(k)"));
}

#[test]
fn a_lambda_parameter_is_not_the_let_it_shadows() {
    // `v` in the lambda is an element of `xs`; after it, `v` is the `let`'s.
    let wrong = program(
        "fn f(xs: List<String>) -> Int {\n    let v = 1\n    let g = List.map(xs, v => v * 2)\n    List.length(g) + v\n}",
    );
    one(&wrong, "PW0609");
    none(&wrong.replace("v => v * 2", "v => \"{v}!\""));
}

#[test]
fn two_arms_binding_one_name_are_typed_apart() {
    let wrong = program(
        "fn f(o: Option<Int>, p: Option<String>) -> Int {\n    let a = match o {\n        Some(x) => x + 1,\n        None => 0,\n    }\n    let b = match p {\n        Some(x) => x * 2,\n        None => 0,\n    }\n    a + b\n}",
    );
    one(&wrong, "PW0609");
    none(&wrong.replace("Some(x) => x * 2", "Some(x) => String.length(x)"));
}

#[test]
fn a_let_binds_from_the_next_statement() {
    // The second `x` is a `String` from the statement after it; its own
    // initialiser reads the first.
    let wrong = program("fn f(n: Int) -> Int {\n    let x = n\n    let x = \"{x}\"\n    x * 2\n}");
    one(&wrong, "PW0609");
    one(
        &program("fn f(n: Int) -> Int {\n    let x = \"{n}\"\n    let x = x * 2\n    x\n}"),
        "PW0609",
    );
    none(&program(
        "fn f(n: Int) -> Int {\n    let x = n\n    let x = x + 1\n    x * 2\n}",
    ));
}

#[test]
fn a_for_loops_name_is_an_element_of_its_list() {
    let wrong = program(
        "fn f(xs: List<String>) -> Int {\n    let mut t = 0\n    for x in xs {\n        t = t + x\n    }\n    t\n}",
    );
    let d = one(&wrong, "PW0609");
    assert!(d.contains("must be `Int`, and this is `String`"), "{d}");
    none(&wrong.replace("xs: List<String>", "xs: List<Int>"));
}

#[test]
fn a_record_shorthand_names_the_binding_in_scope() {
    // Shorthand after a first field: `P { x }` alone parses as the name `P`
    // and a block (KNOWN_LIMITATIONS).
    let wrong = program(
        "type P = P { a: Int, x: Int }\n\nfn f(n: Int) -> P {\n    if n > 0 {\n        let x = n\n        P { a: 1, x }\n    } else {\n        let x = \"n\"\n        P { a: 2, x }\n    }\n}",
    );
    one(&wrong, "PW0605");
    none(&wrong.replace("let x = \"n\"", "let x = 0"));
}

// --- a template's names ----------------------------------------------------

const TABLE: &str =
    "type Row = Row { name: String }\n\ntype Table = Table { title: String, rows: List<Row> }\n\n";

#[test]
fn an_each_blocks_name_is_an_element_of_its_collection() {
    // `e.rows`: a field of an arm's binding, which nothing typed before.
    let wrong = program(&format!(
        "{TABLE}page P(t: Option<Table>) {{\n    view {{\n        {{#match t}}\n            {{:Some(e)}}\n                <ul>\n                    {{#each e.rows as r (r.name)}}\n                        <li>{{r.nmae}}</li>\n                    {{/each}}\n                </ul>\n            {{:None}}\n        {{/match}}\n    }}\n}}"
    ));
    let d = one(&wrong, "PW0610");
    assert!(d.contains("nmae"), "{d}");
    none(&wrong.replace("r.nmae", "r.name"));
}

#[test]
fn two_arms_binding_one_name_in_a_template_are_typed_apart() {
    // `e.title`, a field with a text form. This wrote `e.rows` until ADR-0074,
    // a list, which a template does not write as text.
    let wrong = program(&format!(
        "{TABLE}page P(t: Option<Table>, w: Option<Row>) {{\n    view {{\n        {{#match t}}\n            {{:Some(e)}}\n                <p>{{e.title}}</p>\n            {{:None}}\n        {{/match}}\n        {{#match w}}\n            {{:Some(e)}}\n                <p>{{e.title}}</p>\n            {{:None}}\n        {{/match}}\n    }}\n}}"
    ));
    let d = one(&wrong, "PW0610");
    assert!(d.contains("title"), "{d}");
    let right = wrong
        .replacen("<p>{e.title}</p>", "<p>{e.name}</p>", 2)
        .replacen("<p>{e.name}</p>", "<p>{e.title}</p>", 1);
    none(&right);
}

// --- privacy ------------------------------------------------------------------

fn logs(body: &str) -> String {
    format!(
        "module t\n\nimport log\nimport secrets\nimport List\nimport capability.{{ Payments, Public }}\n\nfn f() -> () !{{ log<Public>, secret<Payments> }} {{\n{body}\n}}\n"
    )
}

#[test]
fn a_secret_logged_through_a_for_loops_name_is_refused() {
    let wrong = logs(
        "    let tokens = [secrets.payments()]\n    for t in tokens {\n        log.public(\"with {t}\")\n    }",
    );
    one(&wrong, "PW5006");
    none(&wrong.replace("[secrets.payments()]", "[\"a\", \"b\"]"));
}

#[test]
fn a_secret_logged_through_a_lambdas_parameter_is_refused() {
    let wrong = logs(
        "    let tokens = [secrets.payments()]\n    List.map(tokens, t => log.public(\"with {t}\"))",
    );
    one(&wrong, "PW5006");
    none(&wrong.replace("[secrets.payments()]", "[\"a\", \"b\"]"));
}

#[test]
fn a_name_bound_twice_carries_each_bindings_label() {
    // The public `t` is logged, and the secret one is not.
    let right = logs(
        "    let n = List.length([1])\n    if n > 0 {\n        let t = secrets.payments()\n        log.public(\"count {n}\")\n    } else {\n        let t = \"public\"\n        log.public(t)\n    }",
    );
    none(&right);
    let wrong = right.replace("log.public(\"count {n}\")", "log.public(t)");
    one(&wrong, "PW5006");
}

#[test]
fn a_secret_rendered_through_an_each_blocks_name_is_refused() {
    let wrong = "module t\n\nimport secrets\nimport capability.{ Secret, Payments }\n\npage P() {\n    placement origin\n\n    let keys: List<Secret<Payments>> = [secrets.payments()]\n\n    view {\n        <ul>\n            {#each keys as k (k)}\n                <li>{k}</li>\n            {/each}\n        </ul>\n    }\n}\n";
    one(wrong, "PW5003");
    none(
        &wrong
            .replace(
                "let keys: List<Secret<Payments>> = [secrets.payments()]",
                "let keys = [\"a\"]",
            )
            .replace("import capability.{ Secret, Payments }\n", ""),
    );
}

// --- a handler's capture ---------------------------------------------------

/// A page whose handler captures `item`, an `Item`, in the first loop. With
/// `SECOND`, a second loop binds `item` too, to a `Shelf`, which has no `id`.
const SHOP: &str = "\
module shop.ui

opaque type Sku = String
type Item = Item { id: Sku, name: String }
type Shelf = Shelf { title: String }

command Buy(id: Sku) -> Int { 0 }

page Shop(items: List<Item>, shelves: List<Shelf>) {
    view {
        <ul>
            {#each items as item (item.id)}
                <li>
                    <button on:press={resumable(captures = { item }) => Buy(item.id)}>Buy</button>
                </li>
            {/each}
            SECOND
        </ul>
    }
}
";

const SECOND: &str = "{#each shelves as item (item.title)}\n                <li>{item.title}</li>\n            {/each}";

/// The capture schema of each resumable handler in `src`, and whether the
/// handler compiles.
fn handlers(src: &str) -> Vec<(String, bool)> {
    use pw_core::backend::js;
    use pw_core::backend::wasm::Encoding;
    let hir = pw_core::lower::lower_file(src, &pw_syntax::parse_tree(src).green);
    let hirs = vec![&hir];
    let ws = pw_core::resolve::Workspace::build(&hirs);
    let sigs = pw_core::signatures::Signatures::build(&ws, &hirs);
    let schemas: Vec<String> =
        pw_core::resume_artifacts::located(src, &hir, &sigs, pw_core::resume_artifacts::BUILD)
            .into_iter()
            .map(|(_, _, m, _)| m.capture_schema)
            .collect();
    let compiled: Vec<bool> = js::handlers(&hirs, &[src], &ws, &sigs)
        .into_iter()
        .map(|h| matches!(h.module, Encoding::Encoded(_)))
        .collect();
    schemas.into_iter().zip(compiled).collect()
}

#[test]
fn a_capture_is_typed_by_the_binding_its_name_means() {
    let alone = handlers(&SHOP.replace("SECOND", ""));
    let beside = handlers(&SHOP.replace("SECOND", SECOND));
    assert_eq!(alone.len(), 1);
    assert!(alone[0].1, "the handler compiles");
    // The second loop's `item` does not change what the first one's handler
    // captures: its schema is an `Item`'s, and it compiles. Typed by the last
    // binding of the name, the capture was a `Shelf`, which has no `id`, and
    // the handler was refused.
    assert_eq!(beside, alone);

    // Control: a handler capturing the second loop's `item` captures a
    // `Shelf`, and its schema says so.
    let second = SHOP
        .replace(
            "<button on:press={resumable(captures = { item }) => Buy(item.id)}>Buy</button>",
            "<p>{item.name}</p>",
        )
        .replace(
            "SECOND",
            "{#each shelves as item (item.title)}\n                <li><button on:press={resumable(captures = { item }) => Buy(Sku(item.title))}>Buy</button></li>\n            {/each}",
        );
    let shelf = handlers(&second);
    assert_eq!(shelf.len(), 1);
    assert_ne!(
        shelf[0].0, alone[0].0,
        "an `Item` and a `Shelf` are two schemas"
    );
}
