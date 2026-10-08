//! **A resource is held by what takes it apart** (ADR-0269). An acquisition
//! a `Result` or an `Option` carries is held by the name an arm binds it to,
//! `Ok(h)` or `Some(h)`, or by `let h = r?`, and must be ended there; an arm
//! that meets no carrying case, `Err(..)`, `None`, or a `_` after `Ok(h)`,
//! holds nothing to end. Until ADR-0269 a `match` on the acquisition was
//! PW2005 however its arms ended it (ADR-0250), and a carrying binding taken
//! apart later was "not consumed on every path". Each test states one case,
//! with its control.

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

/// What the checker reports of `app.pw`, with the platform and the store's
/// domain and library: each diagnostic's code, message and the text it
/// underlines.
fn said(src: &str) -> Vec<(String, String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut program = Vec::new();
    for d in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
        program.extend(files(d));
    }
    program.push((
        "examples/domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"),
    ));
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
            )
        })
        .collect()
}

fn clean(src: &str) {
    assert_eq!(said(src), Vec::<(String, String, String)>::new(), "{src}");
}

fn refused(src: &str, message: &str, at: &str) {
    assert_eq!(
        said(src),
        vec![("PW2005".to_string(), message.to_string(), at.to_string())],
        "{src}"
    );
}

/// A module over the store's library: `fn f` over a map, with `body`, and
/// `more` declared before it.
fn mapping(more: &str, body: &str) -> String {
    format!(
        "module m\n\nimport Maps\nimport browser.{{ ElementRef, MapHandle, MapError }}\n\
         import domain.{{ LatLng }}\n\n{more}\
         fn f(c: ElementRef, at: LatLng) -> Result<(), MapError> \
         !{{ dom.mutate, layout.measure, resource.acquire<MapHandle>, \
         resource.release<MapHandle> }} {{\n    {body}\n}}\n"
    )
}

fn matching(arms: &str) -> String {
    mapping(
        "",
        &format!("match Maps.create(c, at) {{\n{arms}    }}\n    Ok(())"),
    )
}

#[test]
fn an_acquisition_matched_where_it_is_made_is_held_by_its_arm() {
    // Ended in the arm that binds it, and nothing to end where it failed.
    clean(&matching(
        "        Ok(h) => Maps.destroy(h),\n        Err(_) => (),\n",
    ));
    // In either order, and a `_` that meets only the failure.
    clean(&matching(
        "        Err(_) => (),\n        Ok(h) => Maps.destroy(h),\n",
    ));
    clean(&matching(
        "        Ok(h) => Maps.destroy(h),\n        _ => (),\n",
    ));
    // The arm that binds it must end it, once.
    refused(
        &matching("        Ok(h) => (),\n        Err(_) => (),\n"),
        "affine resource `h: MapHandle` is not consumed on every path",
        "match Maps.create(c, at) {\n        Ok(h) => (),\n        Err(_) => (),\n    }",
    );
    refused(
        &matching(
            "        Ok(h) => {\n            Maps.destroy(h)\n            Maps.destroy(h)\n        }\n        \
             Err(_) => (),\n",
        ),
        "affine resource `h: MapHandle` is released twice on one path",
        "match Maps.create(c, at) {\n        Ok(h) => {\n            Maps.destroy(h)\n            \
         Maps.destroy(h)\n        }\n        Err(_) => (),\n    }",
    );
}

#[test]
fn an_arm_that_drops_the_acquisition_is_refused_where_it_drops_it() {
    // `Ok(_)` binds nothing, as `let _` does (ruling 0099-a).
    refused(
        &matching("        Ok(_) => (),\n        Err(_) => (),\n"),
        "affine resource `MapHandle` is bound to `_`, and nothing can release it",
        "Maps.create(c, at)",
    );
    // A `_` that meets a handle drops it.
    refused(
        &matching("        Err(_) => (),\n        _ => (),\n"),
        "affine resource `MapHandle` is bound to `_`, and nothing can release it",
        "Maps.create(c, at)",
    );
    // A name that holds the `Result` whole is not followed.
    refused(
        &matching("        r => (),\n"),
        "affine resource `MapHandle` is matched by a name that holds it whole, which \
         nothing follows",
        "Maps.create(c, at)",
    );
}

#[test]
fn a_carrying_binding_is_held_by_the_arm_that_takes_it_apart() {
    let bound = |arms: &str| {
        mapping(
            "",
            &format!("let r = Maps.create(c, at)\n    match r {{\n{arms}    }}\n    Ok(())"),
        )
    };
    clean(&bound(
        "        Ok(h) => Maps.destroy(h),\n        Err(_) => (),\n",
    ));
    refused(
        &bound("        Ok(h) => (),\n        Err(_) => (),\n"),
        "affine resource `r: MapHandle` is not consumed on every path",
        // A binding's scope, as every binding's is reported.
        "{\n    let r = Maps.create(c, at)\n    match r {\n        Ok(h) => (),\n        \
         Err(_) => (),\n    }\n    Ok(())\n}",
    );
    refused(
        &bound("        Ok(_) => (),\n        Err(_) => (),\n"),
        "affine resource `r: MapHandle` is bound to `_` in an arm, and nothing can \
         release it",
        "Ok(_)",
    );
    // Taken apart twice, it is ended twice.
    let twice = mapping(
        "",
        "let r = Maps.create(c, at)\n    match r {\n        Ok(h) => Maps.destroy(h),\n        \
         Err(_) => (),\n    }\n    match r {\n        Ok(h) => Maps.destroy(h),\n        \
         Err(_) => (),\n    }\n    Ok(())",
    );
    assert_eq!(
        said(&twice)
            .iter()
            .map(|(_, m, _)| m.as_str())
            .collect::<Vec<_>>(),
        ["affine resource `r: MapHandle` is released twice on one path"],
    );
}

#[test]
fn a_carrying_binding_is_held_by_what_its_question_mark_takes_out() {
    clean(&mapping(
        "",
        "let r = Maps.create(c, at)\n    let h = r?\n    Maps.destroy(h)\n    Ok(())",
    ));
    // The control: never ended.
    refused(
        &mapping("", "let r = Maps.create(c, at)\n    let h = r?\n    Ok(())"),
        "affine resource `r: MapHandle` is not consumed on every path",
        "{\n    let r = Maps.create(c, at)\n    let h = r?\n    Ok(())\n}",
    );
}

#[test]
fn an_option_is_taken_apart_as_a_result_is() {
    // A helper giving its caller the handle, or none: its own match gives
    // what its arm binds to the caller.
    let maybe = "fn maybe(c: ElementRef, at: LatLng) -> Option<MapHandle> \
                 !{ dom.mutate, layout.measure, resource.acquire<MapHandle> } {\n    \
                 match Maps.create(c, at) {\n        Ok(h) => Some(h),\n        \
                 Err(_) => None,\n    }\n}\n\n";
    clean(&mapping(
        maybe,
        "match maybe(c, at) {\n        Some(h) => Maps.destroy(h),\n        None => (),\n    }\n    Ok(())",
    ));
    refused(
        &mapping(
            maybe,
            "match maybe(c, at) {\n        Some(_) => (),\n        None => (),\n    }\n    Ok(())",
        ),
        "affine resource `MapHandle` is bound to `_`, and nothing can release it",
        "maybe(c, at)",
    );
}

#[test]
fn a_path_that_leaves_the_arm_before_ending_it_is_refused_where_it_leaves() {
    let other = "fn other() -> Result<(), MapError> !{} {\n    Ok(())\n}\n\n";
    let arm = |first: &str, then: &str| {
        mapping(
            other,
            &format!(
                "match Maps.create(c, at) {{\n        Ok(h) => {{\n            {first}\n            \
                 {then}\n        }}\n        Err(_) => (),\n    }}\n    Ok(())"
            ),
        )
    };
    // A failing `?` in the arm leaves the body owing the handle.
    refused(
        &arm("other()?", "Maps.destroy(h)"),
        "affine resource `h: MapHandle` is not consumed on every path",
        "other()?",
    );
    // The control: ended before it leaves.
    clean(&arm("Maps.destroy(h)", "other()?"));
}
