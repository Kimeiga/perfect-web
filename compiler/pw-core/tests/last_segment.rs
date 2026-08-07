//! No new place may resolve meaning from the last segment of a path.
//!
//! Architect ruling, 2026-08-07:
//!
//! > The test should classify each use as one of: syntax-only spelling
//! > operation (allowed), diagnostic display (allowed), qualified identity
//! > serialization (possibly allowed), semantic resolution from last segment
//! > (FORBIDDEN). Then the allow-list should record the CATEGORY, not merely
//! > prose such as "this is okay".
//!
//! `docs/RISK_QUEUE.md` 34 is what a forbidden one costs: two declarations
//! spelled `Cart` in different modules shared an effect set, and the borrowed
//! fact was plausible for its new owner. 35 is worse — a fixture caught by
//! spelling alone, for a reason unrelated to what it tested.
//!
//! # The ratchet is at zero
//!
//! It was pinned at two — `check.rs`'s privacy-label lookup and `labels.rs`'s
//! `declaration_named` — and both were repaired the same day. The number may
//! only go down, so zero is now the ceiling: a new forbidden site fails
//! immediately, and there is no longer a stock of them to hide one among.

use std::collections::BTreeMap;

const ALLOWED: &[&str] = &["syntax", "display", "encoding", "scoped", "forbidden"];

/// None left. This may only go down, so it stays at zero.
const FORBIDDEN_TODAY: usize = 0;

fn audit() -> BTreeMap<String, (String, String)> {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("last-segment-audit.txt"),
    )
    .expect("last-segment-audit.txt");
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let site = parts.next()?.to_string();
            let category = parts.next()?.to_string();
            let rest = l.split_once(&category).map(|(_, r)| r.trim().to_string())?;
            Some((site, (category, rest)))
        })
        .collect()
}

/// Every `rsplit('.')` in the crate, as `file.rs:enclosing_fn`.
///
/// By FUNCTION rather than by line. The first version keyed on line numbers and
/// `cargo fmt` moved two sites, turning the audit red for no semantic reason —
/// and a guard that fails on formatting teaches people to edit the guard.
fn sites() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("src") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).expect("read");
        let mut enclosing = String::from("<top level>");
        for line in text.lines() {
            let trimmed = line.trim_start();
            // The nearest `fn` above, at any indentation: a nested helper is
            // where the operation actually lives.
            if let Some(rest) = trimmed
                .strip_prefix("pub fn ")
                .or_else(|| trimmed.strip_prefix("pub(crate) fn "))
                .or_else(|| trimmed.strip_prefix("fn "))
                && let Some(name) = rest.split(['(', '<']).next()
            {
                enclosing = name.trim().to_string();
            }
            // The operation itself, not a comment mentioning it.
            if trimmed.contains("rsplit('.')") && !trimmed.starts_with("//") {
                out.push(format!("{file}:{enclosing}"));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn the_scan_finds_the_operation_it_is_looking_for() {
    // The control. A scan that matched nothing would report a clean crate, and
    // "no unclassified sites" is what a broken detector says too.
    let found = sites();
    assert!(
        found.len() >= 8,
        "the scan found {} sites, which is implausibly few: {found:?}",
        found.len()
    );
}

#[test]
fn every_site_is_classified() {
    let audit = audit();
    let found = sites();

    let unclassified: Vec<&String> = found.iter().filter(|s| !audit.contains_key(*s)).collect();
    assert!(
        unclassified.is_empty(),
        "these take the last segment of a path and are not classified in \
         `compiler/pw-core/last-segment-audit.txt`:\n  {}\n\n\
         Classify each as one of {ALLOWED:?}. If it RESOLVES meaning, it is \
         `forbidden` — see `docs/RISK_QUEUE.md` 34.",
        unclassified
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    // A stale entry is a classification nobody has had to re-justify.
    let stale: Vec<&String> = audit.keys().filter(|s| !found.contains(s)).collect();
    assert!(stale.is_empty(), "stale audit entries: {stale:?}");
}

#[test]
fn every_classification_is_one_of_the_declared_categories() {
    for (site, (category, reason)) in audit() {
        assert!(
            ALLOWED.contains(&category.as_str()),
            "{site}: `{category}` is not one of {ALLOWED:?}"
        );
        assert!(
            reason.len() > 40,
            "{site}: the reason must say what it DOES, not that it is fine: {reason:?}"
        );
    }
}

#[test]
fn the_number_of_forbidden_sites_only_goes_down() {
    let forbidden: Vec<String> = audit()
        .into_iter()
        .filter(|(_, (c, _))| c == "forbidden")
        .map(|(s, _)| s)
        .collect();

    assert_eq!(
        forbidden.len(),
        FORBIDDEN_TODAY,
        "forbidden sites are {forbidden:?}.\n\n\
         If you ADDED one: resolve the name instead — `Workspace::resolve_path`, \
         or the scoped-and-unique branch in `effects.rs`.\n\
         If you REPAIRED one: lower `FORBIDDEN_TODAY`. It may only go down."
    );
}
