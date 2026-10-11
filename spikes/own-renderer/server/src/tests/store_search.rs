//! **The store's search page, served** (track `store-search`, W9): the
//! canonical store, `examples/store/search.pw` among its sources, built as
//! `pw build` builds it and asked for `/search/{q}` over HTTP. On the layer
//! the server's tests run on: memory, or PostgreSQL under
//! `PW_STORE_TEST_LAYER=postgres` (`just e14-store-search`).

use super::*;

/// The canonical store, served.
fn served() -> Served {
    served_from_patches_in("examples", |app| app.to_string(), &[])
}

/// `GET path`, its status line and its page's visible text.
fn searched(s: &Server, path: &str) -> (String, String, String) {
    let response: String = fetched_as(s, path, Some("a"))
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    let status = response.lines().next().unwrap_or_default().to_string();
    let html = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    let text = visible(&html)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (status, html, text)
}

/// **What a search lists, in its order, each under its store, linked to
/// it**: the stores whose name holds the term before those whose cuisine
/// does, and their dishes.
#[test]
fn a_search_lists_the_stores_and_dishes_that_match() {
    let s = served();
    let (status, html, text) = searched(&s, "/search/coffee");
    assert!(status.starts_with("HTTP/1.1 200"), "{status}");
    assert!(text.contains("2 stores match “coffee”."), "{text}");
    let harbor = text
        .find("Harbor Coffee Café, Bakery")
        .expect("Harbor Coffee, by its name");
    let drip = text.find("Drip Coffee Drinks").expect("its dish");
    let blue = text
        .find("Blue Bottle Coffee")
        .expect("Blue Bottle, by its cuisine");
    let cortado = text.find("Cortado Coffee").expect("its dish");
    assert!(harbor < drip && drip < blue && blue < cortado, "{text}");
    assert!(text.contains("$3.00"), "a dish's price: {text}");
    // Each store and each of its dishes links to the store's page.
    assert_eq!(html.matches("href=\"/stores/48\"").count(), 2, "{html}");
    assert_eq!(html.matches("href=\"/stores/47\"").count(), 4, "{html}");
    assert!(html.contains("<title>Search: coffee</title>"), "{html}");
    assert!(html.contains("noindex"), "{html}");
    // In the store's layout: the way home, and the reader's cart.
    assert!(html.contains("id=\"header-cart-count\""), "{html}");
}

/// **Case ignored, accents kept** (W9's rule 2), in an address's encoding.
#[test]
fn case_is_ignored_and_accents_are_not() {
    let s = served();
    let (_, _, text) = searched(&s, "/search/CAF%C3%89");
    assert!(text.contains("1 store matches “CAFÉ”."), "{text}");
    assert!(text.contains("Harbor Coffee"), "{text}");
    assert!(!text.contains("Blue Bottle Coffee Small"), "{text}");
    let (_, _, text) = searched(&s, "/search/BLUEBERRY%20scone");
    assert!(
        text.contains("Blueberry Scone Bakery"),
        "several terms: {text}"
    );
    let (_, _, text) = searched(&s, "/search/cafe");
    assert!(
        text.contains("Harbor Coffee"),
        "\"cafe\" is in its description: {text}"
    );
}

/// **No text fails the page** (W9's rule 3): blank, too long, `LIKE`'s
/// characters, quotes and markup are each answered and told.
#[test]
fn no_text_fails_the_page() {
    let s = served();
    for (path, said) in [
        (
            "/search/%20",
            "Type a store, a dish or a cuisine to search for.",
        ),
        (
            "/search/%09%E3%80%80",
            "Type a store, a dish or a cuisine to search for.",
        ),
        ("/search/%25", "No stores or dishes match “%”."),
        ("/search/_", "No stores or dishes match “_”."),
        ("/search/%5C", "No stores or dishes match “\\”."),
        ("/search/'%22", "No stores or dishes match “'\"”."),
        ("/search/%25%25%20_", "No stores or dishes match “%% _”."),
        (
            "/search/%3Cb%3Ex%3C%2Fb%3E",
            "No stores or dishes match “&lt;b&gt;x&lt;/b&gt;”.",
        ),
    ] {
        let (status, html, text) = searched(&s, path);
        assert!(status.starts_with("HTTP/1.1 200"), "{path}: {status}");
        assert!(text.contains(said), "{path}: {text}");
        assert!(!html.contains("<b>x"), "{path}: markup is text");
        assert!(
            !html.contains("<a href=\"/stores/"),
            "{path}: nothing listed"
        );
    }
    let long = format!("/search/{}", "x".repeat(crate::search::MOST_CHARACTERS + 1));
    let (status, _, text) = searched(&s, &long);
    assert!(status.starts_with("HTTP/1.1 200"), "{status}");
    assert!(
        text.contains("That search is too long: 100 characters at most."),
        "{text}"
    );
    let edge = format!("/search/{}", "x".repeat(crate::search::MOST_CHARACTERS));
    let (_, _, text) = searched(&s, &edge);
    assert!(
        text.contains("No stores or dishes match"),
        "100 is searched: {text}"
    );
}

/// **A change to a menu reaches a search kept since** (the query's
/// `invalidates_on MenuChanged(_)`): an item added to store 47 is found.
#[test]
fn a_menu_change_reaches_a_kept_search() {
    let s = served();
    let (_, _, before) = searched(&s, "/search/flat%20white");
    assert!(
        before.contains("No stores or dishes match “flat white”."),
        "{before}"
    );
    let answer = posted(
        &s,
        "/command/menu?op=insert_after&id=flat-white&name=Flat%20White",
        "a",
        "menu-1",
        "",
    );
    assert!(answer.starts_with("HTTP/1.1 202"), "{answer}");
    let (_, html, after) = searched(&s, "/search/flat%20white");
    assert!(after.contains("1 store matches “flat white”."), "{after}");
    assert!(after.contains("Blue Bottle Coffee"), "{after}");
    assert!(
        after.contains("Flat White Coffee"),
        "the dish, in its store's category: {after}"
    );
    assert_eq!(html.matches("href=\"/stores/47\"").count(), 2, "{html}");
}
