//! **A materialization may read another** (ADR-0255, ADR-0195's ruling 10).
//!
//! "A materialization may read another. The build refuses a cycle, and
//! invalidation propagates transitively. A feed needs this." Until ADR-0255
//! a `depends_on` naming a materialization was PW5103, a clause naming
//! another kind (ADR-0088). Now it is a read: its arguments are checked
//! against the materialization's parameters, a cycle of them is refused
//! (PW5109), a shared one reading a private one is refused (PW5101), and
//! what one reads, the one reading it reads too. A `depends_on` naming no
//! node of the graph, a function or a view, was taken as it was, and is
//! refused. Each test states one case, with its control.

use pw_core::check::check_sources;

fn files(dir: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            (p.display().to_string(), src)
        })
        .collect()
}

/// What `app.pw` reports, with the platform and the store's domain and
/// library: each code and message.
fn reported(src: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut program = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        program.extend(files(d));
    }
    program.push((
        "examples/domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"),
    ));
    program.extend(files("examples/lib"));
    program.push(("app.pw".to_string(), src.to_string()));
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A materialization `name`, private, reading `reads`.
fn private(name: &str, reads: &str) -> String {
    format!(
        "materialize {name}(session: Session<SessionId>) {{\n    \
         placement      origin\n    partition      private\n    \
         depends_on     {reads}\n    invalidates_on CartChanged(session)\n    \
         regenerate     on_invalidation\n    stampede       single_flight\n    \
         fallback       last_known_good\n}}\n\n"
    )
}

/// A module with `decls` over the store's cart.
fn module(decls: &str) -> String {
    format!(
        "module m\n\nimport Resources.{{ Cart }}\nimport Events.{{ CartChanged }}\n\
         import capability.{{ Session, SessionId }}\n\n{decls}"
    )
}

#[test]
fn a_materialization_may_read_another() {
    let src = module(&format!(
        "{}{}",
        private("CartSummary", "Cart(session)"),
        private("CartBadge", "CartSummary(session)")
    ));
    assert_eq!(reported(&src), Vec::<String>::new());
}

#[test]
fn what_it_gives_the_one_it_reads_is_checked() {
    // `CartSummary` takes a session; `1` is an `Int`.
    let src = module(&format!(
        "{}{}",
        private("CartSummary", "Cart(session)"),
        private("CartBadge", "CartSummary(1)")
    ));
    let f = reported(&src);
    assert!(
        !f.is_empty() && f.iter().all(|d| d.starts_with("PW06")),
        "{f:#?}"
    );
}

#[test]
fn a_materialization_that_reads_itself_is_refused() {
    // Through another, reported once, by the first in the cycle.
    let src = module(&format!(
        "{}{}",
        private("Alpha", "Beta(session)"),
        private("Beta", "Alpha(session)")
    ));
    assert_eq!(
        reported(&src),
        vec!["PW5109 `Alpha` depends on itself, through `Beta`".to_string()]
    );
    // And directly.
    let src = module(&private("Alone", "Alone(session)"));
    assert_eq!(
        reported(&src),
        vec!["PW5109 `Alone` depends on itself".to_string()]
    );
}

#[test]
fn a_shared_materialization_reads_no_private_one() {
    let shared = |reads: &str| {
        format!(
            "materialize Strip(session: Session<SessionId>) {{\n    \
             placement      edge\n    partition      public\n    \
             depends_on     {reads}\n    invalidates_on CartChanged(session)\n    \
             regenerate     on_invalidation\n}}\n\n"
        )
    };
    let src = module(&format!(
        "{}{}",
        private("CartSummary", "Cart(session)"),
        shared("CartSummary(session)")
    ));
    assert_eq!(
        reported(&src),
        vec![
            "PW5101 `Strip` is materialized into a shared entry and depends on \
             `CartSummary`, which is not materialized `partition public`"
                .to_string()
        ]
    );
}

#[test]
fn a_clause_naming_no_resource_is_refused() {
    // `helper` is a function: an edge to it would carry nothing, and until
    // ADR-0255 the clause was taken as it was.
    let src = module(&format!(
        "fn helper(session: Session<SessionId>) -> Int !{{}} {{\n    0\n}}\n\n{}",
        private("CartSummary", "helper(session)")
    ));
    assert_eq!(
        reported(&src),
        vec![
            "PW5103 `CartSummary` depends on `m.helper`, which is not a resource or a \
             materialization"
                .to_string()
        ]
    );
}

#[test]
fn the_key_audit_counts_what_a_materialization_read_separates() {
    use pw_core::graph::Graph;
    use pw_core::hir::Hir;
    let sources = ["module page\n\n\
         materialize Base(store: Int) {\n    placement      edge\n    \
         partition      public\n    regenerate     on_invalidation\n    \
         locale         included_in_key\n}\n\n\
         materialize Top(store: Int) {\n    placement      edge\n    \
         partition      public\n    depends_on     Base(store)\n    \
         regenerate     on_invalidation\n}\n"];
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| pw_core::lower::lower_file(s, &pw_syntax::parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = pw_core::resolve::Workspace::build(&refs);
    let g = Graph::build(&refs, &ws);
    // `Base` separates its entries by locale, and `Top` does not: one
    // document of `Top` would serve every locale of `Base`.
    let audit = g.key_audit("page.Top").expect("Top is in the graph");
    assert_eq!(
        audit
            .gaps
            .iter()
            .map(|g| (g.resource.as_str(), g.component.as_str()))
            .collect::<Vec<_>>(),
        vec![("Base", "locale")]
    );
    // The control: `Base`'s own store, which `Top` passes, is no gap.
    assert!(audit.gaps.iter().all(|g| g.component != "store"));
}

/// `CartSummary` reading the held cart, and `CartBadge` reading it.
fn chain() -> [String; 2] {
    [
        private("CartSummary", "Held(session)"),
        private("CartBadge", "CartSummary(session)"),
    ]
}

/// A module with a session's held cart, the materializations `built`, and a
/// command clearing the cart with `clauses`.
fn wiping(built: &str, clauses: &str) -> String {
    format!(
        "module m\n\nimport Carts\nimport Events.{{ CartChanged }}\n\
         import capability.{{ Session, SessionId }}\nimport context.{{ current_session }}\n\
         import domain.{{ Cart, CartError }}\n\n\
         session query Held(session: Session<SessionId>) -> Result<Cart, CartError>\n    \
         freshness     0.seconds\n    consistency   read_your_writes\n    \
         cache         private\n    key           session\n    \
         concurrency   one_per_key\n    on_key_change cancel\n{{\n    \
         Carts.current(session)\n}}\n\n{built}\
         command wipe() -> Result<(), CartError>\n    requires SignedIn\n{clauses}{{\n    \
         let _cleared = Carts.clear(current_session())\n    Ok(())\n}}\n"
    )
}

#[test]
fn a_write_reaches_what_reads_a_materialization_it_changes() {
    // `CartBadge` reads `CartSummary`, which reads `Held`, which reads the
    // carts `wipe` writes: a cached reader of them, one level further than
    // the check looked until ADR-0255.
    assert_eq!(
        reported(&wiping(&chain().concat(), "")),
        vec![
            "PW5106 `wipe` writes `Carts` and does not invalidate `Held`, which reads it"
                .to_string(),
            "PW5106 `wipe` writes `Carts`, and no event it emits reaches `CartSummary`, \
             which is built from `Held`"
                .to_string(),
            "PW5106 `wipe` writes `Carts`, and no event it emits reaches `CartBadge`, \
             which is built from `CartSummary`"
                .to_string(),
        ]
    );
    // The control: the query invalidated, and an event both listen for.
    assert_eq!(
        reported(&wiping(
            &chain().concat(),
            "    invalidates Held(current_session())\n    emits CartChanged(current_session())\n"
        )),
        Vec::<String>::new()
    );
}

#[test]
fn a_reader_declared_before_what_it_reads_reads_it_too() {
    // Each round reads what the last made known: `CartBadge`, declared
    // first, learns in the second what `CartSummary` reads.
    let [summary, badge] = chain();
    let found = reported(&wiping(&format!("{badge}{summary}"), ""));
    assert!(
        found.iter().any(|d| d.contains("reaches `CartBadge`")),
        "{found:#?}"
    );
}

#[test]
fn what_a_materialization_reads_itself_passes_to_its_readers() {
    // `CartSummary` reads the carts in its own statements, and depends on
    // nothing: `CartBadge`, built from it, reads them too.
    let src = format!(
        "module m\n\nimport Carts\nimport Events.{{ CartChanged }}\n\
         import capability.{{ Session, SessionId }}\nimport context.{{ current_session }}\n\
         import domain.{{ CartError }}\n\n\
         materialize CartSummary(session: Session<SessionId>) {{\n    \
         placement      origin\n    partition      private\n    \
         invalidates_on CartChanged(session)\n    regenerate     on_invalidation\n    \
         let held = Carts.current(session)\n}}\n\n{}\
         command wipe() -> Result<(), CartError>\n    requires SignedIn\n{{\n    \
         let _cleared = Carts.clear(current_session())\n    Ok(())\n}}\n",
        private("CartBadge", "CartSummary(session)")
    );
    let found = reported(&src);
    assert!(
        found.iter().any(|d| d.contains("reaches `CartBadge`")),
        "{found:#?}"
    );
}
