//! **A read is bounded by construction** (4c; the design note on
//! `track/bounded-reads`; ADR-XXXX-pw0405-pw0406).
//!
//! A ledger of prices, one source, a keyed read `price_of(item)` and its set
//! read `prices_of(items)`, which `set` pairs. A read whose key comes from a
//! loop's element is refused (PW0405), in each loop form: a callback a
//! combinator applies to a list's elements, a `for`, a recursion over data;
//! its key through a field, a `let`, a pure call, a match arm. Its controls
//! are accepted: a key that is no element's, the set read before the loop, a
//! callback applied once, a parameter a recursion passes along, a write. A
//! set read on a source that cannot batch is warned (PW0406), and the pair's
//! shape held (PW0407, a rule of its own since the integrator's ruling of
//! 2026-10-15). The store and the feed read nothing per element, and
//! kiokun.com's four reads per element are the known ones.

use pw_core::check::check_sources;
use pw_core::diagnostics::{Diagnostic, Severity};

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.pw` file of each directory, in order, by its file name.
fn files(dirs: &[&str]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in dirs {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root().join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out
}

/// Every diagnostic of `m.pw`, checked with the platform's packages.
fn diagnostics(src: &str) -> Vec<Diagnostic> {
    let mut sources = files(&["packages/pw-std", "packages/pw-platform-web"]);
    sources.push(("m.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(path, _)| path == "m.pw")
        .flat_map(|(_, ds)| ds)
        .collect()
}

/// `code message` of each diagnostic of `m.pw`.
fn reported(src: &str) -> Vec<String> {
    diagnostics(src)
        .iter()
        .map(|d| format!("{} {}", d.code, d.message))
        .collect()
}

/// What this track's rules report of `m.pw`: PW0405, PW0406, PW0407 (the
/// pair's shape) and PW5105 of a clause on a declaration it is not for.
fn bounded(src: &str) -> Vec<String> {
    reported(src)
        .into_iter()
        .filter(|d| {
            d.starts_with("PW0405")
                || d.starts_with("PW0406")
                || d.starts_with("PW0407")
                || d.starts_with("PW5105")
        })
        .collect()
}

const PROGRAM: &str = r#"module m

import List

type Price = Price { item: Int, cents: Int }

type PriceOf = PriceOf { item: Int, price: Price }

type Line = Line { item: Int, count: Int }

type Group = Group { first: Int, lines: List<Line> }

source Ledger
    holds        Price
    transactions serializable
    reads        strong
    changes      none
    sets         one_call

fn price_of(item: Int) -> Result<Price, String> !{ database.read<Price> }
    host "m:data/prices#for-item"
    set  prices_of

fn prices_of(items: List<Int>) -> List<PriceOf> !{ database.read<Price> }
    host "m:data/prices#for-items"

fn record(item: Int) -> Int !{ database.write<Price> }
    host "m:data/prices#record"

fn cents(r: Result<Price, String>) -> Int !{} {
    match r {
        Ok(p) => p.cents,
        Err(_) => 0,
    }
}

fn cents_in(found: List<PriceOf>, item: Int) -> Int !{} {
    match List.find(found, p => p.item == item) {
        Some(p) => p.price.cents,
        None => 0,
    }
}

fn item_of(l: Line) -> Int !{} {
    l.item
}

fn priced(lines: List<Line>) -> List<Int> !{ database.read<Price> } {
    let prices = prices_of(List.map(lines, l => l.item))
    List.map(lines, l => cents_in(prices, l.item) * l.count)
}
"#;

/// The program and one more declaration.
fn with(decl: &str) -> String {
    format!("{PROGRAM}\n{decl}")
}

#[test]
fn the_ledger_reads_its_lines_together_and_checks() {
    // The repaired form: one set read before the loop, a lookup inside it.
    // Every rule's, not this track's alone: each program below is valid but
    // for the one thing it is about.
    assert_eq!(reported(PROGRAM), Vec::<String>::new());
}

// --- PW0405: each loop form -----------------------------------------------------

#[test]
fn a_read_in_a_combinators_callback_keyed_by_its_element_is_refused() {
    let src = with(
        "fn per(lines: List<Line>) -> List<Result<Price, String>> !{ database.read<Price> } {\n    \
         List.map(lines, l => price_of(l.item))\n}\n",
    );
    assert_eq!(
        bounded(&src),
        ["PW0405 `price_of` reads `Price` once for each element of `lines`"]
    );
    // The diagnostic the design note writes: the key and where it comes
    // from, one read per key, and the set read before the loop.
    let d = diagnostics(&src)
        .into_iter()
        .find(|d| d.code == "PW0405")
        .expect("PW0405");
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(&src[d.primary_span.clone()], "price_of(l.item)");
    let labels: Vec<(&str, &str)> = d
        .related
        .iter()
        .map(|r| (&src[r.span.clone()], r.label.as_str()))
        .collect();
    assert_eq!(
        labels,
        [
            (
                "l.item",
                "its key `l.item` comes from `l`, the element `List.map` passes"
            ),
            ("lines", "one read per item"),
        ]
    );
    assert_eq!(
        d.repairs[0].description,
        "read them together, before the loop:\n    let prices = prices_of(List.map(lines, l => \
         l.item))\n    List.map(lines, l => prices)"
    );
    assert!(d.explanation.as_deref().unwrap().contains("(N+1)"));
}

#[test]
fn every_combinator_that_applies_its_callback_to_elements_is_a_loop() {
    // By its signature: a parameter `fn(.., T, ..)` beside a `List<T>`.
    for (combinator, call) in [
        ("map", "List.map(lines, l => cents(price_of(l.item)))"),
        (
            "filter",
            "List.filter(lines, l => cents(price_of(l.item)) > 0)",
        ),
        ("any", "List.any(lines, l => cents(price_of(l.item)) > 0)"),
        ("all", "List.all(lines, l => cents(price_of(l.item)) > 0)"),
        ("find", "List.find(lines, l => cents(price_of(l.item)) > 0)"),
        (
            "sort_by",
            "List.sort_by(lines, (a, b) => cents(price_of(a.item)) - b.count)",
        ),
        (
            "fold",
            "List.fold(lines, 0, (total, l) => total + cents(price_of(l.item)))",
        ),
        ("a pipe", "lines |> List.map(l => cents(price_of(l.item)))"),
    ] {
        let ty = match combinator {
            "map" | "a pipe" => "List<Int>",
            "filter" | "sort_by" => "List<Line>",
            "any" | "all" => "Bool",
            "find" => "Option<Line>",
            _ => "Int",
        };
        let src = with(&format!(
            "fn per(lines: List<Line>) -> {ty} !{{ database.read<Price> }} {{\n    {call}\n}}\n"
        ));
        assert_eq!(
            bounded(&src),
            ["PW0405 `price_of` reads `Price` once for each element of `lines`"],
            "{combinator}: {:?}",
            reported(&src)
        );
    }
}

#[test]
fn a_read_in_a_for_keyed_by_its_element_through_a_let_is_refused() {
    let src = with(
        "fn per(lines: List<Line>) -> Int !{ database.read<Price> } {\n    let mut n = 0\n    \
         for l in lines {\n        let item = l.item\n        n = n + cents(price_of(item))\n    \
         }\n    n\n}\n",
    );
    assert_eq!(
        bounded(&src),
        ["PW0405 `price_of` reads `Price` once for each element of `lines`"]
    );
    let d = diagnostics(&src)
        .into_iter()
        .find(|d| d.code == "PW0405")
        .expect("PW0405");
    assert_eq!(
        d.related[0].label,
        "its key `item` comes from `l`, the element `for` binds"
    );
    // The key the set read's list computes is the `let`'s.
    assert!(
        d.repairs[0]
            .description
            .contains("prices_of(List.map(lines, l => l.item))"),
        "{}",
        d.repairs[0].description
    );
}

#[test]
fn a_read_in_a_recursion_over_data_keyed_by_its_part_is_refused() {
    // A list walked by dropping its first, and a count walked down.
    let list = with(
        "fn per(lines: List<Line>) -> Int !{ database.read<Price> } {\n    match \
         List.get(lines, 0) {\n        None => 0,\n        Some(l) => cents(price_of(l.item)) + \
         per(List.drop(lines, 1)),\n    }\n}\n",
    );
    assert_eq!(
        bounded(&list),
        [
            "PW0405 `price_of` reads `Price` once for each call of `per`, which recurses over `lines`"
        ]
    );
    let d = diagnostics(&list)
        .into_iter()
        .find(|d| d.code == "PW0405")
        .expect("PW0405");
    assert_eq!(
        d.related[0].label,
        "its key `l.item` comes from `lines`, a part of which `per` passes to its recursive call"
    );
    assert_eq!(&list[d.related[1].span.clone()], "per(List.drop(lines, 1))");
    let count = with(
        "fn per(n: Int) -> Int !{ database.read<Price> } {\n    if n <= 0 {\n        0\n    } \
         else {\n        cents(price_of(n)) + per(n - 1)\n    }\n}\n",
    );
    assert_eq!(
        bounded(&count),
        ["PW0405 `price_of` reads `Price` once for each call of `per`, which recurses over `n`"]
    );
}

#[test]
fn a_key_comes_from_the_element_through_a_field_a_pure_call_and_a_match_arm() {
    for (how, body) in [
        ("a field", "List.map(lines, l => cents(price_of(l.item)))"),
        (
            "a pure call",
            "List.map(lines, l => cents(price_of(item_of(l))))",
        ),
        (
            "an arithmetic",
            "List.map(lines, l => cents(price_of(l.item + l.count)))",
        ),
        (
            "a match arm",
            "List.map(lines, l => match Some(l.item) {\n        Some(i) => \
             cents(price_of(i)),\n        None => 0,\n    })",
        ),
    ] {
        let src = with(&format!(
            "fn per(lines: List<Line>) -> List<Int> !{{ database.read<Price> }} {{\n    {body}\n}}\n"
        ));
        assert_eq!(
            bounded(&src),
            ["PW0405 `price_of` reads `Price` once for each element of `lines`"],
            "{how}: {:?}",
            reported(&src)
        );
    }
}

#[test]
fn a_helper_that_reads_by_what_it_is_given_is_a_read_per_element() {
    let helper = "fn charge(l: Line) -> Int !{ database.read<Price> } {\n    \
                  cents(price_of(l.item)) * l.count\n}\n";
    // Called in the loop.
    let called = with(&format!(
        "{helper}\nfn per(lines: List<Line>) -> List<Int> !{{ database.read<Price> }} {{\n    \
         List.map(lines, l => charge(l))\n}}\n"
    ));
    assert_eq!(
        bounded(&called),
        ["PW0405 `charge` reads `Price` once for each element of `lines`"]
    );
    let d = diagnostics(&called)
        .into_iter()
        .find(|d| d.code == "PW0405")
        .expect("PW0405");
    assert!(
        d.related
            .iter()
            .any(|r| r.label
                == "`charge` reads `Price` through `price_of`, keyed by its parameter `l`"),
        "{:?}",
        d.related
    );
    // Named as the callback.
    let named = with(&format!(
        "{helper}\nfn per(lines: List<Line>) -> List<Int> !{{ database.read<Price> }} {{\n    \
         List.map(lines, charge)\n}}\n"
    ));
    assert_eq!(
        bounded(&named),
        ["PW0405 `charge` reads `Price` once for each element of `lines`"]
    );
}

#[test]
fn a_callback_bound_by_a_let_or_passed_on_by_a_function_is_in_the_loop() {
    let bound = with(
        "fn per(lines: List<Line>) -> List<Int> !{ database.read<Price> } {\n    let f = (l: \
         Line) => cents(price_of(l.item))\n    List.map(lines, f)\n}\n",
    );
    assert_eq!(
        bounded(&bound),
        ["PW0405 `price_of` reads `Price` once for each element of `lines`"]
    );
    // A function of the program's that passes its callback on to `List.map`
    // applies it to its list's elements: the call graph says so.
    let passed = with(
        "fn each<T>(xs: List<T>, f: fn(T) -> Int) -> List<Int> !{} {\n    List.map(xs, f)\n}\n\n\
         fn per(lines: List<Line>) -> List<Int> !{ database.read<Price> } {\n    each(lines, l \
         => cents(price_of(l.item)))\n}\n",
    );
    assert_eq!(
        bounded(&passed),
        ["PW0405 `price_of` reads `Price` once for each element of `lines`"]
    );
    // And one that calls its callback in a loop of its own.
    let called = with(
        "fn each_line(lines: List<Line>, f: fn(Line) -> Int) -> Int !{} {\n    let mut n = 0\n    \
         for l in lines {\n        n = n + f(l)\n    }\n    n\n}\n\n\
         fn per(lines: List<Line>) -> Int !{ database.read<Price> } {\n    each_line(lines, l => \
         cents(price_of(l.item)))\n}\n",
    );
    assert_eq!(
        bounded(&called),
        ["PW0405 `price_of` reads `Price` once for each element of `lines`"]
    );
}

#[test]
fn a_read_keyed_by_an_outer_loops_element_is_refused_for_the_outer_loop() {
    let src = with(
        "fn per(groups: List<Group>) -> List<List<Int>> !{ database.read<Price> } {\n    \
         List.map(groups, g => List.map(g.lines, l => cents(price_of(g.first))))\n}\n",
    );
    assert_eq!(
        bounded(&src),
        ["PW0405 `price_of` reads `Price` once for each element of `groups`"]
    );
}

// --- PW0405's controls ------------------------------------------------------------

#[test]
fn what_is_no_read_per_element_is_accepted() {
    for (what, decl) in [
        (
            "a key that is no element's: one key, the host's cache answers its repeats",
            "fn per(lines: List<Line>, first: Int) -> List<Int> !{ database.read<Price> } {\n    \
             List.map(lines, l => cents(price_of(first)) * l.count)\n}\n",
        ),
        (
            "a callback a function applies once",
            "fn once(x: Line, f: fn(Line) -> Int) -> Int !{} {\n    f(x)\n}\n\n\
             fn per(line: Line) -> Int !{ database.read<Price> } {\n    once(line, l => \
             cents(price_of(l.item)))\n}\n",
        ),
        (
            "a parameter a recursion passes along whole",
            "fn per(lines: List<Line>, first: Int) -> Int !{ database.read<Price> } {\n    match \
             List.get(lines, 0) {\n        None => 0,\n        Some(_) => cents(price_of(first)) \
             + per(List.drop(lines, 1), first),\n    }\n}\n",
        ),
        (
            "a helper whose read is keyed by what is no element's",
            "fn charge_with(l: Line, item: Int) -> Int !{ database.read<Price> } {\n    \
             cents(price_of(item)) * l.count\n}\n\n\
             fn per(lines: List<Line>, first: Int) -> List<Int> !{ database.read<Price> } {\n    \
             List.map(lines, l => charge_with(l, first))\n}\n",
        ),
        (
            "a write per element, which this rule does not bound",
            "fn per(lines: List<Line>) -> Int !{ database.write<Price> } {\n    let mut n = 0\n    \
             for l in lines {\n        n = n + record(l.item)\n    }\n    n\n}\n",
        ),
        (
            "a read before the loop, its answer read in it",
            "fn per(lines: List<Line>, first: Int) -> List<Int> !{ database.read<Price> } {\n    \
             let one = cents(price_of(first))\n    List.map(lines, l => one * l.count)\n}\n",
        ),
    ] {
        let src = with(decl);
        assert_eq!(reported(&src), Vec::<String>::new(), "{what}");
    }
}

#[test]
fn a_call_that_reads_and_writes_per_element_is_refused_as_a_read() {
    // Ruled by the integrator, 2026-10-15 (question 4): a call whose row
    // reads and writes is a read, and refused per element, where one that
    // only writes is not (`what_is_no_read_per_element_is_accepted`).
    let src = with(
        "fn settle(item: Int) -> Int !{ database.read<Price>, database.write<Price> }\n    host \
         \"m:data/prices#settle\"\n\nfn per(lines: List<Line>) -> List<Int> !{ \
         database.read<Price>, database.write<Price> } {\n    List.map(lines, l => \
         settle(l.item))\n}\n",
    );
    assert_eq!(
        bounded(&src),
        ["PW0405 `settle` reads `Price` once for each element of `lines`"]
    );
}

#[test]
fn a_key_a_read_computes_ends_the_trace_and_its_read_is_refused_once() {
    // The design note's limit: a key is traced through a name, a `let`, a
    // field and a pure call, not through another read. The inner read is the
    // loop's read per element, refused; the outer, keyed by what the inner
    // answers, is the same turn of the same loop, and not refused again.
    let src = with(
        "fn per(lines: List<Line>) -> List<Int> !{ database.read<Price> } {\n    \
         List.map(lines, l => cents(price_of(cents(price_of(l.item)))))\n}\n",
    );
    let found = diagnostics(&src);
    let refused: Vec<&str> = found
        .iter()
        .filter(|d| d.code == "PW0405")
        .map(|d| &src[d.primary_span.clone()])
        .collect();
    assert_eq!(refused, ["price_of(l.item)"]);
}

#[test]
fn set_and_sets_head_a_clause_only_where_a_name_follows() {
    // `set` and `sets` are names a body may begin with: in a block whose
    // clauses come first, `set.size()` is a statement, not a clause.
    let src = "module m\n\nimport Set\n\nview Count(set: Set<Int>) {\n    placement browser\n    \
               set.size()\n}\n";
    let p = pw_syntax::parse_tree(src);
    let hir = pw_core::lower::lower_file(src, &p.green);
    let (_, view) = hir
        .all_decls()
        .find(|(_, d)| d.name == "Count")
        .expect("the view");
    let heads: Vec<&str> = view.policies.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(heads, ["placement"]);
    // Where a name follows, each is a clause.
    let clause = with("");
    let p = pw_syntax::parse_tree(&clause);
    let hir = pw_core::lower::lower_file(&clause, &p.green);
    let heads: Vec<&str> = hir
        .all_decls()
        .flat_map(|(_, d)| d.policies.iter().map(|p| p.name.as_str()))
        .filter(|h| *h == "set" || *h == "sets")
        .collect();
    assert_eq!(heads, ["sets", "set"]);
}

#[test]
fn a_reads_verdict_depends_on_its_key_and_on_nothing_else() {
    // ADR-0022's fifth check: renaming the element, binding the key by a
    // `let`, or a same-spelled read of another module's, changes nothing.
    let base = "fn per(lines: List<Line>) -> List<Int> !{ database.read<Price> } {\n    \
                List.map(lines, l => cents(price_of(l.item)))\n}\n";
    let want = ["PW0405 `price_of` reads `Price` once for each element of `lines`"];
    assert_eq!(bounded(&with(base)), want);
    let renamed = base.replace(
        "l => cents(price_of(l.item))",
        "line => cents(price_of(line.item))",
    );
    assert_eq!(bounded(&with(&renamed)), want);
    let rebound = base.replace(
        "List.map(lines, l => cents(price_of(l.item)))",
        "List.map(lines, l => {\n        let key = l.item\n        cents(price_of(key))\n    })",
    );
    assert_eq!(bounded(&with(&rebound)), want);
    // A pure `price_of` in a module this one does not import is not this one.
    let mut sources = files(&["packages/pw-std", "packages/pw-platform-web"]);
    sources.push((
        "decoy.pw".to_string(),
        "module decoy\n\nfn price_of(item: Int) -> Int !{} {\n    item\n}\n".to_string(),
    ));
    sources.push(("m.pw".to_string(), with(base)));
    let found: Vec<String> = check_sources(&sources)
        .into_iter()
        .filter(|(p, _)| p == "m.pw")
        .flat_map(|(_, ds)| ds)
        .map(|d| format!("{} {}", d.code, d.message))
        .collect();
    assert_eq!(found, want);
}

// --- PW0406 -------------------------------------------------------------------------

#[test]
fn a_set_read_on_a_source_that_cannot_batch_is_warned() {
    let per_key = PROGRAM.replace("    sets         one_call\n", "    sets         per_key\n");
    assert_eq!(
        bounded(&per_key),
        ["PW0406 `priced` reads `Price` once per item on `Ledger`, which cannot batch"]
    );
    let d = diagnostics(&per_key)
        .into_iter()
        .find(|d| d.code == "PW0406")
        .expect("PW0406");
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(
        &per_key[d.primary_span.clone()],
        "prices_of(List.map(lines, l => l.item))"
    );
    assert_eq!(
        d.related[0].label,
        "`Ledger` states `sets per_key`: a set read is a call per key"
    );
    assert_eq!(
        d.repairs[0].description,
        "hold `Price` in a source that batches, or bound the list"
    );
    // A source that states no `sets` guarantees nothing: `per_key`.
    let unstated = PROGRAM.replace("    sets         one_call\n", "");
    assert_eq!(
        bounded(&unstated),
        ["PW0406 `priced` reads `Price` once per item on `Ledger`, which cannot batch"]
    );
    let d = diagnostics(&unstated)
        .into_iter()
        .find(|d| d.code == "PW0406")
        .expect("PW0406");
    assert_eq!(
        d.related[0].label,
        "`Ledger` states no `sets`, so `per_key`: a set read is a call per key"
    );
}

#[test]
fn a_set_read_on_a_source_that_batches_is_not_warned() {
    for sets in ["one_call", "chunked(100)"] {
        let src = PROGRAM.replace(
            "    sets         one_call\n",
            &format!("    sets         {sets}\n"),
        );
        assert_eq!(reported(&src), Vec::<String>::new(), "{sets}");
    }
    // A resource no source holds is the host's own database's, which gives
    // every guarantee (ADR-0207; ruled by the integrator, 2026-10-15).
    let hosts = PROGRAM.replace(
        "source Ledger\n    holds        Price\n    transactions serializable\n    reads        \
         strong\n    changes      none\n    sets         one_call\n\n",
        "",
    );
    assert_ne!(hosts, PROGRAM);
    assert_eq!(reported(&hosts), Vec::<String>::new());
    // Beside a source that cannot batch, one of another resource: what no
    // source holds is still the host's own, not that source's.
    let beside = hosts.replace(
        "fn price_of(",
        "type Other = Other { id: Int }\n\nsource Mirror\n    holds        Other\n    transactions \
         serializable\n    reads        strong\n    changes      none\n\nfn price_of(",
    );
    assert_ne!(beside, hosts);
    assert_eq!(reported(&beside), Vec::<String>::new());
    // And a read no `set` names is no set read, whatever its source.
    let unpaired = PROGRAM
        .replace("    set  prices_of\n", "")
        .replace("    sets         one_call\n", "    sets         per_key\n");
    assert_eq!(reported(&unpaired), Vec::<String>::new());
}

// --- the pair and the clause ------------------------------------------------------------

#[test]
fn a_set_names_a_function_of_the_same_source_given_and_answering_lists() {
    let shaped = |set: &str, decl: &str| {
        let src = PROGRAM
            .replace("    set  prices_of\n", &format!("    set  {set}\n"))
            .replace("fn record(", &format!("{decl}\nfn record("));
        bounded(&src)
    };
    assert_eq!(
        shaped("nothing_here", ""),
        ["PW0407 `set nothing_here`: `nothing_here` names no function visible here"]
    );
    assert_eq!(
        shaped("Price", ""),
        ["PW0407 `set Price`: `Price` names no function visible here"]
    );
    assert_eq!(
        shaped("price_of", ""),
        ["PW0407 `set price_of`: `price_of` is `price_of` itself"]
    );
    assert_eq!(
        shaped(
            "price_one",
            "fn price_one(item: Int) -> List<PriceOf> !{ database.read<Price> }\n    host \
             \"m:data/prices#one\"\n"
        ),
        [
            "PW0407 `set price_one`: `price_one` takes `(Int)`, and a set read of `price_of` takes `(List<Int>)`"
        ]
    );
    assert_eq!(
        shaped(
            "cents_of",
            "fn cents_of(items: List<Int>) -> List<Int> !{ database.read<Price> }\n    host \
             \"m:data/prices#cents\"\n"
        ),
        [
            "PW0407 `set cents_of`: `cents_of` answers `List<Int>`, and a set read of `price_of` answers a list of `Price` with its `Int`"
        ]
    );
    assert_eq!(
        shaped(
            "elsewhere",
            "type Other = Other { item: Int }\n\nsource Mirror\n    holds        Other\n    \
             transactions serializable\n    reads        strong\n    changes      none\n\n\
             fn elsewhere(items: List<Int>) -> List<PriceOf> !{ database.read<Other> }\n    host \
             \"m:data/others#for-items\"\n"
        ),
        [
            "PW0407 `set elsewhere`: `elsewhere` reads from `Mirror`, and `price_of` from `Ledger`: a set read is of its keyed read's source"
        ]
    );
    // A list of the keyed result that holds its key is a set read too: here
    // a `Price` holds its `item`.
    assert_eq!(
        shaped(
            "prices_alone",
            "fn prices_alone(items: List<Int>) -> List<Price> !{ database.read<Price> }\n    \
             host \"m:data/prices#alone\"\n"
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_set_that_does_not_fit_is_the_pairs_own_rule_and_a_bad_sets_value_is_a_policys() {
    // The integrator's ruling of 2026-10-15: a `set` that does not fit its
    // keyed read is refused by a rule of its own, PW0407, and not as a policy's
    // value (PW0335): one defect, one diagnostic, and it names its rule.
    let src = PROGRAM
        .replace("    set  prices_of\n", "    set  cents_of\n")
        .replace(
            "fn record(",
            "fn cents_of(items: List<Int>) -> List<Int> !{ database.read<Price> }\n    host \
             \"m:data/prices#cents\"\n\nfn record(",
        );
    let found = diagnostics(&src);
    assert_eq!(found.len(), 1, "{:?}", reported(&src));
    let d = &found[0];
    assert_eq!(d.code, "PW0407");
    assert_eq!(d.symbol(), "set_pair_shape");
    assert_eq!(d.invariant, pw_core::codes::SET_PAIR_SHAPE.invariant);
    assert_eq!(d.reason, "set_pair_shape");
    assert_eq!(d.severity, Severity::Error);
    // At the clause the author wrote, with the set form it names beside it.
    assert_eq!(&src[d.primary_span.clone()], "set  cents_of");
    let notes: Vec<(&str, &str)> = d
        .related
        .iter()
        .map(|r| (&src[r.span.clone()], r.label.as_str()))
        .collect();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].0.starts_with("fn cents_of(items: List<Int>)"),
        "{notes:?}"
    );
    assert_eq!(notes[0].1, "`cents_of` is declared here");
    assert!(
        d.repairs[0]
            .description
            .starts_with("name a function of the same source"),
        "{:?}",
        d.repairs
    );
    // And a fit one reports nothing, so the code is the shape's and nothing
    // else's; while a `sets` value outside its domain stays a policy's value.
    assert!(
        !reported(PROGRAM)
            .iter()
            .any(|d| d.starts_with("PW0407") || d.starts_with("PW0335")),
        "{:?}",
        reported(PROGRAM)
    );
    let bad = PROGRAM.replace("    sets         one_call\n", "    sets         batched\n");
    let shape_or_value: Vec<&str> = diagnostics(&bad)
        .iter()
        .map(|d| d.code)
        .filter(|c| *c == "PW0335" || *c == "PW0407")
        .collect();
    assert_eq!(shape_or_value, ["PW0335"], "{:?}", reported(&bad));
}

#[test]
fn a_set_belongs_to_a_function_that_reads_and_sets_to_a_source() {
    // On a function that reads no source.
    let pure = PROGRAM.replace(
        "fn item_of(l: Line) -> Int !{} {",
        "fn item_of(l: Line) -> Int !{}\n    set  prices_of\n{",
    );
    assert_eq!(
        bounded(&pure),
        ["PW0407 `set prices_of`: `item_of` reads no source, so it has no set read"]
    );
    // `set` on a query, and `sets` on a function, are each another's clause.
    let on_query = PROGRAM.replace(
        "fn priced(lines: List<Line>) -> List<Int> !{ database.read<Price> } {",
        "query Priced(lines: List<Line>) -> List<Int>\n    set  prices_of\n{",
    );
    assert!(
        reported(&on_query)
            .iter()
            .any(|d| d.starts_with("PW5105") && d.contains("`set` belongs to a function")),
        "{:?}",
        reported(&on_query)
    );
    let on_fn = PROGRAM.replace(
        "    set  prices_of\n",
        "    set  prices_of\n    sets one_call\n",
    );
    assert!(
        reported(&on_fn)
            .iter()
            .any(|d| d.starts_with("PW5105") && d.contains("`sets` belongs to a data source")),
        "{:?}",
        reported(&on_fn)
    );
}

#[test]
fn a_sources_sets_is_one_call_chunked_or_per_key() {
    for sets in [
        "chunked(0)",
        "chunked",
        "chunked(two)",
        "chunked(1, 2)",
        "batched",
    ] {
        let src = PROGRAM.replace(
            "    sets         one_call\n",
            &format!("    sets         {sets}\n"),
        );
        assert!(
            reported(&src).iter().any(|d| d.starts_with("PW0335 `sets")),
            "{sets}: {:?}",
            reported(&src)
        );
    }
}

#[test]
fn a_build_states_what_each_set_read_costs_its_source() {
    let clauses = |src: &str| {
        let p = pw_syntax::parse_tree(src);
        let hir = pw_core::lower::lower_file(src, &p.green);
        pw_core::check::source_clauses(&[&hir])
            .into_iter()
            .map(|s| (s.name, s.sets))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        clauses(PROGRAM),
        [("Ledger".to_string(), "one_call".to_string())]
    );
    let chunked = PROGRAM.replace("sets         one_call", "sets         chunked(30)");
    assert_eq!(
        clauses(&chunked),
        [("Ledger".to_string(), "chunked(30)".to_string())]
    );
    let unstated = PROGRAM.replace("    sets         one_call\n", "");
    assert_eq!(
        clauses(&unstated),
        [("Ledger".to_string(), "per_key".to_string())]
    );
}

#[test]
fn a_set_reads_cost_is_what_its_source_states() {
    use pw_core::bounded::Sets;
    assert_eq!(Sets::of(None), Sets::PerKey);
    assert_eq!(Sets::of(Some("one_call")), Sets::OneCall);
    assert_eq!(Sets::of(Some("per_key")), Sets::PerKey);
    assert_eq!(Sets::of(Some("chunked(100)")), Sets::Chunked(100));
    // A value the domain lacks guarantees nothing.
    assert_eq!(Sets::of(Some("chunked(0)")), Sets::PerKey);
    assert_eq!(Sets::of(Some("batched")), Sets::PerKey);
    // The design note's budget: one call, at least one per `k` keys, or one
    // per key.
    assert_eq!(Sets::OneCall.calls(250), 1);
    assert_eq!(Sets::Chunked(100).calls(250), 3);
    assert_eq!(Sets::Chunked(100).calls(200), 2);
    assert_eq!(Sets::PerKey.calls(250), 250);
    assert_eq!(Sets::Chunked(30).written(), "chunked(30)");
}

#[test]
fn a_build_from_before_sets_reads_as_guaranteeing_nothing() {
    // A `sources.json` written before 4c states no `sets`: read again, its
    // sources are `per_key`, as one that states none is.
    let older = r#"{"name":"Ledger","holds":["Price"],"transactions":"serializable","reads":["strong"],"changes":"none"}"#;
    let clauses: pw_core::check::SourceClauses =
        serde_json::from_str(older).expect("an older build's clauses");
    assert_eq!(clauses.sets, "per_key");
}

// --- the reference programs -----------------------------------------------------------

/// The store demo, as the one program it is.
fn store() -> Vec<(String, String)> {
    let mut out = files(&[
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ]);
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root().join("examples/domain.pw")).expect("domain.pw"),
    ));
    out
}

/// What this track's rules report of a program, each with its file.
fn bounded_in(program: &[(String, String)]) -> Vec<String> {
    check_sources(program)
        .into_iter()
        .flat_map(|(path, ds)| {
            ds.into_iter()
                .map(move |d| format!("{path}: {} {}", d.code, d.message))
        })
        .filter(|d| d.contains(": PW0405 ") || d.contains(": PW0406 ") || d.contains(": PW0407 "))
        .collect()
}

/// `program` with one file's text replaced.
fn edited(program: &[(String, String)], file: &str, from: &str, to: &str) -> Vec<(String, String)> {
    program
        .iter()
        .map(|(path, src)| {
            if path == file {
                assert_eq!(src.matches(from).count(), 1, "{file}: {from}");
                (path.clone(), src.replace(from, to))
            } else {
                (path.clone(), src.clone())
            }
        })
        .collect()
}

#[test]
fn the_store_reads_nothing_per_element() {
    // The design note's survey, held: the store's programs read their lists
    // together (`StoresOpen`'s hours, the checkout's fees).
    assert_eq!(bounded_in(&store()), Vec::<String>::new());
}

#[test]
fn the_stores_loop_is_seen_and_refused_where_it_reads_per_element() {
    // ADR-0022's accepted neighbour: the store is clean because the rule ran
    // and its keys are no element's, not because it saw nothing. Its home
    // page's loop, reading each store's hours in it, is refused.
    let per_store = edited(
        &store(),
        "hours.pw",
        "    List.map(stores, s => listed(s, hours))\n",
        "    List.map(stores, s => listed(s, hours_of_stores([s.id])))\n",
    );
    assert_eq!(
        bounded_in(&per_store),
        ["hours.pw: PW0405 `hours_of_stores` reads `Stores` once for each element of `stores`"]
    );
}

#[test]
fn the_stores_pair_and_its_sources_cost_are_the_integrators_to_add_together() {
    // The annotations the ADR lists for the merge, applied here: `set
    // hours_of_stores` on `hours_of`, and `sets one_call` on `StoreData`.
    let paired = edited(
        &store(),
        "hours.pw",
        "    host \"store:data/hours#for-store\"\n",
        "    host \"store:data/hours#for-store\"\n    set  hours_of_stores\n",
    );
    // The pair alone: `StoreData` states no `sets`, so each set read of the
    // hours warns, the design note's example.
    assert_eq!(
        bounded_in(&paired),
        [
            "hours.pw: PW0406 `StoresOpen` reads `Stores` once per store on `StoreData`, which cannot batch",
            "search.pw: PW0406 `Search` reads `Stores` once per store on `StoreData`, which cannot batch"
        ]
    );
    let stated = edited(
        &paired,
        "StoreData.pw",
        "    changes      none\n",
        "    changes      none\n    sets         one_call\n",
    );
    let all: Vec<String> = check_sources(&stated)
        .into_iter()
        .flat_map(|(path, ds)| {
            ds.into_iter()
                .map(move |d| format!("{path}: {} {}", d.code, d.message))
        })
        .collect();
    assert_eq!(all, Vec::<String>::new());
}

#[test]
fn the_feed_reads_nothing_per_element() {
    let mut feed = files(&[
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/feed",
    ]);
    feed.sort();
    assert_eq!(bounded_in(&feed), Vec::<String>::new());
}

#[test]
fn kiokun_reads_per_element_where_it_is_known_to_and_nowhere_else() {
    // 4c's finding: kiokun.com, rewritten in Pleris, asks a search per
    // reading and a pitch file per word, as kiokun.com does. Each is a
    // warning, and says why, until its layer reads them together; the list
    // is held to exactly these.
    let mut site = files(&[
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/kiokun-site",
    ]);
    for f in ["examples/kiokun/Shards.pw", "examples/kiokun/dictionary.pw"] {
        let p = root().join(f);
        site.push((
            p.file_name().unwrap().to_string_lossy().to_string(),
            std::fs::read_to_string(&p).expect("read"),
        ));
    }
    let found: Vec<Diagnostic> = check_sources(&site)
        .into_iter()
        .flat_map(|(_, ds)| ds)
        .filter(|d| d.code == "PW0405")
        .collect();
    let messages: Vec<&str> = found.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            "`search_reading` reads `ReadingHit` once for each element of `unfit_kana(candidates, asked_entries)`",
            "`read_pitch` reads `PitchReading` once for each element of `l.data.japanese`",
            "`search_reading` reads `ReadingHit` once for each element of `List.filter(readings_of(l.data), r => String.trim(r) != \"\")`",
            "`search_reading` reads `ReadingHit` once for each element of `unfit_kana(candidates, asked_entries)`"
        ]
    );
    for d in &found {
        assert_eq!(d.severity, Severity::Warning, "{}", d.message);
        assert_eq!(d.reason, "known_read_per_element");
        assert!(
            d.explanation
                .as_deref()
                .is_some_and(|e| e.contains("its layer has no set form")),
            "{:?}",
            d.explanation
        );
    }
    // Every entry of the list is a read it names: a stale one fails here.
    for (at, read, _) in pw_core::bounded::KNOWN_READS_PER_ELEMENT {
        let short = read.rsplit('.').next().unwrap();
        assert!(
            found
                .iter()
                .any(|d| d.message.starts_with(&format!("`{short}`"))),
            "{at} {read} is listed and reads nothing per element"
        );
    }
    assert_eq!(pw_core::bounded::KNOWN_READS_PER_ELEMENT.len(), 3);
}

#[test]
fn a_known_read_is_known_only_where_the_list_names_it() {
    // The same read per element, in a declaration the list does not name, is
    // refused: the list excuses four reads, not a read.
    let mut site = files(&[
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/kiokun-site",
    ]);
    for f in ["examples/kiokun/Shards.pw", "examples/kiokun/dictionary.pw"] {
        let p = root().join(f);
        site.push((
            p.file_name().unwrap().to_string_lossy().to_string(),
            std::fs::read_to_string(&p).expect("read"),
        ));
    }
    let site = edited(
        &site,
        "app.pw",
        "fn homophone_limit() -> Int {",
        "fn homophones_of(readings: List<String>) -> List<List<ReadingHit>> !{ database.read<ReadingHit> } {\n    \
         List.map(readings, r => search_reading(r, 10))\n}\n\nfn homophone_limit() -> Int {",
    );
    let refused: Vec<String> = check_sources(&site)
        .into_iter()
        .flat_map(|(_, ds)| ds)
        .filter(|d| d.code == "PW0405" && d.severity == Severity::Error)
        .map(|d| d.message)
        .collect();
    assert_eq!(
        refused,
        ["`search_reading` reads `ReadingHit` once for each element of `readings`"]
    );
}
