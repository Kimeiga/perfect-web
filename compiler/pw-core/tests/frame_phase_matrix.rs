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
fn a_layout_affecting_write_inside_an_animate_phase_is_not_rejected() {
    // **A gap the matrix found, and it is not the one it looks like.**
    //
    // `forbidden_in_phase` keys on an effect's FAMILY, and
    // `style.mutate<LayoutAffect>` has family `style` — so the rule
    // `("animate", "layout")` never sees it. A compositor animation that
    // writes a layout-affecting property is exactly what that rule's own
    // comment describes ("animating a property that invalidates it forces a
    // layout every frame") and exactly what it cannot catch.
    //
    // The distinction lives in `layout.rs`, which matches the WRITTEN form
    // `style.mutate<LayoutAffect>` — so it is expressible; `forbidden_in_phase`
    // just works one level too coarse to use it.
    //
    // Recorded rather than repaired: it predates this change, it has no corpus
    // fixture, and widening a phase rule is a separate ruling from deleting a
    // synthesized effect.
    let found = codes(&component("animate { style.set_width(s, \"1px\") }"));
    assert!(
        !found.contains(&"PW0402".to_string()),
        "if this now fires, the gap is closed: {found:?}"
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
