//! **Maps and sets, compiled** (ADR-0057).
//!
//! Each operation runs through the E8 host over generated maps and sets,
//! beside Rust's `BTreeMap` and `BTreeSet`, whose order is the one Pleris
//! keeps: an `Int` by value, a `String` by its UTF-8 bytes, which is code
//! point order. A map or set a query is given is checked on entry, and one
//! out of order, or with a key twice, stops the invocation.

use std::collections::{BTreeMap, BTreeSet};

use pw_conformance::{Calls, Runnable, compile, operation, units};
use pw_host::engine::Val;

const CASES: usize = 150;

const PROGRAM: &str = "module m

import List
import Map
import Set

type Word = Word { text: String, score: Int }

public query Built(keys: List<String>, values: List<Int>) -> Map<String, Int> {
    Map.from_lists(keys, values)
}

public query Lookup(m: Map<String, Int>, k: String) -> Option<Int> { Map.get(m, k) }

public query Has(m: Map<Int, String>, k: Int) -> Bool { Map.contains(m, k) }

public query Added(m: Map<String, Int>, k: String, v: Int) -> Map<String, Int> { Map.insert(m, k, v) }

public query Dropped(m: Map<String, Int>, k: String) -> Map<String, Int> { Map.remove(m, k) }

public query Keys(m: Map<String, Int>) -> List<String> { Map.keys(m) }

public query Values(m: Map<String, Int>) -> List<Int> { Map.values(m) }

public query Size(m: Map<String, Int>) -> Int { Map.size(m) }

public query Nothing() -> Map<String, Int> { Map.empty() }

public query Counted(words: List<String>) -> Map<String, Int> {
    let mut counts: Map<String, Int> = Map.empty()
    for w in words {
        let n = match Map.get(counts, w) {
            Some(c) => c + 1,
            None => 1,
        }
        counts = Map.insert(counts, w, n)
    }
    counts
}

public query Scored(m: Map<Int, Word>, k: Int, w: Word) -> Option<Word> {
    Map.get(Map.insert(m, k + 1, w), k)
}

public query Distinct(xs: List<Int>) -> Set<Int> { Set.from_list(xs) }

public query Joined(a: Set<String>, b: Set<String>) -> Set<String> { Set.union(a, b) }

public query Common(a: Set<Int>, b: Set<Int>) -> Set<Int> { Set.intersection(a, b) }

public query Without(a: Set<Int>, b: Set<Int>) -> Set<Int> { Set.difference(a, b) }

public query Changed(s: Set<Int>, x: Int) -> List<Int> {
    Set.to_list(Set.remove(Set.insert(s, x), x + 1))
}

public query Member(s: Set<String>, x: String) -> Bool { Set.contains(s, x) }

public query Count(s: Set<String>) -> Int { Set.size(s) }

opaque type Sku = String
opaque type Seat = Int
opaque type Flag = Bool

public query Switched(m: Map<Bool, Int>, k: Bool, v: Int) -> Map<Bool, Int> { Map.insert(m, k, v) }

public query Switch(m: Map<Bool, Int>, k: Bool) -> Option<Int> { Map.get(m, k) }

public query Flags(xs: List<Bool>) -> Set<Bool> { Set.from_list(xs) }

public query Stocked(m: Map<Sku, Int>, k: Sku, v: Int) -> Map<Sku, Int> { Map.insert(m, k, v) }

public query Seats(xs: List<Seat>) -> Set<Seat> { Set.from_list(xs) }

public query Flagged(xs: List<Flag>) -> Set<Flag> { Set.from_list(xs) }
";

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn int(&mut self) -> i64 {
        match self.below(8) {
            0 => [i64::MIN, i64::MAX, 0][self.below(3) as usize],
            _ => self.below(40) as i64 - 20,
        }
    }
    /// Few distinct texts, so keys repeat, in scripts whose UTF-16 order is
    /// not their code point order.
    fn text(&mut self) -> String {
        const POOL: [&str; 9] = ["", "a", "b", "ab", "人", "ひ", "\u{FF61}", "\u{1F600}", "B"];
        POOL[self.below(POOL.len() as u64) as usize].to_string()
    }
}

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("m.pw", PROGRAM)]), id))
}

fn call(r: &Runnable, args: &[Val]) -> Result<Val, String> {
    r.call(&BTreeMap::new(), args).map(|mut out| out.remove(0))
}

fn text(s: &str) -> Val {
    Val::String(s.to_string())
}

fn string_map(m: &BTreeMap<String, i64>) -> Val {
    Val::List(
        m.iter()
            .map(|(k, v)| Val::Tuple(vec![text(k), Val::S64(*v)]))
            .collect(),
    )
}

fn int_map(m: &BTreeMap<i64, String>) -> Val {
    Val::List(
        m.iter()
            .map(|(k, v)| Val::Tuple(vec![Val::S64(*k), text(v)]))
            .collect(),
    )
}

fn int_set(s: &BTreeSet<i64>) -> Val {
    Val::List(s.iter().map(|x| Val::S64(*x)).collect())
}

fn text_set(s: &BTreeSet<String>) -> Val {
    Val::List(s.iter().map(|x| text(x)).collect())
}

fn some(v: Option<Val>) -> Val {
    Val::Option(v.map(Box::new))
}

fn word(t: &str, score: i64) -> Val {
    Val::Record(vec![
        ("text".to_string(), text(t)),
        ("score".to_string(), Val::S64(score)),
    ])
}

#[test]
fn a_map_answers_as_a_btreemap_does() {
    let (lookup, has, added, dropped, keys, values, size) = (
        compiled("m.Lookup"),
        compiled("m.Has"),
        compiled("m.Added"),
        compiled("m.Dropped"),
        compiled("m.Keys"),
        compiled("m.Values"),
        compiled("m.Size"),
    );
    let mut rng = Rng(0x3a9);
    for _ in 0..CASES {
        let m: BTreeMap<String, i64> = (0..rng.below(7)).map(|_| (rng.text(), rng.int())).collect();
        let (k, v) = (rng.text(), rng.int());
        let case = format!("{m:?}, {k:?}");
        assert_eq!(
            call(&lookup, &[string_map(&m), text(&k)]),
            Ok(some(m.get(&k).map(|v| Val::S64(*v)))),
            "get {case}"
        );
        let mut with = m.clone();
        with.insert(k.clone(), v);
        assert_eq!(
            call(&added, &[string_map(&m), text(&k), Val::S64(v)]),
            Ok(string_map(&with)),
            "insert {case}, {v}"
        );
        let mut without = m.clone();
        without.remove(&k);
        assert_eq!(
            call(&dropped, &[string_map(&m), text(&k)]),
            Ok(string_map(&without)),
            "remove {case}"
        );
        assert_eq!(
            call(&keys, &[string_map(&m)]),
            Ok(Val::List(m.keys().map(|k| text(k)).collect()))
        );
        assert_eq!(
            call(&values, &[string_map(&m)]),
            Ok(Val::List(m.values().map(|v| Val::S64(*v)).collect()))
        );
        assert_eq!(call(&size, &[string_map(&m)]), Ok(Val::S64(m.len() as i64)));

        let n: BTreeMap<i64, String> = (0..rng.below(7)).map(|_| (rng.int(), rng.text())).collect();
        let probe = rng.int();
        assert_eq!(
            call(&has, &[int_map(&n), Val::S64(probe)]),
            Ok(Val::Bool(n.contains_key(&probe))),
            "contains {n:?}, {probe}"
        );
    }
}

#[test]
fn a_map_is_built_from_two_lists_and_counted_into() {
    let (built, counted, nothing) = (
        compiled("m.Built"),
        compiled("m.Counted"),
        compiled("m.Nothing"),
    );
    let mut rng = Rng(0x61);
    for _ in 0..CASES {
        let words: Vec<String> = (0..rng.below(12)).map(|_| rng.text()).collect();
        let scores: Vec<i64> = words.iter().map(|_| rng.int()).collect();
        // A later key replaces an earlier one, as `collect` into a map does.
        let want: BTreeMap<String, i64> =
            words.iter().cloned().zip(scores.iter().copied()).collect();
        let ws = Val::List(words.iter().map(|w| text(w)).collect());
        let ss = Val::List(scores.iter().map(|s| Val::S64(*s)).collect());
        assert_eq!(
            call(&built, &[ws.clone(), ss]),
            Ok(string_map(&want)),
            "from_lists {words:?}, {scores:?}"
        );
        let mut counts: BTreeMap<String, i64> = BTreeMap::new();
        for w in &words {
            *counts.entry(w.clone()).or_default() += 1;
        }
        assert_eq!(call(&counted, &[ws]), Ok(string_map(&counts)), "{words:?}");
    }
    assert_eq!(call(&nothing, &[]), Ok(Val::List(Vec::new())));
    // Lists of two lengths stop the invocation.
    assert!(call(&built, &[Val::List(vec![text("a")]), Val::List(Vec::new())]).is_err());
}

#[test]
fn a_maps_value_may_be_a_record() {
    let scored = compiled("m.Scored");
    let m: BTreeMap<i64, (String, i64)> =
        [(1, ("a".to_string(), 10)), (5, ("人".to_string(), -3))].into();
    let arg = Val::List(
        m.iter()
            .map(|(k, (t, s))| Val::Tuple(vec![Val::S64(*k), word(t, *s)]))
            .collect(),
    );
    for k in [0, 1, 4, 5] {
        let got = call(&scored, &[arg.clone(), Val::S64(k), word("new", 7)]);
        let mut with = m.clone();
        with.insert(k + 1, ("new".to_string(), 7));
        let want = some(with.get(&k).map(|(t, s)| word(t, *s)));
        assert_eq!(got, Ok(want), "key {k}");
    }
}

#[test]
fn a_set_answers_as_a_btreeset_does() {
    let (distinct, joined, common, without, changed, member, count) = (
        compiled("m.Distinct"),
        compiled("m.Joined"),
        compiled("m.Common"),
        compiled("m.Without"),
        compiled("m.Changed"),
        compiled("m.Member"),
        compiled("m.Count"),
    );
    let mut rng = Rng(0x5e7);
    for _ in 0..CASES {
        let xs: Vec<i64> = (0..rng.below(10)).map(|_| rng.int()).collect();
        let a: BTreeSet<i64> = xs.iter().copied().collect();
        assert_eq!(
            call(
                &distinct,
                &[Val::List(xs.iter().map(|x| Val::S64(*x)).collect())]
            ),
            Ok(int_set(&a)),
            "from_list {xs:?}"
        );
        let b: BTreeSet<i64> = (0..rng.below(10)).map(|_| rng.int()).collect();
        assert_eq!(
            call(&common, &[int_set(&a), int_set(&b)]),
            Ok(int_set(&a.intersection(&b).copied().collect())),
            "{a:?} & {b:?}"
        );
        assert_eq!(
            call(&without, &[int_set(&a), int_set(&b)]),
            Ok(int_set(&a.difference(&b).copied().collect())),
            "{a:?} - {b:?}"
        );
        let x = rng.int();
        let mut c = a.clone();
        c.insert(x);
        if let Some(y) = x.checked_add(1) {
            c.remove(&y);
            assert_eq!(
                call(&changed, &[int_set(&a), Val::S64(x)]),
                Ok(int_set(&c)),
                "{a:?}, {x}"
            );
        }
        let s: BTreeSet<String> = (0..rng.below(6)).map(|_| rng.text()).collect();
        let t: BTreeSet<String> = (0..rng.below(6)).map(|_| rng.text()).collect();
        assert_eq!(
            call(&joined, &[text_set(&s), text_set(&t)]),
            Ok(text_set(&s.union(&t).cloned().collect())),
            "{s:?} | {t:?}"
        );
        let probe = rng.text();
        assert_eq!(
            call(&member, &[text_set(&s), text(&probe)]),
            Ok(Val::Bool(s.contains(&probe)))
        );
        assert_eq!(call(&count, &[text_set(&s)]), Ok(Val::S64(s.len() as i64)));
    }
}

/// **A map or set out of order is sorted on arrival** (ruling 0057-c,
/// ADR-0259): given in any order it is the map or set Rust's `BTreeMap` and
/// `BTreeSet` make of the same entries, its keys in code point order, and
/// a key twice still stops the invocation. Until ADR-0259 one out of order
/// stopped it too.
#[test]
fn a_map_or_set_out_of_order_is_sorted_on_arrival() {
    let (size, count, keys, lookup, member) = (
        compiled("m.Size"),
        compiled("m.Count"),
        compiled("m.Keys"),
        compiled("m.Lookup"),
        compiled("m.Member"),
    );
    let entry = |k: &str, v: i64| Val::Tuple(vec![text(k), Val::S64(v)]);
    // Descending: sorted, and each key found where it is.
    let descending = Val::List(vec![entry("b", 1), entry("a", 2)]);
    assert_eq!(
        call(&size, std::slice::from_ref(&descending)),
        Ok(Val::S64(2))
    );
    assert_eq!(
        call(&keys, std::slice::from_ref(&descending)),
        Ok(Val::List(vec![text("a"), text("b")]))
    );
    assert_eq!(
        call(&lookup, &[descending, text("b")]),
        Ok(some(Some(Val::S64(1))))
    );
    // A key twice, beside each other or apart, still stops it.
    assert!(call(&size, &[Val::List(vec![entry("a", 1), entry("a", 2)])]).is_err());
    assert!(
        call(
            &size,
            &[Val::List(vec![entry("b", 1), entry("a", 2), entry("b", 3)])]
        )
        .is_err()
    );
    // Code point order, not UTF-16's: U+FF61 is below U+1F600, given after.
    let reversed = Val::List(vec![text("\u{1F600}"), text("\u{FF61}")]);
    assert_eq!(
        call(&count, std::slice::from_ref(&reversed)),
        Ok(Val::S64(2))
    );
    assert_eq!(
        call(&member, &[reversed, text("\u{FF61}")]),
        Ok(Val::Bool(true))
    );
    // And any order of any map: as `BTreeMap` makes it, each key found.
    let mut rng = Rng(0x0057_C0DE);
    for _ in 0..200 {
        let mut m: BTreeMap<String, i64> = BTreeMap::new();
        for _ in 0..rng.below(8) {
            m.insert(rng.text(), rng.int());
        }
        let mut given: Vec<Val> = m
            .iter()
            .map(|(k, v)| Val::Tuple(vec![text(k), Val::S64(*v)]))
            .collect();
        // Fisher-Yates, by the generator: an order no host need choose.
        for i in (1..given.len()).rev() {
            given.swap(i, rng.below(i as u64 + 1) as usize);
        }
        let given = Val::List(given);
        assert_eq!(
            call(&keys, std::slice::from_ref(&given)),
            Ok(Val::List(m.keys().map(|k| text(k)).collect())),
            "{m:?}"
        );
        let probe = rng.text();
        assert_eq!(
            call(&lookup, &[given, text(&probe)]),
            Ok(some(m.get(&probe).map(|v| Val::S64(*v)))),
            "{m:?} at {probe:?}"
        );
    }
}

/// A host's answer, sorted as a parameter is (ADR-0057, ADR-0259).
const HOSTED: &str = r#"module h

import Map

type Tally = Tally { word: String }

fn tallies(shard: String) -> Map<String, Int> !{ database.read<Tally> }
    host "kiokun:data/tallies#get"

public query Tallied(shard: String, word: String) -> Option<Int> {
    Map.get(tallies(shard), word)
}
"#;

#[test]
fn a_map_a_host_answers_is_sorted_when_it_arrives() {
    let r = Runnable::new(compile(&units(&[("h.pw", HOSTED)]), "h.Tallied"));
    let answer = |entries: Vec<(&'static str, i64)>| {
        let calls = Calls::default();
        BTreeMap::from([operation(&calls, "kiokun:data/tallies#get", move |_| {
            Val::List(
                entries
                    .iter()
                    .map(|(k, v)| Val::Tuple(vec![text(k), Val::S64(*v)]))
                    .collect(),
            )
        })])
    };
    let args = [text("s"), text("b")];
    let ordered = answer(vec![("a", 1), ("b", 2), ("c", 3)]);
    assert_eq!(
        r.call(&ordered, &args).map(|mut v| v.remove(0)),
        Ok(some(Some(Val::S64(2))))
    );
    // Out of order, sorted as it arrives (ADR-0259); a key twice stops it.
    let unordered = answer(vec![("c", 3), ("b", 2), ("a", 1)]);
    assert_eq!(
        r.call(&unordered, &args).map(|mut v| v.remove(0)),
        Ok(some(Some(Val::S64(2))))
    );
    let twice = answer(vec![("b", 2), ("a", 1), ("b", 3)]);
    assert!(r.call(&twice, &args).is_err());
}

fn bool_map(m: &BTreeMap<bool, i64>) -> Val {
    Val::List(
        m.iter()
            .map(|(k, v)| Val::Tuple(vec![Val::Bool(*k), Val::S64(*v)]))
            .collect(),
    )
}

/// **A map keyed by a `Bool`, or by an opaque type over one, an `Int` or a
/// `String`** (ADR-0248, ruling 0057-a): ordered as its representation,
/// `false` before `true`, as Rust's `bool` is, and an opaque type crossing
/// as its representation.
#[test]
fn a_map_keyed_by_a_bool_or_an_opaque_type_answers_as_a_btreemap_does() {
    let (switched, switch, flags) = (
        compiled("m.Switched"),
        compiled("m.Switch"),
        compiled("m.Flags"),
    );
    let (stocked, seats, flagged) = (
        compiled("m.Stocked"),
        compiled("m.Seats"),
        compiled("m.Flagged"),
    );
    let mut rng = Rng(0xb001);
    for _ in 0..CASES {
        let m: BTreeMap<bool, i64> = (0..rng.below(3))
            .map(|_| (rng.below(2) == 1, rng.int()))
            .collect();
        let (k, v) = (rng.below(2) == 1, rng.int());
        let mut with = m.clone();
        with.insert(k, v);
        assert_eq!(
            call(&switched, &[bool_map(&m), Val::Bool(k), Val::S64(v)]),
            Ok(bool_map(&with)),
            "insert {m:?}, {k}, {v}"
        );
        assert_eq!(
            call(&switch, &[bool_map(&m), Val::Bool(k)]),
            Ok(some(m.get(&k).map(|v| Val::S64(*v)))),
            "get {m:?}, {k}"
        );
        let xs: Vec<bool> = (0..rng.below(5)).map(|_| rng.below(2) == 1).collect();
        let set: BTreeSet<bool> = xs.iter().copied().collect();
        let want = Val::List(set.iter().map(|b| Val::Bool(*b)).collect());
        let given = Val::List(xs.iter().map(|b| Val::Bool(*b)).collect());
        assert_eq!(
            call(&flags, std::slice::from_ref(&given)),
            Ok(want.clone()),
            "{xs:?}"
        );
        assert_eq!(call(&flagged, &[given]), Ok(want), "opaque {xs:?}");

        let s: BTreeMap<String, i64> = (0..rng.below(6)).map(|_| (rng.text(), rng.int())).collect();
        let (sk, sv) = (rng.text(), rng.int());
        let mut stock = s.clone();
        stock.insert(sk.clone(), sv);
        assert_eq!(
            call(&stocked, &[string_map(&s), text(&sk), Val::S64(sv)]),
            Ok(string_map(&stock)),
            "opaque insert {s:?}, {sk:?}"
        );
        let ns: Vec<i64> = (0..rng.below(8)).map(|_| rng.int()).collect();
        let distinct: BTreeSet<i64> = ns.iter().copied().collect();
        assert_eq!(
            call(
                &seats,
                &[Val::List(ns.iter().map(|n| Val::S64(*n)).collect())]
            ),
            Ok(int_set(&distinct)),
            "opaque from_list {ns:?}"
        );
    }
    // Out of order on arrival, `true` before `false`: sorted, each value
    // where its key is (ADR-0259); and a key twice stops it.
    let entry = |k: bool, v: i64| Val::Tuple(vec![Val::Bool(k), Val::S64(v)]);
    for (k, v) in [(true, 1), (false, 2)] {
        assert_eq!(
            call(
                &switch,
                &[
                    Val::List(vec![entry(true, 1), entry(false, 2)]),
                    Val::Bool(k)
                ]
            ),
            Ok(some(Some(Val::S64(v))))
        );
    }
    assert!(
        call(
            &switch,
            &[
                Val::List(vec![entry(false, 1), entry(false, 1)]),
                Val::Bool(true)
            ]
        )
        .is_err()
    );
    assert_eq!(
        call(
            &switch,
            &[
                Val::List(vec![entry(false, 1), entry(true, 1)]),
                Val::Bool(true)
            ]
        ),
        Ok(some(Some(Val::S64(1))))
    );
}
