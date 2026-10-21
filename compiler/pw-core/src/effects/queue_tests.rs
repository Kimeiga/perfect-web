//! Row authority and completed metadata are independent of the queue schedule.
use super::*;

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    row: BTreeSet<Effect>,
    performed: BTreeSet<Effect>,
    sources: Vec<(Effect, String, Via)>,
    calls: Vec<(String, String, Vec<Effect>)>,
    evidence: Vec<(String, String, Vec<crate::provenance::FactId>)>,
}

fn inspect(source: &str) -> (BTreeMap<String, Snapshot>, usize) {
    let hirs = [crate::lower::lower_file(
        source,
        &pw_syntax::parse_tree(source).green,
    )];
    let refs: Vec<&Hir> = hirs.iter().collect();
    let workspace = Workspace::build(&refs);
    let sigs = Signatures::build(&workspace, &refs);
    let mut inference = Inference::new(&sigs, &workspace);
    inference.run(&refs);
    let name = |def: DefId| {
        hirs[def.unit]
            .decl(crate::hir::DeclId(def.decl))
            .name
            .clone()
    };
    let mut snapshots = BTreeMap::new();
    for (id, decl) in hirs[0].all_decls() {
        let Some(body) = decl.body.map(|body| hirs[0].body(body)) else {
            continue;
        };
        let def = DefId {
            unit: 0,
            decl: id.0,
        };
        let found = inference.performed_of(def);
        assert!(
            found.is_some(),
            "every implemented body retains its performed metadata"
        );
        let found = found.unwrap();
        let snapshot = Snapshot {
            row: inference.known[&def].clone(),
            performed: found.effects.clone(),
            sources: found
                .sources
                .iter()
                .map(|s| {
                    (
                        s.effect.clone(),
                        source[s.span.clone()].to_string(),
                        s.via.clone(),
                    )
                })
                .collect(),
            calls: found
                .calls
                .iter()
                .map(|c| {
                    (
                        source[body.expr_span(c.call)].to_string(),
                        name(c.callee),
                        c.effects.clone(),
                    )
                })
                .collect(),
            evidence: found
                .evidence
                .facts()
                .iter()
                .map(|fact| {
                    let kind = match &fact.kind {
                        crate::provenance::FactKind::ResolvedCall { path, to, via } => {
                            format!("{path} -> {} via {via:?}", name(*to))
                        }
                        kind => format!("{kind:?}"),
                    };
                    (
                        kind,
                        source[fact.origin.clone()].to_string(),
                        fact.depends_on.clone(),
                    )
                })
                .collect(),
        };
        snapshots.insert(decl.name.clone(), snapshot);
    }
    (snapshots, inference.body_visits)
}

fn program(mut declarations: Vec<String>, reverse: bool) -> String {
    if reverse {
        declarations.reverse();
    }
    format!(
        "module t\n\ntype Item = Item {{ name: String }}\n\nfn label(i: Item) -> String !{{ database.read<Item> }}\n    host \"shop:data/items#label\"\n\n{}",
        declarations.join("\n\n")
    )
}

fn read_row() -> BTreeSet<Effect> {
    ["database.read<Item>".to_string()].into()
}

#[test]
fn late_rows_refresh_declared_metadata_and_noninvoked_member_values() {
    let declarations = vec![
        "fn first(i: Item) -> String !{ database.read<Item> } { let before = early(i); i.late }"
            .to_string(),
        "fn held(i: Item) -> String { let function = i.decorate; i.name }".to_string(),
        "fn early(i: Item) -> String !{ database.read<Item> } { i.name }".to_string(),
        "fn late(i: Item) -> String { i.label }".to_string(),
        "fn decorate(i: Item, suffix: String) -> String { i.label }".to_string(),
    ];
    let (forward, _) = inspect(&program(declarations.clone(), false));
    let (backward, _) = inspect(&program(declarations, true));
    assert_eq!(
        forward, backward,
        "row, sources, call charges and provenance must all converge"
    );
    let first = &forward["first"];
    assert_eq!(first.row, read_row());
    assert_eq!(first.performed, read_row());
    assert_eq!(
        first
            .sources
            .iter()
            .map(|s| s.1.as_str())
            .collect::<Vec<_>>(),
        ["early(i)", "i.late"]
    );
    assert_eq!(
        first.calls.iter().map(|c| (&c.1, &c.2)).collect::<Vec<_>>(),
        [
            (
                &"early".to_string(),
                &vec!["database.read<Item>".to_string()]
            ),
            (
                &"late".to_string(),
                &vec!["database.read<Item>".to_string()]
            )
        ]
    );
    assert_eq!(
        first
            .evidence
            .iter()
            .filter(|f| f.0.starts_with("Effect {"))
            .map(|f| f.1.as_str())
            .collect::<Vec<_>>(),
        ["i.late"]
    );
    let held = &forward["held"];
    assert_eq!(held.performed, read_row());
    assert_eq!(
        held.sources
            .iter()
            .map(|s| s.1.as_str())
            .collect::<Vec<_>>(),
        ["i.decorate"]
    );
    assert_eq!(
        held.calls,
        [],
        "reading this function value is not an invocation"
    );
    assert_eq!(
        forward["early"].performed,
        BTreeSet::new(),
        "a row permits effects; it does not perform them"
    );
}

#[test]
fn a_self_reader_refreshes_its_own_call_charge_after_row_growth() {
    let (rows, visits) = inspect(&program(
        vec!["fn recur(i: Item) -> String { let again = recur(i); i.label }".to_string()],
        false,
    ));
    assert_eq!(rows["recur"].performed, read_row());
    assert_eq!(
        rows["recur"].calls[0].2,
        ["database.read<Item>"],
        "the self call starts empty and must be revisited"
    );
    assert_eq!(visits, 2);
    let (pure, pure_visits) = inspect(&program(
        vec!["fn recur(i: Item) -> String { let again = recur(i); i.name }".to_string()],
        false,
    ));
    assert_eq!(pure["recur"].performed, BTreeSet::new());
    assert_eq!(pure["recur"].calls[0].2, Vec::<Effect>::new());
    assert_eq!(pure_visits, 1);
}

#[test]
fn single_effect_chains_visit_only_the_bodies_whose_rows_changed() {
    for depth in [15, 100, 250] {
        for mode in ["direct", "member", "value"] {
            let declarations = (0..depth)
                .map(|n| {
                    let next = if n + 1 == depth {
                        "label".to_string()
                    } else {
                        format!("hop{}", n + 1)
                    };
                    let expression = match mode {
                        "member" => format!("i.{next}"),
                        "value" => format!("let f = {next}; f(i)"),
                        _ => format!("{next}(i)"),
                    };
                    format!("fn hop{n}(i: Item) -> String {{ {expression} }}")
                })
                .collect::<Vec<_>>();
            let (forward, visits) = inspect(&program(declarations.clone(), false));
            let (backward, reverse_visits) = inspect(&program(declarations, true));
            assert_eq!(forward, backward, "mode={mode}, depth={depth}");
            assert!(forward.values().all(|body| body.performed == read_row()));
            assert!(
                visits <= 2 * depth && reverse_visits <= 2 * depth,
                "mode={mode}, depth={depth}, visits={visits}/{reverse_visits}"
            );
        }
    }
}

#[test]
fn many_growing_dependencies_enqueue_their_pending_reader_only_once() {
    let depth = 100;
    let body = (0..depth)
        .map(|n| format!("let item{n} = leaf{n}(i);"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut declarations = vec![format!("fn first(i: Item) -> String {{ {body} i.name }}")];
    declarations.extend((0..depth).map(|n| format!("fn leaf{n}(i: Item) -> String {{ i.label }}")));
    let (forward, visits) = inspect(&program(declarations.clone(), false));
    let (backward, reverse_visits) = inspect(&program(declarations, true));
    assert_eq!(forward, backward);
    assert_eq!(forward["first"].sources.len(), depth);
    assert_eq!(forward["first"].calls.len(), depth);
    assert_eq!(
        visits,
        depth + 2,
        "one initial reader visit, one refresh after all dependency changes"
    );
    assert_eq!(reverse_visits, depth + 1);
}

#[test]
fn readers_remain_registered_when_a_dependency_grows_again() {
    let declarations = vec![
        "fn first(i: Item) -> String { second(i) }".to_string(),
        "fn second(i: Item) -> String { let read = fast(i); slow(i) }".to_string(),
        "fn fast(i: Item) -> String { i.label }".to_string(),
        "fn slow(i: Item) -> String { delayed(i) }".to_string(),
        "fn delayed(i: Item) -> String { i.save }".to_string(),
        "fn save(i: Item) -> String !{ database.write<Item> }\n    host \"shop:data/items#save\""
            .to_string(),
    ];
    let (forward, _) = inspect(&program(declarations.clone(), false));
    let (backward, _) = inspect(&program(declarations, true));
    assert_eq!(
        forward, backward,
        "the later write must refresh every earlier reader"
    );
    let both: BTreeSet<Effect> = [
        "database.read<Item>".to_string(),
        "database.write<Item>".to_string(),
    ]
    .into();
    assert_eq!(forward["first"].performed, both);
    assert_eq!(
        forward["first"].calls[0].2,
        ["database.read<Item>", "database.write<Item>"]
    );
    assert_eq!(
        forward["second"]
            .sources
            .iter()
            .map(|s| (s.0.as_str(), s.1.as_str()))
            .collect::<Vec<_>>(),
        [
            ("database.read<Item>", "fast(i)"),
            ("database.write<Item>", "slow(i)")
        ]
    );
}
