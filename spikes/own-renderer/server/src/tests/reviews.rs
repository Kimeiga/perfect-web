//! **A store's ratings and reviews, served** (track `reviews`, ADR-0341): a
//! reader rates each store of their latest order once it is delivered, once;
//! what a store's ratings come to is said on its page, the home page and a
//! search, its reviews on its page without their reviewer, and the order page
//! links each store to rate. On the layer the server's tests serve the store
//! on: memory, or PostgreSQL under `PW_STORE_TEST_LAYER=postgres`.

use super::sign_in::{TestProvider, signed_in};
use super::*;

const ADD: &str = "store.page.add_to_cart";
const PLACE: &str = "store.page.place_order";
const RATE: &str = "store.rate.rate_order";
const STORE_PAGE: &str = "store.page.StorePage";

fn served() -> Served {
    served_from_patches_in("examples", |app| app.to_string(), &[])
}

/// What a page shows `session`, as text.
fn page(s: &Server, session: &str, path: &str) -> String {
    let whole: String = fetched_as(s, path, Some(session))
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    visible(&whole)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `session`'s order of `items`, placed.
fn placed(s: &Server, session: &str, items: &[&str]) {
    for item in items {
        s.command(ADD, session, &add_shown(item, 1), false)
            .expect("added");
    }
    s.command(PLACE, session, &[s.shown_total(session)], false)
        .expect("placed");
}

/// `session`'s order moved along by the kitchen, with its event.
fn moved(s: &Server, session: &str, status: &str) {
    let answer = posted(s, &format!("/bench/order?status={status}"), session, "", "");
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
}

/// `session`'s order of `items`, placed and delivered.
fn delivered(s: &Server, session: &str, items: &[&str]) {
    placed(s, session, items);
    moved(s, session, "delivered");
}

/// `rate_order` pressed by `session`: what it answered, or why it was not
/// run (a predicate's refusal, ADR-0302).
fn rate(
    s: &Server,
    session: &str,
    store: &str,
    stars: i64,
    comment: &str,
) -> Result<Answered, String> {
    s.command_answered(
        RATE,
        session,
        &[
            Val::String(store.to_string()),
            Val::S64(stars),
            Val::String(comment.to_string()),
        ],
        None,
    )
}

/// `rate_order` pressed by `session`, and committed: `Ok`, without its value,
/// which reaches the page from what it reads (ADR-0157).
fn rated(s: &Server, session: &str, store: &str, stars: i64, comment: &str) {
    let answered = rate(s, session, store, stars, comment).expect("answered");
    assert!(answered.committed, "{answered:?}");
    assert_eq!(
        answered.result,
        Some(serde_json::json!({ "$case": "ok" })),
        "{answered:?}"
    );
}

/// `rate_order` pressed by `session`, and refused by its `requires`
/// predicate `predicate` before it ran.
fn refused_by(s: &Server, session: &str, store: &str, predicate: &str) {
    let why = rate(s, session, store, 5, "").expect_err("refused");
    assert!(
        why.contains(&format!("predicate `{predicate}` denied")),
        "{predicate}: {why}"
    );
}

/// A layer's read, as a query's components read it.
fn read(s: &Server, session: &str, op: &str, args: &[Val]) -> Val {
    let ops = s.data.reads(session, None);
    let mut answer = ops[op](args).expect("read");
    assert_eq!(answer.len(), 1, "{op}: {answer:?}");
    answer.remove(0)
}

/// **The layer's `#rate`, past the predicate**, staged and committed as a
/// command's: what it answered, or why it refused.
fn rated_by_layer(
    s: &Server,
    session: &str,
    store: &str,
    stars: i64,
) -> Result<Option<Val>, String> {
    let owner = s.owner(session);
    s.store_write(
        session,
        "store:data/ratings#rate",
        &[
            Val::String(owner),
            Val::String(store.to_string()),
            Val::S64(stars),
            Val::String(String::new()),
        ],
        &[],
        |_| {},
    )
}

/// **A delivered order's store is rated once, and shown** (ADR-0341,
/// decisions 1 to 5): the order page links "Rate Blue Bottle" once the order
/// is delivered and not before; the rating commits; then the order page says
/// what was given, the store's page "1 rating" and the review, the home page
/// and a search "1 rating" for it and "No ratings yet" for the other; and a
/// second rating is refused by `MayRate`, in the program's words.
#[test]
fn a_delivered_orders_store_is_rated_once_and_shown() {
    let s = served();
    assert!(
        page(&s, "a", "/stores/47").contains("No ratings yet"),
        "no ratings at first"
    );
    placed(&s, "a", &["espresso"]);
    assert!(
        !page(&s, "a", "/order").contains("Rate your order"),
        "placed: nothing to rate yet"
    );
    moved(&s, "a", "delivered");
    let order = page(&s, "a", "/order");
    assert!(order.contains("Rate your order"), "{order}");
    assert!(order.contains("Rate Blue Bottle"), "{order}");
    rated(&s, "a", "47", 4, "Great espresso.");
    let order = page(&s, "a", "/order");
    assert!(
        order.contains("You rated Blue Bottle 4 out of 5."),
        "{order}"
    );
    let store = page(&s, "b", "/stores/47");
    assert!(
        store.contains("Open 24 hours 1 rating Small-batch"),
        "{store}"
    );
    assert!(
        store.contains("What customers said 4 out of 5") && store.contains("Great espresso."),
        "the review: {store}"
    );
    let home = page(&s, "b", "/");
    assert!(
        home.contains("Open 24 hours 1 rating Small-batch"),
        "{home}"
    );
    assert!(home.contains("No ratings yet"), "store 48: {home}");
    let found = page(&s, "b", "/search?q=coffee");
    assert!(found.contains("1 rating"), "{found}");
    refused_by(&s, "a", "47", "MayRate");
    assert!(
        page(&s, "a", "/order/rate/47").contains("You rated Blue Bottle 4 out of 5. Thanks!"),
        "the rate page says what was given"
    );
}

/// **An order not yet delivered is not rated**: placed, or on its way, its
/// store's rating refused by `MayRate`, and the rate page says why.
#[test]
fn an_order_not_yet_delivered_is_not_rated() {
    let s = served();
    placed(&s, "a", &["espresso"]);
    refused_by(&s, "a", "47", "MayRate");
    moved(&s, "a", "on-the-way");
    refused_by(&s, "a", "47", "MayRate");
    assert!(
        page(&s, "a", "/order/rate/47")
            .contains("You can rate Blue Bottle once your order is delivered."),
        "the rate page says why"
    );
    assert!(
        !page(&s, "a", "/order").contains("Rate your order"),
        "the order page links none"
    );
}

/// **A store not of the order is not rated**: delivered from Blue Bottle,
/// Harbor Coffee's rating is refused, and its rate page says why.
#[test]
fn a_store_not_of_the_order_is_not_rated() {
    let s = served();
    delivered(&s, "a", &["espresso"]);
    refused_by(&s, "a", "48", "MayRate");
    assert!(
        page(&s, "a", "/order/rate/48").contains("Your latest order isn't from this store."),
        "the rate page says why"
    );
}

/// **Each store of an order is rated once** (ADR-0341, decision 1): an
/// order from both stores links both, and each is rated apart.
#[test]
fn each_store_of_an_order_is_rated_once() {
    let s = served();
    delivered(&s, "a", &["espresso", "drip"]);
    let order = page(&s, "a", "/order");
    assert!(
        order.contains("Rate Blue Bottle") && order.contains("Rate Harbor Coffee"),
        "{order}"
    );
    rated(&s, "a", "47", 5, "");
    rated(&s, "a", "48", 2, "Cold.");
    let order = page(&s, "a", "/order");
    assert!(
        order.contains("You rated Blue Bottle 5 out of 5.")
            && order.contains("You rated Harbor Coffee 2 out of 5."),
        "{order}"
    );
    refused_by(&s, "a", "48", "MayRate");
}

/// **Only the order's owner rates it** (ADR-0341, decision 3): ada's
/// delivered order is hers; ben, signed in with no order, is refused by
/// `MayRate`, and a guest by `SignedIn`; ada rates it.
#[test]
fn only_the_orders_owner_rates_it() {
    let provider = Arc::new(TestProvider::default());
    let s = served();
    s.identity
        .use_provider(provider.clone(), super::sign_in::REDIRECT);
    let ada = signed_in(&s, &provider, "came-ada", "ada");
    let ben = signed_in(&s, &provider, "came-ben", "ben");
    delivered(&s, &ada, &["espresso"]);
    refused_by(&s, &ben, "47", "MayRate");
    refused_by(&s, "a-guest", "47", "SignedIn");
    rated(&s, &ada, "47", 5, "");
    assert!(
        page(&s, &ben, "/stores/47").contains("1 rating"),
        "one rating, ada's"
    );
}

/// **The layer holds its rules without the predicate** (ADR-0341, decision
/// 3): `#rate`, called past `MayRate`, refuses an order not delivered, a
/// store not of the order, and a second rating, by name; a reader with no
/// order is not delivered.
#[test]
fn the_layer_holds_its_rules_without_the_predicate() {
    let s = served();
    let refused = |why: Result<Option<Val>, String>, case: &str| {
        let why = why.expect_err("refused");
        assert!(why.contains(case), "{case}: {why}");
    };
    refused(rated_by_layer(&s, "a", "47", 4), "not-delivered");
    placed(&s, "a", &["espresso"]);
    refused(rated_by_layer(&s, "a", "47", 4), "not-delivered");
    moved(&s, "a", "delivered");
    refused(rated_by_layer(&s, "a", "48", 4), "not-in-order");
    assert_eq!(rated_by_layer(&s, "a", "47", 4), Ok(Some(Val::S64(4))));
    refused(rated_by_layer(&s, "a", "47", 5), "already-rated");
    // Stars past 1 to 5 are no program's: the operation traps.
    let past = rated_by_layer(&s, "a", "48", 6).expect_err("past 5");
    assert!(past.contains("past 1 to 5"), "{past}");
}

/// **The latest order alone is rated** (DoorDash's rule, ADR-0341 decision
/// 1): a delivered order left unrated closes when the next is placed; the
/// next, delivered, is rated, a second rating of the same store through
/// another order.
#[test]
fn the_latest_order_alone_is_rated() {
    let s = served();
    delivered(&s, "a", &["espresso"]);
    placed(&s, "a", &["cortado"]);
    refused_by(&s, "a", "47", "MayRate");
    moved(&s, "a", "delivered");
    rated(&s, "a", "47", 5, "");
    delivered(&s, "a", &["espresso"]);
    rated(&s, "a", "47", 3, "");
    assert!(
        page(&s, "b", "/stores/47").contains("2 ratings"),
        "two orders, two ratings"
    );
}

/// **A store's mean is said from ten ratings, rounded half up** (ADR-0341,
/// decision 4): nine say "9 ratings"; the tenth, a mean of 4.1; twelve
/// summing 51, a mean of 4.25, said 4.3, not 4.2.
#[test]
fn a_stores_mean_is_said_from_ten_ratings() {
    let s = served();
    let give = |stars: i64| {
        delivered(&s, "a", &["espresso"]);
        rated(&s, "a", "47", stars, "");
    };
    for _ in 0..9 {
        give(4);
    }
    let nine = page(&s, "b", "/stores/47");
    assert!(nine.contains("9 ratings"), "{nine}");
    assert!(!nine.contains("out of 5 ·"), "no mean below ten: {nine}");
    give(5);
    assert!(
        page(&s, "b", "/stores/47").contains("4.1 out of 5 · 10 ratings"),
        "ten: 41 / 10"
    );
    give(5);
    give(5);
    let twelve = page(&s, "b", "/stores/47");
    assert!(twelve.contains("4.3 out of 5 · 12 ratings"), "{twelve}");
    assert!(
        page(&s, "b", "/").contains("4.3 out of 5 · 12 ratings"),
        "the home page says the same"
    );
}

/// **Reviews show without their reviewer** (ADR-0341, decision 5): the
/// layer's `#reviews` answers each review's id, stars and words alone; a
/// rating with no comment is no review; the newest come first, five at most;
/// and the store's page names no reader.
#[test]
fn reviews_show_without_their_reviewer() {
    let s = served();
    for (n, comment) in [
        "First.", "Second.", "", "Third.", "Fourth.", "Fifth.", "Sixth.",
    ]
    .iter()
    .enumerate()
    {
        delivered(&s, "reviewer", &["espresso"]);
        rated(&s, "reviewer", "47", 1 + (n as i64) % 5, comment);
    }
    let Val::List(reviews) = read(
        &s,
        "b",
        "store:data/ratings#reviews",
        &[Val::String("47".into())],
    ) else {
        panic!("a list")
    };
    let words: Vec<String> = reviews
        .iter()
        .map(|r| {
            let Val::Record(fields) = r else {
                panic!("a record: {r:?}")
            };
            let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
            assert_eq!(names, ["id", "stars", "comment"], "no reviewer");
            match val_at(r, &["comment"]) {
                Some(Val::String(c)) => c.clone(),
                other => panic!("{other:?}"),
            }
        })
        .collect();
    assert_eq!(words, ["Sixth.", "Fifth.", "Fourth.", "Third.", "Second."]);
    let store = page(&s, "b", "/stores/47");
    assert!(
        store.contains("Sixth.") && !store.contains("First."),
        "{store}"
    );
    let owner = s.owner("reviewer");
    assert!(
        !store.contains(&owner) && !store.contains("reviewer"),
        "the store's page names no reader: {store}"
    );
}

/// **The ratings of a list of stores are read at once, in the stores'
/// order**: `#for-stores` counts each listed store's and sums its stars, a
/// store no one rated left out, as both layers answer it.
#[test]
fn the_ratings_of_a_list_of_stores_are_read_at_once() {
    let s = served();
    delivered(&s, "a", &["espresso", "drip"]);
    rated(&s, "a", "47", 5, "");
    rated(&s, "a", "48", 2, "");
    delivered(&s, "b", &["drip"]);
    rated(&s, "b", "48", 3, "");
    let counted = |stores: &[&str]| {
        read(
            &s,
            "c",
            "store:data/ratings#for-stores",
            &[Val::List(
                stores
                    .iter()
                    .map(|id| Val::String(id.to_string()))
                    .collect(),
            )],
        )
    };
    let row = |store: &str, count: i64, stars: i64| {
        Val::Record(vec![
            ("store".into(), Val::String(store.into())),
            ("count".into(), Val::S64(count)),
            ("stars".into(), Val::S64(stars)),
        ])
    };
    assert_eq!(
        counted(&["48", "47", "nowhere"]),
        Val::List(vec![row("48", 2, 5), row("47", 1, 5)])
    );
    assert_eq!(counted(&[]), Val::List(vec![]));
}

/// **A rating is told to the pages that show it** (ADR-0219): another
/// reader's open store page is sent the new count; the home page and a
/// search, read again, say it, their kept lists dropped by `StoreRated`.
#[test]
fn a_rating_is_told_to_the_pages_that_show_it() {
    let s = served();
    delivered(&s, "a", &["espresso"]);
    assert!(page(&s, "b", "/").contains("No ratings yet"));
    assert!(page(&s, "b", "/search?q=bottle").contains("No ratings yet"));
    s.serve_document_settled("b", STORE_PAGE, &store_params("47"), &[])
        .expect("served");
    let doc = latest(&s.pending.lock().expect("pending"), "b");
    let before = sets_of(&s, &doc).len();
    rated(&s, "a", "47", 4, "Fine.");
    s.tell_waiting();
    let told = format!("{:?}", &sets_of(&s, &doc)[before..]);
    assert!(told.contains("1 rating"), "b's store page is told: {told}");
    assert!(page(&s, "b", "/").contains("1 rating"), "the home page");
    assert!(
        page(&s, "b", "/search?q=bottle").contains("1 rating"),
        "a search"
    );
}

/// **A comment is the program's to bound** (ADR-0341, decision 2): 500
/// characters are taken; the page's own check refuses more before any press
/// reaches the store, and the layer traps on one past it.
#[test]
fn a_comment_is_at_most_five_hundred_characters() {
    let s = served();
    delivered(&s, "a", &["espresso", "drip"]);
    let most = "é".repeat(500);
    rated(&s, "a", "47", 5, &most);
    let past = rate(&s, "a", "48", 5, &"é".repeat(501));
    assert!(
        past.as_ref().is_err() || past.as_ref().is_ok_and(|a| !a.committed),
        "{past:?}"
    );
    assert!(
        page(&s, "b", "/stores/47").contains(&most),
        "the comment shown whole"
    );
}

/// **What the build says of the store**, its `reviews.pw` changed by
/// `change`: built, or why it was refused.
fn built_with(change: fn(&str) -> String) -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let pw_files = |dir: &str| -> Vec<std::path::PathBuf> {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        paths
    };
    let mut files = pw_files("packages/pw-std");
    files.extend(pw_files("packages/pw-platform-web"));
    files.push(root.join("examples/domain.pw"));
    files.extend(pw_files("examples/lib"));
    files.extend(pw_files("examples/store"));
    let units: Vec<pw_core::check::Unit> = files
        .into_iter()
        .map(|p| {
            let read = std::fs::read_to_string(&p).expect("read");
            let src = if p.ends_with("examples/store/reviews.pw") {
                change(&read)
            } else {
                read
            };
            pw_core::check::Unit {
                hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
                path: p.display().to_string(),
                src,
            }
        })
        .collect();
    pw_core::build::build(&units).map(|_| ())
}

/// **The checker keeps the reader out of a store's shared ratings**
/// (ADR-0341, decision 5): `StoreRatings`, every reader's and kept shared,
/// made to say whether the reader may rate the store, reads through the
/// reader's handle, and the build is refused (PW5001); as it is, the store
/// builds. So whose a review is cannot reach what every reader is served.
#[test]
fn the_checker_keeps_the_reader_out_of_a_stores_shared_ratings() {
    assert_eq!(
        built_with(|src| src.to_string()),
        Ok(()),
        "the store as it is"
    );
    let why = built_with(|src| {
        let (import, body) = (
            "import Events.{ UserOrderChanged }\n",
            "    let all = Ratings.for_stores([id])\n    RatingsShown { said: said_of(id, all), reviews: Ratings.reviews(id) }\n",
        );
        assert!(src.contains(import) && src.contains(body), "the anchors");
        src.replace(
            import,
            "import Events.{ UserOrderChanged }\nimport context.{ current_user }\n",
        )
        .replace(
            body,
            "    let all = Ratings.for_stores([id])\n    let mine = Ratings.for_order(current_user())\n    \
             let said = if mine.delivered { \"Rate it\" } else { said_of(id, all) }\n    \
             RatingsShown { said: said, reviews: Ratings.reviews(id) }\n",
        )
    })
    .expect_err("refused");
    assert!(
        why.contains("`StoreRatings` is User<UserId> and declares a shared cache"),
        "{why}"
    );
}

/// **A rating is told to the rater's own open pages**: the order page they
/// rated from says what was given, and the rate page what was given and
/// thanks, each told after the commit, by the event its query listens for.
#[test]
fn a_rating_is_told_to_the_raters_open_pages() {
    let s = served();
    delivered(&s, "a", &["espresso"]);
    s.serve_document_settled("a", "store.page.OrderPage", &Params::new(), &[])
        .expect("the order page");
    let order = latest(&s.pending.lock().expect("pending"), "a");
    s.serve_document_settled("a", "store.rate.RatePage", &store_params("47"), &[])
        .expect("the rate page");
    let rating = latest(&s.pending.lock().expect("pending"), "a");
    let (o, r) = (sets_of(&s, &order).len(), sets_of(&s, &rating).len());
    rated(&s, "a", "47", 4, "Fine.");
    s.tell_waiting();
    let told = format!("{:?}", &sets_of(&s, &order)[o..]);
    assert!(
        told.contains("You rated Blue Bottle 4 out of 5."),
        "the order page: {told}"
    );
    let told = format!("{:?}", &sets_of(&s, &rating)[r..]);
    assert!(
        told.contains("You rated Blue Bottle 4 out of 5. Thanks!"),
        "the rate page: {told}"
    );
}
