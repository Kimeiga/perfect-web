//! **A type with a semantic comparison must not also derive a syntactic one.**
//!
//! `ResolvedType` defines `same_as` — *do these two mean the same type?* — and
//! derived `PartialEq` as well. The derive includes `origin`, a span and a
//! spelling, so:
//!
//! ```text
//! a.same_as(&b)   true    the same type, used in two places
//! a == b          false   two spans
//! ```
//!
//! Two answers to one question, in the module written to delete exactly that,
//! shipped in a commit whose message claimed `same_as` was *the ONLY
//! comparison*. A consumer reaching for `==` — which is the reflex, and which
//! the compiler suggests — got *different types* for the same type.
//!
//! I found it by reading the derive while making an unrelated change. That is
//! luck, not method. The crate's other structural guards each caught something
//! real this milestone — the name-keyed-map gate caught `wit.rs:interfaces`,
//! the head-only gate caught `resolved.rs:resolve`, the registry caught an
//! invariant naming an implementation — and none of them covers this shape.
//! This is that gap closed.
//!
//! # What it can and cannot decide
//!
//! It cannot tell a legitimate derive from a dangerous one; that judgement
//! needs a person. What it can do is make sure nobody adds one without
//! recording the judgement, which is the shape every allow-list here has.
//!
//! # Why `PartialEq` — and `Eq`, `Ord`, `Hash`
//!
//! Because `==` is the reflex and `same_as` is the deliberate act. If the wrong
//! one is the easy one, the wrong one gets used — the same reasoning that put
//! `DeclaredType`'s fields behind a module boundary and named its escape hatch
//! `constructor_head_only`.
//!
//! Architect ruling, 2026-08-21, widening it past equality:
//!
//! > There should be exactly one semantic comparison/keying mechanism for
//! > `ResolvedType`. […] extend the ratchet so the next person doesn't solve a
//! > map-key problem by adding `#[derive(Eq, Hash)]` and recreate the same bug
//! > in a different operator.
//!
//! `Ord` and `Hash` are the ones to watch, because they arrive for a reason
//! that feels unrelated: someone needs a `BTreeMap` or a `HashSet` key and
//! reaches for the derive. Over a structure still holding `origin`, that keys
//! by SPAN — two uses of one type land in two buckets, and the map silently
//! holds duplicates that `same_as` would call one.
//!
//! When a semantic key is genuinely needed, it should be a named projection —
//! `semantic_key()`, or a `TypeIdentity` built from `DefId` + semantic
//! arguments, binder + index, or constructor + semantic arguments. **Never
//! provenance.** Not built yet, because nothing needs hashing; this is the
//! guard that keeps the shortcut from being taken instead.

use std::collections::BTreeSet;

/// Types that define a semantic comparison, and whether they also derive a
/// structural one.
///
/// Deliberately a source scan rather than a trait bound: the defect is the
/// PRESENCE of a second comparison, and a trait bound can only constrain one
/// that is already there.
fn offenders(dir: &std::path::Path, out: &mut Vec<String>, seen: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(dir).expect("readable directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            offenders(&path, out, seen);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = text.lines().collect();

        // Which types define a semantic comparison in this file. `same_as` is
        // the spelling this crate uses; the near-misses are listed so a new one
        // does not slip in under a synonym.
        let mut semantic: BTreeSet<String> = BTreeSet::new();
        let mut current: Option<String> = None;
        for line in &lines {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("impl ") {
                // `impl Foo {` or `impl<'a> Foo {` — the type is the last
                // word before the brace.
                let name = rest
                    .trim_end_matches('{')
                    .trim()
                    .rsplit(' ')
                    .next()
                    .unwrap_or_default()
                    .to_string();
                current = (!name.is_empty()).then_some(name);
            }
            for m in ["fn same_as", "fn equivalent", "fn semantically_eq"] {
                if (t.starts_with("pub fn ") || t.starts_with("fn "))
                    && t.contains(m)
                    && let Some(ty) = &current
                {
                    semantic.insert(ty.clone());
                }
            }
        }
        if semantic.is_empty() {
            continue;
        }

        // And which of those also carry a derived `PartialEq`. Read from the
        // derive immediately above the declaration, which is where it always
        // is in this crate.
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            let Some(rest) = t
                .strip_prefix("pub struct ")
                .or_else(|| t.strip_prefix("struct "))
                .or_else(|| t.strip_prefix("pub enum "))
                .or_else(|| t.strip_prefix("enum "))
            else {
                continue;
            };
            let name = rest
                .split([' ', '{', '(', '<', ';'])
                .next()
                .unwrap_or_default()
                .to_string();
            if !semantic.contains(&name) {
                continue;
            }
            // Every derive that makes a SECOND way to compare or key the
            // structure, not only `PartialEq`. `Ord` and `Hash` are the ones a
            // map-key problem invites.
            let derived: Vec<&str> = lines[..i]
                .iter()
                .rev()
                .take(3)
                .filter(|l| l.contains("derive("))
                .flat_map(|l| {
                    ["PartialEq", "Eq", "PartialOrd", "Ord", "Hash"]
                        .into_iter()
                        .filter(|d| {
                            // Whole-word, so `Eq` does not match inside
                            // `PartialEq` and report one derive twice.
                            l.split(|c: char| !c.is_ascii_alphanumeric())
                                .any(|w| w == *d)
                        })
                })
                .collect();
            if derived.is_empty() {
                continue;
            }
            let key = format!("{file}:{name} derives {}", derived.join("+"));
            if seen.insert(key.clone()) {
                out.push(key);
            }
        }
    }
}

/// Recorded exceptions. Empty, and that is the current truth rather than an
/// oversight — nothing in the crate legitimately needs both comparisons today.
const ALLOWED: &[&str] = &[];

#[test]
fn no_type_has_both_a_semantic_and_a_derived_comparison() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    offenders(&src, &mut found, &mut seen);
    found.retain(|f| !ALLOWED.contains(&f.as_str()));
    found.sort();

    assert!(
        found.is_empty(),
        "these types define a semantic comparison AND derive a structural one, \
         so the two can disagree:\n  {}\n\nRemove the derive. If a map or set \
         needs a semantic key, project one — `semantic_key()` built from \
         identity and never from provenance — rather than deriving over a \
         structure that still holds a span. Or record the judgement in ALLOWED \
         with a reason.",
        found.join("\n  ")
    );
}

#[test]
fn the_detector_finds_the_shape_it_is_looking_for() {
    // **A scan that matched nothing would report a clean crate, and a clean
    // crate looks exactly the same.** So the detector is shown going red
    // against the defect it exists for — the one that shipped.
    let dir = std::env::temp_dir().join("pw-one-comparison-control");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("bad.rs"),
        "#[derive(Debug, Clone, PartialEq, Eq)]\n\
         pub struct Thing {\n    span: usize,\n}\n\n\
         impl Thing {\n    pub fn same_as(&self, other: &Thing) -> bool {\n        true\n    }\n}\n",
    )
    .expect("write");

    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    offenders(&dir, &mut found, &mut seen);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(found, ["bad.rs:Thing derives PartialEq+Eq"]);
}

#[test]
fn a_semantic_comparison_without_a_derive_is_not_reported() {
    // The discriminator. Without it the detector could be reporting every type
    // with a `same_as`, which would be a rule against having one at all.
    let dir = std::env::temp_dir().join("pw-one-comparison-ok");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("fine.rs"),
        "#[derive(Debug, Clone)]\n\
         pub struct Thing {\n    span: usize,\n}\n\n\
         impl Thing {\n    pub fn same_as(&self, other: &Thing) -> bool {\n        true\n    }\n}\n",
    )
    .expect("write");

    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    offenders(&dir, &mut found, &mut seen);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn the_type_the_rule_exists_for_still_defines_one() {
    // If `ResolvedType::same_as` were renamed or deleted, the scan above would
    // examine nothing and pass — the vacuity shape, which has appeared six
    // times in this milestone. This is the floor.
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/resolved.rs");
    let text = std::fs::read_to_string(src).expect("resolved.rs");
    assert!(
        text.contains("pub fn same_as"),
        "`ResolvedType::same_as` is what this rule guards; if it moved, move the \
         rule with it"
    );
    assert!(
        !text.contains("PartialEq, Eq)]\n    pub struct ResolvedType"),
        "and it must not have regained a derived comparison"
    );
}

#[test]
fn the_map_key_shortcut_is_what_it_is_really_guarding() {
    // **The case the widening exists for.** Nobody adds `Hash` to defeat
    // `same_as`; they add it because a `HashSet` needs a key. Over a structure
    // still holding `origin`, that keys by SPAN — two uses of one type land in
    // two buckets, and the set holds duplicates `same_as` would call one.
    //
    // Reported without a `PartialEq` in sight, which the first version of this
    // detector would have missed entirely.
    let dir = std::env::temp_dir().join("pw-one-comparison-hash");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("keyed.rs"),
        "#[derive(Debug, Clone, Hash)]\n\
         pub struct Thing {\n    span: usize,\n}\n\n\
         impl Thing {\n    pub fn same_as(&self, other: &Thing) -> bool {\n        true\n    }\n}\n",
    )
    .expect("write");

    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    offenders(&dir, &mut found, &mut seen);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(found, ["keyed.rs:Thing derives Hash"]);
}

#[test]
fn eq_inside_partial_eq_is_not_reported_twice() {
    // `#[derive(PartialEq, Eq)]` is two derives, not three: a substring match
    // would find `Eq` inside `PartialEq` and report a derive that is not there,
    // which is the same class of sloppiness as the `"decl"`-inside-`"declared"`
    // proxy deleted from the evidence test.
    let dir = std::env::temp_dir().join("pw-one-comparison-substring");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("both.rs"),
        "#[derive(PartialEq)]\n\
         pub struct Thing {\n    span: usize,\n}\n\n\
         impl Thing {\n    pub fn same_as(&self, other: &Thing) -> bool {\n        true\n    }\n}\n",
    )
    .expect("write");

    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    offenders(&dir, &mut found, &mut seen);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(found, ["both.rs:Thing derives PartialEq"]);
}
