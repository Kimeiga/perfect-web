//! **A template leaves the document's base and its links' targets alone**
//! (ADR-0096).
//!
//! The page a view renders into loads the platform's runtime after the
//! view's markup: `<script type="module" src="/pw-runtime.mjs">`. A `<base>`
//! element anywhere in the document moves every relative URL that follows,
//! so `<base href={msg}>` loaded the runtime from wherever `msg` names. And
//! an SVG animation sets the attribute it names: `<animate
//! attributeName="href" values={msg}>` gives a link any URL, past the check
//! a URL attribute gets. Until 2026-09-26 both checked and built. Each test
//! states one case, with a control.

use pw_core::check::check_sources;

fn reported(markup: &str) -> Vec<String> {
    let src = format!(
        "module t\n\nview Note(msg: String) !{{}} {{\n    <div>\n        {markup}\n    </div>\n}}\n"
    );
    check_sources(&[("t.pw".to_string(), src)])
        .into_iter()
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn one(markup: &str, says: &str) {
    let found = reported(markup);
    assert_eq!(found.len(), 1, "{markup}\n{found:#?}");
    assert!(found[0].contains(says), "{markup}\n{found:#?}");
}

fn clean(markup: &str) {
    let found = reported(markup);
    assert!(found.is_empty(), "{markup}\n{found:#?}");
}

#[test]
fn a_template_writes_no_base() {
    one(
        "<base href={msg} />",
        "PW5024 `<base>` moves every relative URL",
    );
    one(
        "<base href=\"/\" />",
        "PW5024 `<base>` moves every relative URL",
    );
    clean("<a href=\"/menu\">menu</a>");
}

#[test]
fn an_animation_sets_no_link_and_no_handler() {
    one(
        "<svg><animate attributeName=\"href\" values={msg} /></svg>",
        "PW5024 `<animate>` sets `href`",
    );
    one(
        "<svg><set attributeName=\"xlink:HREF\" to={msg} /></svg>",
        "PW5024 `<set>` sets `xlink:HREF`",
    );
    one(
        "<svg><set attributeName=\"onclick\" to={msg} /></svg>",
        "PW5024 `<set>` sets `onclick`",
    );
    clean("<svg><animate attributeName=\"opacity\" values={msg} /></svg>");
}
