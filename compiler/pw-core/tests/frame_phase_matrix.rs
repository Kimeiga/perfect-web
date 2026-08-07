//! **A frame phase says WHEN work runs. An effect says WHAT it does.**
//!
//! Architect ruling, 2026-08-07:
//!
//! > `mutate` is a scheduling context, not shorthand for
//! > `style.mutate<LayoutAffect>`.
//!
//! and:
//!
//! > A frame-phase block says when this body's work executes. An effect says
//! > what that work actually does. Those are independent. […] I would
//! > therefore remove `intrinsic_effect` for frame-phase keywords rather than
//! > repair its arities.
//!
//! `effects.rs::intrinsic_effect` maps four phase keywords to effect spellings,
//! so `mutate { pure_computation() }` claims it mutated style and
//! `measure { pure_computation() }` claims it read layout. Both are false, and
//! one of them — `"mutate" => "style.mutate"` — now has the wrong arity as
//! well, because the declaration binds a parameter.
//!
//! # This file is the instrument, frozen before the change
//!
//! Same discipline as `contract_matrix.rs`, and for the same reason: the last
//! authority change was attributable only because the matrix predated it, and
//! it caught a live defect on its first run that the change would otherwise
//! have silently repaired into the baseline.
//!
//! Every `..._today` assertion recorded what the compiler did BEFORE the
//! change; each is now renamed to what it does after, with the before-value in
//! the comment. Removing `intrinsic_effect` turned exactly the predicted rows
//! red — and two that were not predicted, which is what the matrix was for.
//!
//! # What the two unpredicted failures were
//!
//! **A bare `measure(el)` never resolved to `style.measure`.** `measure` is a
//! phase keyword, so the parser makes `measure(el)` a keyword expression rather
//! than a call — and `intrinsic_effect` then supplied `layout.measure` anyway.
//! The right effect, from a mechanism unrelated to the call. Every row here now
//! writes `style.measure(el)`; no corpus file calls a phase-keyword-named
//! function bare, so the corpus never depended on it. `docs/RISK_QUEUE.md`.
//!
//! **`forbidden_in_phase("animate", ..)` and `("post_paint", ..)` are
//! unreachable.** `contexts::elsewhere` classifies `post_paint`, `frame` and
//! `animate` as FramePhase regions and `effect_rows` drops every source inside
//! one before the phase loop runs, so the effect never reaches the rule that
//! forbids it. `mutate` and `measure` are not in that list, which is the only
//! reason those two neighbours work. Recorded rather than repaired: it predates
//! this change and fixing it is a separate question about what "elsewhere"
//! means for a phase that runs in the same frame.
//!
//! # The rows the ruling requires
//!
//! ```text
//! mutate     { pure() }                    effects {}
//! mutate     { paint_only_write() }        effects { style.mutate<PaintOnly> }
//! mutate     { layout_affecting_write() }  effects { style.mutate<LayoutAffect> }
//! measure    { pure() }                    effects {}
//! measure    { read_geometry() }           effects { layout.measure }
//! post_paint { pure() }                    effects {}
//! animate    { compositor_operation() }    effects { animation.composite }
//! ```
//!
//! plus invalid neighbours proving **phase legality still works independently
//! of effect inference**: removing the synthesized effects must not accidentally
//! disable the frame-phase rules.

use std::collections::BTreeSet;

use pw_core::effects::Inference;
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// The real platform, so the operations inside each block are the ones the
/// corpus uses. A fixture library would freeze a copy of the vocabulary and
/// the matrix would then verify the copy.
fn platform() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out: Vec<String> = ["packages/pw-std", "packages/pw-platform-web"]
        .iter()
        .flat_map(|d| {
            let mut v: Vec<String> = std::fs::read_dir(root.join(d))
                .expect("package")
                .filter_map(|e| {
                    let p = e.expect("entry").path();
                    (p.extension()? == "pw").then(|| std::fs::read_to_string(&p).expect("read"))
                })
                .collect();
            v.sort();
            v
        })
        .collect();
    out.push(std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"));
    out
}

/// A component whose `body` is one phase block.
fn component(body: &str) -> String {
    format!(
        "module app\n\n\
         import style\n\
         import browser.{{ Element, Style }}\n\n\
         fn pure(x: Int) -> Int !{{}} {{ x }}\n\n\
         component Panel(el: Element, s: Style) {{\n\
         \x20   placement browser\n\
         \x20   {body}\n\
         \x20   view {{ <main></main> }}\n\
         }}\n"
    )
}

/// The effects inferred for the component's body, as written text.
fn effects_of(src: &str) -> BTreeSet<String> {
    let mut sources = platform();
    sources.push(src.to_string());
    let hirs: Vec<Hir> = sources
        .iter()
        .map(|s| lower_file(s, &parse_tree(s).green))
        .collect();
    let refs: Vec<&Hir> = hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let mut inf = Inference::new(&sigs, &ws);
    inf.run(&refs);

    let unit = refs.len() - 1;
    let hir = refs[unit];
    let (_, decl) = hir
        .all_decls()
        .find(|(_, d)| d.name == "Panel")
        .expect("Panel");
    let body = decl.body.expect("a body");
    inf.infer_at(unit, hir.body(body)).effects
}

/// Diagnostic codes for one program, from the whole checker.
fn codes(src: &str) -> Vec<String> {
    let mut sources: Vec<(String, String)> = platform()
        .into_iter()
        .enumerate()
        .map(|(i, s)| (format!("p{i}.pw"), s))
        .collect();
    sources.push(("user.pw".to_string(), src.to_string()));
    pw_core::check::check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "user.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| d.code.to_string()))
        .collect()
}

// --- the harness itself ------------------------------------------------------

#[test]
fn the_matrix_program_checks_clean_before_any_phase_block_is_added() {
    // The control. Every row asserts on one component, and a harness whose
    // base program did not even resolve would give each of them an empty
    // effect set for reasons that have nothing to do with phases.
    let src = component("view_helper { }");
    let found = codes(&src);
    // `view_helper` is not a phase keyword and not a declaration; what matters
    // is that the imports and the component resolve.
    assert!(
        !found.iter().any(|c| c.starts_with("PW00")),
        "the base program must resolve: {found:?}"
    );
}

#[test]
fn the_operations_the_matrix_uses_carry_the_effects_it_claims() {
    // Without this every row below could pass by measuring nothing. These are
    // the platform's own rows, read through inference rather than restated.
    assert_eq!(
        effects_of(&component("mutate { style.set_width(s, \"1px\") }")),
        BTreeSet::from(["style.mutate<LayoutAffect>".to_string(),])
    );
}

// --- the seven rows, as the compiler answers them TODAY ----------------------

#[test]
fn an_empty_mutate_phase_performs_nothing() {
    // **The row the ruling is about.** Was `{style.mutate}`; nothing in this
    // block writes anything, and now it says so.
    assert!(effects_of(&component("mutate { pure(1) }")).is_empty());
}

#[test]
fn a_paint_only_write_carries_exactly_its_own_effect() {
    // Was `{style.mutate<PaintOnly>, style.mutate}` — the operation's real
    // effect and the keyword's invented one, at two different arities.
    let found = effects_of(&component("mutate { style.set_custom(s, \"--x\", \"1\") }"));
    assert_eq!(
        found,
        BTreeSet::from(["style.mutate<PaintOnly>".to_string()])
    );
}

#[test]
fn a_layout_affecting_write_carries_exactly_its_own_effect() {
    // Was `{style.mutate<LayoutAffect>, style.mutate}`.
    let found = effects_of(&component("mutate { style.set_width(s, \"1px\") }"));
    assert_eq!(
        found,
        BTreeSet::from(["style.mutate<LayoutAffect>".to_string()])
    );
}

#[test]
fn an_empty_measure_phase_performs_nothing() {
    // Was `{layout.measure}`.
    assert!(effects_of(&component("measure { pure(1) }")).is_empty());
}

#[test]
fn a_geometry_read_carries_layout_measure_on_its_own() {
    // The one row that is already right, and must stay right: the effect comes
    // from `style.measure`'s declared row, not from the keyword.
    let found = effects_of(&component("measure { style.measure(el) }"));
    assert!(found.contains("layout.measure"), "{found:?}");
}

#[test]
fn an_empty_post_paint_phase_performs_nothing() {
    // Was `{paint.post}` — an effect NO corpus row ever wrote, existing only
    // because the keyword invented it.
    assert!(effects_of(&component("post_paint { pure(1) }")).is_empty());
}

#[test]
fn an_empty_animate_phase_performs_nothing() {
    // Was `{animation.composite}`.
    assert!(effects_of(&component("animate { pure(1) }")).is_empty());
}

#[test]
fn a_compositor_operation_carries_its_own_effect() {
    let found = effects_of(&component(
        "animate { style.composite(s, \"opacity\", \"1\") }",
    ));
    assert!(found.contains("animation.composite"), "{found:?}");
}

// --- phase legality, which must survive the change ---------------------------
//
// The invalid neighbours. Removing the synthesized effects must not disable
// the frame-phase rules: each of these is illegal because of the phase/effect
// COMBINATION, and the effect in every one comes from a real operation.

#[test]
fn a_layout_affecting_write_inside_a_measure_phase_is_rejected() {
    // Charter §7.5A: the measure phase reads geometry, and writing inside it
    // invalidates what the rest of the phase is about to read. The effect here
    // is `style.mutate<LayoutAffect>` from `set_width`, so this must keep
    // failing when the keyword stops synthesizing anything.
    let found = codes(&component("measure { style.set_width(s, \"1px\") }"));
    assert!(
        found.contains(&"PW0402".to_string()),
        "a write in the measure phase is `wrong_frame_phase`: {found:?}"
    );
}

#[test]
fn a_geometry_read_inside_a_post_paint_phase_is_rejected() {
    // R-042's shape. The frame is already presented; measuring forces a second
    // layout for a frame nobody will see. The effect comes from
    // `style.measure`'s declared row, so this survives the synthesis being
    // deleted — which is the property the ruling asked to be proved.
    let found = codes(&component("post_paint { style.measure(el) }"));
    assert!(
        found.contains(&"PW0402".to_string()),
        "a measurement after paint is `wrong_frame_phase`: {found:?}"
    );
}

#[test]
fn a_geometry_read_inside_an_animate_phase_is_rejected() {
    // The `animate` neighbour, with the effect the rule actually names.
    // `forbidden_in_phase` keys on the effect's FAMILY, and the rule is
    // `("animate", "layout")` — so it is `layout.measure` that a compositor
    // animation may not do.
    let found = codes(&component("animate { style.measure(el) }"));
    assert!(
        found.contains(&"PW0402".to_string()),
        "measuring inside a compositor animation is `wrong_frame_phase`: {found:?}"
    );
}

#[test]
fn a_layout_affecting_write_inside_an_animate_phase_is_rejected() {
    // **The gap semantic facets closed.**
    //
    // `forbidden_in_phase` keyed on an effect's FAMILY, and
    // `style.mutate<LayoutAffect>` has family `style` — so the rule
    // `("animate", "layout")` could not see the exact case its own comment
    // described. It now asks whether the effect has the `layout_write` facet,
    // which `style.mutate<T>` declares conditionally on its argument:
    //
    // ```pleris
    // effect style.mutate<T> {
    //     impact paint_write
    //     impact layout_write when LayoutAffect
    // }
    // ```
    let found = codes(&component("animate { style.set_width(s, \"1px\") }"));
    assert!(
        found.contains(&"PW0402".to_string()),
        "animating a layout-affecting property forces a layout every frame: {found:?}"
    );
}

#[test]
fn a_pure_phase_block_is_legal_in_every_phase() {
    // The positive neighbour for all three above. Without it, "rejected" could
    // mean the rule fires on any phase block at all.
    for phase in ["mutate", "measure", "post_paint", "animate"] {
        let found = codes(&component(&format!("{phase} {{ pure(1) }}")));
        assert!(
            !found.contains(&"PW3008".to_string()),
            "`{phase} {{ pure(1) }}` does nothing a phase forbids: {found:?}"
        );
    }
}

#[test]
fn a_paint_only_write_is_legal_in_the_mutate_phase() {
    // The mutate phase is FOR writing. The distinction the corpus turns on is
    // which write, and a `PaintOnly` one is legal wherever a style write is.
    let found = codes(&component("mutate { style.set_custom(s, \"--x\", \"1\") }"));
    assert!(!found.contains(&"PW3008".to_string()), "{found:?}");
}

// --- the structural gate -----------------------------------------------------

/// **No correctness analysis may infer an effect from a phase keyword's
/// spelling.**
///
/// Architect ruling, 2026-08-07:
///
/// > Phase keywords may create an `ExecutionContext`; actual `EffectInstance`s
/// > must originate from resolved operations in the body.
///
/// A behavioural test cannot prove that absence — it can only show that the
/// programs it happens to write do not do it. So this is structural: it scans
/// the crate for a phase keyword appearing next to an effect spelling, which is
/// what `intrinsic_effect` was.
#[test]
fn no_source_file_maps_a_phase_keyword_to_an_effect_spelling() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let phases = ["measure", "mutate", "post_paint", "animate"];
    let mut offenders = Vec::new();
    let mut scanned = 0usize;

    for e in std::fs::read_dir(&dir).expect("src") {
        let p = e.expect("entry").path();
        if p.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        scanned += 1;
        let file = p.file_name().unwrap().to_string_lossy().to_string();
        for (n, line) in std::fs::read_to_string(&p)
            .expect("read")
            .lines()
            .enumerate()
        {
            let l = line.trim();
            if l.starts_with("//") || !l.contains("=>") {
                continue;
            }
            // `"measure" => "layout.measure"` — a phase keyword on the left of
            // a match arm, an effect spelling on the right.
            let Some((lhs, rhs)) = l.split_once("=>") else {
                continue;
            };
            if phases.iter().any(|p| lhs.contains(&format!("\"{p}\"")))
                && rhs.contains('.')
                && rhs.contains('"')
            {
                offenders.push(format!("{file}:{}: {l}", n + 1));
            }
        }
    }

    assert!(
        scanned >= 25,
        "the scan read {scanned} files, which is implausibly few"
    );
    assert!(
        offenders.is_empty(),
        "a phase keyword is being mapped to an effect spelling:\n  {}\n\n\
         A frame phase says WHEN work runs. An `EffectInstance` must come from \
         a resolved operation in the body — `phases_at` gives the execution \
         context, and `effective_effects` gives the effects.",
        offenders.join("\n  ")
    );
}

#[test]
fn the_gate_can_detect_the_thing_it_forbids() {
    // The control for the scan above, since "no offenders" is what a broken
    // detector says too. This is `intrinsic_effect`'s exact shape.
    let sample = r#"        "measure" => "layout.measure","#;
    let (lhs, rhs) = sample.split_once("=>").expect("an arm");
    assert!(lhs.contains("\"measure\""));
    assert!(rhs.contains('.') && rhs.contains('"'));
}

// --- phase impact, frozen before facets ---------------------------------------
//
// Architect ruling, 2026-08-07:
//
// > `style.mutate<LayoutAffect>` is not a "layout-family" effect, but it IS
// > layout-affecting. So `forbidden_in_phase("animate", "layout")` is now too
// > crude. […] give resolved `EffectInstance`s semantic traits/facets that
// > phase checking consumes.
//
// The rows below are the ruling's matrix. One was wrong when it was written —
// a layout-affecting style write in the animate phase was allowed — because the
// rule keyed on an effect's FAMILY, and `style.mutate<LayoutAffect>` is in the
// `style` family. `forbidden_in_phase` now reads declared FACETS, and the two
// `style.mutate` rows differ while sharing a family, which is the whole point.

/// Does this program report a wrong-frame-phase error?
fn wrong_phase(body: &str) -> bool {
    codes(&component(body)).contains(&"PW0402".to_string())
}

#[test]
fn the_phase_impact_matrix() {
    // allowed, and must stay allowed
    assert!(
        !wrong_phase("animate { style.composite(s, \"opacity\", \"1\") }"),
        "a compositor operation is what the animate phase is FOR"
    );
    assert!(
        !wrong_phase("measure { pure(1) }"),
        "a pure measure phase does nothing"
    );
    assert!(
        !wrong_phase("measure { style.measure(el) }"),
        "reading geometry is what the measure phase is FOR"
    );

    // rejected, and already correct
    assert!(
        wrong_phase("animate { style.measure(el) }"),
        "a compositor animation may not force layout"
    );
    assert!(
        wrong_phase("measure { style.set_width(s, \"1px\") }"),
        "writing inside the measure phase invalidates what it is about to read"
    );

    // **The gap semantic facets closed.** Was allowed; the family was the same
    // as a paint-only write's, so no family-keyed rule could tell them apart.
    assert!(
        wrong_phase("animate { style.set_width(s, \"1px\") }"),
        "a layout-affecting write in the animate phase forces a layout every frame"
    );

    // And the row that proves the fix is not collateral damage: a paint-only
    // write shares the family and must stay legal.
    assert!(
        !wrong_phase("animate { style.set_custom(s, \"--x\", \"1\") }"),
        "a paint-only write does not invalidate layout"
    );
}

#[test]
fn a_family_only_check_cannot_see_a_layout_affecting_write() {
    // **The negative control the ruling asks for**, stated against the rule
    // itself rather than through a program: a checker that looks only at the
    // `layout` family gives the same answer for a layout-affecting style write
    // as for a paint-only one. That is the defect, in one assertion.
    use pw_core::effects::family_of;
    assert_eq!(family_of("style.mutate<LayoutAffect>"), "style");
    assert_eq!(family_of("style.mutate<PaintOnly>"), "style");
    assert_eq!(
        family_of("style.mutate<LayoutAffect>"),
        family_of("style.mutate<PaintOnly>"),
        "the family cannot distinguish them, so no rule keyed on it can"
    );
    // Whereas `layout.measure` is a different family, which is the ONLY reason
    // the animate/layout rule ever fires.
    assert_eq!(family_of("layout.measure"), "layout");
}

// --- the two integrity guards ------------------------------------------------

#[test]
fn an_impact_condition_that_names_nothing_is_a_build_error() {
    // Architect ruling, 2026-08-07:
    //
    // > Silently dropping `impact layout_write when LayoutAffect` because
    // > `LayoutAffect` failed to resolve recreates the same class of problem
    // > that `LayoutAffect` already exposed: source looks meaningful, compiler
    // > quietly assigns it no meaning.
    //
    // It was inert for one commit. Inert is better than a textual fallback and
    // still wrong: a package could lose an import and the facet would stop
    // applying with nothing said.
    let broken = "module p\n\nprelude Effect\n\n\
                  effect style.mutate<T> {\n    \
                  capability none\n    \
                  impact     layout_write when NoSuchMarker\n}\n";
    let found = pw_core::check::check_sources(&[("p.pw".to_string(), broken.to_string())]);
    let codes: Vec<&str> = found
        .iter()
        .flat_map(|(_, ds)| ds.iter().map(|d| d.code))
        .collect();
    assert!(codes.contains(&"PW5204"), "{codes:?}");

    // The neighbour: the same declaration with the marker declared here.
    let ok = "module p\n\nprelude Effect\n\ntype LayoutAffect = LayoutAffect {}\n\n\
              effect style.mutate<T> {\n    \
              capability none\n    \
              impact     layout_write when LayoutAffect\n}\n";
    let found = pw_core::check::check_sources(&[("p.pw".to_string(), ok.to_string())]);
    let codes: Vec<&str> = found
        .iter()
        .flat_map(|(_, ds)| ds.iter().map(|d| d.code))
        .collect();
    assert!(!codes.contains(&"PW5204"), "{codes:?}");
}

/// **Every policy head written in a package must be one the parser knows.**
///
/// Architect ruling, 2026-08-07, after `impact` was written in
/// `packages/pw-platform-web/effects.pw` for a whole commit while
/// `POLICY_KEYWORDS` did not contain it:
///
/// > That is too dangerous to leave as convention.
///
/// An unregistered policy keyword parses as nothing. The clause vanishes, the
/// declaration still checks clean, and every rule reading that clause quietly
/// gets an empty answer — the phase rules stopped firing entirely and nothing
/// errored. It is the policy-clause equivalent of the diagnostic registry.
///
/// **Directional**: `POLICY_KEYWORDS` is the source of truth and package source
/// may only use heads from it. There is deliberately no second list of expected
/// package policy names to drift against the first.
#[test]
fn every_policy_head_written_in_a_package_is_one_the_parser_knows() {
    let (unknown, scanned) = package_policy_heads();
    assert!(
        scanned >= 15,
        "the scan read {scanned} effect declarations, which is implausibly few"
    );
    assert!(
        unknown.is_empty(),
        "these policy clauses are written in a package and the parser discards \
         them, silently:\n  {}\n\n\
         Add the keyword to `pw_syntax::grammar::POLICY_KEYWORDS`, or fix the \
         spelling. A clause the parser does not know does not become a `Policy` \
         node at all, so every rule that reads it sees nothing and says nothing.",
        unknown.join("\n  ")
    );
}

#[test]
fn the_policy_head_guard_can_detect_an_unknown_clause() {
    // The control. "No unknown heads" is what a broken scanner says too.
    let heads =
        heads_of("module p\n\neffect a.b {\n    capability none\n    impakt layout_write\n}\n");
    assert!(heads.contains(&"capability".to_string()), "{heads:?}");
    assert!(
        heads.contains(&"impakt".to_string()),
        "the scan must SEE the misspelled head, or it cannot judge it: {heads:?}"
    );
    assert!(!pw_syntax::grammar::POLICY_KEYWORDS.contains(&"impakt"));
}

/// Every clause head inside an `effect { .. }` block, from source text.
///
/// From the TEXT rather than from `Policy` nodes, deliberately: an unrecognised
/// head produces no node, so a scan over the tree would be blind to exactly the
/// thing this looks for.
fn heads_of(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    for line in src.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with("//") {
            continue;
        }
        if depth > 0
            && let Some(head) = l.split_whitespace().next()
            && head.chars().all(|c| c.is_alphanumeric() || c == '_')
            && l != "}"
        {
            out.push(head.to_string());
        }
        depth += l.matches('{').count() as i32 - l.matches('}').count() as i32;
        if !l.starts_with("effect ") && depth <= 0 {
            depth = 0;
        }
        if l.starts_with("effect ") && l.ends_with('{') {
            depth = 1;
        }
    }
    out
}

fn package_policy_heads() -> (Vec<String>, usize) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages");
    let mut unknown = Vec::new();
    let mut scanned = 0usize;
    for d in ["pw-std", "pw-platform-web"] {
        for e in std::fs::read_dir(root.join(d)).expect("package") {
            let p = e.expect("entry").path();
            if p.extension().and_then(|x| x.to_str()) != Some("pw") {
                continue;
            }
            let src = std::fs::read_to_string(&p).expect("read");
            let file = p.file_name().unwrap().to_string_lossy().to_string();
            scanned += src
                .lines()
                .filter(|l| l.trim_start().starts_with("effect "))
                .count();
            for head in heads_of(&src) {
                if !pw_syntax::grammar::POLICY_KEYWORDS.contains(&head.as_str()) {
                    unknown.push(format!("{file}: `{head}`"));
                }
            }
        }
    }
    unknown.sort();
    unknown.dedup();
    (unknown, scanned)
}
