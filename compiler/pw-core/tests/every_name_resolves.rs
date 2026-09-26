//! **Every name resolves** (ADR-0047).
//!
//! `PW0021` examined calls and qualified paths. A name used as a value, in an
//! expression or in a template, was read as unknown by every analysis and
//! passed `pw check`: `let x = nothing` checked clean. Each test here is one
//! scoping rule, with the control that shows it discriminates.

use pw_core::check::check_sources;

/// The standard library, so `List` and its callbacks are what they are in a
/// real program.
fn std_sources() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/pw-std");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("pw-std")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .map(|p| {
            (
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            )
        })
        .collect();
    out.sort();
    out
}

/// The names `PW0021` reports in `src`, checked with the standard library.
fn unresolved(src: &str) -> Vec<String> {
    let mut files = std_sources();
    files.push(("t.pw".to_string(), src.to_string()));
    let checked = check_sources(&files);
    let (_, diags) = checked.last().expect("t.pw");
    diags
        .iter()
        .filter(|d| d.code == "PW0021")
        .map(|d| {
            d.message
                .trim_end_matches(" does not resolve")
                .trim_matches('`')
                .to_string()
        })
        .collect()
}

#[test]
fn a_value_named_nothing_is_refused() {
    let src = "module m\n\nfn f() -> Int !{} {\n    let x = nothing\n    1\n}\n";
    assert_eq!(unresolved(src), ["nothing"]);

    // Control: the same name, declared as a parameter.
    let bound = src.replace("fn f()", "fn f(nothing: Int)");
    assert_eq!(unresolved(&bound), Vec::<String>::new());
}

#[test]
fn a_let_binds_for_the_statements_after_it_and_not_before() {
    let before =
        "module m\n\nfn f() -> Int !{} {\n    let y = later\n    let later = 2\n    y\n}\n";
    assert_eq!(unresolved(before), ["later"]);

    // Its own initialiser cannot see it either.
    let own = "module m\n\nfn f() -> Int !{} {\n    let x = x\n    x\n}\n";
    assert_eq!(unresolved(own), ["x"]);

    // Control: in order.
    let after = "module m\n\nfn f() -> Int !{} {\n    let later = 2\n    let y = later\n    y\n}\n";
    assert_eq!(unresolved(after), Vec::<String>::new());
}

#[test]
fn a_block_ends_its_bindings() {
    let src = "module m\n\nfn f(c: Bool) -> Int !{} {\n    if c {\n        let inner = 1\n    }\n    inner\n}\n";
    assert_eq!(unresolved(src), ["inner"]);

    let inside = "module m\n\nfn f(c: Bool) -> Int !{} {\n    if c {\n        let inner = 1\n        inner\n    } else {\n        0\n    }\n}\n";
    assert_eq!(unresolved(inside), Vec::<String>::new());
}

#[test]
fn a_lambda_parameter_binds_its_body_only() {
    let src = "module m\n\nimport List\n\nfn f(xs: List<Int>) -> Int !{} {\n    let ys = List.map(xs, fn(y) y + 1)\n    y\n}\n";
    assert_eq!(unresolved(src), ["y"]);

    let inside = "module m\n\nimport List\n\nfn f(xs: List<Int>) -> List<Int> !{} {\n    List.map(xs, fn(y) y + 1)\n}\n";
    assert_eq!(unresolved(inside), Vec::<String>::new());
}

#[test]
fn a_match_arm_binds_its_own_body_only() {
    let src = "module m\n\nfn f(o: Option<Int>) -> Int !{} {\n    match o {\n        Some(v) => v,\n        None => v,\n    }\n}\n";
    assert_eq!(unresolved(src), ["v"]);

    let fine = src.replace("None => v,", "None => 0,");
    assert_eq!(unresolved(&fine), Vec::<String>::new());
}

#[test]
fn a_loop_pattern_binds_its_body_only() {
    let src = "module m\n\nfn f(xs: List<Int>) -> Int !{} {\n    for x in xs {\n        x\n    }\n    x\n}\n";
    assert_eq!(unresolved(src), ["x"]);
}

#[test]
fn a_template_hole_resolves() {
    let src = "module m\n\nview V(name: String) !{} {\n    <p>{nothing.here} {name}</p>\n}\n";
    assert_eq!(unresolved(src), ["nothing"]);
}

#[test]
fn an_each_block_binds_its_item_for_its_children() {
    let src = "module m\n\nview V(items: List<Int>) !{} {\n    <ul>\n        {#each ghosts as g (g)}\n            <li>{g}</li>\n        {/each}\n    </ul>\n}\n";
    assert_eq!(unresolved(src), ["ghosts"], "the source, not the item");

    // After the block, inside the same element: the item was the block's.
    let after = "module m\n\nview V(items: List<Int>) !{} {\n    <ul>\n        {#each items as g (g)}\n            <li>{g}</li>\n        {/each}\n        <li>{g}</li>\n    </ul>\n}\n";
    assert_eq!(unresolved(after), ["g"], "the item is the block's");
}

#[test]
fn a_match_block_arm_binds_for_its_own_arm() {
    let src = "module m\n\nview V(o: Option<String>) !{} {\n    {#match o}\n        {:Some(x)}\n            <p>{x}</p>\n        {:None}\n            <p>{x}</p>\n    {/match}\n}\n";
    assert_eq!(unresolved(src), ["x"], "only the `None` arm's use");
}

#[test]
fn a_nested_template_is_read_in_its_own_scope() {
    // Markup inside a match arm: its holes are the arm's, and the enclosing
    // template holds them too. Walked once, in the arm.
    let src = "module m\n\ntype S = | On(Int) | Off\n\nview V(s: S) !{} {\n    <section>\n        {match s {\n            On(c) => <p>On: {c}</p>,\n            Off => <p>Off</p>,\n        }}\n    </section>\n}\n";
    assert_eq!(unresolved(src), Vec::<String>::new());
}

#[test]
fn a_record_shorthand_reads_scope() {
    // `b` alone is the shorthand for `b: b`. (A literal is a record once a
    // field is written `name:`, so `Point { b }` alone is a name and a block.)
    let src = "module m\n\ntype Point = Point { a: Int, b: Int }\n\nfn f() -> Point !{} {\n    Point { a: 1, b }\n}\n";
    assert_eq!(unresolved(src), ["b"]);

    let bound = src.replace("fn f()", "fn f(b: Int)");
    assert_eq!(unresolved(&bound), Vec::<String>::new());
}

#[test]
fn a_receiver_resolves() {
    let src = "module m\n\nfn f() -> Int !{} {\n    unbound.method()\n}\n";
    assert_eq!(unresolved(src), ["unbound"]);
}

#[test]
fn a_module_member_used_as_a_value_resolves() {
    let src =
        "module m\n\nimport List\n\nfn f() -> Int !{} {\n    let g = List.nothing\n    1\n}\n";
    assert_eq!(unresolved(src), ["List.nothing"]);

    let real = src.replace("List.nothing", "List.length");
    assert_eq!(unresolved(&real), Vec::<String>::new());
}

#[test]
fn a_constructor_and_the_language_values_resolve() {
    let src = "module m\n\ntype S = | On | Off\n\nfn f(b: Bool) -> S !{} {\n    let t = true\n    let n = None\n    if b { On } else { Off }\n}\n";
    assert_eq!(unresolved(src), Vec::<String>::new());
}

#[test]
fn clauses_the_grammar_keeps_as_names_are_syntax() {
    // A component's resource block, a named statement argument, and a view
    // section: the shape A-019 has.
    let src = "module m\n\ncomponent C(center: Int) {\n    placement browser\n\n    let visible = observe intersection(self, threshold = 0.1) -> Bool\n\n    resource map when visible {\n        scope component\n        acquire { center }\n        release(handle) { handle }\n    }\n\n    view { <section aria-label=\"Map\" /> }\n}\n";
    assert_eq!(unresolved(src), Vec::<String>::new());

    // `release(h)` binds `h` in its block and nowhere else.
    let leaked = src.replace(
        "        release(handle) { handle }\n    }\n",
        "        release(handle) { handle }\n    }\n    let after = handle\n",
    );
    assert_eq!(unresolved(&leaked), ["handle"]);
}

#[test]
fn a_clause_word_alone_is_a_name() {
    // A clause needs a value on its line or a block after it. A lone word
    // spelled like one is an ordinary use.
    let src = "module m\n\nfn f() -> Int !{} {\n    let x = 1\n    scope\n    x\n}\n";
    assert_eq!(unresolved(src), ["scope"]);
}

#[test]
fn derived_is_one_expression() {
    // `let total = derived xs |> ..` was `let total = derived` and a discarded
    // statement, so `total` named nothing.
    let src = "module m\n\nimport List\n\nfn f(xs: List<Int>) -> Int !{} {\n    let total = derived List.length(xs)\n    total\n}\n";
    assert_eq!(unresolved(src), Vec::<String>::new());

    let parse = pw_syntax::parse_tree(src);
    assert!(parse.errors.is_empty(), "{:?}", parse.errors);
    let hir = pw_core::lower::lower_file(src, &parse.green);
    let (_, f) = hir.all_decls().find(|(_, d)| d.name == "f").expect("f");
    let body = hir.body(f.body.expect("a body"));
    let pw_core::hir::Expr::Block { stmts } = body.expr(body.root) else {
        panic!("a block body")
    };
    assert_eq!(
        stmts.len(),
        2,
        "the let and its use, and no stray statement"
    );
    let pw_core::hir::Expr::Let {
        init: Some(init), ..
    } = body.expr(stmts[0])
    else {
        panic!("a let")
    };
    assert!(
        matches!(body.expr(*init), pw_core::hir::Expr::Keyword { keyword, args, .. }
            if keyword == "derived" && args.len() == 1),
        "the value is `derived` of the call"
    );

    // And it cannot name a binding, as no statement keyword can (PW0013).
    let named = "module m\n\nfn f() -> Int !{} {\n    let derived = 1\n    1\n}\n";
    let errors = pw_syntax::parse_tree(named).errors;
    assert!(
        errors.iter().any(|e| e.code == "PW0013"),
        "{:?}",
        errors.iter().map(|e| e.code).collect::<Vec<_>>()
    );
}

#[test]
fn a_stream_part_binds_its_name_for_its_children() {
    let src = "module m\n\nquery Q(id: Int) -> List<Int> !{} { todo }\n\nview V(id: Int) !{} {\n    <stream query={Q(id)}>\n        <placeholder><p>Loading</p></placeholder>\n        <ready as={items}><p>{items}</p></ready>\n        <failed as={e}><p>Failed</p></failed>\n    </stream>\n}\n";
    assert_eq!(unresolved(src), Vec::<String>::new());

    let outside = src.replace("    </stream>\n", "    </stream>\n    <p>{items}</p>\n");
    assert_eq!(unresolved(&outside), ["items"]);
}

#[test]
fn a_nested_declaration_sees_its_parents_parameters() {
    let src = "module m\n\ncomponent C(store_id: Int) {\n    fn load() -> Int !{} {\n        store_id\n    }\n\n    view { <p>Store</p> }\n}\n";
    assert_eq!(unresolved(src), Vec::<String>::new());

    let other = src.replace("        store_id\n", "        other_id\n");
    assert_eq!(unresolved(&other), ["other_id"]);
}

#[test]
fn a_spawn_forms_words_are_not_uses() {
    let src = "module m\n\ncomponent C() {\n    fn go() -> () !{ task.spawn } {\n        task.spawn(scope = component) { 1 }\n    }\n\n    view { <p>Go</p> }\n}\n";
    assert_eq!(unresolved(src), Vec::<String>::new());
}
