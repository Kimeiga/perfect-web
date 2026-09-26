//! **A lambda's parameters take the types its use declares** (ADR-0053).
//!
//! Until 2026-09-25 the checker typed a lambda's parameters one way: the
//! FIRST parameter took the element type of any list passed beside it,
//! whatever the callee declared. `fold`'s callback is `fn(A, T) -> A`, so its
//! first parameter is the accumulator, and
//! `List.fold(ws, 0, (t, w) => t + w.score)` was refused: "the left side of
//! `+` must be `Int or Float`, and this is `t.Word`". Every other parameter
//! was untyped, so nothing in a lambda's body that read one was checked.
//! Found compiling an opaque type's fold (ADR-0054).

use pw_core::check::check_sources;

fn std_lib() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/pw-std");
    let mut out = Vec::new();
    for e in std::fs::read_dir(root).expect("pw-std") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "pw") {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.sort();
    out
}

/// `code message` for each diagnostic on `t.pw`, checked with the standard
/// library.
fn reported(src: &str) -> Vec<String> {
    let mut files = std_lib();
    files.push(("t.pw".to_string(), src.to_string()));
    check_sources(&files)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn codes(src: &str) -> Vec<String> {
    reported(src)
        .into_iter()
        .map(|r| r.split(' ').next().unwrap_or_default().to_string())
        .collect()
}

fn program(decls: &str) -> String {
    format!(
        "module t\n\nimport List\nimport String\n\n\
         type Word = Word {{ text: String, score: Int }}\n\n{decls}\n"
    )
}

const CLEAN: [&str; 0] = [];

/// `x + ..` with `x` a `String`. The right side is reported too, for not
/// being a `String` like its left.
const STRING_SUM: &str = "PW0609 the left side of `+` must be `Int or Float`, and this is `String`";

#[test]
fn a_folds_accumulator_is_its_seeds_type() {
    let src =
        program("fn total(ws: List<Word>) -> Int { List.fold(ws, 0, (t, w) => t + w.score) }");
    assert_eq!(codes(&src), CLEAN);
}

#[test]
fn a_folds_accumulator_and_element_are_each_checked() {
    // `t` is the `Int` seed, so it is not a condition. Before 2026-09-25
    // this was refused too, as a `Word`.
    let t = program(
        "fn total(ws: List<Word>) -> Int {\n    \
         List.fold(ws, 0, (t, w) => if t { w.score } else { 0 })\n}",
    );
    assert_eq!(
        reported(&t),
        ["PW0609 the condition of `if` must be `Bool`, and this is `Int`"]
    );
    // `w` is a `Word`, which `+` does not take.
    let w = program("fn total(ws: List<Word>) -> Int { List.fold(ws, 0, (t, w) => t + w) }");
    assert_eq!(codes(&w), ["PW0609"]);
    assert!(
        reported(&w)[0].ends_with("this is `t.Word`"),
        "{:?}",
        reported(&w)
    );
}

#[test]
fn a_programs_own_callback_takes_its_declared_parameter_types() {
    // `render`'s callback takes the text so far, then the word. Its first
    // parameter was typed as the word.
    let src = |body: &str| {
        program(&format!(
            "fn render(ws: List<Word>, f: fn(String, Word) -> String) -> String {{ \"\" }}\n\n\
             fn shown(ws: List<Word>) -> String {{\n    render(ws, (acc, w) => {body})\n}}"
        ))
    };
    assert_eq!(
        codes(&src("if String.length(acc) > 0 { acc } else { w.text }")),
        CLEAN
    );
    assert_eq!(codes(&src("if acc { acc } else { w.text }")), ["PW0609"]);
}

#[test]
fn a_lambda_bound_with_a_written_type_takes_its_parameters_from_it() {
    let src = |param: &str| {
        program(&format!(
            "fn f() -> Int {{\n    let inc: fn({param}) -> Int = x => x + 1\n    0\n}}"
        ))
    };
    assert_eq!(codes(&src("Int")), CLEAN);
    assert_eq!(reported(&src("String"))[0], STRING_SUM);
}

#[test]
fn a_returned_lambda_takes_its_parameters_from_the_declared_result() {
    let src = |param: &str| {
        program(&format!(
            "fn adder(k: Int) -> fn({param}) -> Int {{ x => x + k }}"
        ))
    };
    assert_eq!(codes(&src("Int")), CLEAN);
    assert_eq!(reported(&src("String"))[0], STRING_SUM);
}

#[test]
fn a_piped_lists_callback_still_takes_its_element() {
    let src = |field: &str| {
        program(&format!(
            "fn f(ws: List<Word>) -> List<Int> {{ ws |> List.map(w => w.{field}) }}"
        ))
    };
    assert_eq!(codes(&src("score")), CLEAN);
    assert_eq!(codes(&src("scores")), ["PW0610"]);
}

#[test]
fn a_name_two_lambdas_bind_is_still_not_guessed() {
    // `x` is a `Word` in one lambda and an `Int` in the other. A flat
    // environment cannot say which a use means, so neither use is typed.
    let src = program(
        "fn f(ws: List<Word>, ns: List<Int>) -> Int {\n    \
         let a = List.map(ws, x => x.score)\n    \
         let b = List.map(ns, x => x + 1)\n    0\n}",
    );
    assert_eq!(codes(&src), CLEAN);
}

#[test]
fn a_lambda_over_a_bound_list_is_typed_once_the_binding_is() {
    // `kept` is typed by solving `filter`, so `w`'s type is known only in a
    // later round. It must not be fixed as unknown in the first.
    let src = |body: &str| {
        program(&format!(
            "fn total(ws: List<Word>) -> Int {{\n    \
             let kept = List.filter(ws, w => w.score > 0)\n    \
             List.fold(kept, 0, (t, k) => {body})\n}}"
        ))
    };
    assert_eq!(codes(&src("t + k.score")), CLEAN);
    let bad = reported(&src("t + k"));
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].ends_with("this is `t.Word`"), "{bad:?}");
}
