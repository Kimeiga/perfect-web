//! **A form goes where something answers it** (ADR-0265). A form's `action`
//! is a request the browser sends with the form's `method`, `get` where it
//! states none, and a route answers it: a page a `get`, the relying party
//! its own (`/sign-in`, `/sign-up` and its callback a `get`, `/sign-out` a
//! `post`), an upload a `post` of a file. Until ADR-0265 no form but a
//! file's was checked, and a link to `/sign-in` was PW5009's, its route the
//! host's. Each test states one case, with its control.

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

/// What a page at `/stores/{id}` reports, rendering `markup`, with `more`
/// declared beside it: each diagnostic's code and the text its span covers.
fn reported(markup: &str, more: &str) -> Vec<(String, String)> {
    let src = format!(
        "module app\n\n{more}page Store(id: String) {{\n    route \"/stores/{{id}}\"\n\n    \
         view {{\n        <title>Store</title>\n        <main>{markup}</main>\n    }}\n}}\n"
    );
    let mut program = platform();
    program.push(("app.pw".to_string(), src.clone()));
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| {
            ds.into_iter().map(|d| {
                let at = src[d.primary_span.start..d.primary_span.end].to_string();
                (d.code.to_string(), at)
            })
        })
        .collect()
}

fn codes(markup: &str) -> Vec<String> {
    reported(markup, "").into_iter().map(|(c, _)| c).collect()
}

const BUTTON: &str = "<button type=\"submit\">Go</button>";

#[test]
fn a_form_is_sent_where_its_method_is_answered() {
    for (form, at) in [
        // A page answers a `get`.
        (
            "<form method=\"post\" action=\"/stores/1\">",
            "action=\"/stores/1\"",
        ),
        // `/sign-out` answers a `post`, and a form states `get` by default.
        ("<form action=\"/sign-out\">", "action=\"/sign-out\""),
        // Nothing answers this.
        (
            "<form method=\"post\" action=\"/nowhere\">",
            "action=\"/nowhere\"",
        ),
    ] {
        let got = reported(&format!("{form}{BUTTON}</form>"), "");
        assert_eq!(got, vec![("PW5041".to_string(), at.to_string())], "{form}");
    }
    // The controls: each where its method is answered, whatever its case;
    // a form with no `action`; and one sent to another origin.
    for form in [
        "<form action=\"/sign-in\">",
        "<form method=\"get\" action=\"/sign-up\">",
        "<form method=\"POST\" action=\"/sign-out\">",
        "<form action=\"/stores/2\">",
        "<form>",
        "<form method=\"post\" action=\"https://example.com/elsewhere\">",
    ] {
        assert!(
            codes(&format!("{form}{BUTTON}</form>")).is_empty(),
            "{form}: {:?}",
            codes(&format!("{form}{BUTTON}</form>"))
        );
    }
}

#[test]
fn a_link_reaches_what_answers_a_get() {
    // The relying party's routes are every deployment's: a link to one is
    // no dead link. Until ADR-0265 it was PW5009.
    assert!(codes("<a href=\"/sign-in\">Sign in</a>").is_empty());
    assert!(codes("<a href=\"/sign-up\">Sign up</a>").is_empty());
    // `/sign-out` answers a `post` alone: a link would sign out whoever
    // followed it, and none reaches it.
    assert_eq!(codes("<a href=\"/sign-out\">Sign out</a>"), ["PW5009"]);
    // The control: a link to nothing is dead, as before.
    assert_eq!(codes("<a href=\"/nowhere\">Nowhere</a>"), ["PW5009"]);
}

#[test]
fn a_form_that_sends_a_file_is_the_uploads_to_check() {
    let upload = "upload PostImage\n    route      \"/uploads/post-image\"\n    \
                  serves     \"/images\"\n    max_bytes  5_000_000\n    \
                  types      png\n    max_width  4096\n    max_height 4096\n\n";
    // Sent to no upload: PW5603 says so, and this check says nothing more.
    let got = reported(
        &format!(
            "<form method=\"post\" action=\"/nowhere\" enctype=\"multipart/form-data\">\
             <label>An image <input type=\"file\" name=\"image\"></label>{BUTTON}</form>"
        ),
        upload,
    );
    assert_eq!(
        got.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(),
        ["PW5603"]
    );
    // The control: sent to the upload, it is answered.
    assert!(
        reported(
            &format!(
                "<form method=\"post\" action=\"/uploads/post-image\" \
                 enctype=\"multipart/form-data\"><label>An image <input type=\"file\" name=\"image\"></label>{BUTTON}</form>"
            ),
            upload,
        )
        .is_empty()
    );
    // And a link to what an upload serves is answered by a `get`.
    assert!(reported("<a href=\"/images/0a1b.png\">The image</a>", upload).is_empty());
}
