//! **A nested declaration sees the bindings around it** (ADR-0066).
//!
//! A declaration may nest another: a `fn` inside a `fn`, a `command` or a
//! `component`. Until 2026-09-26 the nested body was analysed alone, and a
//! name it read from the declaration around it was bound to nothing by the
//! value relations, the declared-type environment and the privacy labels.
//! So a secret the enclosing body held, logged publicly by the nested
//! function, passed `pw check`. The name check saw every binding of the
//! enclosing body, wherever it was, where the other three saw none.
//!
//! One resolver answers for all four now: a nested declaration sees the
//! enclosing declaration's parameters and the bindings in scope where it is
//! written. Also here: a lambda's one parenthesised parameter binds, and a
//! call to a function value outside its scope is reported. Each test states
//! one case, with a control that what is right is not refused.

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

fn reported(src: &str) -> Vec<String> {
    let mut all = files(&["packages/pw-std", "packages/pw-platform-web"]);
    all.push(("t.pw".to_string(), src.to_string()));
    check_sources(&all)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn refused(src: &str, code: &str) -> Vec<String> {
    let found = reported(src);
    assert!(!found.is_empty(), "nothing reported");
    assert!(found.iter().all(|d| d.starts_with(code)), "{found:#?}");
    found
}

fn none(src: &str) {
    let found = reported(src);
    assert!(found.is_empty(), "{found:#?}");
}

const SECRET: &str =
    "module t\n\nimport log\nimport secrets\nimport capability.{ Payments, Public }\n\n";

#[test]
fn a_nested_function_carries_the_label_of_what_it_reads_around_it() {
    let wrong = format!(
        "{SECRET}fn outer() -> () !{{ log<Public>, secret<Payments> }} {{\n    let key = secrets.payments()\n\n    fn inner() -> () !{{ log<Public>, secret<Payments> }} {{\n        log.public(\"key {{key}}\")\n    }}\n\n    inner()\n}}\n"
    );
    refused(&wrong, "PW5006");
    none(&wrong.replace("let key = secrets.payments()", "let key = \"public\""));
}

#[test]
fn a_nested_function_types_what_it_reads_around_it() {
    let wrong = "module t\n\nfn outer(n: Int) -> Int {\n    let word = \"a\"\n\n    fn inner(m: Int) -> Int {\n        m + word\n    }\n\n    inner(n)\n}\n";
    refused(wrong, "PW0609");
    none(&wrong.replace("let word = \"a\"", "let word = 2"));
    // The enclosing declaration's parameters too.
    refused(
        "module t\n\nfn outer(n: String) -> Int {\n    fn inner(m: Int) -> Int {\n        m + n\n    }\n\n    inner(1)\n}\n",
        "PW0609",
    );
}

#[test]
fn a_nested_function_sees_the_binding_in_scope_where_it_is_written() {
    // Two bindings of `word` around it: the one before it is the one it
    // reads.
    let right = "module t\n\nfn outer(n: Int) -> Int {\n    let word = 2\n\n    fn inner(m: Int) -> Int {\n        m + word\n    }\n\n    let word = \"a\"\n    inner(n)\n}\n";
    none(right);
}

#[test]
fn one_parenthesised_parameter_binds() {
    none(
        "module t\n\nimport List\n\nfn f(xs: List<Int>) -> List<Int> { List.map(xs, (x) => x + 1) }\n",
    );
    refused(
        "module t\n\nimport List\n\nfn f(xs: List<String>) -> List<Int> { List.map(xs, (x) => x * 2) }\n",
        "PW0609",
    );
}

#[test]
fn a_call_to_a_function_outside_its_scope_is_reported() {
    let wrong = "module t\n\nfn f(n: Int) -> Int {\n    let v = if n > 0 {\n        let g = x => x + 1\n        g(n)\n    } else {\n        0\n    }\n    g(v)\n}\n";
    let found = refused(wrong, "PW0021");
    assert!(found.iter().any(|d| d.contains("`g`")), "{found:#?}");
    none(&wrong.replace("    g(v)\n", "    v\n"));
}

#[test]
fn a_nested_declarations_own_annotations_resolve() {
    // A module listed only its top-level declarations, so a nested one had no
    // module, and the types its body read its parameters by were left
    // unresolved: `m` was untyped, and nothing through it was checked.
    let wrong = "module t\n\nfn outer(n: Int) -> Int {\n    fn inner(m: String) -> Int {\n        m * 2\n    }\n\n    inner(\"a\")\n}\n";
    refused(wrong, "PW0609");
    none(
        &wrong
            .replace("m: String", "m: Int")
            .replace("inner(\"a\")", "inner(n)"),
    );
}

#[test]
fn a_streams_parts_are_its_answer_and_its_failure() {
    // `<ready as={items}>` is the query's success: a list of `Rec`, whose
    // element has no `nmae`.
    let wrong = "module t\n\ntype Rec = Rec { name: String }\n\npublic query Recs(n: Int) -> Result<List<Rec>, String> { Ok([]) }\n\nview V(n: Int) !{} {\n    <section>\n        <stream query={Recs(n)}>\n            <ready as={items}>\n                <ul>\n                    {#each items as item (item.name)}\n                        <li>{item.nmae}</li>\n                    {/each}\n                </ul>\n            </ready>\n            <failed as={e}><p>failed</p></failed>\n        </stream>\n    </section>\n}\n";
    let found = refused(wrong, "PW0610");
    assert!(found.iter().any(|d| d.contains("nmae")), "{found:#?}");
    none(&wrong.replace("item.nmae", "item.name"));
}

#[test]
fn a_nested_function_reads_the_declared_label_of_a_field_around_it() {
    // `acct` is public, and its `token` is declared a secret: what the
    // nested function reads is a field of the enclosing parameter's type.
    let wrong = "module t\n\nimport log\nimport capability.{ Secret, Payments, Public }\n\ntype Account = Account { token: Secret<Payments>, name: String }\n\nfn outer(acct: Account) -> () !{ log<Public> } {\n    fn inner() -> () !{ log<Public> } {\n        log.public(\"t {acct.token}\")\n    }\n\n    inner()\n}\n";
    refused(wrong, "PW5006");
    none(&wrong.replace("{acct.token}", "{acct.name}"));
}

#[test]
fn a_release_clause_names_what_its_resource_acquires() {
    // `h` is the `Handle` the resource's `acquire` produces.
    let wrong = "module t\n\ntype Handle = Handle { id: Int }\n\nfn make() -> Result<Handle, String> !{} { Ok(Handle { id: 1 }) }\n\nfn close(h: Handle) -> Int !{} { h.id }\n\nfn count(n: Int) -> Int !{} { n }\n\ncomponent C() {\n    placement browser\n\n    resource thing when true {\n        scope component\n        acquire { make() }\n        release(h) { count(h) }\n    }\n\n    view { <p>x</p> }\n}\n";
    refused(wrong, "PW0605");
    none(&wrong.replace("count(h)", "close(h)"));
}
