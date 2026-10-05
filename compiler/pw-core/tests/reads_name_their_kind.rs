//! **A `query` reads a query or a resource, and a `subscription` a
//! subscription** (ADR-0212, the owner's ruling 0108-a; ADR-0210's urgent
//! defect 6).
//!
//! `let v = query helper(n)`, `helper` a `fn`, checked: the page kept the
//! function's answer as an entry, with a key, a freshness and a cache the
//! function never declared. The name is refused where it is read now.

use pw_core::check::check_sources;

fn sources(src: &str) -> Vec<(String, String)> {
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
    out.push(("m.pw".to_string(), src.to_string()));
    out
}

/// What PW5108 reports for a page whose body reads `read`.
fn refused(read: &str) -> Vec<String> {
    let src = format!(
        r#"module m

import stream.{{ Stream }}

fn helper(n: String) -> String {{ n }}

fn found(n: String) -> String !{{ database.read<Found> }}
    host "m:data/found#found"

fn ticking(n: String) -> Stream<String, String> !{{ network.subscribe }}
    host "m:data/ticks#follow"

public query Found(n: String) -> String
    freshness     30.seconds
    consistency   snapshot
    cache         shared
    key           n
    concurrency   one_per_key
    on_key_change cancel
    timeout       2.seconds
{{
    found(n)
}}

subscription Ticks(n: String) -> Stream<String, String>
    scope         component
    transport     websocket
    reconnect     bounded_exponential(max = 5, jitter = true)
    dedupe_by     n
    on_scope_exit close
{{
    ticking(n)
}}

page P(n: String) {{
    route        "/p/{{n}}"
    placement    origin
    cache        private

    let v = {read}

    view {{
        <title>P</title>
        <main><p>{{n}}</p></main>
    }}
}}
"#
    );
    check_sources(&sources(&src))
        .into_iter()
        .filter(|(path, _)| path == "m.pw")
        .flat_map(|(_, ds)| ds)
        .filter(|d| d.code == "PW5108")
        .map(|d| d.message)
        .collect()
}

#[test]
fn a_query_of_a_function_is_refused() {
    assert_eq!(
        refused("query helper(n)"),
        ["`query helper` reads a function, and a `query` reads a query or a resource"]
    );
}

#[test]
fn a_query_of_a_subscription_and_a_subscription_of_a_query_are_refused() {
    assert_eq!(
        refused("query Ticks(n)"),
        ["`query Ticks` reads a subscription, and a `query` reads a query or a resource"]
    );
    assert_eq!(
        refused("subscription Found(n)"),
        ["`subscription Found` reads a query, and a `subscription` reads a subscription"]
    );
}

#[test]
fn each_reads_its_own_kind() {
    assert_eq!(refused("query Found(n)"), Vec::<String>::new());
    assert_eq!(refused("subscription Ticks(n)"), Vec::<String>::new());
}
