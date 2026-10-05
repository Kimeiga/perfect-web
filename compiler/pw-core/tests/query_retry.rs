//! **A query's `retry` reaches its runtime as declared** (ADR-0215, the
//! owner's ruling 0089-b; ADR-0210's urgent defect 4).
//!
//! A page's plan carried a query's attempts and nothing else, so `jitter =
//! false` ran with jitter. It carries whether the delays vary now; the cache
//! honours it, and a strategy the runtime would not honour, `fixed`, is no
//! longer one the language has.

use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

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

/// The policy the plan gives the page's one binding, its query declaring
/// `retry`.
fn policy(retry: &str) -> serde_json::Value {
    let src = format!(
        r#"module t

fn read(id: String) -> String !{{ database.read<Note> }}
    host "t:data/notes#read"

type Note = Note {{ text: String }}

public query Note(id: String) -> String
    freshness     30.seconds
    consistency   snapshot
    cache         shared
    key           id
    concurrency   one_per_key
    on_key_change cancel
    timeout       2.seconds
    {retry}
{{
    read(id)
}}

page P(id: String) {{
    route        "/p/{{id}}"
    placement    origin
    cache        private

    let note = query Note(id)

    view {{
        <title>P</title>
        <main><p>{{note}}</p></main>
    }}
}}
"#
    );
    let mut sources = library();
    sources.push(("t.pw".to_string(), src));
    let units: Vec<Unit> = sources
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    let plan = pw_core::build::build(&units)
        .expect("builds")
        .pages
        .into_iter()
        .find(|p| p.page == "t.P")
        .expect("a page")
        .plan
        .expect("planned");
    serde_json::to_value(&plan.bindings[0].policy).expect("encodes")
}

#[test]
fn a_querys_jitter_reaches_its_plan() {
    let jittered = policy("retry         bounded_exponential(max = 3, jitter = true)");
    assert_eq!(
        (jittered["attempts"].clone(), jittered["jitter"].clone()),
        (serde_json::json!(3), serde_json::json!(true))
    );
    // Not varying is not written, and reads as `false`.
    let exact = policy("retry         transport_only(max = 2, jitter = false)");
    assert_eq!(
        (exact["attempts"].clone(), exact.get("jitter").cloned()),
        (serde_json::json!(2), None)
    );
    let once = policy("retry         none");
    assert_eq!(
        (once["attempts"].clone(), once.get("jitter").cloned()),
        (serde_json::json!(1), None)
    );
}
