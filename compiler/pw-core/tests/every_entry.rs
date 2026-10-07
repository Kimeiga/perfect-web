//! **An entry written `_` is every entry at the rest** (ADR-0256, the first
//! half of ADR-0195's ruling 10).
//!
//! "A bare `invalidates Cart` stays refused. Every entry is written out as
//! `Cart(_)`, as `InventoryChanged(id, _)` is." Until ADR-0256 an
//! `invalidates` key's `_` was a name, and resolved to nothing (PW0021), so
//! a command could drop one entry of a query or the entry its key computed,
//! and no more: a session's timeline at every length it was read was out of
//! reach. Each test states one case, with its control.

use pw_core::check::check_sources;

fn files(dir: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pw"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let src = std::fs::read_to_string(&p).expect("read");
            (p.display().to_string(), src)
        })
        .collect()
}

/// What `app.pw` reports, with the platform: each code and message.
fn reported(src: &str) -> Vec<String> {
    let mut program = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        program.extend(files(d));
    }
    program.push(("app.pw".to_string(), src.to_string()));
    check_sources(&program)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

/// A session's count of what it follows, read at a limit, and a command
/// that follows someone with `clauses`.
fn following(clauses: &str) -> String {
    format!(
        "module m\n\nimport capability.{{ Session, SessionId }}\n\
         import context.{{ current_session }}\n\n\
         opaque type UserId = String\n\n\
         type Follow = Follow {{ follower: UserId, followee: UserId }}\n\n\
         type FollowError =\n    | NotFound\n\n\
         event Followed(follower: UserId, followee: UserId)\n\n\
         fn me(s: Session<SessionId>) -> UserId !{{ database.read<Follow> }}\n    \
         host \"m:data/follows#me\"\n\n\
         fn counted(s: Session<SessionId>, limit: Int) -> Int !{{ database.read<Follow> }}\n    \
         host \"m:data/follows#count\"\n\n\
         fn add(s: Session<SessionId>, user: UserId) -> Result<UserId, FollowError> \
         !{{ database.write<Follow> }}\n    host \"m:data/follows#add\"\n\n\
         session query Counted(session: Session<SessionId>, limit: Int) -> Int\n    \
         freshness      0.seconds\n    consistency    read_your_writes\n    \
         cache          private\n    key            session, limit\n    \
         concurrency    one_per_key\n    on_key_change  cancel\n    \
         timeout        2.seconds\n{{\n    counted(session, limit)\n}}\n\n\
         command follow(user: UserId) -> Result<UserId, FollowError>\n    \
         requires      SignedIn\n{clauses}{{\n    add(current_session(), user)\n}}\n"
    )
}

#[test]
fn an_entry_written_with_a_wildcard_is_every_entry_at_the_rest() {
    // The session's count at every limit it was read.
    assert_eq!(
        reported(&following(
            "    invalidates   Counted(current_session(), _)\n"
        )),
        Vec::<String>::new()
    );
    // And by name.
    assert_eq!(
        reported(&following(
            "    invalidates   Counted(session = current_session(), limit = _)\n"
        )),
        Vec::<String>::new()
    );
    // The control: a name in a key resolves as it did.
    assert_eq!(
        reported(&following(
            "    invalidates   Counted(current_session(), nothing)\n"
        )),
        vec!["PW0021 `nothing` does not resolve".to_string()]
    );
}

#[test]
fn a_wildcard_is_a_whole_argument() {
    // `_ + 1` is a value computed from a name, and `_` names nothing.
    assert_eq!(
        reported(&following(
            "    invalidates   Counted(current_session(), _ + 1)\n"
        )),
        vec!["PW0021 `_` does not resolve".to_string()]
    );
}

#[test]
fn an_event_is_given_a_value_at_each_parameter() {
    // A listener matches an event by its values: `_` is none.
    let found = reported(&following(
        "    emits         Followed(_, user)\n    invalidates   Counted(current_session(), _)\n",
    ));
    assert_eq!(
        found,
        vec!["PW0021 `emits Followed` is given no value at `follower`: `_` is none".to_string()]
    );
}

#[test]
fn a_bare_entry_names_none() {
    // Refused as it was, and the repair says how every entry is written.
    let src = following("    invalidates   Counted\n");
    let mut program = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        program.extend(files(d));
    }
    program.push(("app.pw".to_string(), src));
    let found: Vec<pw_core::diagnostics::Diagnostic> = check_sources(&program)
        .into_iter()
        .filter(|(n, _)| n == "app.pw")
        .flat_map(|(_, ds)| ds)
        .collect();
    assert_eq!(
        found.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["PW0604"],
        "{found:#?}"
    );
    assert!(
        found[0]
            .repairs
            .iter()
            .any(|r| r.description.contains("`Counted(_, _)`")),
        "{found:#?}"
    );
}
