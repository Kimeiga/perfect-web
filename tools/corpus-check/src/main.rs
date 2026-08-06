//! corpus-check — validates the accepted/rejected specification corpus.
//!
//! The corpus (charter §16) is the executable specification: "Add examples
//! before features." Until Milestone 2 there is no `pw` compiler, so nothing can
//! actually *compile* these files. What can be checked today, and what this tool
//! checks, is that the corpus is well-formed and that it covers every category
//! the charter enumerates.
//!
//! From Milestone 2 onward this tool gains a second phase: run `pw check` over
//! each file and assert that accepted files produce no diagnostics and rejected
//! files produce exactly the declared `@rule` code at the declared span.
//!
//! Usage: corpus-check <examples-dir>
//! Exit:  0 = corpus valid and complete, 1 = problems found.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Charter §16.1. Every one of these must be covered by >=1 accepted example.
const ACCEPTED_CATEGORIES: &[&str] = &[
    "pure calculation",
    "exhaustive order-state rendering",
    "public store query",
    "private cart query",
    "idempotent command",
    "scoped WebSocket subscription",
    "map widget resource with cleanup",
    "streamed public recommendations",
    "edge materialized menu",
    "origin-only secret operation",
    "offline draft with explicit conflict policy",
    "decoder handling malformed JSON",
    "build-time deterministic page",
    "content-addressed resumable handler",
    // Charter v2 §7.5A layout/DOM phase families. Added because v2 introduced
    // eight effect families without extending §16's category lists, which
    // contradicted §16's own "add examples before features" rule.
    "phase-scheduled measure then mutate",
    "multiple measures share one layout snapshot",
    "batched layout-affecting mutations",
    "resize observation updates derived state",
    "intersection observation controls resource lifetime",
    "compositor-only animation",
    "custom paint with declared inputs",
    "post-paint non-layout work",
    "semantically independent contained subtree",
    "audited imperative widget escape",
];

/// Charter §16.2: "Create at least one minimal file for each".
const REJECTED_CATEGORIES: &[&str] = &[
    "network request inside view",
    "database access in browser",
    "secret serialized to browser",
    "private cart in shared cache",
    "cross-tenant cache key missing tenant",
    "public log of secret value",
    "unhandled ADT variant",
    "ambient null assumption",
    "unchecked external cast",
    "nonserializable handler capture",
    "open transaction not committed or rolled back",
    "affine resource leaked",
    "detached ordinary task",
    "retry of non-idempotent command",
    "unbounded retry",
    "nondeterministic static render",
    "wall-clock read in shared deterministic materialization",
    "unkeyed mutable list",
    "invalid HTML nesting",
    "button behavior implemented on inaccessible div",
    "label missing from form control",
    "form handler type mismatch",
    "dead internal route",
    "raw unsafe HTML without capability",
    "browser-only API on origin",
    "server secret referenced by edge artifact",
    "query key changes without stale-work cancellation policy",
    "subscription outlives component scope",
    "optimistic state with no rollback path",
    "private data in resume manifest",
    "unsafe escape hatch without justification",
    // Charter v2 §7.5A. Note the three-way discipline the architect required:
    // every effect family needs one accepted use, one DIRECT rejected use, and
    // one rejected use HIDDEN behind a helper or generic callback. The last is
    // the one that matters — a checker that only catches direct calls is worth
    // approximately nothing.
    "synchronous geometry read in ordinary code",
    "geometry read during render",
    "layout read after layout-affecting write",
    "layout write during measure phase",
    "measure helper hides forbidden geometry read",
    "generic callback smuggles layout measure",
    "resize observer layout feedback cycle",
    "intersection observer unscoped subscription",
    "layout-affecting operation in compositor animation",
    "custom paint mutates document",
    "post-paint same-frame layout measure",
    "containment with cross-boundary layout dependency",
    "unaudited imperative dom escape",
    // E6, corpus C3. Two invariants the milestone introduced that no existing
    // category names, added on the architect's ruling of 2026-08-06:
    //
    //   > A new user-facing language invariant introduced by E6 should have at
    //   > least one canonical accepted/rejected specification pair. Otherwise
    //   > your dashboard eventually says `generality-tested 29 / 29` while the
    //   > compiler actually contains 31 or 32 semantic invariants.
    //
    // The denominator has to grow, or it stops meaning "all invariants".
    "private dependency of a shared materialization",
    "dependency graph edge to an undeclared target",
];

/// Milestone 0 gate (charter §14 M0): ">= 10 accepted and 20 rejected".
const GATE_MIN_ACCEPTED: usize = 10;
const GATE_MIN_REJECTED: usize = 20;

#[derive(Debug)]
struct Example {
    fields: BTreeMap<String, Vec<String>>,
}

impl Example {
    fn one(&self, key: &str) -> Option<&str> {
        self.fields
            .get(key)
            .and_then(|v| v.first())
            .map(|s| s.as_str())
    }
    fn many(&self, key: &str) -> &[String] {
        self.fields.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

fn parse_header(text: &str) -> Example {
    let mut fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix("// @") else {
            // The header must be a contiguous block at the top of the file.
            if line.starts_with("//") || line.is_empty() {
                continue;
            }
            break;
        };
        let Some((k, v)) = rest.split_once(':') else {
            continue;
        };
        fields
            .entry(k.trim().to_string())
            .or_default()
            .push(v.trim().to_string());
    }
    Example { fields }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out)?;
        } else if p.extension().is_some_and(|x| x == "pw") {
            out.push(p);
        }
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "examples".to_string());
    let root = Path::new(&root);

    let mut problems: Vec<String> = Vec::new();
    let mut report = String::new();

    for (bucket, categories, min) in [
        ("accepted", ACCEPTED_CATEGORIES, GATE_MIN_ACCEPTED),
        ("rejected", REJECTED_CATEGORIES, GATE_MIN_REJECTED),
    ] {
        let dir = root.join(bucket);
        let mut files = Vec::new();
        if let Err(e) = collect(&dir, &mut files) {
            problems.push(format!("cannot read {}: {e}", dir.display()));
            continue;
        }

        let mut seen_ids: BTreeSet<String> = BTreeSet::new();
        let mut covered: BTreeSet<String> = BTreeSet::new();

        for path in &files {
            let text = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    problems.push(format!("{}: cannot read: {e}", path.display()));
                    continue;
                }
            };
            let ex = parse_header(&text);
            let where_ = path
                .strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string();

            // --- required fields, same for both buckets ---
            match ex.one("corpus") {
                Some(c) if c == bucket => {}
                Some(c) => problems.push(format!(
                    "{where_}: @corpus is `{c}` but the file is under {bucket}/"
                )),
                None => problems.push(format!("{where_}: missing `// @corpus: {bucket}`")),
            }
            match ex.one("id") {
                Some(id) => {
                    if !seen_ids.insert(id.to_string()) {
                        problems.push(format!("{where_}: duplicate @id `{id}`"));
                    }
                }
                None => problems.push(format!("{where_}: missing `// @id:`")),
            }
            match ex.one("category") {
                Some(cat) => {
                    if categories.contains(&cat) {
                        covered.insert(cat.to_string());
                    } else {
                        problems.push(format!(
                            "{where_}: @category `{cat}` is not one of the charter §16 categories"
                        ));
                    }
                }
                None => problems.push(format!("{where_}: missing `// @category:`")),
            }
            if ex.one("charter").is_none() {
                problems.push(format!(
                    "{where_}: missing `// @charter:` (which charter section this encodes)"
                ));
            }
            match ex.one("milestone").map(|m| m.parse::<u32>()) {
                Some(Ok(m)) if m <= 15 => {}
                Some(Ok(m)) => {
                    problems.push(format!("{where_}: @milestone {m} is out of range 0..=15"))
                }
                Some(Err(_)) => problems.push(format!("{where_}: @milestone must be a number")),
                None => problems.push(format!(
                    "{where_}: missing `// @milestone:` (which milestone makes this enforceable)"
                )),
            }

            // --- bucket-specific ---
            if bucket == "rejected" {
                if ex.many("expect-error").is_empty() {
                    problems.push(format!(
                        "{where_}: rejected examples need >=1 `// @expect-error:`"
                    ));
                }
                if ex.one("rule").is_none() {
                    problems.push(format!(
                        "{where_}: rejected examples need a `// @rule:` diagnostic code"
                    ));
                }
            } else if !ex.many("expect-error").is_empty() {
                problems.push(format!(
                    "{where_}: accepted examples must not declare `@expect-error`"
                ));
            }

            if text.lines().count() < 3 {
                problems.push(format!("{where_}: file has no body below the header"));
            }
        }

        // --- gate arithmetic ---
        let _ = writeln!(report, "{bucket}: {} files", files.len());
        if files.len() < min {
            problems.push(format!(
                "gate: {bucket} corpus has {} files, charter Milestone 0 requires >= {min}",
                files.len()
            ));
        }

        let missing: Vec<&&str> = categories
            .iter()
            .filter(|c| !covered.contains(**c))
            .collect();
        let _ = writeln!(
            report,
            "{bucket}: {}/{} charter §16 categories covered",
            covered.len(),
            categories.len()
        );
        for m in &missing {
            problems.push(format!("{bucket}: charter §16 category not covered: `{m}`"));
        }
    }

    print!("{report}");
    if problems.is_empty() {
        println!("corpus-check: OK");
        std::process::ExitCode::SUCCESS
    } else {
        println!();
        for p in &problems {
            println!("corpus-check: {p}");
        }
        println!("\ncorpus-check: {} problem(s)", problems.len());
        std::process::ExitCode::from(1)
    }
}
