//! **The case tables are Rust's mapping, under one Unicode version**
//! (ADR-0056).
//!
//! `String.to_lower` and `String.to_upper` read tables generated from the
//! compiler's own `char::to_lowercase` and `char::to_uppercase`. These tests
//! hold the tables to that mapping for every code point, and pin the Unicode
//! version it comes from.

use pw_core::backend::case::{Case, GROWTH, table};

fn rust(c: char, case: Case) -> Vec<u32> {
    match case {
        Case::Lower => c.to_lowercase().map(|x| x as u32).collect(),
        Case::Upper => c.to_uppercase().map(|x| x as u32).collect(),
    }
}

#[test]
fn the_tables_are_generated_under_unicode_17() {
    // A toolchain whose `char` knows another Unicode version changes what
    // `String.to_lower` and `String.to_upper` answer. That is a change to
    // the language, made here deliberately, never by an upgrade alone.
    assert_eq!(char::UNICODE_VERSION, (17, 0, 0));
}

#[test]
fn the_tables_reproduce_rusts_mapping_for_every_code_point() {
    for case in [Case::Lower, Case::Upper] {
        let t = table(case);
        for cp in 0..=char::MAX as u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            assert_eq!(t.map(cp), rust(c, case), "{case:?} U+{cp:04X}");
        }
    }
}

#[test]
fn no_mapping_grows_a_string_past_its_bound() {
    // The component allocates `GROWTH` times the input's bytes and writes
    // the result into them.
    for case in [Case::Lower, Case::Upper] {
        for cp in 0..=char::MAX as u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let bytes: usize = rust(c, case)
                .iter()
                .map(|x| char::from_u32(*x).expect("a char").len_utf8())
                .sum();
            assert!(
                bytes <= GROWTH as usize * c.len_utf8(),
                "{case:?} U+{cp:04X} grows to {bytes} bytes"
            );
        }
    }
}

#[test]
fn a_table_is_ranges_then_code_points_that_map_to_more_than_one() {
    let lower = table(Case::Lower);
    // `A`-`Z` is one range: 32 on.
    let a = lower.ranges.iter().find(|r| r.lo == 'A' as u32).expect("A");
    assert_eq!((a.hi, a.delta, a.stride), ('Z' as u32, 32, 1));
    // `İ` lowers to `i` and a combining dot: the one such code point.
    assert_eq!(lower.multi.len(), 1);
    assert_eq!(lower.map('İ' as u32), ['i' as u32, 0x307]);
    // `ß` uppers to `SS`.
    assert_eq!(table(Case::Upper).map('ß' as u32), ['S' as u32, 'S' as u32]);
    // Every entry is 16 bytes.
    for case in [Case::Lower, Case::Upper] {
        let t = table(case);
        assert_eq!(t.bytes().len(), 16 * (t.ranges.len() + t.multi.len()));
    }
}
