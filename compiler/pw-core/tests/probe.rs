#[test]
fn probe() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let p = root
        .join("examples/rejected/R-017-wall-clock-read-in-shared-deterministic-materialization.pw");
    let src = std::fs::read_to_string(&p).unwrap();
    let hir = pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green);
    for (_, d) in hir.all_decls() {
        println!(
            "decl {} kind {:?} policies {:?} effects {:?}",
            d.name,
            d.kind,
            d.policies,
            d.declared_effects
                .as_ref()
                .map(|r| r.iter().map(|e| &e.written).collect::<Vec<_>>())
        );
        if let Some(b) = d.body {
            let body = hir.body(b);
            for e in body.walk() {
                println!("   {e:?} {:?}", body.expr(e));
            }
        }
    }
}
