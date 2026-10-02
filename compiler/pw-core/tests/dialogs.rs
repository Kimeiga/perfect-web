//! **A dialog a signal shows is the browser's modal dialog** (ADR-0141).
//!
//! A `<dialog>` written without `open`, in a block a signal decides, is shown
//! with the browser's `showModal` while the block renders it: it takes focus,
//! the page behind it is inert, and Escape closes it, by itself. So it
//! handles `close`, and the signal that shows it hears of it: without that,
//! the signal would say the dialog is shown after Escape closed it, and the
//! control that opens it could not open it again. A `<dialog>` nothing shows
//! is refused, and a `<dialog open>` is HTML's dialog shown in place.

use pw_core::check::check_sources;

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out
}

fn reported(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A page with a signal `open` and a parameter `shown`, whose view holds
/// `markup` after the button that opens it.
fn page(markup: &str) -> String {
    format!(
        "module t\n\nimport events.{{ CloseEvent }}\n\ntype Panel = Shut | Open\n\n\
         page P(shown: Bool) {{\n    cache private\n\n    signal open: Bool = false\n    \
         signal panel: Panel = Panel.Shut\n    signal said: String = \"\"\n\n    \
         view {{\n        <main><button type=\"button\" on:press={{() => open = true}}>Open</button>\
         <p>{{said}}</p>{markup}</main>\n    }}\n}}\n"
    )
}

const DIALOG: &str = "<dialog aria-label=\"Confirm\"{close}><button type=\"button\" \
                      on:press={() => open = false}>Done</button></dialog>";

fn dialog(close: &str) -> String {
    DIALOG.replace("{close}", close)
}

#[test]
fn a_dialog_a_signal_shows_says_what_closing_it_does() {
    let unheard = page(&format!("{{#if open}}{}{{/if}}", dialog("")));
    assert_eq!(
        reported(&unheard),
        [
            "PW5303 this `<dialog>` is shown while `open` renders it, and its closing is heard by \
          nothing"
        ]
    );
    // Control: its closing tells the signal.
    let heard = page(&format!(
        "{{#if open}}{}{{/if}}",
        dialog(" on:close={() => open = false}")
    ));
    assert_eq!(reported(&heard), Vec::<String>::new());
    // And deeper in the block a signal decides, through an arm.
    let arm = page(&format!(
        "{{#match panel}}{{:Shut}}<p>shut</p>{{:Open}}<div>{}</div>{{/match}}",
        dialog(" on:close={() => panel = Panel.Shut}")
    ));
    assert_eq!(reported(&arm), Vec::<String>::new());
}

#[test]
fn a_dialog_nothing_shows_is_refused() {
    for markup in [
        dialog(" on:close={() => open = false}"),
        // A block a parameter decides is decided once, on the server.
        format!(
            "{{#if shown}}{}{{/if}}",
            dialog(" on:close={() => open = false}")
        ),
    ] {
        assert_eq!(
            reported(&page(&markup)),
            [
                "PW5303 this `<dialog>` is shown by nothing: a modal dialog is shown while a \
              signal's block renders it"
            ],
            "{markup}"
        );
    }
    // Control: HTML's dialog shown in place.
    let open = page("<dialog open aria-label=\"Note\"><p>Shown in place.</p></dialog>");
    assert_eq!(reported(&open), Vec::<String>::new());
}

#[test]
fn a_dialog_s_close_gives_its_return_value() {
    let src = page(&format!(
        "{{#if open}}{}{{/if}}",
        dialog(" on:close={(e: CloseEvent) => { said = e.value\n open = false }}")
    ));
    assert_eq!(reported(&src), Vec::<String>::new());
    let wrong = page(&format!(
        "{{#if open}}{}{{/if}}",
        dialog(" on:close={(e) => { said = e.key\n open = false }}")
    ));
    let found = reported(&wrong);
    assert!(
        found
            .iter()
            .any(|d| d.starts_with("PW0610") && d.contains("`key`")),
        "{found:#?}"
    );
}

#[test]
fn a_view_with_a_dialog_alone_is_read() {
    // The signal rules skipped a body with no signal and no handler: a
    // `<dialog>` in one would have gone unread.
    let src = "module t\n\nview V() !{} {\n    <dialog aria-label=\"Lost\"><p>Never shown.</p></dialog>\n}\n";
    assert_eq!(
        reported(src),
        [
            "PW5303 this `<dialog>` is shown by nothing: a modal dialog is shown while a signal's \
          block renders it"
        ]
    );
}
