//! **An attribute's context is read as HTML reads its name** (ADR-0095).
//!
//! HTML lowercases an attribute's name, so `HREF` is `href` and `STYLE` is
//! `style`. The template IR chose a value's escaping from the name as
//! written. Until 2026-09-26 `<a HREF={msg}>` was escaped as an ordinary
//! attribute, which refuses no scheme, so `msg = "javascript:alert(1)"` ran,
//! and `HREF="javascript:go()"` escaped ADR-0094's rule, which reads the same
//! table. Each test states one case, with a control.

use pw_core::check::check_sources;
use pw_core::template_ir::Context;

#[test]
fn an_attributes_context_ignores_its_case() {
    for name in [
        "href",
        "HREF",
        "Href",
        "xlink:HREF",
        "Src",
        "ACTION",
        "formAction",
    ] {
        assert_eq!(Context::of_attribute(name), Context::Url, "{name}");
    }
    for name in ["style", "STYLE", "Style"] {
        assert_eq!(Context::of_attribute(name), Context::Style, "{name}");
    }
    assert_eq!(Context::of_attribute("TITLE"), Context::Attribute);
}

#[test]
fn a_script_url_is_refused_however_its_attribute_is_spelled() {
    let src = |attr: &str| {
        format!(
            "module t\n\nview Note(msg: String) !{{}} {{\n    <a {attr}=\"javascript:go()\">x</a>\n}}\n"
        )
    };
    for attr in ["href", "HREF"] {
        let found: Vec<String> = check_sources(&[("t.pw".to_string(), src(attr))])
            .into_iter()
            .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
            .collect();
        assert_eq!(found.len(), 1, "{attr}: {found:#?}");
        assert!(
            found[0].contains("PW5023") && found[0].contains("begins with `javascript:`"),
            "{attr}: {found:#?}"
        );
    }
}
