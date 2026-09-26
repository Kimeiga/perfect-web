//! **A named argument is given to the parameter of its name, compiled**
//! (ADR-0081).
//!
//! Until 2026-09-26 the backend passed arguments in written order, so
//! `g(b = 1, a = n)` computed `g(1, n)`: `Named(10)` answered `-9` through the
//! E8 host, where it means `10 - 1`. Each query here runs through the host
//! beside the answer it must give.

use std::collections::BTreeMap;

use pw_conformance::{Runnable, compile, units};
use pw_host::engine::Val;

const PROGRAM: &str = r#"module na

fn g(a: Int, b: Int) -> Int !{} { a - b }

public query Named(n: Int) -> Int {
    g(b = 1, a = n)
}

public query Mixed(n: Int) -> Int {
    g(n, b = 1)
}

public query Positional(n: Int) -> Int {
    g(n, 1)
}
"#;

fn call(id: &str, n: i64) -> Val {
    Runnable::new(compile(&units(&[("na.pw", PROGRAM)]), id))
        .call(&BTreeMap::new(), &[Val::S64(n)])
        .expect("runs")
        .remove(0)
}

#[test]
fn a_named_argument_is_passed_to_its_parameter() {
    for n in [0, 3, 10] {
        assert_eq!(call("na.Named", n), Val::S64(n - 1), "Named({n})");
        assert_eq!(call("na.Mixed", n), Val::S64(n - 1), "Mixed({n})");
        // The control: the same call, positional.
        assert_eq!(call("na.Positional", n), Val::S64(n - 1), "Positional({n})");
    }
}
