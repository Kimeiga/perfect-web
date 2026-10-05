//! **A data source states what it guarantees, and nothing asks it for more**
//! (ADR-0207, ADR-0195's ruling 11).
//!
//! `source Search  holds Listing  transactions none  reads eventual  changes
//! none`: a search index the host writes to and reads from, which commits each
//! write alone and shows a write when it shows it. A query that asks it for a
//! consistent snapshot, a command that asks it for a serializable transaction,
//! that writes it and another database in one command, that emits events of
//! its writes, or that is idempotent by its interaction, is refused. A
//! resource no source holds is the host's own database's, which gives each.

use pw_core::check::check_sources;

fn reported(src: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .expect("dir")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            sources.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    sources.push(("m.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(path, _)| path == "m.pw")
        .flat_map(|(_, ds)| ds)
        .map(|d| format!("{} {}", d.code, d.message))
        .collect()
}

/// Codes reported in the PW0344..PW0349 range, with their messages.
fn sourced(src: &str) -> Vec<String> {
    reported(src)
        .into_iter()
        .filter(|d| {
            ["PW0344", "PW0345", "PW0346", "PW0347", "PW0348", "PW0349"]
                .iter()
                .any(|c| d.starts_with(c))
        })
        .collect()
}

const PROGRAM: &str = r#"module m

opaque type InteractionId = String

type Listing = Listing { id: Int, title: String }

type Cart = Cart { lines: Int }

type ListingEvent = ListingEvent { id: Int }

event Indexed(id: Int)

source Search
    holds        Listing
    transactions none
    reads        eventual
    changes      none

fn find(text: String) -> List<Listing> !{ database.read<Listing> }
    host "m:data/listings#find"

fn index(l: Listing) -> Int !{ database.write<Listing> }
    host "m:data/listings#index"

fn add(n: Int) -> Int !{ database.write<Cart> }
    host "m:data/carts#add"

public query Found(text: String) -> List<Listing>
    freshness     30.seconds
    consistency   eventual
    cache         shared
    concurrency   one_per_key
    timeout       2.seconds
{
    find(text)
}

command list(l: Listing) -> Int
    requires      SignedIn
{
    index(l)
}
"#;

#[test]
fn a_program_that_asks_what_its_sources_give_checks() {
    assert_eq!(sourced(PROGRAM), Vec::<String>::new());
}

#[test]
fn a_query_asks_no_more_consistency_than_its_source_reads() {
    let snapshot = PROGRAM.replace(
        "    consistency   eventual\n",
        "    consistency   snapshot\n",
    );
    assert_eq!(
        sourced(&snapshot),
        ["PW0344 `Found` asks `consistency snapshot` of `Listing`, and `Search` reads `eventual`"]
    );
    // A source that gives it, alone or among others, or `strong`, which
    // gives every promise.
    for reads in ["snapshot", "snapshot, read_your_writes", "strong"] {
        let given = snapshot.replace(
            "    reads        eventual\n",
            &format!("    reads        {reads}\n"),
        );
        assert_eq!(sourced(&given), Vec::<String>::new(), "{reads}");
    }
    // `read_your_writes` is not a snapshot.
    let mine = snapshot.replace(
        "    reads        eventual\n",
        "    reads        read_your_writes\n",
    );
    assert_eq!(sourced(&mine).len(), 1, "{:?}", sourced(&mine));
    // And every source gives `eventual`, whatever else it gives.
    let given = PROGRAM.replace(
        "    reads        eventual\n",
        "    reads        read_your_writes\n",
    );
    assert_eq!(sourced(&given), Vec::<String>::new());
}

#[test]
fn a_commands_writes_are_in_one_source() {
    // The listing and a cart, the host's own database's: two commits.
    let two = PROGRAM.replace("    index(l)\n", "    add(index(l))\n");
    assert_eq!(
        sourced(&two),
        [
            "PW0345 `list` writes `Listing` in `Search` and `Cart` in the host's database, and no one transaction spans two sources"
        ]
    );
    // The cart alone, the host's: one.
    let one = PROGRAM.replace("    index(l)\n", "    add(l.id)\n");
    assert_eq!(sourced(&one), Vec::<String>::new());
}

#[test]
fn a_command_asks_no_more_isolation_than_its_source_gives() {
    let asked = PROGRAM.replace(
        "    requires      SignedIn\n{\n    index(l)",
        "    requires      SignedIn\n    transaction   serializable\n{\n    index(l)",
    );
    assert_eq!(
        sourced(&asked),
        [
            "PW0346 `list` asks `transaction serializable` of `Search`, whose transactions are `none`"
        ]
    );
    // Given read committed, snapshot asks more; given serializable, nothing
    // asks more.
    let committed = asked.replace(
        "    transactions none\n",
        "    transactions read_committed\n",
    );
    assert_eq!(sourced(&committed).len(), 1, "{:?}", sourced(&committed));
    let snapshot = committed.replace(
        "    transaction   serializable\n",
        "    transaction   snapshot\n",
    );
    assert_eq!(sourced(&snapshot).len(), 1, "{:?}", sourced(&snapshot));
    let serializable = asked.replace("    transactions none\n", "    transactions serializable\n");
    assert_eq!(sourced(&serializable), Vec::<String>::new());
    let below = serializable.replace(
        "    transaction   serializable\n",
        "    transaction   read_committed\n",
    );
    assert_eq!(sourced(&below), Vec::<String>::new());
}

#[test]
fn a_commands_events_are_sent_if_and_only_if_its_writes_commit() {
    let emits = PROGRAM.replace(
        "    requires      SignedIn\n{\n    index(l)",
        "    requires      SignedIn\n    emits         Indexed(l.id)\n{\n    index(l)",
    );
    assert_eq!(
        sourced(&emits),
        [
            "PW0347 `list` emits events of its writes to `Search`, which commits them in no transaction and tells no change"
        ]
    );
    // A transaction they commit in, the outbox; or the source's own feed.
    let committed = emits.replace(
        "    transactions none\n",
        "    transactions read_committed\n",
    );
    assert_eq!(sourced(&committed), Vec::<String>::new());
    let fed = emits.replace("    changes      none\n", "    changes      feed\n");
    assert_eq!(sourced(&fed), Vec::<String>::new());
}

#[test]
fn a_commands_record_of_an_interaction_commits_with_its_writes() {
    let idempotent = PROGRAM.replace(
        "    requires      SignedIn\n{\n    index(l)",
        "    requires      SignedIn\n    idempotent_by InteractionId\n{\n    index(l)",
    );
    assert_eq!(
        sourced(&idempotent),
        [
            "PW0348 `list` is idempotent by its interaction, and `Search` commits its writes in no transaction"
        ]
    );
    // A feed tells what changed after it committed, and is no transaction.
    let fed = idempotent.replace("    changes      none\n", "    changes      feed\n");
    assert_eq!(sourced(&fed).len(), 1, "{:?}", sourced(&fed));
    let committed = idempotent.replace("    transactions none\n", "    transactions snapshot\n");
    assert_eq!(sourced(&committed), Vec::<String>::new());
}

#[test]
fn a_source_holds_what_the_programs_effects_name_once() {
    // Nothing held.
    let nothing = PROGRAM.replace("    holds        Listing\n", "");
    assert!(
        sourced(&nothing).contains(&"PW0349 `Search` holds nothing".to_string()),
        "{:?}",
        sourced(&nothing)
    );
    // A name nothing declares.
    let unknown = PROGRAM.replace(
        "    holds        Listing\n",
        "    holds        Listing, Listings\n",
    );
    assert_eq!(
        sourced(&unknown),
        ["PW0349 `Search` holds `Listings`, which names no type or module visible here"]
    );
    // One resource in two sources.
    let twice = PROGRAM.replace(
        "fn find(",
        "source Mirror\n    holds        Listing\n    transactions none\n    reads        eventual\n\nfn find(",
    );
    let found = sourced(&twice);
    assert!(
        found.contains(&"PW0349 `Listing` is held by `Search` and by `Mirror`".to_string())
            && found.contains(&"PW0349 `Listing` is held by `Mirror` and by `Search`".to_string()),
        "{found:?}"
    );
}

#[test]
fn a_clause_takes_only_the_words_its_domain_has() {
    for (from, to) in [
        ("    transactions none\n", "    transactions atomic\n"),
        (
            "    reads        eventual\n",
            "    reads        eventual, linear\n",
        ),
        ("    changes      none\n", "    changes      stream\n"),
    ] {
        let wrong = PROGRAM.replace(from, to);
        assert!(
            reported(&wrong).iter().any(|d| d.starts_with("PW0335")),
            "{to}: {:?}",
            reported(&wrong)
        );
    }
}

/// The store demo, as the one program it is.
fn store() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out
}

/// The store's codes in the PW0344..PW0349 range, each with its file.
fn store_sourced(store: &[(String, String)]) -> Vec<String> {
    check_sources(store)
        .into_iter()
        .flat_map(|(path, ds)| {
            ds.into_iter()
                .map(move |d| format!("{path}: {} {}", d.code, d.message))
        })
        .filter(|d| {
            ["PW0344", "PW0345", "PW0346", "PW0347", "PW0348", "PW0349"]
                .iter()
                .any(|c| d.contains(&format!(": {c} ")))
        })
        .collect()
}

#[test]
fn the_store_states_its_source_and_asks_only_what_it_gives() {
    let store = store();
    assert_eq!(store_sourced(&store), Vec::<String>::new());
    // Declared as a search index is, its database refuses what it asks.
    let weaker: Vec<(String, String)> = store
        .iter()
        .map(|(path, src)| {
            let src = if path == "StoreData.pw" {
                src.replace("transactions serializable", "transactions none")
                    .replace("reads        strong", "reads        eventual")
            } else {
                src.clone()
            };
            (path.clone(), src)
        })
        .collect();
    let found = store_sourced(&weaker);
    let named = |code: &str| -> Vec<&str> {
        found
            .iter()
            .filter(|d| d.contains(&format!(": {code} ")))
            .map(|d| d.split('`').nth(1).expect("named"))
            .collect()
    };
    // Each query but `Recommendations`, which asks `eventual`.
    assert_eq!(
        named("PW0344"),
        ["Menu", "Store", "StoreList", "Cart", "Estimate", "Order"]
    );
    // The commands that ask a serializable transaction.
    assert_eq!(
        named("PW0346"),
        ["add_to_cart", "increase_in_cart", "place_order"]
    );
    // And every command emits its changes and is idempotent by its
    // interaction.
    let commands = [
        "add_to_cart",
        "increase_in_cart",
        "decrease_in_cart",
        "remove_from_cart",
        "clear_cart",
        "place_order",
    ];
    assert_eq!(named("PW0347"), commands);
    assert_eq!(named("PW0348"), commands);
    assert_eq!(found.len(), 21, "{found:#?}");
}
