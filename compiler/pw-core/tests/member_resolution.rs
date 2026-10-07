//! **A member names the declaration the calling module sees** (ADR-0254).
//!
//! A member was keyed by its receiver's type and its name alone, and a
//! second declaration replaced the first: `h.destroy()` named whichever
//! `destroy(MapHandle)` was registered last, `VendorSdk.destroy`, in a module
//! that imports only `Maps`. Four members are declared so by the store's
//! library: `destroy`, `is_available`, `current` and `for_store`. Now one
//! declaration resolves as before; of several, the one the module declares
//! or imports; and a module that sees several, or none, is refused, PW0628.
//! Each test states one case, with its control.

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

/// What `app.pw` reports, checked with the platform and the store's domain
/// and library: each code and message.
fn reported(src: &str) -> Vec<String> {
    reported_with(&[], src)
}

/// [`reported`], with `modules` beside it.
fn reported_with(modules: &[&str], src: &str) -> Vec<String> {
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
    for (i, m) in modules.iter().enumerate() {
        program.push((format!("beside{i}.pw"), m.to_string()));
    }
    program.push(("app.pw".to_string(), src.to_string()));
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A module importing `IMPORTS`, with `f` destroying a handle as a member,
/// its row `ROW`.
fn destroying(imports: &str, row: &str) -> String {
    format!(
        "module m\n\nimport browser.{{ MapHandle }}\n{imports}\n\n\
         fn f(h: MapHandle) -> () !{{ {row} }} {{\n    h.destroy()\n}}\n"
    )
}

#[test]
fn a_member_two_modules_declare_is_the_one_imported() {
    // `VendorSdk.destroy` is `dom.mutate`; `Maps.destroy` releases the
    // handle as well, which a row that does not say so may not do.
    assert_eq!(
        reported(&destroying("import VendorSdk", "dom.mutate")),
        Vec::<String>::new()
    );
    assert_eq!(
        reported(&destroying(
            "import Maps",
            "dom.mutate, resource.release<MapHandle>"
        )),
        Vec::<String>::new()
    );
    // The control: `Maps.destroy`, imported, releases what the row does not
    // declare. Until ADR-0254 the member was `VendorSdk.destroy` here, the
    // one registered last, and nothing was said.
    let f = reported(&destroying("import Maps", "dom.mutate"));
    assert!(
        !f.is_empty() && f.iter().all(|d| d.starts_with("PW0400")),
        "{f:#?}"
    );
}

#[test]
fn a_member_two_imported_modules_declare_is_refused() {
    let f = reported(&destroying("import Maps\nimport VendorSdk", "dom.mutate"));
    assert_eq!(
        f,
        vec![
            "PW0628 `destroy` of a `browser.MapHandle` could be `Maps.destroy` or \
             `VendorSdk.destroy`, and this module does not say which"
                .to_string()
        ]
    );
}

#[test]
fn a_member_of_modules_none_imported_is_refused() {
    let f = reported(&destroying("", "dom.mutate"));
    assert_eq!(
        f,
        vec![
            "PW0628 `destroy` of a `browser.MapHandle` could be `Maps.destroy` or \
             `VendorSdk.destroy`, and this module does not say which"
                .to_string()
        ]
    );
}

#[test]
fn a_members_effects_are_the_imported_ones() {
    // `Menu.is_available` is pure; `Menus.is_available` reads the database.
    let checking = |import: &str| {
        format!(
            "module m\n\nimport domain.{{ MenuItemId }}\n{import}\n\n\
             fn f(item: MenuItemId) -> Bool !{{}} {{\n    item.is_available()\n}}\n"
        )
    };
    assert_eq!(reported(&checking("import Menu")), Vec::<String>::new());
    let f = reported(&checking("import Menus"));
    assert!(
        !f.is_empty() && f.iter().all(|d| d.starts_with("PW0400")),
        "{f:#?}"
    );
}

#[test]
fn a_members_type_is_the_imported_ones() {
    // `Carts.current` answers a cart; `Orders.current` an order's status.
    let reading = |import: &str, row: &str| {
        format!(
            "module m\n\nimport capability.{{ Session, SessionId }}\n\
             import domain.{{ Cart, CartError }}\n{import}\n\n\
             fn f(s: Session<SessionId>) -> Result<Cart, CartError> !{{ {row} }} {{\n    \
             s.current()\n}}\n"
        )
    };
    assert_eq!(
        reported(&reading("import Carts", "database.read<Carts>")),
        Vec::<String>::new()
    );
    let f = reported(&reading("import Orders", "database.read<Orders>"));
    assert!(
        !f.is_empty() && f.iter().all(|d| d.starts_with("PW06")),
        "{f:#?}"
    );
}

#[test]
fn a_member_declared_beside_its_type_is_seen_without_an_import() {
    // `shapes.area` is declared with `Shape`, `other.area` elsewhere: a module
    // importing only the type sees the one beside it.
    let shapes = "module shapes\n\ntype Shape = Shape { side: Int }\n\n\
                  fn area(s: Shape) -> Int !{} {\n    s.side * s.side\n}\n";
    let other = "module other\n\nimport shapes.{ Shape }\n\n\
                 fn area(s: Shape) -> Int !{} {\n    0\n}\n";
    let app = "module m\n\nimport shapes.{ Shape }\n\n\
               fn f(s: Shape) -> Int !{} {\n    s.area()\n}\n";
    assert_eq!(reported_with(&[shapes, other], app), Vec::<String>::new());
    // The control: importing `other` too, the module sees two.
    let both = app.replace(
        "import shapes.{ Shape }",
        "import shapes.{ Shape }\nimport other",
    );
    let f = reported_with(&[shapes, other], &both);
    assert!(
        !f.is_empty() && f.iter().all(|d| d.starts_with("PW0628")),
        "{f:#?}"
    );
}

#[test]
fn an_ambiguous_member_is_no_function_value() {
    // A handle `f` must release, given to a member no module tells apart:
    // nothing ends it, and the call is PW0628's, not a function value's.
    let f = reported(&destroying(
        "import Maps\nimport VendorSdk",
        "dom.mutate, resource.release<MapHandle>",
    ));
    assert!(f.iter().any(|d| d.starts_with("PW0628")), "{f:#?}");
    assert!(!f.iter().any(|d| d.contains("function value")), "{f:#?}");
}
