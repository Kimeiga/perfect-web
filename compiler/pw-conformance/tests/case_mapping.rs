//! **Unicode case mapping, compiled** (ADR-0056).
//!
//! `String.to_lower` and `String.to_upper` run through the E8 host over
//! every code point Unicode maps and over mixed text, beside Rust mapping
//! each code point with `char::to_lowercase` and `char::to_uppercase`.
//!
//! The mapping is per code point. It is Rust's `str::to_uppercase` exactly,
//! and `str::to_lowercase` but for one rule: a capital sigma at the end of a
//! word lowers to `σ` here, and `ς` there (ADR-0056, ruling needed).

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = "module c

import String

public query Lower(text: String) -> String { String.to_lower(text) }

public query Upper(text: String) -> String { String.to_upper(text) }

public query Folded(a: String, b: String) -> Bool { String.to_lower(a) == String.to_lower(b) }

public query Pair(a: String, b: String) -> String {
    let x = String.to_upper(a)
    let y = String.to_upper(b)
    \"{x}|{y}\"
}
";

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("c.pw", PROGRAM)]), id))
}

fn call(r: &Runnable, args: &[Val]) -> Val {
    r.call(&BTreeMap::new(), args)
        .map(|mut out| out.remove(0))
        .unwrap_or_else(|e| panic!("{e}"))
}

fn text(s: &str) -> Val {
    Val::String(s.to_string())
}

fn lower(s: &str) -> String {
    s.chars().flat_map(char::to_lowercase).collect()
}

fn upper(s: &str) -> String {
    s.chars().flat_map(char::to_uppercase).collect()
}

#[test]
fn every_code_point_unicode_maps_is_mapped_as_rust_maps_it() {
    let (lo, up) = (compiled("c.Lower"), compiled("c.Upper"));
    // Every code point either direction changes, a few hundred to a call.
    let changed: Vec<char> = (0..=char::MAX as u32)
        .filter_map(char::from_u32)
        .filter(|c| {
            lower(&c.to_string()) != c.to_string() || upper(&c.to_string()) != c.to_string()
        })
        .collect();
    assert!(changed.len() > 2800, "{} code points", changed.len());
    for chunk in changed.chunks(400) {
        let s: String = chunk.iter().collect();
        assert_eq!(call(&lo, &[text(&s)]), text(&lower(&s)), "lower {s:?}");
        assert_eq!(call(&up, &[text(&s)]), text(&upper(&s)), "upper {s:?}");
    }
}

#[test]
fn mixed_text_is_mapped_as_rust_maps_it() {
    let (lo, up) = (compiled("c.Lower"), compiled("c.Upper"));
    const POOL: &str = "aZ09 !ÀéÇñßẞİıſ\u{149}ΑΣσςΐ\u{390}ДЖжЎԱﬃﬀＡａ𐐀𐐨𞤀ǅǆǄ人ひカ😀\u{301}";
    let pool: Vec<char> = POOL.chars().collect();
    let mut state = 0x2545_f491_u64;
    for _ in 0..300 {
        let mut s = String::new();
        for _ in 0..(state % 12) {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            s.push(pool[(state % pool.len() as u64) as usize]);
        }
        state = state.wrapping_add(1);
        assert_eq!(call(&lo, &[text(&s)]), text(&lower(&s)), "lower {s:?}");
        assert_eq!(call(&up, &[text(&s)]), text(&upper(&s)), "upper {s:?}");
    }
}

#[test]
fn the_mapping_is_per_code_point() {
    let (lo, up) = (compiled("c.Lower"), compiled("c.Upper"));
    // Grows: ß is SS, and ΐ three code points.
    assert_eq!(call(&up, &[text("straße")]), text("STRASSE"));
    assert_eq!(call(&up, &[text("ΐ")]), text("\u{399}\u{308}\u{301}"));
    // İ lowers to i and a combining dot above.
    assert_eq!(call(&lo, &[text("İ")]), text("i\u{307}"));
    // A final capital sigma is σ: no rule looks at its neighbours. Rust's
    // `str::to_lowercase`, which applies Final_Sigma, answers ς.
    assert_eq!(call(&lo, &[text("ΟΔΟΣ")]), text("οδοσ"));
    assert_eq!("ΟΔΟΣ".to_lowercase(), "οδος");
    // Nothing else changes.
    assert_eq!(call(&lo, &[text("人ひ😀")]), text("人ひ😀"));
    assert_eq!(call(&up, &[text("")]), text(""));
}

#[test]
fn texts_that_differ_in_case_compare_equal_lowered() {
    let folded = compiled("c.Folded");
    assert_eq!(
        call(&folded, &[text("Straße"), text("STRAßE")]),
        Val::Bool(true)
    );
    assert_eq!(call(&folded, &[text("ΑΒΓ"), text("αβγ")]), Val::Bool(true));
    assert_eq!(call(&folded, &[text("abc"), text("abd")]), Val::Bool(false));
}

#[test]
fn a_mapping_that_grows_keeps_what_was_mapped_after_it() {
    // `ΐ` is two bytes and upper-cases to six. Each result is allocated
    // before the next is made, so one written past its room would be
    // overwritten by the next.
    let pair = compiled("c.Pair");
    let (a, b) = ("ΐΰß", "ﬃŉǰ");
    assert_eq!(
        call(&pair, &[text(a), text(b)]),
        text(&format!("{}|{}", upper(a), upper(b)))
    );
}
