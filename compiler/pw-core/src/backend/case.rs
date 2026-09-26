//! **Unicode case mapping** (ADR-0056): the tables `String.to_lower` and
//! `String.to_upper` read, in the component and in the JavaScript module.
//!
//! Both backends read these tables, so they cannot disagree, and neither
//! calls a platform's own mapping: JavaScript's `toLowerCase` follows its
//! engine's Unicode version, which is not the compiler's. The tables are
//! generated from the compiler's own `char::to_lowercase` and
//! `char::to_uppercase`, one code point at a time. `tests/case_mapping.rs`
//! pins the Unicode version they are generated under, so a toolchain that
//! changes it changes the language's mapping only through a failing test.

use std::sync::OnceLock;

/// Which way a string is mapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Case {
    Lower,
    Upper,
}

/// `lo..=hi`: each code point `c` in it with `(c - lo) % stride == 0` maps
/// to `c + delta`, and every other code point in it maps to itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub lo: u32,
    pub hi: u32,
    pub delta: i32,
    pub stride: u32,
}

/// A code point that maps to more than one: at most three, and a zero
/// after the last. No code point maps to U+0000.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Multi {
    pub from: u32,
    pub to: [u32; 3],
}

/// One direction's mapping: the ranges, then the code points that map to
/// more than one, each sorted by code point. A code point in neither maps
/// to itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub ranges: Vec<Range>,
    pub multi: Vec<Multi>,
}

/// A string mapped this way is at most this many times its UTF-8 length:
/// `ΐ`, two bytes, is three code points of two bytes each in upper case.
/// `tests/case_mapping.rs` holds the tables to it.
pub const GROWTH: u32 = 3;

fn mapped(c: char, case: Case) -> Vec<char> {
    match case {
        Case::Lower => c.to_lowercase().collect(),
        Case::Upper => c.to_uppercase().collect(),
    }
}

/// The table for `case`, generated once.
pub fn table(case: Case) -> &'static Table {
    static LOWER: OnceLock<Table> = OnceLock::new();
    static UPPER: OnceLock<Table> = OnceLock::new();
    match case {
        Case::Lower => LOWER.get_or_init(|| generate(Case::Lower)),
        Case::Upper => UPPER.get_or_init(|| generate(Case::Upper)),
    }
}

/// Every code point's mapping, compressed. A range grows while the next
/// code point mapped one-to-one has its delta and is `stride` further on.
/// Code points are read in order, so one between them mapped one-to-one
/// otherwise would have begun a later range: a code point is in at most one
/// range, and the ranges are found by binary search.
fn generate(case: Case) -> Table {
    let one = |cp: u32| -> Option<i32> {
        let c = char::from_u32(cp)?;
        let m = mapped(c, case);
        (m.len() == 1 && m[0] != c).then(|| m[0] as i32 - cp as i32)
    };
    let mut ranges: Vec<Range> = Vec::new();
    let mut multi = Vec::new();
    for cp in 0..=char::MAX as u32 {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let m = mapped(c, case);
        if m.len() > 1 {
            let mut to = [0; 3];
            for (slot, x) in to.iter_mut().zip(&m) {
                *slot = *x as u32;
            }
            assert!(m.len() <= 3, "{c:?} maps to {} code points", m.len());
            multi.push(Multi { from: cp, to });
            continue;
        }
        let Some(delta) = one(cp) else {
            continue;
        };
        if let Some(last) = ranges.last_mut()
            && last.delta == delta
        {
            let gap = cp - last.hi;
            let stride = if last.lo == last.hi { gap } else { last.stride };
            if (stride == 1 || stride == 2) && gap == stride {
                last.hi = cp;
                last.stride = stride;
                continue;
            }
        }
        ranges.push(Range {
            lo: cp,
            hi: cp,
            delta,
            stride: 1,
        });
    }
    Table { ranges, multi }
}

impl Table {
    /// What one code point maps to, read from the table the way both
    /// backends read it.
    pub fn map(&self, cp: u32) -> Vec<u32> {
        if let Ok(i) = self.multi.binary_search_by_key(&cp, |m| m.from) {
            return self.multi[i]
                .to
                .iter()
                .copied()
                .take_while(|x| *x != 0)
                .collect();
        }
        // The last range starting at or before `cp`.
        let at = self.ranges.partition_point(|r| r.lo <= cp);
        if let Some(r) = at.checked_sub(1).map(|i| self.ranges[i])
            && cp <= r.hi
            && (cp - r.lo).is_multiple_of(r.stride)
        {
            return vec![(cp as i64 + r.delta as i64) as u32];
        }
        vec![cp]
    }

    /// The ranges, then the multi entries, as the component's data segment
    /// holds them: each field a little-endian `u32`, 16 bytes an entry.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 * (self.ranges.len() + self.multi.len()));
        for r in &self.ranges {
            for x in [r.lo, r.hi, r.delta as u32, r.stride] {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        for m in &self.multi {
            for x in [m.from, m.to[0], m.to[1], m.to[2]] {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        out
    }
}
