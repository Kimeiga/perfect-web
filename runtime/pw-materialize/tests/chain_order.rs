//! **A chain is regenerated in its order** (ADR-0277): each materialization
//! after every one it reads, through any number of reads, so none is built
//! from one not yet regenerated.

use pw_materialize::graph::{Edge, EdgeKind, Graph, Node};

fn node(path: &str) -> Node {
    Node {
        path: path.to_string(),
        name: path.to_string(),
        node: "materialization".to_string(),
        params: vec!["id".to_string()],
        partition: Some("public".to_string()),
        privacy: None,
        placement: None,
        regenerate: Some("on_invalidation".to_string()),
        stampede: None,
        fallback: None,
        varies_by: Vec::new(),
    }
}

fn reads(from: &str, to: &str) -> Edge {
    Edge {
        from: from.to_string(),
        to: to.to_string(),
        kind: EdgeKind::Reads,
        key: vec!["id".to_string()],
    }
}

fn graph(edges: Vec<Edge>) -> Graph {
    Graph {
        nodes: ["a.Line", "a.Size", "a.Menu", "a.Top"].map(node).to_vec(),
        edges,
        ..Graph::default()
    }
}

fn order(g: &Graph, paths: &[&str]) -> Vec<String> {
    g.in_dependency_order(&paths.iter().map(|p| p.to_string()).collect::<Vec<_>>())
}

#[test]
fn each_comes_after_what_it_reads() {
    // `Line` reads `Size`, which reads the query `Menu`: `Size` first,
    // though `Line` comes first by its path.
    let g = graph(vec![reads("a.Line", "a.Size"), reads("a.Size", "a.Menu")]);
    assert_eq!(order(&g, &["a.Line", "a.Size"]), ["a.Size", "a.Line"]);
    // Through any number of reads: `Top` reads `Line`, which reads `Size`.
    let g = graph(vec![
        reads("a.Top", "a.Line"),
        reads("a.Line", "a.Size"),
        reads("a.Size", "a.Menu"),
    ]);
    assert_eq!(
        order(&g, &["a.Top", "a.Line", "a.Size"]),
        ["a.Size", "a.Line", "a.Top"]
    );
    // And through one not among those regenerated: `Top` reads `Size` by
    // `Line`, which this event did not reach.
    assert_eq!(order(&g, &["a.Top", "a.Size"]), ["a.Size", "a.Top"]);
}

#[test]
fn the_controls_keep_their_order_by_path() {
    // None reads another: by path.
    let g = graph(vec![reads("a.Size", "a.Menu"), reads("a.Line", "a.Menu")]);
    assert_eq!(order(&g, &["a.Size", "a.Line"]), ["a.Line", "a.Size"]);
    // One alone, and none twice.
    assert_eq!(order(&g, &["a.Size", "a.Size"]), ["a.Size"]);
    // A cycle, refused where it is written (PW5109), is left in path order
    // rather than lost.
    let g = graph(vec![reads("a.Line", "a.Size"), reads("a.Size", "a.Line")]);
    assert_eq!(order(&g, &["a.Size", "a.Line"]), ["a.Line", "a.Size"]);
}
