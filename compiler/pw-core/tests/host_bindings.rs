//! **A host binding is an operation a host provides** (ADR-0262): `host
//! "namespace:package/interface#name"`, each part a WIT identifier, held to
//! that where it is written (PW0335). Until ADR-0262 any string was taken,
//! and the uploads track's `host "feed:uploads#claim"`, with no interface,
//! broke WIT generation for every component, by an error naming no line.
//! Each test states one case, with its control.

use pw_core::check::check_sources;

fn platform() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            let src = std::fs::read_to_string(&p).expect("read");
            out.push((p.display().to_string(), src));
        }
    }
    out
}

/// What `app.pw` reports with the platform: each diagnostic's code, its
/// message, and the text its primary span covers.
fn reported(src: &str) -> Vec<(String, String, String)> {
    let mut program = platform();
    program.push(("app.pw".to_string(), src.to_string()));
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| {
            ds.into_iter().map(|d| {
                let at = src[d.primary_span.start..d.primary_span.end].to_string();
                (d.code.to_string(), d.message, at)
            })
        })
        .collect()
}

/// A function the host provides, bound as `binding` is written.
fn bound(binding: &str) -> String {
    format!(
        "module app\n\ntype Cart = Cart {{ items: Int }}\n\n\
         fn cart() -> Cart !{{ database.read<Cart> }}\n    host {binding}\n"
    )
}

#[test]
fn the_uploads_tracks_binding_with_no_interface_is_refused_where_it_is_written() {
    let src = bound("\"feed:uploads#claim\"");
    let got = reported(&src);
    assert_eq!(got.len(), 1, "{got:?}");
    let (code, message, at) = &got[0];
    assert_eq!(code, "PW0335");
    assert!(message.contains("names no interface"), "{message}");
    assert!(at.contains("feed:uploads#claim"), "{at}");
    // The control: the same operation with its interface.
    assert!(reported(&bound("\"feed:uploads/leases#claim\"")).is_empty());
}

#[test]
fn each_part_of_a_binding_is_a_wit_identifier() {
    for (binding, why) in [
        ("feed:data/posts#timeline", "not quoted"),
        ("\"feed:data/posts\"", "no operation after `#`"),
        ("\"data/posts#timeline\"", "no namespace"),
        ("\"feed:data/posts#\"", "a part is empty"),
        ("\"feed:data/Posts#timeline\"", "mixes cases"),
        ("\"feed:data/posts#2nd\"", "begins with `2`"),
        ("\"feed:data/posts#time_line\"", "has `_`"),
        ("\"feed:data/posts#time--line\"", "an empty word"),
        ("\"feed::data/posts#timeline\"", "begins with `:`"),
    ] {
        let got = reported(&bound(binding));
        assert!(
            got.len() == 1 && got[0].0 == "PW0335" && got[0].1.contains(why),
            "{binding}: {got:?}"
        );
    }
    // The controls, as wit-parser takes them: an uppercase word, a word of
    // digits after the first, and the platform's own.
    for binding in [
        "\"pw:host/session#read\"",
        "\"feed:data/HTTP-posts#read-2\"",
        "\"a:b/c#d\"",
    ] {
        assert!(
            reported(&bound(binding)).is_empty(),
            "{binding}: {:?}",
            reported(&bound(binding))
        );
    }
}

#[test]
fn a_malformed_binding_stops_the_build_at_check_and_never_reaches_wit() {
    let units = |src: &str| -> Vec<pw_core::check::Unit> {
        let mut program = platform();
        program.push(("app.pw".to_string(), src.to_string()));
        program
            .into_iter()
            .map(|(path, src)| pw_core::check::Unit {
                hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
                path,
                src,
            })
            .collect()
    };
    let query = "\npublic query Carted() -> Cart\n    freshness 0.seconds\n    \
                 consistency strong\n    cache shared\n{\n    cart()\n}\n";
    let refused = match pw_core::build::build(&units(&(bound("\"feed:uploads#claim\"") + query))) {
        Ok(build) => build.refusals().join("\n"),
        Err(why) => why,
    };
    assert!(refused.contains("PW0335"), "{refused}");
    assert!(!refused.contains("WIT"), "{refused}");
    // The control builds.
    let built = pw_core::build::build(&units(&(bound("\"feed:uploads/leases#claim\"") + query)))
        .expect("builds");
    assert!(built.refusals().is_empty(), "{:?}", built.refusals());
}
