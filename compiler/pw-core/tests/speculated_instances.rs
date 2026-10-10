//! **A view's instance given a speculated value is rendered again with it**
//! (ADR-0234).
//!
//! A view that contains itself is an instance made at run time (ADR-0203).
//! Given a value a page speculates on, its instance was no region of the
//! page's speculation module: a press changed what the page computes from
//! the value, a count, and the instance kept what the server rendered, the
//! thread without the reply. It is a region now, rendered again as a block a
//! speculated value decides is, by the view's template, as the browser
//! renders an instance a signal gives. One the module would not reach is
//! refused by name.

use pw_core::backend::wasm::Encoding;
use pw_core::check::Unit;
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

/// A thread a session replies to, shown through a view that contains
/// itself, `markup` in the page.
fn program(markup: &str) -> String {
    format!(
        r#"module m

import List
import context.{{ current_session }}
import capability.{{ Session, SessionId }}

opaque type InteractionId = String

type Comment = Comment {{ id: Int, text: String, replies: List<Comment> }}

event Replied(id: Int)

fn thread_of(s: Session<SessionId>) -> Comment !{{ database.read<Comment> }}
    host "m:data/comments#thread"

fn flag_of(s: Session<SessionId>) -> Bool !{{ database.read<Comment> }}
    host "m:data/comments#flag"

fn add(s: Session<SessionId>, text: String) -> Comment !{{ database.write<Comment> }}
    host "m:data/comments#add"

session query Thread(session: Session<SessionId>) -> Comment
    freshness      0.seconds
    consistency    read_your_writes
    cache          private
    key            session
    invalidates_on Replied(_)
    concurrency    one_per_key
    on_key_change  cancel
    timeout        2.seconds
{{
    thread_of(session)
}}

session query Pinned(session: Session<SessionId>) -> Comment
    freshness      0.seconds
    consistency    read_your_writes
    cache          private
    key            session
    invalidates_on Replied(_)
    concurrency    one_per_key
    on_key_change  cancel
    timeout        2.seconds
{{
    thread_of(session)
}}

session query Flag(session: Session<SessionId>) -> Bool
    freshness      0.seconds
    consistency    read_your_writes
    cache          private
    key            session
    invalidates_on Replied(_)
    concurrency    one_per_key
    on_key_change  cancel
    timeout        2.seconds
{{
    flag_of(session)
}}

command reply(text: String) -> Comment
    requires      SignedIn
    idempotent_by InteractionId
    emits         Replied(1)
    optimistic    Thread(current_session()) as t => replied(t, text)
{{
    add(current_session(), text)
}}

fn replied(t: Comment, text: String) -> Comment !{{}} {{
    Comment {{ id: t.id, text: t.text, replies: List.concat(t.replies, [Comment {{ id: 0, text: text, replies: [] }}]) }}
}}

view Replies(c: Comment, marked: Bool) {{
    <article>
        <p>{{c.text}}</p>
        {{#if marked}}<p>marked</p>{{/if}}
        <ul>
            {{#each c.replies as r (r.id)}}
                <li><Replies c={{r}} marked={{marked}} /></li>
            {{/each}}
        </ul>
    </article>
}}

session page P() {{
    route     "/"
    placement origin
    cache     private

    signal open: Bool = true
    let thread = query Thread(current_session())
    let flag = query Flag(current_session())
    let pinned = query Pinned(current_session())

    view {{
        <title>P</title>
        <main>
            {markup}
            <p id="count">{{List.length(thread.replies)}}</p>
            <button type="button" on:press|refusable={{() => reply("new")}}>Reply</button>
            <button type="button" on:press|refusable={{() => open = !open}}>Toggle</button>
        </main>
    }}
}}
"#
    )
}

fn units(program: &str) -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .expect(dir)
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        files.extend(paths);
    }
    let mut out: Vec<Unit> = files
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            Unit {
                path: p.display().to_string(),
                hir: lower_file(&src, &parse_tree(&src).green),
                src,
            }
        })
        .collect();
    out.push(Unit {
        path: "m.pw".into(),
        hir: lower_file(program, &parse_tree(program).green),
        src: program.to_string(),
    });
    out
}

/// The page's speculation, or why it was refused.
fn speculation(program: &str) -> Result<pw_core::backend::speculation::Speculation, String> {
    pw_core::backend::speculation::compile(&units(program))
        .expect("the program checks")
        .into_iter()
        .find(|c| c.page == "m.P")
        .map(|c| match c.module {
            Encoding::Encoded(m) => Ok(m),
            other => Err(format!("{other:?}")),
        })
        .expect("the page's speculation")
}

#[test]
fn an_instance_given_a_speculated_value_is_a_region() {
    let s = speculation(&program("<Replies c={thread} marked={open} />"))
        .unwrap_or_else(|e| panic!("{e}"));
    // Part 0, the instance: a region of `thread`, rendered as a block is.
    assert!(s.regions.contains(&0), "{:?}", s.regions);
    assert!(
        s.source
            .contains("\"thread\": [{ kind: \"block\", part: 0 }]"),
        "{}",
        s.source
    );
    // Control: a view given a value the page does not speculate on is no
    // region of the speculation.
    let quiet = speculation(&program("<Replies c={pinned} marked={open} />"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(!quiet.regions.contains(&0), "{:?}", quiet.regions);
}

#[test]
fn an_instance_the_module_would_not_reach_is_refused_by_name() {
    for (markup, why) in [
        // Inside a block a query's value decides, which the module does not
        // render again.
        (
            "{#if flag}<Replies c={thread} marked={open} />{/if}",
            "is a view given `thread` inside a block, which a speculation would not reach",
        ),
        // Given beside the speculated value one the browser does not hold.
        (
            "<Replies c={thread} marked={flag} />",
            "is a view given `flag` beside `thread`, which the browser renders again with a \
             speculation of it",
        ),
    ] {
        match speculation(&program(markup)) {
            Ok(_) => panic!("compiled: {markup}"),
            Err(e) => assert!(e.contains(why), "{markup}: {e}"),
        }
    }
}
