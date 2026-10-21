//! Qualified device signals preserve their declaration's type and privacy.
use pw_core::check::{Unit, check_sources};
use pw_core::diagnostics::Diagnostic;

const PREFS: &str = "module prefs\nsignal word: String = \"\"\n    storage device \"prefs-word\"\n    privacy Device\n    conflict last_write_wins\n";

fn sources(import: &str, signals: &str, handler: &str, takes: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    for directory in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<_> = std::fs::read_dir(root.join(directory))
            .expect("packages")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "pw"))
            .collect();
        paths.sort();
        for path in paths {
            sources.push((
                path.display().to_string(),
                std::fs::read_to_string(path).expect("source"),
            ));
        }
    }
    sources.push(("prefs.pw".into(), PREFS.into()));
    sources.push((
        "app.pw".into(),
        format!(
            r#"module app
{import}
type PublicPrefs = PublicPrefs {{ word: String }}
command send(text: {takes}) -> Result<String, String>
    idempotent_by Press
{{ Ok("sent") }}
opaque type Press = String
page P() {{
    route "/"
    cache private
    signal draft: String = ""
    {signals}
    view {{
        <title>t</title>
        <main><button type="button" on:press={{() => {handler}}}>Send</button></main>
    }}
}}
"#
        ),
    ));
    sources
}

fn send(value: &str) -> String {
    format!("match send({value}) {{ Ok(_) => draft = \"\", Err(_) => draft = \"\", }}")
}
fn diagnostics(sources: &[(String, String)]) -> Vec<Diagnostic> {
    check_sources(sources)
        .into_iter()
        .flat_map(|(_, ds)| ds)
        .collect()
}
fn build(sources: &[(String, String)]) -> Result<pw_core::build::Build, String> {
    let units: Vec<_> = sources
        .iter()
        .map(|(path, src)| Unit {
            path: path.clone(),
            src: src.clone(),
            hir: pw_core::lower::lower_file(src, &pw_syntax::parse_tree(src).green),
        })
        .collect();
    pw_core::build::build(&units)
}

#[test]
fn qualified_and_selectively_imported_device_arguments_are_refused_at_their_declaration() {
    for (import, value) in [
        ("import prefs", "prefs.word"),
        ("import prefs.{ word }", "word"),
    ] {
        let sources = sources(import, "", &send(value), "String");
        let found = diagnostics(&sources);
        assert_eq!(
            found.iter().map(|d| d.code).collect::<Vec<_>>(),
            ["PW6801"],
            "{found:?}"
        );
        assert!(
            found[0]
                .related
                .iter()
                .any(|r| r.file.as_deref() == Some("prefs.pw")
                    && PREFS.get(r.span.clone()) == Some("word")
                    && r.label.contains("privacy Device")),
            "{found:?}"
        );
        assert!(
            build(&sources).is_err(),
            "a refused device argument must produce no build"
        );
    }
    let public = sources("import prefs", "", &send("\"public\""), "String");
    assert!(diagnostics(&public).is_empty());
    assert!(
        build(&public)
            .expect("public neighbor builds")
            .refusals()
            .is_empty()
    );
}

#[test]
fn qualified_device_arguments_keep_their_declared_value_type() {
    let sources = sources("import prefs", "", &send("prefs.word"), "Int");
    let found = diagnostics(&sources);
    assert!(
        found.iter().any(|d| d.code == "PW0605"
            && d.message.contains("String")
            && d.message.contains("Int")),
        "{found:?}"
    );
    assert!(found.iter().any(|d| d.code == "PW6801"), "{found:?}");
}

#[test]
fn lexical_record_and_signal_names_do_not_read_a_module_device_value() {
    for (import, signals, value) in [
        (
            "import prefs",
            "signal prefs: PublicPrefs = PublicPrefs { word: \"public\" }",
            "prefs.word",
        ),
        (
            "import prefs.{ word }",
            "signal word: String = \"public\"",
            "word",
        ),
    ] {
        let sources = sources(import, signals, &send(value), "String");
        assert!(
            diagnostics(&sources).is_empty(),
            "{:?}",
            diagnostics(&sources)
        );
        let built = build(&sources).expect("lexical neighbor builds");
        assert!(
            built.refusals().is_empty(),
            "{import}: {:?}",
            built.refusals()
        );
    }
}

#[test]
fn qualified_browser_only_read_still_refuses_unsupported_backend_lowering() {
    let sources = sources("import prefs", "", "draft = prefs.word", "String");
    assert!(
        diagnostics(&sources).is_empty(),
        "{:?}",
        diagnostics(&sources)
    );
    let built = build(&sources).expect("browser-only read checks");
    assert!(
        built
            .refusals()
            .iter()
            .any(|r| r.contains("`prefs` is not bound here")),
        "{:?}",
        built.refusals()
    );
}
