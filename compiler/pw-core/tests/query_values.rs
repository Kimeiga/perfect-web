//! **A page's query binding is the query's value** (ADR-0147).
//!
//! `let order = query Order(..)` binds the `Ok` value of the query's declared
//! result, as the host gives it and every template read takes it. Until
//! 2026-10-02 such a binding had no type, so a `{#match}` over it typed
//! nothing below it: T04's `{#match order} {:Some(status)} {#match status}`
//! was refused because `status` had no type, and a loop over an arm's binding
//! had none either, because loops were typed before arms.

use pw_core::check::check_sources;

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

fn reported(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A page with an order's status, and a list a query may have.
fn page(status: &str, list: &str) -> String {
    format!(
        "module t\n\nimport capability.{{ Session, SessionId }}\nimport context.{{ current_session }}\n\n\
         type Status =\n    | Open\n    | Shut\n\n\
         type Item = Item {{ id: String, name: String }}\n\n\
         type Oops =\n    | Oops\n\n\
         session query Q(session: Session<SessionId>) -> Result<Option<Status>, Oops>\n    \
         freshness     0.seconds\n    consistency   read_your_writes\n    cache         private\n    \
         key           session\n    concurrency   one_per_key\n    on_key_change cancel\n    \
         timeout       2.seconds\n{{\n    todo\n}}\n\n\
         public query L() -> Result<Option<List<Item>>, Oops>\n    \
         freshness     30.seconds\n    consistency   snapshot\n    cache         shared\n    \
         concurrency   one_per_key\n    on_key_change cancel\n    timeout       2.seconds\n{{\n    todo\n}}\n\n\
         page P() {{\n    cache private\n    let s = query Q(current_session())\n    let l = query L()\n    \
         signal picked: String = \"\"\n\n    view {{\n        <main>\n            {status}\n            {list}\n        </main>\n    }}\n}}\n"
    )
}

const STATUS: &str = "{#match s}{:Some(x)}{#match x}{:Open}<p>open</p>{:Shut}<p>shut</p>{/match}{:None}<p>none</p>{/match}";
const LIST: &str = "{#match l}{:Some(items)}<ul>{#each items as item (item.id)}<li><button type=\"button\" \
                    on:press={resumable(captures = { item }) => picked = item.name}>{item.name}</button></li>\
                    {/each}</ul>{:None}<p>nothing</p>{/match}";

#[test]
fn a_query_binding_is_its_value() {
    assert_eq!(reported(&page(STATUS, LIST)), Vec::<String>::new());
}

#[test]
fn a_match_over_its_value_covers_every_case() {
    let missing = STATUS.replace("{:Shut}<p>shut</p>", "");
    assert_eq!(
        reported(&page(&missing, LIST)),
        ["PW0305 `{#match x}` does not cover `Shut`"]
    );
}

#[test]
fn a_loop_over_an_arm_s_binding_is_typed() {
    // `item` has the type the list's element has, so the handler that
    // captures it has a schema. The control is the test above: the same page
    // with nothing refused.
    let found = reported(&page(STATUS, LIST));
    assert!(!found.iter().any(|d| d.starts_with("PW5016")), "{found:?}");
    // And a loop over a list nothing types is still refused, by name.
    let untyped = LIST.replace("{#each items as item", "{#each others as item");
    let found = reported(&page(STATUS, &untyped));
    assert!(found.iter().any(|d| d.contains("others")), "{found:?}");
}

#[test]
fn a_query_s_failure_is_not_taken_apart_yet() {
    // The host gives a query's `Ok` value; a page that showed its `Err`
    // waits for T10's ruling, and is refused rather than rendering wrong.
    let ok = STATUS.replace("{#match s}{:Some(x)}", "{#match s}{:Ok(x)}");
    let found = reported(&page(&ok, LIST));
    assert!(
        found
            .iter()
            .any(|d| d == "PW0608 `Ok` is not a constructor of `Option`"),
        "{found:?}"
    );
}

#[test]
fn loops_and_arms_preserve_the_enclosing_generic_parameter() {
    // Phantom T keeps the binding generic while every captured field still
    // has a concrete schema. Solved concrete-type fallback cannot replace
    // the declared loop/arm propagation that carries this binding.
    let right = "module t\n\ntype Row<T> = Row { id: String, result: Result<Int, String> }\n\nfn consume(value: String) -> () !{} { () }\n\nfn outer<T>(rows: Option<List<Row<T>>>) -> () !{} {\n    <main>\n        {#match rows}\n        {:Some(items)}\n            {#each items as item (item.id)}\n                {#match item.result}{:Ok(value)}<button type=\"button\" on:press={resumable(captures = { item }) => consume(item.id)}>ok</button>{:Err(error)}<p>{error}</p>{/match}\n            {/each}\n        {:None}<p>none</p>\n        {/match}\n    </main>\n()\n}\n";
    assert_eq!(reported(right), Vec::<String>::new());
    assert_eq!(
        reported(&right.replace("{:Err(error)}<p>{error}</p>", "")),
        ["PW0305 `{#match item.result}` does not cover `Err`"]
    );
}

#[test]
fn a_query_values_type_is_checked_at_a_function_argument() {
    let right = page(STATUS, LIST)
        .replace(
            "page P()",
            "fn consume(value: Option<Status>) -> () !{} { () }\n\npage P()",
        )
        .replace(
            "    signal picked:",
            "    let _ = consume(s)\n    signal picked:",
        );
    assert_eq!(reported(&right), Vec::<String>::new());
    let wrong = right.replace(
        "consume(value: Option<Status>)",
        "consume(value: Option<Int>)",
    );
    assert_eq!(
        reported(&wrong),
        [
            "PW0605 argument 1 of `t.consume` is declared `Option<Int>` and this is `Option<t.Status>`"
        ]
    );
}
