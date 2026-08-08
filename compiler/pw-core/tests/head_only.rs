//! **Discarding a type's arguments is a listed act.**
//!
//! Architect ruling, 2026-08-08, after three independent consumers made the same
//! mistake in one day:
//!
//! > Three independent consumers have now made the same mistake […] and two
//! > failed permissively. That means the representation itself is too easy to
//! > misuse. […] Preserving generic arguments should be effortless. Discarding
//! > them should require an explicit choice.
//!
//! ```text
//! Interface::of      List<OpenTransaction> -> List -> no resource -> remotable
//! wit.rs fields      List<MenuItem>        -> list (no argument)   wit-parser caught it
//! koka.rs type_name  List<CartLine>        -> list (no argument)   Koka caught it
//! ```
//!
//! Two of the three failed in the permissive direction, and neither was found by
//! a test — one by an external parser, one by an external compiler, one by
//! reading the emitted artifact.
//!
//! # Why this is not a proximity grep
//!
//! The ruling was explicit that the primary protection should not be
//! "`.ty` must have `ty_args` nearby". It is the representation:
//! `hir::DeclaredType` keeps the head and arguments together with PRIVATE
//! fields, in its own module so the rest of the crate must ask.
//! [`written()`] is the easy path and `constructor_head_only()` is deliberately
//! unpleasant to type.
//!
//! This test is the second line: it asserts the unpleasant one appears only
//! where somebody wrote down why. That is the same shape as `last_segment.rs`
//! and `name_keyed_maps.rs`, and it is much stronger than scanning for a field
//! access somebody has already made wrong.
//!
//! [`written()`]: pw_core::hir::DeclaredType::written

use std::collections::BTreeSet;

/// Every `constructor_head_only()` call site in the crate, as `file.rs:fn`.
///
/// By FUNCTION rather than by line, for `last_segment.rs`'s reason: keying on
/// line numbers means `cargo fmt` turns the audit red for no semantic reason,
/// and a guard that fails on formatting teaches people to edit the guard.
fn sites() -> BTreeSet<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("src") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        // `hir.rs` DECLARES it. A declaration is not a use.
        if file == "hir.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read");
        let mut current = String::from("(top level)");
        for line in text.lines() {
            let t = line.trim_start();
            if let Some(rest) = t
                .strip_prefix("pub fn ")
                .or_else(|| t.strip_prefix("fn "))
                .or_else(|| t.strip_prefix("pub(crate) fn "))
            {
                current = rest
                    .split(['(', '<'])
                    .next()
                    .unwrap_or("?")
                    .trim()
                    .to_string();
            }
            // A comment mentioning the method is prose, not a call.
            if t.starts_with("//") {
                continue;
            }
            if line.contains("constructor_head_only(") {
                out.insert(format!("{file}:{current}"));
            }
        }
    }
    out
}

fn allowed() -> BTreeSet<String> {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("head-only-allow.txt"),
    )
    .expect("head-only-allow.txt");
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect()
}

#[test]
fn every_head_only_call_has_a_written_reason() {
    let found = sites();
    let allowed = allowed();
    let undocumented: Vec<&String> = found.difference(&allowed).collect();
    assert!(
        undocumented.is_empty(),
        "these discard a type's arguments with no reason in \
         `compiler/pw-core/head-only-allow.txt`:\n  {}\n\n\
         `DeclaredType::written()` keeps the whole type and is what almost every \
         consumer wants. `List<OpenTransaction>` read as `List` found no \
         resource and came out remotely transferable; the same shape emitted \
         `list` with no argument twice more. If the CONSTRUCTOR is genuinely the \
         question, say so in the allow-list.",
        undocumented
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn the_allow_list_has_no_entries_for_sites_that_are_gone() {
    // A ratchet only ratchets if it is trimmed. An entry for a deleted call is
    // a reason nobody can check, and it makes the list read longer than the
    // problem is.
    let found = sites();
    let allowed = allowed();
    let stale: Vec<&String> = allowed.difference(&found).collect();
    assert!(
        stale.is_empty(),
        "the allow-list documents call sites that no longer exist: {stale:?}"
    );
}

#[test]
fn the_scan_finds_a_call_that_is_there() {
    // The control, and it has to be one: a scan that matched nothing would
    // report a clean crate, and "no undocumented discards" is what a broken
    // detector says too.
    let found = sites();
    assert!(
        !found.is_empty(),
        "the scan found no call sites at all, which is not what the crate contains"
    );
    assert!(
        found.iter().any(|s| s.starts_with("signatures.rs:")),
        "the audited receiver-type discard is in `signatures.rs`: {found:?}"
    );
}
