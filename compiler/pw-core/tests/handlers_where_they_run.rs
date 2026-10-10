//! **A handler is held where it runs** (ADR-0287).
//!
//! A page's handler runs in the browser when its element is pressed, held
//! there by its own rule (ADR-0113), where a command it calls is a request
//! the command performs at its own placement. A page's declared placement
//! grants what the page does where it is placed: what it renders. Until
//! ADR-0287 the handler's work was held to the page's placement too, so a page
//! placed at `build` whose button sends a command was refused.

use pw_core::check::check_sources;

/// `code message` for each diagnostic on `t.pw`, beside the platform.
fn reported(program: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        ps.sort();
        for p in ps {
            sources.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    sources.push(("t.pw".to_string(), program.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A shop's data, a command that writes it, and a page placed at `build`
/// whose view is `view`.
fn shop(view: &str) -> String {
    format!(
        "module t\n\n\
         source Things\n    holds        Thing\n    transactions serializable\n    reads        strong\n    changes      none\n\n\
         type Thing = Thing {{\n    name: String,\n}}\n\n\
         opaque type InteractionId = String\n\n\
         fn read_thing(name: String) -> Int !{{ database.read<Thing> }}\n    host \"t:data/things#read\"\n\n\
         fn write_thing(name: String) -> Int !{{ database.write<Thing> }}\n    host \"t:data/things#write\"\n\n\
         command SaveAll() -> Int\n    requires      SignedIn\n    idempotent_by InteractionId\n{{\n    write_thing(\"all\")\n}}\n\n\
         page Shop() {{\n    route     \"/shop\"\n    placement build\n\n    view {{\n        <title>Shop</title>\n        <main>\n            {view}\n        </main>\n    }}\n}}\n"
    )
}

#[test]
fn a_page_built_ahead_may_have_a_button_that_sends_a_command() {
    // A lambda that calls the command, and the command named as the
    // handler (ADR-0199): each a request the command performs.
    for button in [
        "<button type=\"button\" on:press|refusable={() => SaveAll()}>Save</button>",
        "<button type=\"button\" on:press|refusable={SaveAll}>Save</button>",
    ] {
        assert_eq!(reported(&shop(button)), Vec::<String>::new(), "{button}");
    }
}

#[test]
fn a_handler_that_writes_itself_is_still_refused_where_it_runs() {
    // The control on the handler's side: the browser cannot grant a write.
    let found = reported(&shop(
        "<button type=\"button\" on:press|refusable={() => write_thing(\"x\")}>Save</button>",
    ));
    assert_eq!(
        found,
        [
            "PW5005 `Shop`'s handler performs `database.write<Thing>`, which the browser cannot grant"
        ],
    );
}

#[test]
fn what_a_page_renders_is_still_held_to_its_placement() {
    // The control on the page's side: a read it renders runs at build,
    // which cannot grant it.
    let found = reported(&shop("<p>{read_thing(\"x\")}</p>"));
    assert!(
        found.iter().any(|d| d.starts_with("PW5005")
            && d.contains("`database.read<Thing>` is not available at placement Build")),
        "{found:#?}"
    );
}
