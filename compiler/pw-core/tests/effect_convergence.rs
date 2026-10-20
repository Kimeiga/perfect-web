//! Inferred effect rows reach every helper before checking or granting authority.

use pw_core::check::check_sources;

fn program(src: &str) -> Vec<(String, String)> {
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
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

/// Each diagnostic on `t.pw`, as its code and the text its primary span
/// covers: where it is reported is part of what is claimed.
fn reported(src: &str) -> Vec<(String, String)> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| {
            ds.into_iter().map(|d| {
                let at = src.get(d.primary_span.clone()).unwrap_or("?").to_string();
                (d.code.to_string(), at)
            })
        })
        .collect()
}

fn required(src: &str, component: &str) -> Vec<String> {
    use pw_core::contract::{Capability, contracts};
    let files = program(src);
    let hirs: Vec<pw_core::hir::Hir> = files
        .iter()
        .map(|(_, s)| pw_core::lower::lower_file(s, &pw_syntax::parse_tree(s).green))
        .collect();
    let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();
    let ws = pw_core::resolve::Workspace::build(&refs);
    let sigs = pw_core::signatures::Signatures::build(&ws, &refs);
    contracts(&refs, &sigs, &ws)
        .into_iter()
        .find(|c| c.component_id == component)
        .unwrap_or_else(|| panic!("no contract for {component}"))
        .required_capabilities
        .iter()
        .map(Capability::name)
        .collect()
}

fn chain(depth: usize, reverse: bool, cycle: bool, leaf: &str, boundary: &str) -> String {
    let mut helpers = (0..depth)
        .map(|n| {
            let expression = if n + 1 < depth {
                format!("hop{}(item)", n + 1)
            } else if cycle {
                format!("if item.name == \"stop\" {{ {leaf} }} else {{ hop0(item) }}")
            } else {
                leaf.to_string()
            };
            format!("fn hop{n}(item: Item) -> String {{\n    {expression}\n}}\n\n")
        })
        .collect::<Vec<_>>();
    if reverse {
        helpers.reverse();
    }
    format!(
        "module t\n\ntype Item = Item {{ name: String }}\n\nfn label(item: Item) -> String !{{ database.read<Item> }}\n    host \"shop:data/items#label\"\n\n{}{boundary}\n",
        helpers.join("")
    )
}

const VIEW: &str = "view Menu(item: Item) !{} { <p>{hop0(item)}</p> }";
const QUERY: &str = "query Answer(item: Item) -> String { hop0(item) }";

fn view_refuses(leaf: &str, cycle: bool) {
    for reverse in [false, true] {
        let source = chain(15, reverse, cycle, leaf, VIEW);
        assert_eq!(
            reported(&source),
            [("PW0401".to_string(), "hop0(item)".to_string())],
            "effect escaped: reverse={reverse}, cycle={cycle}, leaf={leaf}"
        );
        let pure = chain(15, reverse, cycle, "item.name", VIEW);
        assert_eq!(
            reported(&pure),
            [],
            "pure neighbor: reverse={reverse}, cycle={cycle}"
        );
    }
}

#[test]
fn a_deep_direct_helper_chain_is_effectful_in_either_declaration_order() {
    view_refuses("label(item)", false);
}

#[test]
fn a_deep_member_helper_chain_is_effectful_in_either_declaration_order() {
    view_refuses("item.label", false);
}

#[test]
fn recursive_helper_cycles_converge_with_and_without_an_effect() {
    for leaf in ["label(item)", "item.label"] {
        view_refuses(leaf, true);
    }
}

#[test]
fn deep_helper_effects_reach_the_query_contract_in_either_declaration_order() {
    for reverse in [false, true] {
        for cycle in [false, true] {
            for leaf in ["label(item)", "item.label"] {
                let source = chain(15, reverse, cycle, leaf, QUERY);
                assert_eq!(reported(&source), [], "the effectful query is valid");
                assert_eq!(
                    required(&source, "t.Answer"),
                    ["database.read<Item>"],
                    "capability escaped: reverse={reverse}, cycle={cycle}, leaf={leaf}"
                );
            }
            let pure = chain(15, reverse, cycle, "item.name", QUERY);
            assert_eq!(reported(&pure), [], "the pure query is valid");
            assert_eq!(
                required(&pure, "t.Answer"),
                Vec::<String>::new(),
                "pure neighbor gains no capability"
            );
        }
    }
}

/// Timing evidence is opt-in: the ordinary suite has no machine-speed gate.
#[test]
#[ignore = "bounded checker scale smoke; run with --ignored --nocapture"]
fn a_bounded_deep_chain_scale_smoke() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let feed = std::fs::read_to_string(root.join("examples/feed/app.pw")).expect("reference feed");
    let mut sources = vec![("reference_feed".to_string(), feed, 0)];
    for depth in [15, 100, 250] {
        for reverse in [false, true] {
            sources.push((
                format!("chain_{depth}_reverse_{reverse}"),
                chain(depth, reverse, false, "item.label", VIEW),
                1,
            ));
        }
    }
    let mut all_expected = true;
    for (name, source, expected) in sources {
        let files = program(&source);
        let mut times = Vec::new();
        let mut count = 0;
        for _ in 0..6 {
            let started = std::time::Instant::now();
            let findings = check_sources(&files);
            count = findings.iter().map(|(_, ds)| ds.len()).sum::<usize>();
            all_expected &= count == expected;
            times.push(started.elapsed().as_micros());
        }
        times.remove(0);
        times.sort();
        println!(
            "{name}: median_us={}, min_us={}, max_us={}, diagnostics={count}",
            times[2], times[0], times[4]
        );
    }
    assert!(
        all_expected,
        "every measured program must retain its expected diagnostic count"
    );
}
