//! **A page binds a signal to its address's fragment** (track `todomvc`,
//! ADR-0367; the integrator's ruling of 2026-10-14).
//!
//! `fragment filter "/" => "all", "/active" => "active"` names a signal of
//! the page and each form the fragment takes with the value the signal takes
//! then. The checker holds the clause (PW6101 to PW6104), the signal's
//! writers (PW6105) and the page's links to its fragment (PW6106); the plan
//! carries the binding to the browser. Each test states one rule, with its
//! control: the same page written well, which checks clean.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn packages() -> Vec<(String, String)> {
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

/// What checking `src` says of it, each as `code message | help: repair`.
fn found(src: &str) -> Vec<String> {
    let mut sources = packages();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| {
            ds.into_iter().map(|d| {
                let helps: Vec<&str> = d.repairs.iter().map(|r| r.description.as_str()).collect();
                format!("{} {} | help: {}", d.code, d.message, helps.join("; "))
            })
        })
        .collect()
}

/// A page `P` stating `clause` (a `fragment` line, or none) with `signals`
/// declared and `markup` in its view.
fn page(clause: &str, signals: &str, markup: &str) -> String {
    format!(
        "module t\n\nimport String\n\npage P() {{\n    route \"/\"\n    cache private\n    {clause}\n\n    \
         {signals}\n\n    view {{\n        <title>t</title>\n        <main>\n            \
         {markup}\n        </main>\n    }}\n}}\n"
    )
}

const FILTER: &str = "signal filter: String = \"all\"";
const FORMS: &str = "fragment filter \"/\" => \"all\", \"/active\" => \"active\"";

fn clean(src: &str) {
    assert_eq!(found(src), Vec::<String>::new(), "{src}");
}

fn refused(src: &str, code: &str, why: &str) {
    let found = found(src);
    assert!(
        found.iter().any(|f| f.starts_with(code) && f.contains(why)),
        "{code} `{why}` expected of\n{src}\ngot {found:#?}"
    );
    // And nothing else of the others: a rule's mutant must not be hidden by
    // a neighbour's refusal.
    assert!(
        found.iter().all(|f| f.starts_with(code)),
        "only {code} expected of\n{src}\ngot {found:#?}"
    );
}

/// The plan `pw build` writes for `P` in `src`.
fn plan(src: &str) -> pw_core::page_values::PageValues {
    let mut sources = packages();
    sources.push(("t.pw".to_string(), src.to_string()));
    let units: Vec<Unit> = sources
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    let build = pw_core::build::build(&units).expect("builds");
    build
        .pages
        .iter()
        .find(|p| p.page == "t.P")
        .expect("the page is planned")
        .plan
        .clone()
        .expect("the page has a plan")
}

#[test]
fn a_clause_binds_a_signal_to_the_fragment_and_the_plan_carries_it() {
    let src = page(FORMS, FILTER, "<p>{filter}</p>");
    clean(&src);
    let bound = plan(&src).fragment.expect("the plan names the binding");
    assert_eq!(bound.signal, "filter");
    let forms: Vec<(&str, &str)> = bound
        .forms
        .iter()
        .map(|f| (f.form.as_str(), f.value.as_str()))
        .collect();
    // Each form with the value it gives, in the order written.
    assert_eq!(forms, [("/", "all"), ("/active", "active")]);
    // The control: a page that states none binds none.
    let none = page("", FILTER, "<p>{filter}</p>");
    clean(&none);
    assert!(plan(&none).fragment.is_none());
}

#[test]
fn the_clause_names_a_signal_the_page_declares() {
    refused(
        &page("fragment filtr \"/\" => \"all\"", FILTER, "<p>{filter}</p>"),
        "PW6101",
        "`filtr`",
    );
    // It says which the page has.
    refused(
        &page("fragment filtr \"/\" => \"all\"", FILTER, "<p>{filter}</p>"),
        "PW6101",
        "its signals are `filter`",
    );
    // A page with none.
    refused(
        &page("fragment filter \"/\" => \"all\"", "", "<p>x</p>"),
        "PW6101",
        "declares none",
    );
    // A clause with no name at all.
    refused(
        &page("fragment \"/\" => \"all\"", FILTER, "<p>{filter}</p>"),
        "PW6101",
        "names no signal",
    );
    // The control: the signal named by its name.
    clean(&page(FORMS, FILTER, "<p>{filter}</p>"));
}

#[test]
fn the_signal_is_a_string_since_a_fragment_is_text() {
    for ty in ["Int", "Bool", "List<String>"] {
        let value = match ty {
            "Int" => "0",
            "Bool" => "false",
            _ => "[]",
        };
        refused(
            &page(FORMS, &format!("signal filter: {ty} = {value}"), "<p>x</p>"),
            "PW6102",
            &format!(
                "a signal of `{}`",
                if ty == "List<String>" { "List<..>" } else { ty }
            ),
        );
    }
    // The control: a String.
    clean(&page(FORMS, FILTER, "<p>{filter}</p>"));
}

#[test]
fn the_forms_are_pairs_of_the_characters_a_fragment_holds() {
    let bad = |clause: &str, why: &str| {
        refused(&page(clause, FILTER, "<p>{filter}</p>"), "PW6103", why);
    };
    // A form with no value.
    bad("fragment filter \"/\"", "gives no value");
    // A form and a value with no arrow between.
    bad("fragment filter \"/\" \"all\"", "gives no value");
    // A value that is no text.
    bad("fragment filter \"/\" => all", "expected a quoted text");
    // No form at all.
    bad("fragment filter", "expected a quoted text");
    // A form written twice.
    bad(
        "fragment filter \"/\" => \"all\", \"/\" => \"active\"",
        "written twice",
    );
    // Characters a fragment does not hold as they are: a space, a quote, a
    // `#`, a `%`, a `<`, and a non-ASCII letter.
    for form in ["/a b", "/a#b", "/a%20b", "/a<b", "/é"] {
        bad(
            &format!("fragment filter \"/\" => \"all\", \"{form}\" => \"x\""),
            "which a fragment does not hold as it is",
        );
    }
    // An escape: a form is written as it is.
    bad(
        "fragment filter \"/\" => \"all\", \"/a\\\\b\" => \"x\"",
        "holds an escape",
    );
    // A comma that ends the forms.
    bad(
        "fragment filter \"/\" => \"all\", ",
        "ends the forms with none after it",
    );
    // Something between the forms that is no comma.
    bad(
        "fragment filter \"/\" => \"all\" \"/x\" => \"x\"",
        "expected `,` or the end of the clause",
    );
    // The controls: the empty form (an address with no fragment), several
    // forms for one value (as `#!/` is TodoMVC's second all), and every
    // character a fragment holds.
    clean(&page(
        "fragment filter \"\" => \"all\", \"/\" => \"all\", \"!/\" => \"all\", \
         \"/a-b_c.d~e:f@g/h?i=j&k+l,m;n!o$p'q(r)s*t\" => \"x\"",
        FILTER,
        "<p>{filter}</p>",
    ));
}

#[test]
fn the_signal_starts_at_a_value_a_form_gives() {
    refused(
        &page(FORMS, "signal filter: String = \"zzz\"", "<p>{filter}</p>"),
        "PW6104",
        "starts at `zzz`, which no form of the fragment gives",
    );
    // It says which values there are.
    refused(
        &page(FORMS, "signal filter: String = \"zzz\"", "<p>{filter}</p>"),
        "PW6104",
        "start `filter` at one of \"active\", \"all\"",
    );
    // A first value that is no literal text.
    refused(
        &page(
            FORMS,
            "signal filter: String = String.trim(\"all\")",
            "<p>{filter}</p>",
        ),
        "PW6104",
        "not a text written there",
    );
    // The control: a value the second form gives is as good as the first's.
    clean(&page(
        FORMS,
        "signal filter: String = \"active\"",
        "<p>{filter}</p>",
    ));
}

#[test]
fn the_address_is_the_signals_only_writer() {
    refused(
        &page(
            FORMS,
            FILTER,
            "<button type=\"button\" on:press={() => filter = \"active\"}>x</button>",
        ),
        "PW6105",
        "a handler assigns it",
    );
    // A block handler, assigning among others.
    refused(
        &page(
            FORMS,
            &format!("{FILTER}\n    signal n: Int = 0"),
            "<button type=\"button\" on:press={() => {\n n = n + 1\n filter = \"all\"\n }}>x</button>",
        ),
        "PW6105",
        "a handler assigns it",
    );
    // The controls: another signal's assignment, and reading it in a
    // handler, are a handler's to do.
    clean(&page(
        FORMS,
        &format!("{FILTER}\n    signal n: Int = 0"),
        "<button type=\"button\" on:press={() => n = n + 1}>x</button>\n            \
         <button type=\"button\" on:press={() => n = if filter == \"all\" { 1 } else { 2 }}>y</button>",
    ));
    // And a page that binds nothing assigns what it likes.
    clean(&page(
        "",
        FILTER,
        "<button type=\"button\" on:press={() => filter = \"active\"}>x</button>",
    ));
}

#[test]
fn a_link_to_a_fragment_names_a_form_the_page_takes() {
    refused(
        &page(FORMS, FILTER, "<a href=\"#/nope\">x</a>"),
        "PW6106",
        "`href=\"#/nope\"` is a fragment the page does not take: it takes `#/`, `#/active`",
    );
    // A bare `#` is the empty fragment, which these forms do not take.
    refused(
        &page(FORMS, FILTER, "<a href=\"#\">top</a>"),
        "PW6106",
        "`href=\"#\"`",
    );
    // The controls: each form; and a link that is no fragment, which the
    // route table holds.
    clean(&page(
        FORMS,
        FILTER,
        "<a href=\"#/\">a</a><a href=\"#/active\">b</a><a href=\"https://todomvc.com\">c</a>",
    ));
    // And the empty fragment, where a form is written for it.
    clean(&page(
        "fragment filter \"\" => \"all\", \"/active\" => \"active\"",
        FILTER,
        "<a href=\"#\">top</a>",
    ));
    // A page that binds nothing owns no fragment: its links to any are its.
    clean(&page("", FILTER, "<a href=\"#anything\">x</a>"));
}

#[test]
fn a_page_states_its_fragment_once_and_a_page_alone_does() {
    // Written twice: a policy is stated once (the generic rule).
    let twice = page(
        &format!("{FORMS}\n    fragment filter \"/x\" => \"x\""),
        FILTER,
        "<p>{filter}</p>",
    );
    assert!(
        found(&twice).iter().any(|f| f.contains("fragment")),
        "{:#?}",
        found(&twice)
    );
    // A clause on a view or a query is no page's.
    let on_a_query = "module t\n\nimport capability.{ Session, SessionId }\n\n\
         session query Q(s: Session<SessionId>) -> Int\n    freshness 0.seconds\n    \
         consistency read_your_writes\n    cache private\n    key s\n    \
         concurrency one_per_key\n    timeout 2.seconds\n    fragment x \"/\" => \"y\"\n{\n    1\n}\n";
    assert!(
        found(on_a_query).iter().any(|f| f.contains("fragment")),
        "{:#?}",
        found(on_a_query)
    );
}
