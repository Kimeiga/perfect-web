//! Every correctness cache keyed by a raw name is a deliberate act.
//!
//! Architect ruling, 2026-08-07, after `docs/RISK_QUEUE.md` 34:
//!
//! > I would add a structural guard against newly introduced correctness caches
//! > keyed solely by raw declaration strings where a resolved ID is available.
//!
//! `Inference::known` was `"Cart" → effects`, so two declarations spelled
//! `Cart` in different modules shared one entry and one silently received the
//! other's facts. It is now `DefId → effects`.
//!
//! This test cannot tell a legitimate name key from a dangerous one — that
//! judgement needs a person. What it can do is make sure nobody adds one
//! without recording the judgement, which is the same shape as the project's
//! other allow-lists.

use std::collections::BTreeMap;

/// Every `BTreeMap<String, ..>` / `HashMap<String, ..>` FIELD in a directory
/// of Rust sources, as `file.rs:field`.
///
/// A free function rather than a closure, so the detector itself can be shown
/// going red — a scan that quietly matched nothing would report a clean crate
/// and a clean crate looks exactly the same.
fn scan(dir: &std::path::Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).expect("readable directory") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).expect("read");
        for line in text.lines() {
            let trimmed = line.trim();
            // A struct FIELD declaration: `name: BTreeMap<String, ..>`.
            let Some((field, rest)) = trimmed.split_once(':') else {
                continue;
            };
            let rest = rest.trim_start();
            if !(rest.starts_with("BTreeMap<String,") || rest.starts_with("HashMap<String,")) {
                continue;
            }
            if field.contains(' ') || field.contains('(') || field.starts_with("//") {
                continue;
            }
            found.push(format!("{file}:{field}"));
        }
    }
    found
}

#[test]
fn the_scan_finds_a_name_keyed_map_that_is_there() {
    // The negative control, and it has to be one: a scan that matched nothing
    // would report every crate clean, and "no undocumented maps" is what a
    // broken detector says too.
    //
    // Written to a temporary directory rather than into the crate, because a
    // deliberately-wrong field in `src/` does not compile and the test would
    // never run to observe it — which is exactly what happened on the first
    // attempt at this control.
    let dir = std::env::temp_dir().join("pw-name-keyed-control");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("sneaky.rs"),
        "struct S {\n    by_name: BTreeMap<String, u32>,\n    fine: BTreeMap<DefId, u32>,\n}\n",
    )
    .expect("write");

    let found = scan(&dir);
    assert_eq!(
        found,
        ["sneaky.rs:by_name"],
        "the scan sees the String key and not the DefId one"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_name_keyed_map_has_a_written_reason() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let allow =
        std::fs::read_to_string(root.join("name-keyed-allow.txt")).expect("name-keyed-allow.txt");
    let allowed: BTreeMap<&str, &str> = allow
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split_once("  "))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();

    let found = scan(&root.join("src"));
    let undocumented: Vec<String> = found
        .iter()
        .filter(|k| !allowed.contains_key(k.as_str()))
        .cloned()
        .collect();

    assert!(
        !found.is_empty(),
        "the scan found nothing, so it is measuring nothing"
    );
    assert!(
        undocumented.is_empty(),
        "these maps are keyed by a raw name and have no reason in \
         `compiler/pw-core/name-keyed-allow.txt`:\n  {}\n\n\
         If a resolved id is available, use it — see `Inference::known` and \
         `docs/RISK_QUEUE.md` 34. If a name is genuinely the right key, say why \
         in the allow-list.",
        undocumented.join("\n  ")
    );

    // And the allow-list may not outlive what it allows: a stale entry is a
    // reason nobody has had to re-justify.
    let stale: Vec<&str> = allowed
        .keys()
        .filter(|k| !found.contains(&k.to_string()))
        .copied()
        .collect();
    assert!(stale.is_empty(), "stale allow-list entries: {stale:?}");
}
