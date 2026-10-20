//! **A document of a page that holds values the device keeps** (track
//! `device-store`, ADR-0366, decision 3).
//!
//! The host never has what the device keeps: it renders each one's declared
//! default, as it renders any signal's first value (ADR-0130), and says in
//! the document what the browser is to read, under which name and at which
//! version (`device`, the page's plan's). Until the browser has read it,
//! nothing it decides is shown: the compiler marks each element holding a
//! part it decides `data-pw-device`, the document's root says it is
//! `reading`, and the head's style keeps a marked element hidden while it
//! is. The runtime reads the device, renders what its values change, and
//! takes the root's mark away in one task, so the first frame that shows a
//! marked element shows the device's value, or the default where the device
//! holds none. A document that will run no script shows the default
//! (`<noscript>`): with scripts off, no device can be read.

/// **What hides a marked element while the document reads the device**: a
/// style a document's policy allows by its hash (ADR-0333).
pub(crate) const READING: &str = "html[data-pw-device=reading] [data-pw-device]{visibility:hidden}";

/// **What shows it where no script will run**, inside `<noscript>`: the
/// declared default, the best a document that reads no device can show.
pub(crate) const UNSCRIPTED: &str =
    "html[data-pw-device=reading] [data-pw-device]{visibility:visible}";

/// Whether `plan`'s page holds a value the device keeps.
pub(crate) fn holds(plan: &serde_json::Value) -> bool {
    plan["device"].as_array().is_some_and(|d| !d.is_empty())
}

/// Whether it holds one at a version after the first, whose steps from an
/// earlier one are in the page's computed module.
pub(crate) fn migrates(plan: &serde_json::Value) -> bool {
    plan["device"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|h| h["version"].as_u64().is_some_and(|v| v > 1))
}

/// **The document's root's attributes**: `lang`, and `data-pw-device=
/// "reading"` where the page holds a value the device keeps.
pub(crate) fn root(plan: &serde_json::Value) -> &'static str {
    match holds(plan) {
        true => " lang=\"en\" data-pw-device=\"reading\"",
        false => " lang=\"en\"",
    }
}

/// **What the head holds for it**: the style that hides what the device
/// decides while it is read, and the one that shows it with scripts off.
pub(crate) fn head(plan: &serde_json::Value) -> String {
    match holds(plan) {
        true => {
            format!("<style>{READING}</style>\n<noscript><style>{UNSCRIPTED}</style></noscript>\n")
        }
        false => String::new(),
    }
}

/// **What the browser is to read**, into the document's manifest: each value
/// the device keeps that the page holds, as the plan states it.
pub(crate) fn with_device(manifest: &mut serde_json::Value, plan: &serde_json::Value) {
    if holds(plan) {
        manifest["device"] = plan["device"].clone();
    }
}
