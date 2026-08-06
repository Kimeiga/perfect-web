//! Lowering the corpus into HIR.
//!
//! The properties here are the ones a checker depends on. Each has a control:
//! four of the measurement bugs in `docs/RISK_QUEUE.md` produced favourable
//! results while measuring nothing, and a lowering pass that silently drops a
//! subtree fails in exactly that shape — every checker downstream then reports
//! "no violations" on a body it never saw.

use pw_core::hir::*;
use pw_core::lower::lower_file;
use pw_syntax::kind::SyntaxKind as K;
use pw_syntax::parse_tree;

fn corpus() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .canonicalize()
        .expect("examples/ exists");
    let mut out = Vec::new();
    for dir in ["accepted", "rejected"] {
        for e in std::fs::read_dir(root.join(dir)).expect("corpus dir") {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn lower(src: &str) -> Hir {
    let p = parse_tree(src);
    assert!(p.ok(), "test source must parse: {:?}", p.errors);
    lower_file(src, &p.green)
}

/// Every expression in every body, with its span.
fn all_exprs(hir: &Hir) -> Vec<(&Expr, Span)> {
    hir.bodies
        .iter()
        .flat_map(|(_, b, _)| b.exprs().map(|(_, e, s)| (e, s.clone())))
        .collect()
}

#[test]
fn the_generic_callback_case_survives_lowering() {
    // docs/NEXT.md calls this load-bearing: an effect reaching the program
    // through a callback the callee knows nothing about. If the lambda body
    // is misassociated or dropped, an effect checker inspects the wrong
    // expression and still reports something plausible.
    let src = "fn f(items: List<Id>) -> List<Store> !{ database.read } {\n    List.map(items, item => database.read(item.id))\n}\n";
    let hir = lower(src);

    let (_, decl) = hir.all_decls().next().expect("one declaration");
    let body = hir.body(decl.body.expect("a body"));

    // The lambda must be an argument of the call, not a sibling of it.
    let (call_id, args) = body
        .walk()
        .into_iter()
        .find_map(|id| match body.expr(id) {
            Expr::Call { args, .. } if args.len() == 2 => Some((id, args.clone())),
            _ => None,
        })
        .expect("the two-argument call to List.map");
    let lambda_id = args[1].value;
    assert!(
        matches!(body.expr(lambda_id), Expr::Lambda { .. }),
        "the second argument must be the lambda, got {:?}",
        body.expr(lambda_id)
    );
    assert!(
        body.children(call_id).contains(&lambda_id),
        "the lambda must be reachable as a child of the call"
    );

    // And the effectful call inside the lambda must be reachable from it, or
    // an effect walk starting at the body root would never find it.
    let Expr::Lambda { body: inner, .. } = body.expr(lambda_id) else {
        unreachable!()
    };
    let inner_calls: Vec<String> = std::iter::once(*inner)
        .chain(body.children(*inner))
        .filter_map(|id| match body.expr(id) {
            Expr::Call { callee, .. } => Some(render_path(body, *callee)),
            _ => None,
        })
        .collect();
    assert!(
        inner_calls.iter().any(|c| c == "database.read"),
        "the lambda body must contain the database.read call, saw {inner_calls:?}"
    );

    // Control: the same walk on a body WITHOUT the callback must not find it.
    let clean = lower("fn f() -> Int !{} {\n    List.map(items, item => item.id)\n}\n");
    let (_, d2) = clean.all_decls().next().unwrap();
    let b2 = clean.body(d2.body.unwrap());
    assert!(
        !b2.walk().iter().any(|id| matches!(
            b2.expr(*id),
            Expr::Call { callee, .. } if render_path(b2, *callee) == "database.read"
        )),
        "the control body must not report a database call"
    );
}

/// `a.b.c` as written, reassembled from nested `Field` nodes.
fn render_path(body: &Body, id: ExprId) -> String {
    match body.expr(id) {
        Expr::Name(n) => n.clone(),
        Expr::Field { base, name } => format!("{}.{}", render_path(body, *base), name),
        _ => String::new(),
    }
}

#[test]
fn a_dotted_path_lowers_to_nested_fields_not_one_name() {
    // ADR-0014: lowering does not decide whether `Stores.get` is a module path
    // or a field access on a local. That is name resolution's call.
    let hir = lower("fn f() !{} {\n    Stores.get(id)\n}\n");
    let (_, d) = hir.all_decls().next().unwrap();
    let b = hir.body(d.body.unwrap());
    let field = b
        .walk()
        .into_iter()
        .find(|id| matches!(b.expr(*id), Expr::Field { .. }))
        .expect("a field access");
    assert_eq!(render_path(b, field), "Stores.get");
    let Expr::Field { base, .. } = b.expr(field) else {
        unreachable!()
    };
    assert!(matches!(b.expr(*base), Expr::Name(n) if n == "Stores"));
}

#[test]
fn a_resumable_handler_keeps_its_capture_descriptor() {
    // `resumable(captures = { item }) => ..` is call-shaped in parameter
    // position. It was silently dropped until the coverage property below
    // caught it — and it is exactly what R-010 (a non-serializable capture)
    // and R-030 (private data in a resume manifest) must read.
    let src = "view V() !{} {\n    <button on:press={resumable(captures = { item }) => add(item)} />\n}\n";
    let hir = lower(src);
    let (_, d) = hir.all_decls().next().unwrap();
    let b = hir.body(d.body.unwrap());

    let (desc, body_id) = b
        .walk()
        .into_iter()
        .find_map(|id| match b.expr(id) {
            Expr::Lambda {
                descriptor: Some(d),
                body,
                ..
            } => Some((*d, *body)),
            _ => None,
        })
        .expect("a lambda carrying a descriptor");

    // The span stops at the descriptor: trailing trivia belongs to the
    // lambda that follows, not to the call.
    assert_eq!(&src[b.expr_span(desc)], "resumable(captures = { item })");
    let Expr::Call { callee, args } = b.expr(desc) else {
        panic!("the descriptor must stay a call: {:?}", b.expr(desc))
    };
    assert_eq!(render_path(b, *callee), "resumable");
    assert_eq!(args[0].name.as_deref(), Some("captures"));
    // The handler body is still reachable — the descriptor did not replace it.
    assert!(matches!(b.expr(body_id), Expr::Call { .. }));

    // Control: an ordinary lambda has no descriptor.
    let plain = lower("fn f() !{} { g(x => x) }\n");
    let (_, pd) = plain.all_decls().next().unwrap();
    let pb = plain.body(pd.body.unwrap());
    assert!(pb.walk().iter().any(|id| matches!(
        pb.expr(*id),
        Expr::Lambda {
            descriptor: None,
            ..
        }
    )));
}

#[test]
fn every_span_points_inside_its_own_source() {
    // A span that is out of range, or belongs to a different file, produces a
    // diagnostic that underlines the wrong text — or panics the renderer.
    for path in corpus() {
        let src = std::fs::read_to_string(&path).expect("read");
        let p = parse_tree(&src);
        let hir = lower_file(&src, &p.green);
        let name = path.file_name().unwrap().to_string_lossy();

        for (_, body, _) in hir.bodies.iter() {
            for (id, expr, span) in body.exprs() {
                assert!(
                    span.end <= src.len() && span.start <= span.end,
                    "{name}: {expr:?} has span {span:?} outside a {}-byte file",
                    src.len()
                );
                // Charter §16.3: a diagnostic needs an origin span. An id that
                // cannot produce one is unusable, so the accessor must agree.
                assert_eq!(body.expr_span(id), *span);
            }
        }
    }
}

#[test]
fn no_call_lambda_or_match_in_the_tree_is_lost_in_lowering() {
    // The coverage property. A lowering that quietly skipped a node kind would
    // pass every test above — they all assert what IS there, never what is
    // missing. This asserts the converse across all 68 corpus files.
    let mut missing: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for path in corpus() {
        let src = std::fs::read_to_string(&path).expect("read");
        let p = parse_tree(&src);
        let hir = lower_file(&src, &p.green);
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        let lowered: std::collections::HashSet<(usize, usize)> = all_exprs(&hir)
            .iter()
            .map(|(_, s)| (s.start, s.end))
            .collect();

        for node in p.green.descendants() {
            if !matches!(
                node.kind(),
                K::CallExpr | K::LambdaExpr | K::MatchExpr | K::FieldExpr
            ) {
                continue;
            }
            // Nodes inside a declaration signature (a default value in a
            // parameter list) are not part of any body.
            if node.ancestors().all(|a| a.kind() != K::Body) {
                continue;
            }
            checked += 1;
            let r = node.text_range();
            let key = (usize::from(r.start()), usize::from(r.end()));
            if !lowered.contains(&key) {
                missing.push(format!("{name}: {:?} at {key:?}", node.kind()));
            }
        }
    }

    assert!(checked > 200, "expected a real sample, checked {checked}");
    assert!(
        missing.is_empty(),
        "{} of {checked} expression nodes never reached HIR:\n{}",
        missing.len(),
        missing
            .iter()
            .take(15)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_allocated_expression_is_reachable_from_the_root() {
    // Orphans mean a subtree was lowered but never attached. A checker that
    // walks from the root would miss it, while a test that iterates the arena
    // would not notice — so this compares the two.
    for path in corpus() {
        let src = std::fs::read_to_string(&path).expect("read");
        let p = parse_tree(&src);
        let hir = lower_file(&src, &p.green);
        let name = path.file_name().unwrap().to_string_lossy();

        for (bi, body, _) in hir.bodies.iter() {
            let reachable: std::collections::HashSet<ExprId> = body.walk().into_iter().collect();
            let orphans: Vec<_> = body
                .exprs()
                .filter(|(id, _, _)| !reachable.contains(id))
                .map(|(_, e, s)| format!("{e:?} at {s:?}"))
                .collect();
            assert!(
                orphans.is_empty(),
                "{name} body {bi}: {} orphaned expressions:\n{}",
                orphans.len(),
                orphans
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}

#[test]
fn declared_effect_rows_reach_hir_with_their_spans() {
    let src = "fn load() -> Store !{ database.read<Stores>, trace } {\n    Stores.get(id)\n}\n";
    let hir = lower(src);
    let (_, d) = hir.all_decls().next().unwrap();
    let row = d.declared_effects.as_ref().expect("a declared row");
    assert_eq!(
        row.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
        ["database.read", "trace"],
        "the type argument is not part of the effect's identity"
    );
    // The span must underline the effect, not the whole row.
    assert_eq!(&src[row[0].span.clone()], "database.read<Stores>");

    // `!{}` is an explicit claim of purity and must not be confused with an
    // absent row: one is "I declare nothing", the other is "I declare none".
    let pure = lower("fn f() -> Int !{} { 1 }\n");
    let (_, pd) = pure.all_decls().next().unwrap();
    assert_eq!(pd.declared_effects.as_deref(), Some(&[][..]));

    let none = lower("component C() {\n    placement browser\n}\n");
    let (_, nd) = none.all_decls().next().unwrap();
    assert!(nd.declared_effects.is_none());
}

#[test]
fn a_nested_fn_becomes_a_child_declaration_with_its_own_body() {
    let src = "component C() {\n    placement browser\n\n    fn load() -> Store !{ database.read } {\n        Stores.get(id)\n    }\n}\n";
    let hir = lower(src);

    let (_, comp) = hir
        .all_decls()
        .find(|(_, d)| d.kind == DeclKind::Component)
        .expect("the component");
    assert_eq!(comp.children.len(), 1, "the nested fn is a child decl");

    let inner = hir.decl(comp.children[0]);
    assert_eq!(inner.name, "load");
    assert_eq!(inner.kind, DeclKind::Fn);
    assert_eq!(
        inner
            .declared_effects
            .as_ref()
            .map(|r| r.iter().map(|e| e.path.clone()).collect::<Vec<_>>()),
        Some(vec!["database.read".to_string()]),
        "the row belongs to the nested fn, not the component"
    );
    assert!(inner.body.is_some(), "and it has its own body");
}

#[test]
fn named_arguments_keep_their_names() {
    // `retry transport_only(max = 2, jitter = true)` — the rule that rejects an
    // unbounded retry reads `max`, so losing the name loses the rule.
    let hir = lower("fn f() !{} {\n    g(1, max = 2, jitter: true)\n}\n");
    let (_, d) = hir.all_decls().next().unwrap();
    let b = hir.body(d.body.unwrap());
    let args = b
        .walk()
        .into_iter()
        .find_map(|id| match b.expr(id) {
            Expr::Call { args, .. } if args.len() == 3 => Some(args.clone()),
            _ => None,
        })
        .expect("the three-argument call");
    let names: Vec<Option<&str>> = args.iter().map(|a| a.name.as_deref()).collect();
    assert_eq!(names, [None, Some("max"), Some("jitter")]);
}

#[test]
fn lowering_is_total_on_a_body_it_cannot_fully_parse() {
    // ADR-0014 property 4. A file with one broken expression must still yield
    // a walkable body for everything else, or one typo blinds every checker.
    let src = "fn f() !{} {\n    let a = @@@\n    database.read(id)\n}\n";
    let p = parse_tree(src);
    assert!(!p.ok(), "this source is meant to be broken");
    let hir = lower_file(src, &p.green);

    let (_, d) = hir.all_decls().next().expect("the fn still lowers");
    let b = hir.body(d.body.expect("with a body"));
    let paths: Vec<String> = b
        .walk()
        .into_iter()
        .filter_map(|id| match b.expr(id) {
            Expr::Call { callee, .. } => Some(render_path(b, *callee)),
            _ => None,
        })
        .collect();
    assert!(
        paths.iter().any(|p| p == "database.read"),
        "the good expression after the broken one must survive, saw {paths:?}"
    );
    assert!(
        b.walk().iter().any(|id| matches!(b.expr(*id), Expr::Error)),
        "and the broken one must be recorded as an error, not dropped"
    );
}

#[test]
fn the_whole_corpus_lowers_without_panicking() {
    let mut bodies = 0;
    let mut exprs = 0;
    for path in corpus() {
        let src = std::fs::read_to_string(&path).expect("read");
        let p = parse_tree(&src);
        let hir = lower_file(&src, &p.green);
        assert!(
            hir.modules.len() == 1,
            "{:?} must yield one module",
            path.file_name().unwrap()
        );
        bodies += hir.bodies.len();
        exprs += all_exprs(&hir).len();
    }
    // Asserted, not printed: a lowering that produced almost nothing would
    // otherwise satisfy every property above vacuously.
    assert!(
        bodies >= 68,
        "expected at least one body per file, got {bodies}"
    );
    assert!(
        exprs >= 1000,
        "expected a substantial HIR, got {exprs} expressions"
    );
}
