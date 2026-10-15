//! **A value from a browser is held to its type's invariant where it
//! arrives** (track `argument-invariants`, ADR-XXXX): `examples/demo/
//! invariants.pw`, built as `pw build` builds it and served. A value of an
//! opaque type with a `where` invariant exists only if it passed the
//! invariant: a command's arguments and a page's parameters are decoded
//! through the checks their types state, at the top of a value and inside
//! one, and one that breaks them is refused before anything reads it, 400
//! and why.

use super::*;

/// The fixture, built and served: the platform's packages, the domain, and
/// `demo/invariants.pw`.
fn served() -> Served {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = tempfile::TempDir::with_prefix("pw-invariants-").expect("a temporary directory");
    let mut paths = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut here: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        here.sort();
        paths.extend(here);
    }
    paths.push(root.join("examples/domain.pw"));
    paths.push(root.join("examples/demo/invariants.pw"));
    let units: Vec<pw_core::check::Unit> = paths
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            pw_core::check::Unit {
                hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
                path: p.display().to_string(),
                src,
            }
        })
        .collect();
    let build = pw_core::build::build(&units).expect("the fixture builds");
    assert!(build.refusals().is_empty(), "{:?}", build.refusals());
    let out = dir.path().join("build");
    build.write(&out).expect("the build is written");
    Served {
        server: Server::from_build(out.clone(), out).expect("served"),
        _dir: dir,
    }
}

const RATE: &str = "demo.invariants.rate";
const RATE_ALL: &str = "demo.invariants.rate_all";
const MARK: &str = "demo.invariants.mark";
const ANNOTATE: &str = "demo.invariants.annotate";

const STARS: &str = "`demo.invariants.Stars` holds `value >= 1 & value <= 5`";
const NOTE: &str = "`demo.invariants.Note` holds `String.length(value) <= 20`";
const CODE: &str =
    "`demo.invariants.Code` holds `String.length(value) >= 2 & String.length(value) <= 8`";

/// `command` pressed with `args` through [`handle`], as a page sends it:
/// the response's status line and its body.
fn pressed(
    s: &Server,
    command: &str,
    interaction: &str,
    args: serde_json::Value,
) -> (String, String) {
    let answer = posted(
        s,
        &format!("/command/{command}"),
        "reader",
        interaction,
        &args.to_string(),
    );
    let status = answer.lines().next().unwrap_or_default().to_string();
    let body = answer
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body)
}

/// Pressed, and refused before it ran: 400, nothing committed, and why.
fn refused(s: &Server, command: &str, interaction: &str, args: serde_json::Value) -> String {
    let (status, body) = pressed(s, command, interaction, args.clone());
    assert!(
        status.starts_with("HTTP/1.1 400"),
        "{args}: {status} {body}"
    );
    let body: serde_json::Value = serde_json::from_str(&body).expect("a JSON answer");
    assert_eq!(body["committed"], false, "{body}");
    assert!(
        body.get("result").is_none(),
        "the command did not run: {body}"
    );
    body["error"].as_str().expect("why").to_string()
}

/// Pressed, run, and committed: `Ok`, as a handler is answered (ADR-0157).
fn committed(s: &Server, command: &str, interaction: &str, args: serde_json::Value) {
    let (status, body) = pressed(s, command, interaction, args.clone());
    assert!(
        status.starts_with("HTTP/1.1 202"),
        "{args}: {status} {body}"
    );
    let body: serde_json::Value = serde_json::from_str(&body).expect("a JSON answer");
    assert_eq!(body["committed"], true, "{args}: {body}");
    assert_eq!(
        body["result"],
        serde_json::json!({ "$case": "ok" }),
        "{body}"
    );
}

/// **Each kind of invariant is held at both its ends**: an `Int` from 1 to
/// 5, a `String` of 2 to 8 code points and one of at most 20. Each value just
/// outside is refused by name before the command runs, and each just inside
/// is taken. A length is code points, whatever their bytes: 20 emoji are a
/// note, 21 are not.
#[test]
fn a_commands_arguments_are_held_at_both_ends_of_each_invariant() {
    let s = served();
    let rate = |code: &str, stars: i64, note: &str| serde_json::json!([code, stars, note]);
    for (args, why) in [
        (
            rate("ab", 0, ""),
            format!("argument refused: value 2 is 0, and {STARS}"),
        ),
        (
            rate("ab", 6, ""),
            format!("argument refused: value 2 is 6, and {STARS}"),
        ),
        (
            rate("ab", i64::MIN, ""),
            format!("argument refused: value 2 is {}, and {STARS}", i64::MIN),
        ),
        (
            rate("a", 3, ""),
            format!("argument refused: value 1 is 1 code points long, and {CODE}"),
        ),
        (
            rate("abcdefghi", 3, ""),
            format!("argument refused: value 1 is 9 code points long, and {CODE}"),
        ),
        (
            rate("ab", 3, &"x".repeat(21)),
            format!("argument refused: value 3 is 21 code points long, and {NOTE}"),
        ),
        (
            rate("ab", 3, &"\u{1F600}".repeat(21)),
            format!("argument refused: value 3 is 21 code points long, and {NOTE}"),
        ),
    ] {
        assert_eq!(refused(&s, RATE, "forged", args), why);
    }
    // Refused before anything was kept: the interaction a forged request
    // named is free, and a well-formed press with it commits (ADR-0121).
    committed(&s, RATE, "forged", rate("ab", 1, ""));
    for (i, args) in [
        rate("ab", 5, ""),
        rate("abcdefgh", 3, ""),
        rate("ab", 3, &"x".repeat(20)),
        rate("ab", 3, &"\u{1F600}".repeat(20)),
    ]
    .into_iter()
    .enumerate()
    {
        committed(&s, RATE, &format!("held-{i}"), args);
    }
}

/// **A value inside an argument is held where it is**: a list's item, a
/// record's field, an `Option`'s inside and a case's payload, each named by
/// its place. A list is refused for one item, whichever, and nothing it
/// holds that breaks no invariant is refused.
#[test]
fn a_value_inside_an_argument_is_held_where_it_is() {
    let s = served();
    let some = |note: &str| serde_json::json!({ "$case": "some", "value": note });
    let none = serde_json::json!({ "$case": "none" });
    let rating =
        |stars: i64, note: serde_json::Value| serde_json::json!({ "stars": stars, "note": note });
    for (command, args, why) in [
        (
            RATE_ALL,
            serde_json::json!([[rating(3, none.clone()), rating(9, none.clone())]]),
            format!("argument refused: value 1's `[1].stars` is 9, and {STARS}"),
        ),
        (
            RATE_ALL,
            serde_json::json!([[rating(3, some(&"y".repeat(21)))]]),
            format!(
                "argument refused: value 1's `[0].note.some` is 21 code points long, and {NOTE}"
            ),
        ),
        (
            MARK,
            serde_json::json!([{ "$case": "starred", "value": 0 }]),
            format!("argument refused: value 1's `starred` is 0, and {STARS}"),
        ),
        (
            MARK,
            serde_json::json!([{ "$case": "noted", "value": "z".repeat(21) }]),
            format!("argument refused: value 1's `noted` is 21 code points long, and {NOTE}"),
        ),
        (
            ANNOTATE,
            serde_json::json!([some(&"w".repeat(21))]),
            format!("argument refused: value 1's `some` is 21 code points long, and {NOTE}"),
        ),
    ] {
        assert_eq!(refused(&s, command, "forged", args), why, "{command}");
    }
    for (i, (command, args)) in [
        (
            RATE_ALL,
            serde_json::json!([[rating(1, some(&"y".repeat(20))), rating(5, none.clone())]]),
        ),
        (RATE_ALL, serde_json::json!([[]])),
        (
            MARK,
            serde_json::json!([{ "$case": "starred", "value": 5 }]),
        ),
        (
            MARK,
            serde_json::json!([{ "$case": "noted", "value": "z".repeat(20) }]),
        ),
        (ANNOTATE, serde_json::json!([some(&"w".repeat(20))])),
        (ANNOTATE, serde_json::json!([none.clone()])),
    ]
    .into_iter()
    .enumerate()
    {
        committed(&s, command, &format!("held-{i}"), args);
    }
}

/// **A value the host built itself is held as one a browser sent**: the
/// server's entry for values already decoded, which a stored run or a test
/// calls, refuses `Stars` 6 and a 21-character note as the browser's path
/// does, before the command runs. Track `reviews` met the command running
/// with them through this entry (its finding of 2026-10-14).
#[test]
fn a_value_the_host_built_is_held_as_one_a_browser_sent() {
    let s = served();
    let call = |stars: i64, note: &str| {
        s.command_answered(
            RATE,
            "reader",
            &[
                Val::String("ab".into()),
                Val::S64(stars),
                Val::String(note.into()),
            ],
            None,
        )
    };
    for (stars, note, why) in [
        (
            6,
            "",
            format!("argument refused: value 2 is 6, and {STARS}"),
        ),
        (
            3,
            &*"x".repeat(21),
            format!("argument refused: value 3 is 21 code points long, and {NOTE}"),
        ),
    ] {
        let refusal = call(stars, note).expect_err("refused before it runs");
        assert!(refusal.contains(&why), "{refusal}");
    }
    let answered = call(5, "").expect("runs");
    assert!(answered.committed, "{answered:?}");
}

/// **A page's parameters are held where its address gives them**: the
/// code its path names and the note its query may give, at both ends, each
/// refused 400 and why before the page is rendered, as a query's bytes that
/// are no UTF-8 are (ADR-0309); and the same page asked for by name.
#[test]
fn a_pages_parameters_are_held_where_its_address_gives_them() {
    let s = served();
    let asked = |path: &str| {
        let response: String = fetched_as(&s, path, Some("reader"))
            .into_iter()
            .map(|(_, c)| c)
            .collect();
        let status = response.lines().next().unwrap_or_default().to_string();
        let body = response
            .split_once("\r\n\r\n")
            .map(|(_, b)| b.to_string())
            .unwrap_or_default();
        (status, body)
    };
    let twenty_emoji = "%F0%9F%98%80".repeat(20);
    for path in [
        "/demo/invariants/ab".to_string(),
        "/demo/invariants/abcdefgh".to_string(),
        format!("/demo/invariants/ab?note={}", "n".repeat(20)),
        format!("/demo/invariants/ab?note={twenty_emoji}"),
        "/page/demo.invariants.InvariantsPage?code=ab&note=x".to_string(),
    ] {
        let (status, body) = asked(&path);
        assert!(
            status.starts_with("HTTP/1.1 200"),
            "{path}: {status} {body}"
        );
        assert!(body.contains("Rate a code"), "{path}: {body}");
    }
    for (path, why) in [
        (
            "/demo/invariants/a".to_string(),
            format!("parameter refused: value 1 is 1 code points long, and {CODE}"),
        ),
        (
            "/demo/invariants/abcdefghi".to_string(),
            format!("parameter refused: value 1 is 9 code points long, and {CODE}"),
        ),
        (
            format!("/demo/invariants/ab?note={}", "n".repeat(21)),
            format!("parameter refused: value 2's `some` is 21 code points long, and {NOTE}"),
        ),
        (
            format!("/demo/invariants/ab?note={twenty_emoji}%F0%9F%98%80"),
            format!("parameter refused: value 2's `some` is 21 code points long, and {NOTE}"),
        ),
        (
            "/page/demo.invariants.InvariantsPage?code=a&note=x".to_string(),
            format!("parameter refused: value 1 is 1 code points long, and {CODE}"),
        ),
    ] {
        let (status, body) = asked(&path);
        assert!(
            status.starts_with("HTTP/1.1 400"),
            "{path}: {status} {body}"
        );
        assert_eq!(body, why, "{path}");
    }
}
