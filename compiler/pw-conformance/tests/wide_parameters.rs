//! **An export's parameters past the flat limit arrive in memory**
//! (ADR-0266). The Canonical ABI passes a function's parameters as flat core
//! values up to `MAX_FLAT_PARAMS`, 16, and past it stores them in memory, a
//! tuple of them, and passes one pointer. Until ADR-0266 such an export was
//! refused at build, so a row's derived value could not take a record past
//! 16 flat values (found by the uploads track, ADR-0260). Each parameter is
//! held now where the host stores it: at its offset in the tuple, aligned
//! as its type is. Each test states one case, through the host, with its
//! control.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const FIELDS: [&str; 17] = [
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
];

/// Each of `names` weighted by its position, so a value read from another
/// position's place answers another sum.
fn weighted(prefix: &str, names: &[&str]) -> String {
    names
        .iter()
        .enumerate()
        .map(|(i, f)| format!("{prefix}{f} * {}", i + 1))
        .collect::<Vec<_>>()
        .join(" + ")
}

fn program() -> String {
    let all = FIELDS
        .iter()
        .map(|f| format!("{f}: Int"))
        .collect::<Vec<_>>()
        .join(", ");
    let sixteen = FIELDS[..16]
        .iter()
        .map(|f| format!("{f}: Int"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "module w\n\ntype Wide = Wide {{ {all} }}\n\n\
         public query Seventeen({all}) -> Int {{\n    {}\n}}\n\n\
         public query Sixteen({sixteen}) -> Int {{\n    {}\n}}\n\n\
         public query Summed(flag: Bool, w: Wide) -> Int {{\n    {}\n}}\n\n\
         public query Picked(flag: Bool, w: Wide) -> Bool {{\n    flag\n}}\n\n\
         public query Named(w: Wide, name: String) -> String {{\n    name\n}}\n",
        weighted("", &FIELDS),
        weighted("", &FIELDS[..16]),
        weighted("w.", &FIELDS),
    )
}

fn compiled(id: &str) -> Runnable {
    Runnable::new(compile(&units(&[("w.pw", &program())]), id))
}

fn call(r: &Runnable, args: &[Val]) -> Result<Val, String> {
    r.call(&BTreeMap::new(), args).map(|mut out| out.remove(0))
}

/// The value at each position, and what the weighted sum of them is.
fn values(n: usize) -> (Vec<i64>, i64) {
    let v: Vec<i64> = (0..n as i64).map(|i| 1000 + 7 * i).collect();
    let sum = v.iter().enumerate().map(|(i, x)| x * (i as i64 + 1)).sum();
    (v, sum)
}

fn wide(v: &[i64]) -> Val {
    Val::Record(
        FIELDS
            .iter()
            .zip(v)
            .map(|(f, x)| (f.to_string(), Val::S64(*x)))
            .collect(),
    )
}

#[test]
fn seventeen_parameters_arrive_each_in_its_place() {
    let (v, sum) = values(17);
    let args: Vec<Val> = v.iter().map(|x| Val::S64(*x)).collect();
    assert_eq!(call(&compiled("w.Seventeen"), &args), Ok(Val::S64(sum)));
    // The control: sixteen arrive flat, as they did.
    let (v, sum) = values(16);
    let args: Vec<Val> = v.iter().map(|x| Val::S64(*x)).collect();
    assert_eq!(call(&compiled("w.Sixteen"), &args), Ok(Val::S64(sum)));
}

#[test]
fn a_record_past_the_limit_arrives_aligned_after_what_comes_before_it() {
    let (v, sum) = values(17);
    // A `Bool` first, then the record at the next multiple of 8, its
    // alignment: read one place off, every field would be wrong.
    assert_eq!(
        call(&compiled("w.Summed"), &[Val::Bool(true), wide(&v)]),
        Ok(Val::S64(sum))
    );
    for flag in [true, false] {
        assert_eq!(
            call(&compiled("w.Picked"), &[Val::Bool(flag), wide(&v)]),
            Ok(Val::Bool(flag))
        );
    }
    // And what comes after the record, past its 136 bytes.
    assert_eq!(
        call(
            &compiled("w.Named"),
            &[wide(&v), Val::String("after the record".to_string())]
        ),
        Ok(Val::String("after the record".to_string()))
    );
}
