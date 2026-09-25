//! **The Koka oracle for pure `Int` computation** (ADR-0039 §1).
//!
//! The same Pleris functions go two ways: through `pw_core::koka` to Koka
//! 3.2.3, whose `int` is unbounded and whose `/` and `%` are Euclidean; and
//! through the component backend, inlined into queries and run by the E8
//! host. Wherever Pleris produces a value, Koka's must be the same value.
//! Where Pleris traps, the count is reported: an intermediate result did not
//! fit in 64 bits, or a divisor was zero, where Koka answers `0` and the
//! dividend. `tests/computation.rs` holds the traps to an exact `i128`
//! reference; this holds the values to an independent implementation.
//!
//! Ignored by default: CI has no Koka. `just e10-pure` runs it with the pinned
//! toolchain on the PATH.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const SOURCE: &str = "module pure.arith

fn plus(a: Int, b: Int) -> Int !{} { a + b }

fn minus(a: Int, b: Int) -> Int !{} { a - b }

fn times(a: Int, b: Int) -> Int !{} { a * b }

fn quotient(a: Int, b: Int) -> Int !{} { a / b }

fn remainder(a: Int, b: Int) -> Int !{} { a % b }

fn negated(a: Int, b: Int) -> Int !{} { -a + b - b }

fn sign(a: Int, b: Int) -> Int !{} {
    if a < b { -1 } else { if a == b { 0 } else { 1 } }
}

fn mixed(a: Int, b: Int) -> Int !{} {
    if a > b & b != 0 { a / b + a % b } else { a * 2 - b }
}

fn poly(a: Int, b: Int) -> Int !{} {
    (a - b) * (a + b) - a / 3 + b % 7
}

public query Plus(a: Int, b: Int) -> Int { plus(a, b) }

public query Minus(a: Int, b: Int) -> Int { minus(a, b) }

public query Times(a: Int, b: Int) -> Int { times(a, b) }

public query Quotient(a: Int, b: Int) -> Int { quotient(a, b) }

public query Remainder(a: Int, b: Int) -> Int { remainder(a, b) }

public query Negated(a: Int, b: Int) -> Int { negated(a, b) }

public query Sign(a: Int, b: Int) -> Int { sign(a, b) }

public query Mixed(a: Int, b: Int) -> Int { mixed(a, b) }

public query Poly(a: Int, b: Int) -> Int { poly(a, b) }
";

/// `(pleris query, koka function)`.
const FUNCTIONS: [(&str, &str); 9] = [
    ("pure.arith.Plus", "plus"),
    ("pure.arith.Minus", "minus"),
    ("pure.arith.Times", "times"),
    ("pure.arith.Quotient", "quotient"),
    ("pure.arith.Remainder", "remainder"),
    ("pure.arith.Negated", "negated"),
    ("pure.arith.Sign", "sign"),
    ("pure.arith.Mixed", "mixed"),
    ("pure.arith.Poly", "poly"),
];

/// xorshift64*, as `computation.rs`: the edges, mostly.
fn inputs() -> Vec<(i64, i64)> {
    let mut s: u64 = 0x0ca_0ca_0ca;
    let mut next = move || {
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        s.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    let mut int = move || -> i64 {
        let r = next();
        match r % 9 {
            0 => [0, 1, -1, 2, -2, 3, -3, 7, -7][(r >> 8) as usize % 9],
            1 => [i64::MIN, i64::MAX, i64::MIN + 1, i64::MAX - 1][(r >> 8) as usize % 4],
            2 => (r as i64) >> 33,
            3 => 3_037_000_499 + (r >> 60) as i64,
            4 => ((r >> 8) % 2000) as i64 - 1000,
            _ => r as i64,
        }
    };
    let mut out: Vec<(i64, i64)> = vec![(-7, 2), (7, -2), (-7, -2), (7, 2), (5, 0)];
    while out.len() < 240 {
        out.push((int(), int()));
    }
    out
}

fn koka() -> std::process::Command {
    std::process::Command::new(std::env::var("KOKA").unwrap_or_else(|_| "koka".into()))
}

/// Koka's answer for every function and input, as exact decimal text.
fn koka_answers(
    dir: &std::path::Path,
    inputs: &[(i64, i64)],
) -> BTreeMap<(String, i64, i64), String> {
    let src = pw_syntax::parse_tree(SOURCE);
    assert!(src.ok(), "{:?}", src.errors);
    let hir = pw_core::lower::lower_file(SOURCE, &src.green);
    let lowered = pw_core::koka::lower_module(&hir, "pure.arith");
    for (_, f) in FUNCTIONS {
        assert!(
            lowered.emitted.iter().any(|e| e == f),
            "`{f}` must reach Koka: skipped {:?}",
            lowered.skipped
        );
    }
    std::fs::write(dir.join("pure_arith.kk"), &lowered.source).expect("write the module");
    let pairs = inputs
        .iter()
        .map(|(a, b)| format!("({a}, {b})"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut main =
        String::from("module main\n\nimport pure_arith\n\npub fun main() : console ()\n");
    main.push_str(&format!("  val inputs = [{pairs}]\n"));
    main.push_str("  inputs.foreach fn(p)\n    val (a, b) = p\n");
    for (_, f) in FUNCTIONS {
        main.push_str(&format!(
            "    println(\"{f} \" ++ a.show ++ \" \" ++ b.show ++ \" \" ++ {f}(a, b).show)\n"
        ));
    }
    std::fs::write(dir.join("main.kk"), main).expect("write main");
    let out = koka()
        .current_dir(dir)
        .args(["-e", "--outputdir=out", "main.kk"])
        .output()
        .expect("koka runs: put the pinned toolchain on the PATH, or set KOKA");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "koka failed:\n{}\n{}",
        stdout,
        String::from_utf8_lossy(&out.stderr)
    );
    let mut answers = BTreeMap::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(' ').collect();
        if let [f, a, b, v] = parts.as_slice()
            && FUNCTIONS.iter().any(|(_, k)| k == f)
            && let (Ok(a), Ok(b)) = (a.parse::<i64>(), b.parse::<i64>())
        {
            answers.insert((f.to_string(), a, b), v.to_string());
        }
    }
    answers
}

#[test]
#[ignore = "needs Koka 3.2.3; `just e10-pure` runs it"]
fn compiled_int_computation_agrees_with_koka() {
    let dir = std::env::temp_dir().join(format!("pw-koka-oracle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let inputs = inputs();
    let koka = koka_answers(&dir, &inputs);
    assert_eq!(
        koka.len(),
        inputs
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            * FUNCTIONS.len(),
        "Koka answered every function for every input"
    );

    let program = units(&[("arith.pw", SOURCE)]);
    let (mut agreed, mut trapped, mut zero) = (0, 0, 0);
    for (query, f) in FUNCTIONS {
        let r = Runnable::new(compile(&program, query));
        for (a, b) in &inputs {
            let want = &koka[&(f.to_string(), *a, *b)];
            match r.call(&BTreeMap::new(), &[Val::S64(*a), Val::S64(*b)]) {
                Ok(v) => {
                    assert_eq!(v, vec![Val::S64(want.parse().unwrap_or_else(|_| {
                        panic!("{f}({a}, {b}): Pleris produced {v:?} and Koka {want}, which is not an Int")
                    }))], "{f}({a}, {b})");
                    agreed += 1;
                }
                Err(_) if *b == 0 && matches!(f, "quotient" | "remainder") => {
                    // The divergence ADR-0039 chose: Koka's answer is 0 or the
                    // dividend, Pleris refuses to produce one.
                    let koka_total = if f == "quotient" {
                        "0".to_string()
                    } else {
                        a.to_string()
                    };
                    assert_eq!(want, &koka_total, "{f}({a}, 0): Koka's total division");
                    zero += 1;
                }
                Err(_) => trapped += 1,
            }
        }
    }
    println!(
        "oracle: {} functions x {} inputs: {agreed} values identical to Koka's, \
         {trapped} traps where a 64-bit result did not fit, {zero} zero divisors \
         (Koka: 0 and the dividend; Pleris: a trap)",
        FUNCTIONS.len(),
        inputs.len()
    );
    assert!(agreed > 1500, "most cases produce a value");

    // The control: a Koka answer altered by one is caught, so the comparison
    // above is not vacuous.
    let r = Runnable::new(compile(&program, "pure.arith.Plus"));
    let wrong: i64 = koka[&("plus".to_string(), 7, 2)].parse::<i64>().unwrap() + 1;
    assert_ne!(
        r.call(&BTreeMap::new(), &[Val::S64(7), Val::S64(2)]),
        Ok(vec![Val::S64(wrong)]),
        "a wrong reference is not matched"
    );
    println!("oracle: a Koka answer altered by one is not matched");
    let _ = std::fs::remove_dir_all(&dir);
}
