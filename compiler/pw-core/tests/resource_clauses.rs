//! **A resource's clauses are held to PW2005** (ADR-0251): what its
//! `acquire` clause makes, the resource holds; what its `release` clause is
//! given, it ends, exactly once on every path.
//!
//! A `resource` declaration's clauses are terms, lowered outside the body
//! the affine check walked; a statement's, in a component, are a call and
//! the block after it. Until ADR-0251 neither `release` was held to
//! anything: one that never ended its handle passed, in either form. Each
//! test states one case, with its control.

use pw_core::check::check_sources;

/// The `.pw` files of `dir`, under the repository, by name.
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

/// One diagnostic of `app.pw`: its code, its message, the text it
/// underlines, its related labels, and its repairs.
type Found = (String, String, String, Vec<String>, Vec<String>);

/// What the checker reports of `app.pw`, with the platform and the store's
/// domain and library.
fn found(src: &str) -> Vec<Found> {
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
        .flat_map(|(_, ds)| ds)
        .map(|d| {
            (
                d.code.to_string(),
                d.message.clone(),
                src[d.primary_span.clone()].to_string(),
                d.related.iter().map(|r| r.label.clone()).collect(),
                d.repairs.into_iter().map(|r| r.description).collect(),
            )
        })
        .collect()
}

/// Each diagnostic's code and message.
fn said(f: &[Found]) -> Vec<(&str, &str)> {
    f.iter().map(|f| (f.0.as_str(), f.1.as_str())).collect()
}

/// A `resource` declaration, as A-007 writes it: `ACQUIRE` its `acquire`
/// clause's body, `RELEASE` its `release` clause's.
const DECLARED: &str = "module m\n\nimport browser.{ ElementRef, MapHandle, MapError }\n\
     import domain.{ LatLng }\n\nimport Maps\nimport VendorSdk\n\n\
     resource StoreMap(container: ElementRef, center: LatLng)\n    \
     -> Result<MapHandle, MapError>\n{\n    placement browser\n    affine\n    \
     acquire {\n        ACQUIRE\n    }\n    \
     release(handle) {\n        RELEASE\n    }\n}\n";

/// A component's `resource` statement, as A-019 writes it.
const STATEMENT: &str = "module m\n\nimport Maps\nimport VendorSdk\nimport domain.{ LatLng }\n\n\
     component LazyMap(center: LatLng) {\n    placement browser\n\n    \
     let visible = observe intersection(self, threshold = 0.1) -> Bool\n\n    \
     resource map when visible {\n        scope component\n        \
     acquire {\n            ACQUIRE\n        }\n        \
     release(handle) {\n            RELEASE\n        }\n    }\n\n    \
     view { <section aria-label=\"Store location\" /> }\n}\n";

/// The resource in each form, acquiring with `acquire` and releasing with
/// `release`: `container` is `self` in the component.
fn both(acquire: &str, release: &str) -> [String; 2] {
    [
        DECLARED
            .replace("ACQUIRE", acquire)
            .replace("RELEASE", release),
        STATEMENT
            .replace("ACQUIRE", &acquire.replace("container", "self"))
            .replace("RELEASE", &release.replace("container", "self")),
    ]
}

const CREATE: &str = "Maps.create(container, center)";

#[test]
fn a_release_clause_ends_what_it_is_given() {
    for src in both(CREATE, "Maps.destroy(handle)") {
        assert_eq!(found(&src), vec![], "{src}");
    }
    // The control: a `release` that ends nothing, in each form.
    for src in both(CREATE, "()") {
        let f = found(&src);
        assert_eq!(
            said(&f),
            vec![(
                "PW2005",
                "affine resource `handle: MapHandle` is not consumed on every path"
            )],
            "{src}"
        );
        assert_eq!(
            f[0].3[0],
            "`handle` is what its resource's `acquire` made, and this `release` must end it"
        );
        assert_eq!(
            f[0].4,
            vec!["end `handle` on this path, with `destroy`".to_string()]
        );
    }
}

#[test]
fn a_release_clause_ends_it_once_on_every_path() {
    for src in both(CREATE, "Maps.destroy(handle)\n        Maps.destroy(handle)") {
        assert_eq!(
            said(&found(&src)),
            vec![(
                "PW2005",
                "affine resource `handle: MapHandle` is released twice on one path"
            )],
            "{src}"
        );
    }
    for src in both(
        CREATE,
        "if center.lat > 0.0 {\n            Maps.destroy(handle)\n        }",
    ) {
        assert_eq!(
            said(&found(&src)),
            vec![(
                "PW2005",
                "affine resource `handle: MapHandle` is not consumed on every path"
            )],
            "{src}"
        );
    }
}

#[test]
fn a_release_owes_nothing_where_its_acquire_acquires_nothing() {
    // `VendorSdk.mount` declares no `resource.acquire`, so what it makes is
    // no resource, and its `release` may end it any way it likes (A-024).
    for src in both("VendorSdk.mount(container, center)", "()") {
        assert_eq!(found(&src), vec![], "{src}");
    }
}

#[test]
fn an_acquisition_in_a_release_clause_is_held_or_refused() {
    // The `acquire` clause holds its value; a `release` clause's statement
    // drops one, and so does the clause, in which it is the last.
    for (release, drops) in [
        (
            format!("{CREATE}\n        Maps.destroy(handle)"),
            "affine resource `MapHandle` is acquired, and its statement drops it",
        ),
        (
            format!("Maps.destroy(handle)\n        {CREATE}"),
            "affine resource `MapHandle` is acquired, and its clause drops it",
        ),
    ] {
        for src in both(CREATE, &release) {
            let f = found(&src);
            assert!(said(&f).contains(&("PW2005", drops)), "{src}: {f:#?}");
        }
    }
}

#[test]
fn an_acquisition_in_a_key_is_the_clauses() {
    // A command gives its caller its body's value, and not a clause's: a
    // key's value names an entry, and nothing ends what it acquires.
    let src = "module m\n\nimport Carts\nimport Database\nimport context.{ current_session }\n\
               import Events.{ CartChanged }\nimport domain.{ CartError }\n\n\
               command clear_cart() -> Result<(), CartError>\n    requires SignedIn\n    \
               emits CartChanged(Database.begin())\n{\n    \
               let _cleared = Carts.clear(current_session())\n    Ok(())\n}\n";
    let f = found(src);
    assert!(
        said(&f).contains(&(
            "PW2005",
            "affine resource `DatabaseTransaction` is acquired, and its clause drops it"
        )),
        "{f:#?}"
    );
}

#[test]
fn a_release_clause_reads_its_own_bindings() {
    // `let end = Maps.destroy` in the clause is `Maps.destroy` where it is
    // called (ADR-0080), in a declaration's clause as in a statement's.
    for src in both(CREATE, "let end = Maps.destroy\n        end(handle)") {
        assert_eq!(found(&src), vec![], "{src}");
    }
}

#[test]
fn what_an_acquire_clause_binds_moves_to_its_resource() {
    // Bound, then the clause's value: the resource holds it.
    for src in both(
        &format!("let made = {CREATE}\n        made"),
        "Maps.destroy(handle)",
    ) {
        assert_eq!(found(&src), vec![], "{src}");
    }
    // The control: bound, and not its value.
    for src in both(
        &format!("let made = {CREATE}\n        {CREATE}"),
        "Maps.destroy(handle)",
    ) {
        let f = found(&src);
        assert!(
            said(&f).contains(&(
                "PW2005",
                "affine resource `made: MapHandle` is not consumed on every path"
            )),
            "{src}: {f:#?}"
        );
    }
}
