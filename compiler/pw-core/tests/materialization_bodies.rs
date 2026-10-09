//! **A materialization is a value its body derives** (ADR-0273, ruling 10's
//! last part). A `materialize` that declares its type derives it in its
//! body, reading what it depends on as a page reads a resource, `query
//! R(..)`; its body is held to its type, and what it reads to what it
//! declares it depends on. A page reads one as it reads a query. Until
//! ADR-0273 a materialization had no body, no type and no generator, and no
//! page could read one. Each test states one case, with its control.

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

/// The store's library, `decls` declared over it.
fn module(decls: &str) -> String {
    format!(
        "module m\n\nimport List\nimport Resources.{{ Store, Menu }}\n\
         import Events.{{ MenuChanged, StoreChanged }}\nimport domain.{{ StoreId }}\n\n\
         type MenuCount = MenuCount {{ name: String, items: Int }}\n\n{decls}"
    )
}

/// A public materialization `name(id: StoreId)` of type `ty`, `more` clauses
/// beside its own, whose body is `body`.
fn derived(name: &str, ty: &str, more: &str, body: &str) -> String {
    format!(
        "materialize {name}(id: StoreId) -> {ty} {{\n    \
         placement      edge\n    partition      public\n{more}    \
         invalidates_on MenuChanged(id), StoreChanged(id)\n    \
         regenerate     on_invalidation\n    stampede       single_flight\n    \
         fallback       last_known_good\n\n    {body}\n}}\n\n"
    )
}

const SIZED: &str = "let store = query Store(id)\n    let menu = query Menu(id)\n    \
                     MenuCount { name: store.name, items: List.length(menu) }";

#[test]
fn a_materializations_body_is_held_to_its_type() {
    assert_eq!(
        reported(&module(&derived("MenuSize", "MenuCount", "", SIZED))),
        Vec::<String>::new()
    );
    // A body of another type is refused, as a query's is.
    let found = reported(&module(&derived(
        "MenuSize",
        "MenuCount",
        "",
        "let store = query Store(id)\n    store.name",
    )));
    assert!(
        found
            .iter()
            .any(|d| d.contains("MenuCount") && d.contains("String")),
        "{found:#?}"
    );
}

#[test]
fn what_it_depends_on_is_stated_once_by_reading_it() {
    let found = reported(&module(&derived(
        "MenuSize",
        "MenuCount",
        "    depends_on     Store(id), Menu(id)\n",
        SIZED,
    )));
    assert_eq!(
        found,
        [
            "PW5110 `MenuSize` derives its value, and states what it depends on by reading it: \
          `depends_on` states it again"
        ]
    );
    // The control: a fragment the host renders keeps its `depends_on`.
    let fragment = "materialize MenuFragment(id: StoreId) {\n    placement      edge\n    \
                    partition      public\n    depends_on     Store(id), Menu(id)\n    \
                    invalidates_on MenuChanged(id), StoreChanged(id)\n    \
                    regenerate     on_invalidation\n    stampede       single_flight\n    \
                    fallback       last_known_good\n}\n";
    assert_eq!(reported(&module(fragment)), Vec::<String>::new());
}

#[test]
fn one_that_derives_its_value_reads_another_and_a_cycle_of_reads_is_refused() {
    // A chain: the line reads the size, which reads the store and its menu.
    let line = derived(
        "MenuLine",
        "String",
        "",
        "let size = query MenuSize(id)\n    \"{size.name}: {size.items}\"",
    );
    let chain = format!("{}{line}", derived("MenuSize", "MenuCount", "", SIZED));
    assert_eq!(reported(&module(&chain)), Vec::<String>::new());
    // What it reads has the type the other declares: a field it has not is
    // refused.
    let wrong = derived(
        "MenuLine",
        "String",
        "",
        "let size = query MenuSize(id)\n    size.label",
    );
    let found = reported(&module(&format!(
        "{}{wrong}",
        derived("MenuSize", "MenuCount", "", SIZED)
    )));
    assert!(found.iter().any(|d| d.contains("label")), "{found:#?}");
    // Its reads are the graph's edges: two that read each other are a cycle.
    let cycle = format!(
        "{}{}",
        derived("Left", "Int", "", "let r = query Right(id)\n    r"),
        derived("Right", "Int", "", "let l = query Left(id)\n    l"),
    );
    let found = reported(&module(&cycle));
    assert!(found.iter().any(|d| d.starts_with("PW5109")), "{found:#?}");
}

#[test]
fn a_fragment_the_host_renders_is_no_value_to_read() {
    let fragment = "materialize MenuFragment(id: StoreId) {\n    placement      edge\n    \
                    partition      public\n    depends_on     Store(id), Menu(id)\n    \
                    invalidates_on MenuChanged(id), StoreChanged(id)\n    \
                    regenerate     on_invalidation\n    stampede       single_flight\n    \
                    fallback       last_known_good\n}\n\n";
    let reads = derived("Reader", "Int", "", "let f = query MenuFragment(id)\n    1");
    let found = reported(&module(&format!("{fragment}{reads}")));
    assert!(
        found.iter().any(|d| d.starts_with("PW5108")
            && d.contains("`query MenuFragment` reads a materialization that declares no type")),
        "{found:#?}"
    );
}

/// **A page reads a public one, which the host keeps** (ADR-0277), as it
/// reads a query. Until ADR-0277 a page reading one was refused, the host
/// serving none.
#[test]
fn a_page_reads_a_public_one_the_host_keeps() {
    let page = "page Sized(id: StoreId) {\n    route \"/sized/{id}\"\n\n    \
                let size = query MenuSize(id)\n\n    view {\n        <title>Sized</title>\n        \
                <main><p>{size.items}</p></main>\n    }\n}\n";
    let public = derived("MenuSize", "MenuCount", "", SIZED);
    assert_eq!(
        reported(&module(&format!("{public}{page}"))),
        Vec::<String>::new()
    );
    // The controls: a private one, which the host keeps for no reader yet,
    let private = public.replace("partition      public", "partition      private");
    assert_ne!(private, public);
    let found = reported(&module(&format!("{private}{page}")));
    assert!(
        found.iter().any(|d| d.starts_with("PW5108")
            && d.contains("`query MenuSize` reads a materialization private to its reader")),
        "{found:#?}"
    );
    // and a query, which reads what a materialization reads, not what it
    // derives (ADR-0210).
    let query = "public query Twice(id: StoreId) -> Int\n    freshness 30.seconds\n    \
                 cache shared\n{\n    let size = query MenuSize(id)\n    size.items\n}\n\n";
    let found = reported(&module(&format!("{public}{query}")));
    assert!(
        found.iter().any(|d| d.starts_with("PW5108")
            && d.contains("which a query does not read: a page reads one")),
        "{found:#?}"
    );
}

#[test]
fn a_body_begins_where_the_clauses_end_whatever_it_begins_with() {
    // Inside a block a clause's value ended at a known head only, so a body
    // that began with a name, a literal or a constructor was the last
    // clause's value: `regenerate on_invalidation` took `n` as
    // `on_invalidation n`.
    for (ty, body) in [
        ("StoreId", "id"),
        ("Int", "1"),
        ("String", "\"a menu\""),
        ("MenuCount", "MenuCount { name: \"a menu\", items: 0 }"),
        ("Int", "List.length([1, 2])"),
    ] {
        let src = module(&format!(
            "materialize Bare(id: StoreId) -> {ty} {{\n    placement      edge\n    \
             partition      public\n    invalidates_on MenuChanged(id)\n    \
             regenerate     on_invalidation\n    {body}\n}}\n"
        ));
        assert_eq!(reported(&src), Vec::<String>::new(), "{body}");
    }
    // The control: a value whose line ends wanting more goes on to the next.
    let src = module(
        "materialize Bare(id: StoreId) -> Int {\n    placement      edge\n    \
         partition      public\n    invalidates_on MenuChanged(id),\n        \
         StoreChanged(id)\n    regenerate     on_invalidation\n    1\n}\n",
    );
    assert_eq!(reported(&src), Vec::<String>::new());
}
