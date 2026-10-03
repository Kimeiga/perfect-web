//! **A stream region shows its query's state** (ADR-0148).
//!
//! `<stream query={Q(..)}>` holds a `<placeholder>` while a query declared
//! `delivery streamed` is pending, a `<ready as={v}>` for what it answered,
//! and a `<failed as={why}>` for why it did not. Until 2026-10-03 nothing
//! checked what a stream holds, or what reads a streamed query:
//! - a page could read a streamed query with `let`, and wait for it;
//! - a stream could leave out the arm for a state its query reaches;
//! - a streamed query could declare no bound on how long its region waits;
//! - `<failed as={e}>` was typed as the declared error, which a host's
//!   failure, a spent budget or a trap, has none of.

use pw_core::check::check_sources;

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out
}

fn reported(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// The query's policies, the page's other bindings, and its markup.
fn page(policies: &str, lets: &str, markup: &str) -> String {
    format!(
        "module t\n\n\
         type Item = Item {{ id: String, name: String }}\n\n\
         type Oops =\n    | Down\n    | Busy\n\n\
         public query Recs(id: String) -> Result<List<Item>, Oops>\n    \
         freshness     0.seconds\n    consistency   eventual\n    cache         shared\n    \
         concurrency   one_per_key\n    on_key_change cancel\n{policies}\n{{\n    todo\n}}\n\n\
         public query Count(id: String) -> Int\n    \
         freshness     0.seconds\n    consistency   eventual\n    cache         shared\n    \
         concurrency   one_per_key\n    on_key_change cancel\n    timeout       2.seconds\n{{\n    todo\n}}\n\n\
         page P(id: String) {{\n    cache private\n{lets}\n\n    view {{\n        <main>\n            \
         {markup}\n        </main>\n    }}\n}}\n"
    )
}

const STREAMED: &str = "    delivery      streamed\n    timeout       3.seconds";
const AT_ONCE: &str = "    timeout       3.seconds";

const PLACEHOLDER: &str = "<placeholder><p>Finding</p></placeholder>";
const READY: &str =
    "<ready as={items}><ul>{#each items as item (item.id)}<li>{item.name}</li>{/each}</ul></ready>";
const FAILED: &str = "<failed as={why}><p>None right now</p></failed>";

fn stream(parts: &[&str]) -> String {
    format!("<stream query={{Recs(id)}}>{}</stream>", parts.concat())
}

#[test]
fn a_stream_that_shows_every_state_checks() {
    let all = stream(&[PLACEHOLDER, READY, FAILED]);
    assert_eq!(reported(&page(STREAMED, "", &all)), Vec::<String>::new());
    // One the page waits for shows no placeholder, since none is shown.
    let settled = stream(&[READY, FAILED]);
    assert_eq!(reported(&page(AT_ONCE, "", &settled)), Vec::<String>::new());
}

#[test]
fn a_page_does_not_wait_for_a_streamed_query() {
    let waits = "    let recs = query Recs(id)";
    let list = "<ul>{#each recs as item (item.id)}<li>{item.name}</li>{/each}</ul>";
    assert_eq!(
        reported(&page(STREAMED, waits, list)),
        [
            "PW5400 `Recs` is declared `delivery streamed`, so a page does not wait for it; read \
          it in a `<stream query={Recs(..)}>`"
        ]
    );
    // The control: the same page reading a query it may wait for.
    assert_eq!(reported(&page(AT_ONCE, waits, list)), Vec::<String>::new());
}

#[test]
fn a_streamed_stream_shows_a_placeholder() {
    let found = reported(&page(STREAMED, "", &stream(&[READY, FAILED])));
    assert_eq!(
        found,
        [
            "PW5401 `Recs` is declared `delivery streamed`, so the page is sent before it answers: \
          the `<stream>` needs a `<placeholder>` to show until then"
        ]
    );
}

#[test]
fn a_placeholder_the_page_waits_past_is_refused() {
    let found = reported(&page(AT_ONCE, "", &stream(&[PLACEHOLDER, READY, FAILED])));
    assert_eq!(
        found,
        [
            "PW5401 `Recs` is not declared `delivery streamed`, so the page waits for it and this \
          `<placeholder>` is never shown; declare `delivery streamed` to send the page first"
        ]
    );
}

#[test]
fn a_stream_shows_its_failure() {
    let found = reported(&page(STREAMED, "", &stream(&[PLACEHOLDER, READY])));
    assert_eq!(
        found,
        [
            "PW5401 a `<stream>` shows that its query failed, as every query can, its budget spent \
          or its source down: it needs a `<failed>`"
        ]
    );
}

#[test]
fn a_stream_shows_what_its_query_answered() {
    let found = reported(&page(STREAMED, "", &stream(&[PLACEHOLDER, FAILED])));
    assert_eq!(
        found,
        ["PW5401 a `<stream>` shows what its query answered: it needs a `<ready as={..}>`"]
    );
}

#[test]
fn a_stream_holds_its_parts_once_and_nothing_else() {
    let twice = stream(&[PLACEHOLDER, READY, READY, FAILED]);
    assert_eq!(
        reported(&page(STREAMED, "", &twice)),
        ["PW5401 a `<stream>` has one `<ready>`, and this is a second"]
    );
    let stray = stream(&[PLACEHOLDER, "<p>Hello</p>", READY, FAILED]);
    assert_eq!(
        reported(&page(STREAMED, "", &stray)),
        [
            "PW5401 a `<stream>` holds only its `<placeholder>`, `<ready>` and `<failed>`; put \
          this inside one of them"
        ]
    );
    let classed = stream(&[PLACEHOLDER, READY, FAILED]).replace("<stream ", "<stream class=\"x\" ");
    assert_eq!(
        reported(&page(STREAMED, "", &classed)),
        [
            "PW5401 a `<stream>` writes no markup of its own, so `class` would be dropped; put it \
          on an element inside its parts"
        ]
    );
    let outside = format!(
        "{}<ready><p>x</p></ready>",
        stream(&[PLACEHOLDER, READY, FAILED])
    );
    assert_eq!(
        reported(&page(STREAMED, "", &outside)),
        [
            "PW5401 `<ready>` is a `<stream>`'s part, and this one is not directly inside a \
          `<stream>`"
        ]
    );
}

#[test]
fn a_stream_shows_a_query() {
    let found = reported(&page(
        STREAMED,
        "",
        &stream(&[PLACEHOLDER, READY, FAILED]).replace("Recs(id)", "missing(id)"),
    ));
    assert!(
        found.iter().any(
            |d| d == "PW5401 a `<stream>`'s `query` is a call of a query, whose state it shows"
        ),
        "{found:?}"
    );
}

#[test]
fn a_streamed_query_declares_how_long_it_may_take() {
    let unbounded = "    delivery      streamed";
    let found = reported(&page(unbounded, "", &stream(&[PLACEHOLDER, READY, FAILED])));
    assert_eq!(
        found,
        [
            "PW5402 `Recs` is declared `delivery streamed` and no `timeout`: a region waiting on \
          it would wait as long as its source does"
        ]
    );
}

#[test]
fn the_failed_arm_is_given_the_declared_error_or_nothing() {
    // `Some(e)` is the query's declared error, `None` the host's failure, and
    // a match over it covers both.
    let both = "<failed as={why}>{#match why}{:Some(e)}{#match e}{:Down}<p>down</p>\
                {:Busy}<p>busy</p>{/match}{:None}<p>no answer</p>{/match}</failed>";
    let all = stream(&[PLACEHOLDER, READY, both]);
    assert_eq!(reported(&page(STREAMED, "", &all)), Vec::<String>::new());
    let host_unshown = both.replace("{:None}<p>no answer</p>", "");
    let found = reported(&page(
        STREAMED,
        "",
        &stream(&[PLACEHOLDER, READY, &host_unshown]),
    ));
    assert_eq!(found, ["PW0305 `{#match why}` does not cover `None`"]);
}

#[test]
fn the_failed_arm_s_value_is_an_option_to_every_reader() {
    // The value relations agree with the typer: a view that takes an
    // `Option<Oops>` is given it, and one that takes the declared error bare
    // is not. Until 2026-10-03 the two passes would each have to be told.
    let given = |param: &str| {
        let mut src = page(
            STREAMED,
            "",
            &stream(&[
                PLACEHOLDER,
                READY,
                "<failed as={why}><Why reason={why} /></failed>",
            ]),
        );
        src.push_str(&format!(
            "\nview Why(reason: {param}) !{{}} {{\n    <p>No recommendations right now</p>\n}}\n"
        ));
        reported(&src)
    };
    assert_eq!(given("Option<Oops>"), Vec::<String>::new());
    assert_eq!(
        given("Oops"),
        [
            "PW0605 argument 1 of `Why's prop `reason`` is declared `t.Oops` and this is \
          `Option<t.Oops>`"
        ]
    );
}

#[test]
fn a_query_that_declares_no_error_gives_its_failed_arm_nothing() {
    let count = |failed: &str| {
        format!("<stream query={{Count(id)}}><ready as={{n}}><p>{{n}}</p></ready>{failed}</stream>")
    };
    let found = reported(&page(STREAMED, "", &count(FAILED)));
    assert_eq!(
        found,
        [
            "PW5401 `Count` declares no error, so its failure is the host's and has no value to \
          bind; write `<failed>`"
        ]
    );
    let bare = count("<failed><p>None right now</p></failed>");
    assert_eq!(reported(&page(STREAMED, "", &bare)), Vec::<String>::new());
}
