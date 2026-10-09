//! **A materialization is kept, and a page reads it** (ADR-0277), the
//! compiler's part. A materialization that derives its value is a component
//! the host runs to keep it. Each `query R(..)` its body reads is a call of
//! the platform's read of R, `pw:host/reads#<R>`, taking R's parameters and
//! answering R's value: its `Ok` where R answers a `Result`. Its contract
//! names what each read reads, and no capability: a read is a dependency, as
//! a page's reading of a query is. And a page that reads one depends on it,
//! and binds it as it binds a query. Until ADR-0277 no materialization was
//! compiled. Each test states one case, with its control.

use pw_core::backend::component::Built;
use pw_core::build::build;
use pw_core::check::Unit;
use pw_core::contract::ImportKind;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn unit(path: &str, src: &str) -> Unit {
    Unit {
        path: path.to_string(),
        hir: lower_file(src, &parse_tree(src).green),
        src: src.to_string(),
    }
}

/// The store, as the one program it is.
fn store() -> pw_core::build::Build {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        paths.extend(ps);
    }
    paths.push(root.join("examples/domain.pw"));
    let units: Vec<Unit> = paths
        .iter()
        .map(|p| {
            unit(
                &p.display().to_string(),
                &std::fs::read_to_string(p).expect("read"),
            )
        })
        .collect();
    build(&units).expect("the store checks")
}

#[test]
fn each_read_is_the_platforms_and_names_what_it_reads() {
    let b = store();
    let contract = |id: &str| {
        b.contracts
            .iter()
            .find(|c| c.component_id == id)
            .unwrap_or_else(|| panic!("no contract for {id}"))
    };
    for (id, read, reads) in [
        ("store.page.MenuSize", "store-page-menu", "store.page.Menu"),
        (
            "store.page.MenuLine",
            "store-page-menu-size",
            "store.page.MenuSize",
        ),
    ] {
        let c = contract(id);
        let found: Vec<_> = c.imports.iter().filter(|i| i.reads.is_some()).collect();
        let [i] = found.as_slice() else {
            panic!("{id}: one read, {found:?}");
        };
        assert_eq!(
            (i.interface.as_str(), i.name.as_str()),
            ("pw:host/reads", read)
        );
        assert_eq!(i.reads.as_deref(), Some(reads));
        assert_eq!(i.kind, ImportKind::HostCapability);
        assert!(
            i.capability.is_empty() && i.capabilities.is_empty(),
            "{id}: a read holds no authority of the reader's: {i:?}"
        );
    }
    // The count's read answers the menu's `Ok`, a list of its sections, and
    // not its `Result`.
    let menu = contract("store.page.MenuSize")
        .imports
        .iter()
        .find(|i| i.reads.is_some())
        .and_then(|i| i.signature.as_ref())
        .map(|s| format!("{:?}", s.result))
        .expect("a signature");
    assert!(menu.contains("List") && !menu.contains("Result"), "{menu}");
    // The control: a query reads nothing so; it reads its data layer.
    assert!(
        contract("store.page.Menu")
            .imports
            .iter()
            .all(|i| i.reads.is_none())
    );
}

#[test]
fn one_that_derives_its_value_is_a_component_that_imports_its_reads() {
    let b = store();
    for (id, read) in [
        ("store.page.MenuSize", "pw:host/reads#store-page-menu"),
        ("store.page.MenuLine", "pw:host/reads#store-page-menu-size"),
    ] {
        let built = b
            .components
            .iter()
            .find(|(c, _)| c == id)
            .map(|(_, x)| x)
            .unwrap_or_else(|| panic!("{id} is no component"));
        let Built::Component { compiled, audited } = built else {
            panic!("{id}: {built:?}");
        };
        assert!(*audited > 0, "{id}: the audit compared nothing");
        assert_eq!(compiled.component.imports, [read.to_string()], "{id}");
    }
    // The WIT declares the platform's reads, each by what it reads.
    for read in ["store-page-menu:", "store-page-menu-size:"] {
        assert!(b.wit.contains(read), "{read}: {}", b.wit);
    }
    // The control: a materialization that declares no type, a fragment the
    // host renders, is no component.
    assert!(
        !b.components
            .iter()
            .any(|(c, _)| c.ends_with("MenuFragment")),
        "{:?}",
        b.components.iter().map(|(c, _)| c).collect::<Vec<_>>()
    );
}

#[test]
fn a_page_that_reads_one_depends_on_it_and_binds_it() {
    let b = store();
    let page = b
        .contracts
        .iter()
        .find(|c| c.component_id == "store.page.StorePage")
        .expect("the store's page");
    assert!(
        page.imports.iter().any(|i| i.kind == ImportKind::Component
            && i.interface == "pw:app/store.page.MenuLine"),
        "{:?}",
        page.imports
    );
    // Bound as a query is, by its resource and its key.
    let plan = b
        .pages
        .iter()
        .find(|p| p.page == "store.page.StorePage")
        .and_then(|p| p.plan.as_ref().ok())
        .expect("the store's page is planned");
    let summary = serde_json::to_value(plan).expect("a plan is JSON");
    let bound = summary["bindings"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|b| b["binding"] == "summary")
        .expect("the page binds its summary");
    assert_eq!(bound["resource"], "store.page.MenuLine");
    assert_eq!(bound["args"], serde_json::json!(["id"]));
    // The control: the page depends on no fragment it does not read.
    assert!(
        !page
            .imports
            .iter()
            .any(|i| i.interface.ends_with("MenuFragment")),
        "{:?}",
        page.imports
    );
}
