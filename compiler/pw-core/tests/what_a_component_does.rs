//! **A component's contract is what its code does** (ADR-0283).
//!
//! What a component imports, what authority it requires and where it may
//! run are read off its code: its body, and every function compiled into it
//! with it. Until ADR-0283 three readings fell short of the code:
//! - a host call one function down was in no world, so `pw check` passed and
//!   `pw build` refused it (W6, on kiokun's word page);
//! - a database read through a function passed by name, `List.map(names,
//!   found)`, required no capability: placement, the source checks and the
//!   contract counted calls alone, where the row check counts what ADR-0078
//!   counts;
//! - a lambda handed to `List.map` was deferred as a page's handler is, so a
//!   query that read inside one required nothing, and could run in the
//!   browser.
//!
//! Each test states one case, with its control.

use pw_core::build::build;
use pw_core::check::Unit;
use pw_core::contract::{ComponentContract, contracts};
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// The platform's packages, then `t.pw`.
fn units(program: &str) -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        paths.extend(ps);
    }
    let mut out: Vec<Unit> = paths
        .iter()
        .map(|p| {
            let src = std::fs::read_to_string(p).expect("read");
            Unit {
                path: p.display().to_string(),
                hir: lower_file(&src, &parse_tree(&src).green),
                src,
            }
        })
        .collect();
    out.push(Unit {
        path: "t.pw".to_string(),
        hir: lower_file(program, &parse_tree(program).green),
        src: program.to_string(),
    });
    out
}

/// The contract of the component `id`.
fn contract(program: &str, id: &str) -> ComponentContract {
    let units = units(program);
    let hirs: Vec<&Hir> = units.iter().map(|u| &u.hir).collect();
    let ws = Workspace::build(&hirs);
    let sigs = Signatures::build(&ws, &hirs);
    contracts(&hirs, &sigs, &ws)
        .into_iter()
        .find(|c| c.component_id == id)
        .unwrap_or_else(|| panic!("no contract for {id}"))
}

/// The host operations a component imports, by name.
fn imports(c: &ComponentContract) -> Vec<String> {
    c.imports
        .iter()
        .filter(|i| i.interface.starts_with("probe:"))
        .map(|i| format!("{}#{}", i.interface, i.name))
        .collect()
}

/// The capabilities a component requires, by name.
fn requires(c: &ComponentContract) -> Vec<String> {
    c.required_capabilities.iter().map(|x| x.name()).collect()
}

/// What `pw build` refuses of the program.
fn refusals(program: &str) -> Vec<String> {
    build(&units(program))
        .expect("the program checks")
        .refusals()
        .into_iter()
        .map(|r| r.to_string())
        .collect()
}

/// A host operation that reads, and a function that calls it.
const THINGS: &str = "\
module probe.things

import List

source Things
    holds        Thing
    transactions none
    reads        strong
    changes      none

type Thing = Thing {
    name: String,
}

fn read_thing(name: String) -> Option<Thing> !{ database.read<Thing> }
    host \"probe:data/things#read\"

fn found(name: String) -> Bool !{ database.read<Thing> } {
    match read_thing(name) {
        Some(_) => true,
        None => false,
    }
}
";

fn query(name: &str, params: &str, result: &str, body: &str) -> String {
    format!(
        "{THINGS}\npublic query {name}({params}) -> {result}\n    freshness   1.hours\n    \
         consistency snapshot\n{{\n    {body}\n}}\n"
    )
}

const READ: &str = "probe:data/things#read";

#[test]
fn a_host_call_one_function_down_is_imported_and_built() {
    // W6's finding: the query's own body calls no host, the function it
    // calls does, and the backend compiles that function into the query.
    let helper = query("Helper", "name: String", "Bool", "found(name)");
    assert_eq!(imports(&contract(&helper, "probe.things.Helper")), [READ]);
    assert_eq!(refusals(&helper), Vec::<String>::new());
    // The control: called directly, as before.
    let direct = query(
        "Direct",
        "name: String",
        "Option<Thing>",
        "read_thing(name)",
    );
    assert_eq!(imports(&contract(&direct, "probe.things.Direct")), [READ]);
}

#[test]
fn a_function_passed_by_name_is_compiled_in_and_performed_where_named() {
    // `found` is passed to `List.map`, not called: its code is compiled into
    // the query, its host call is imported, and its read is the query's, so
    // the query runs where the database is.
    let mapped = query(
        "Mapped",
        "names: List<String>",
        "List<Bool>",
        "List.map(names, found)",
    );
    let c = contract(&mapped, "probe.things.Mapped");
    assert_eq!(imports(&c), [READ]);
    assert_eq!(requires(&c), ["database.read<Thing>"]);
    assert_eq!(c.allowed_placements, ["origin"]);
    assert_eq!(refusals(&mapped), Vec::<String>::new());
}

#[test]
fn a_lambda_handed_to_a_list_operation_runs_where_it_is_written() {
    let mapped = query(
        "Mapped",
        "names: List<String>",
        "List<Bool>",
        "List.map(names, n => found(n))",
    );
    let c = contract(&mapped, "probe.things.Mapped");
    assert_eq!(requires(&c), ["database.read<Thing>"]);
    assert_eq!(c.allowed_placements, ["origin"]);
    assert_eq!(imports(&c), [READ]);
    // The control: a pure lambda requires nothing, and runs anywhere.
    let pure = query(
        "Lengths",
        "names: List<String>",
        "List<Int>",
        "List.map(names, n => String.length(n))",
    )
    .replace("import List\n", "import List\nimport String\n");
    let c = contract(&pure, "probe.things.Lengths");
    assert_eq!(requires(&c), Vec::<String>::new());
    assert!(c.allowed_placements.contains(&"browser".to_string()));
}

#[test]
fn a_handlers_work_is_still_deferred() {
    // ADR-0283 narrows what is deferred to a handler's, and keeps it: an
    // `on:` attribute's lambda, and the declaration it names (ADR-0199). A
    // page that renders a button calling a command does not write to render,
    // whether the button's handler is a lambda or the command's name.
    let program = format!(
        "{THINGS}\nfn write_thing(name: String) -> Int !{{ database.write<Thing> }}\n    \
         host \"probe:data/things#write\"\n\n\
         command Save(name: String) -> Int\n    requires SignedIn\n{{\n    write_thing(name)\n}}\n\n\
         command SaveAll() -> Int\n    requires SignedIn\n{{\n    write_thing(\"all\")\n}}\n\n\
         page Shop(name: String) {{\n    route \"/shop/{{name}}\"\n\n    view {{\n        \
         <button on:press={{() => Save(name)}}>Save</button>\n    }}\n}}\n\n\
         page All() {{\n    route \"/all\"\n\n    view {{\n        \
         <button on:press={{SaveAll}}>Save all</button>\n    }}\n}}\n"
    );
    for page in ["probe.things.Shop", "probe.things.All"] {
        let page = contract(&program, page);
        assert_eq!(
            requires(&page),
            Vec::<String>::new(),
            "{}",
            page.component_id
        );
        assert!(page.allowed_placements.contains(&"browser".to_string()));
    }
    // The control: the command performs it.
    let command = contract(&program, "probe.things.Save");
    assert_eq!(requires(&command), ["database.write<Thing>"]);
}

#[test]
fn a_local_that_shadows_a_function_is_the_locals() {
    // A call through a binding is the binding's value (ADR-0068), whatever
    // declaration shares its name, and the backend compiles none of it in.
    let shadowed = query(
        "Shadowed",
        "name: String",
        "Bool",
        "let found: fn(String) -> Bool = (n) => n == \"\"\n    found(name)",
    );
    assert_eq!(
        imports(&contract(&shadowed, "probe.things.Shadowed")),
        Vec::<String>::new()
    );
    assert_eq!(refusals(&shadowed), Vec::<String>::new());
}

#[test]
fn a_recursion_is_walked_once() {
    let program = format!(
        "{THINGS}\nfn ping(name: String, n: Int) -> Bool !{{ database.read<Thing> }} {{\n    \
         if n <= 0 {{ found(name) }} else {{ pong(name, n - 1) }}\n}}\n\n\
         fn pong(name: String, n: Int) -> Bool !{{ database.read<Thing> }} {{\n    \
         ping(name, n - 1)\n}}\n\n\
         public query Bounce(name: String) -> Bool\n    freshness   1.hours\n    \
         consistency snapshot\n{{\n    ping(name, 3)\n}}\n"
    );
    assert_eq!(imports(&contract(&program, "probe.things.Bounce")), [READ]);
    assert_eq!(refusals(&program), Vec::<String>::new());
}

#[test]
fn a_handlers_code_is_the_handlers() {
    // What an `on:` attribute's lambda calls is compiled into the handler,
    // not into the page that renders it: a function the handler calls is
    // walked for the handler, and the page imports nothing of it.
    let program = format!(
        "{THINGS}\nfn beep() -> Int !{{ dom.mutate }}\n    host \"probe:ui/beep#beep\"\n\n\
         fn ring() -> Int !{{ dom.mutate }} {{\n    beep()\n}}\n\n\
         page Bell() {{\n    route \"/bell\"\n\n    view {{\n        \
         <button on:press={{() => ring()}}>Ring</button>\n    }}\n}}\n\n\
         view Chime() !{{ dom.mutate }} {{\n    <p>{{ring()}}</p>\n}}\n"
    );
    assert_eq!(
        imports(&contract(&program, "probe.things.Bell")),
        Vec::<String>::new()
    );
    // The control: a view that calls it while it renders imports it.
    assert_eq!(
        imports(&contract(&program, "probe.things.Chime")),
        ["probe:ui/beep#beep"]
    );
}

/// `code message` for each diagnostic on `t.pw`.
fn reported(program: &str) -> Vec<String> {
    let units = units(program);
    let sources: Vec<(String, String)> = units
        .iter()
        .map(|u| (u.path.clone(), u.src.clone()))
        .collect();
    pw_core::check::check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

#[test]
fn a_write_reaches_a_cached_query_that_reads_through_a_function_value() {
    // The checker reads what a body performs as the contract does: a cached
    // query that reads `Thing` only through `found`, passed by name, is a
    // reader of `Thing`, so a command that writes it must invalidate it
    // (PW5106). Counting calls alone, it read nothing, and kept what it read
    // after every write.
    let program = format!(
        "{THINGS}\nfn write_thing(name: String) -> Int !{{ database.write<Thing> }}\n    \
         host \"probe:data/things#write\"\n\n\
         public query Mapped(names: List<String>) -> List<Bool>\n    freshness   0.seconds\n    \
         consistency snapshot\n{{\n    List.map(names, found)\n}}\n\n\
         command Rename(name: String) -> Int\n    requires SignedIn\n{{\n    write_thing(name)\n}}\n"
    );
    let found = reported(&program);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW5106") && d.contains("`Mapped`")),
        "{found:#?}"
    );
    // The control: a reader whose entries expire is left to its window.
    let expiring = program.replace("freshness   0.seconds", "freshness   1.hours");
    assert!(
        !reported(&expiring).iter().any(|d| d.starts_with("PW5106")),
        "{:#?}",
        reported(&expiring)
    );
}
