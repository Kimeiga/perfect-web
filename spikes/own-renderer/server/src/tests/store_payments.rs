//! **An order awaits its payment** (NEXT 6c, step seven). Where the
//! deployment takes payments, `place_order` places the reader's cart as an
//! order awaiting its payment and begins the payment at the provider, and
//! the order page frames the provider's page. The provider's signed word,
//! read again from the provider, settles it through the command the program
//! declares `invoked_by Payments`, run on behalf of the reader who began it.
//! On the layer the server's tests run on: memory, or PostgreSQL under
//! `PW_STORE_TEST_LAYER=postgres`. Each rule's test has its control.

use super::*;

const ADD: &str = "store.page.add_to_cart";
const PLACE: &str = "store.page.place_order";

/// **The store, taking payments from the development provider**, its
/// webhook served by the server's own [`handle`] while `test` runs: the
/// provider tells the host what a card did as it would in a deployment.
fn paying(test: impl FnOnce(&Served)) {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let port = listener.local_addr().expect("address").port();
    let provider_port = TcpListener::bind(("127.0.0.1", 0))
        .and_then(|l| l.local_addr())
        .expect("a free port")
        .port();
    let configured = payments::configure(
        true,
        &format!("http://127.0.0.1:{port}"),
        port,
        Some("dev"),
        Some(&provider_port.to_string()),
    )
    .expect("configured")
    .expect("payments");
    assert!(s.payments.set(configured).is_ok(), "set once");
    let done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for stream in listener.incoming() {
                if done.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                if let Ok(stream) = stream {
                    handle(&s, stream);
                }
            }
        });
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test(&s)));
        done.store(true, std::sync::atomic::Ordering::SeqCst);
        // The listener's last connection, so it sees it is done.
        let _ = TcpStream::connect(("127.0.0.1", port));
        if let Err(failed) = ran {
            std::panic::resume_unwind(failed);
        }
    });
}

/// The reader's order's status as the layer reads it: its case, and the
/// provider's page an awaiting one is paid on.
fn status_of(s: &Server, session: &str) -> Option<(String, Option<String>)> {
    let owner = s.owner(session);
    match s.layer_read(session, "store:data/user-orders#current", &owner) {
        Val::Result(Ok(Some(v))) => match *v {
            Val::Option(None) => None,
            Val::Option(Some(case)) => match *case {
                Val::Variant(case, None) => Some((case, None)),
                Val::Variant(case, Some(page)) => match *page {
                    Val::Record(fields) => match fields.as_slice() {
                        [(url, Val::String(page))] if url == "url" => {
                            Some((case, Some(page.clone())))
                        }
                        other => panic!("an awaiting order's page is {other:?}"),
                    },
                    other => panic!("an awaiting order's page is {other:?}"),
                },
                other => panic!("user-orders#current answered {other:?}"),
            },
            other => panic!("user-orders#current answered {other:?}"),
        },
        other => panic!("user-orders#current answered {other:?}"),
    }
}

/// **A card paid in the provider's page**, as the reader's browser posts
/// its form there: the page the provider answers with.
fn pay(page: &str, card: &str) -> String {
    let rest = page.strip_prefix("http://").expect("an http page");
    let (authority, path) = rest.split_once('/').expect("a path");
    let body = format!("card={card}");
    let mut stream = TcpStream::connect(authority).expect("the provider");
    let head = format!(
        "POST /{path} HTTP/1.1\r\nhost: {authority}\r\ncontent-type: application/x-www-form-urlencoded\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).expect("write");
    stream.write_all(body.as_bytes()).expect("write");
    let mut all = String::new();
    stream.read_to_string(&mut all).expect("read");
    all
}

/// The reader's cart, of an espresso and a cortado, placed: its order
/// awaiting its payment, on the provider's page.
fn placed_awaiting(s: &Server) -> String {
    s.command(ADD, "a", &add_shown("espresso", 2), false)
        .expect("added");
    s.command(ADD, "a", &add_shown("cortado", 1), false)
        .expect("added");
    // As a page presses it: a payment is begun by an interaction, so a
    // press sent again reaches the payment the first began.
    let placed = s
        .command_answered(PLACE, "a", &[s.shown_total("a")], Some("place-1"))
        .expect("placed");
    assert!(placed.committed, "{:?}", placed.result);
    let Some((case, Some(page))) = status_of(s, "a") else {
        panic!("not awaiting: {:?}", status_of(s, "a"));
    };
    assert_eq!(case, "awaiting-payment");
    page
}

/// **Where the deployment takes payments, an order awaits its payment** on
/// the provider's page, which the order page frames: the cart's lines are
/// its order and the cart is empty, as when it is placed, and the reader
/// cannot place another until it is paid. The control: a deployment that
/// takes none places it at once (`an_order_is_placed_from_the_cart_and_
/// reaches_its_open_page`).
#[test]
fn an_order_awaits_its_payment_on_the_providers_page() {
    paying(|s| {
        let page = placed_awaiting(s);
        let provider = &s.payments.get().expect("payments").provider;
        assert!(page.starts_with(&format!("{provider}/pay/")), "{page}");
        assert_eq!(
            s.order_of("a").map(|(_, lines)| lines),
            Some(vec![
                ("espresso".to_string(), 2),
                ("cortado".to_string(), 1)
            ])
        );
        assert!(s.cart_lines("a").is_empty(), "the cart is empty after");
        let (html, _, _, _) = s
            .serve_document_settled("a", "store.page.OrderPage", &Params::new(), &[])
            .expect("served");
        assert!(visible(&html).contains("Awaiting your payment"), "{html}");
        assert!(
            html.contains(&format!("src=\"{page}\"")),
            "the order page frames the provider's page: {html}"
        );
        // Nothing has charged it yet: its total is to pay.
        assert!(visible(&html).contains("Total to pay: $"), "{html}");
        assert!(!visible(&html).contains("Total charged"), "{html}");
        // Another order waits for this one to be paid.
        s.command(ADD, "a", &add_shown("espresso", 1), false)
            .expect("added");
        let answered = s
            .command_answered(PLACE, "a", &[s.shown_total("a")], Some("place-2"))
            .expect("answered");
        assert!(!answered.committed, "{:?}", answered.result);
        assert!(
            answered
                .result
                .as_ref()
                .is_some_and(|r| r.to_string().contains("awaiting-payment")),
            "{:?}",
            answered.result
        );
        assert_eq!(s.cart_lines("a"), vec![("espresso".to_string(), 1)]);
    });
}

/// **A card paid places the order, and the page open on it is told**: the
/// provider's signed word, read again from the provider, settled by the
/// program's `settle_payment` as the reader who began it.
#[test]
fn a_card_paid_places_the_order_and_its_open_page_is_told() {
    paying(|s| {
        let page = placed_awaiting(s);
        s.serve_document_settled("a", "store.page.OrderPage", &Params::new(), &[])
            .expect("served");
        let doc = latest(&s.pending.lock().expect("pending"), "a");
        let paid = pay(&page, "4242424242424242");
        assert!(paid.contains("Paid."), "{paid}");
        assert_eq!(status_of(s, "a"), Some(("placed".to_string(), None)));
        assert_eq!(
            s.order_of("a").map(|(_, lines)| lines),
            Some(vec![
                ("espresso".to_string(), 2),
                ("cortado".to_string(), 1)
            ])
        );
        // Told once the webhook's connection is answered (ADR-0219), as
        // every other session a commit reaches is: the reader's page is not
        // in the session the settling ran in.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !sets_of(s, &doc)
            .iter()
            .any(|set| written(set).contains("Placed: the store has your order."))
        {
            assert!(
                std::time::Instant::now() < deadline,
                "the open page was not told: {:?}",
                sets_of(s, &doc)
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // Paid, its total is charged.
        assert!(
            sets_of(s, &doc)
                .iter()
                .any(|set| visible(&written(set)).contains("Total charged: $")),
            "{:?}",
            sets_of(s, &doc)
        );
        let settled = s
            .payments
            .get()
            .expect("payments")
            .settled
            .lock()
            .expect("settled")
            .clone();
        assert_eq!(settled.len(), 1, "{settled:?}");
        assert_eq!(settled[0].owner, s.owner("a"));
        assert_eq!(settled[0].status, "succeeded");
    });
}

/// **A card declined leaves the order awaiting, and the same payment takes
/// another card**: a decline is no outcome for the order, and the page the
/// reader pays on stays the same.
#[test]
fn a_card_declined_leaves_the_order_awaiting_and_the_payment_takes_another() {
    paying(|s| {
        let page = placed_awaiting(s);
        let declined = pay(&page, "4000000000000002");
        assert!(declined.contains("Your card was declined."), "{declined}");
        assert_eq!(
            status_of(s, "a"),
            Some(("awaiting-payment".to_string(), Some(page.clone())))
        );
        let paid = pay(&page, "4242424242424242");
        assert!(paid.contains("Paid."), "{paid}");
        assert_eq!(status_of(s, "a"), Some(("placed".to_string(), None)));
    });
}

/// **A payment the reader's order does not await changes nothing**: an
/// outcome for another order's reference, heard as the provider's word,
/// leaves the order awaiting. The control: its own reference places it.
#[test]
fn a_payment_for_an_order_the_reader_has_not_placed_changes_nothing() {
    paying(|s| {
        let page = placed_awaiting(s);
        let owner = s.owner("a");
        let other = payments::Settled {
            intent: "pi_dev_other".into(),
            order: "o-another".into(),
            owner: owner.clone(),
            amount: 1,
            status: "succeeded".into(),
        };
        s.settle_payment(&other).expect("answered");
        assert_eq!(
            status_of(s, "a"),
            Some(("awaiting-payment".to_string(), Some(page.clone())))
        );
        let paid = pay(&page, "4242424242424242");
        assert!(paid.contains("Paid."), "{paid}");
        assert_eq!(status_of(s, "a"), Some(("placed".to_string(), None)));
    });
}

/// **A provider that cannot be reached places nothing**: `place_order`
/// answers `PaymentUnavailable`, and nothing it wrote is committed: no
/// order, and the cart as it was. The control: a provider that answers
/// places the order awaiting its payment (above).
#[test]
fn a_provider_that_cannot_be_reached_places_nothing() {
    let s = served_from_patches_in("examples", |app| app.to_string(), &[]);
    assert!(s.payments.set(payments::unreachable()).is_ok(), "set once");
    s.command(ADD, "a", &add_shown("espresso", 2), false)
        .expect("added");
    let answered = s
        .command_answered(PLACE, "a", &[s.shown_total("a")], Some("place-1"))
        .expect("answered");
    assert!(!answered.committed, "{:?}", answered.result);
    assert!(
        answered
            .result
            .as_ref()
            .is_some_and(|r| r.to_string().contains("payment-unavailable")),
        "{:?}",
        answered.result
    );
    assert_eq!(s.order_of("a"), None);
    assert_eq!(s.cart_lines("a"), vec![("espresso".to_string(), 2)]);
}

/// **A reader's press of the settling command is a command that is not
/// there** (step six's guard, held on the store): what the provider's word
/// carries, pressed by the reader, settles nothing. The control: the same
/// route takes a command a reader presses.
#[test]
fn a_readers_press_of_the_settling_command_is_not_a_command() {
    paying(|s| {
        let page = placed_awaiting(s);
        let forged = posted(
            s,
            "/command/store.page.settle_payment",
            "a",
            "forged-1",
            r#"[{"payment":"pi_dev000001","order":"o-0000000000000000","status":{"$case":"succeeded"}}]"#,
        );
        assert!(forged.starts_with("HTTP/1.1 404"), "{forged}");
        assert_eq!(
            status_of(s, "a"),
            Some(("awaiting-payment".to_string(), Some(page)))
        );
        let control = posted(s, "/command/store.page.clear_cart", "a", "control-1", "[]");
        assert!(control.contains("\"committed\":true"), "{control}");
    });
}

/// **A session the platform opens on a reader's behalf is never a
/// browser's** (step seven): its name is no secret, the payment's, so a
/// cookie naming it is a new session, even while the platform's is open and
/// bound to the reader. Until 2026-10-13 a cookie was taken as it came, and
/// a request sent while a payment was settled was the reader's. The
/// control: a cookie's own session is its.
#[test]
fn a_session_opened_on_a_readers_behalf_is_no_browsers_to_present() {
    paying(|s| {
        placed_awaiting(s);
        let behalf = s
            .identity
            .principals()
            .on_behalf("payment-pi_dev000001", &s.owner("a"));
        let (taken, fresh) = session_of(&format!("Cookie: pw-session={}\r\n", behalf.session()));
        assert!(fresh, "{taken}");
        assert_ne!(taken, behalf.session());
        assert_eq!(
            session_of("Cookie: pw-session=a\r\n"),
            ("a".to_string(), false)
        );
        // Through the server: the order's page, asked with that cookie, is
        // a new reader's, given a cookie of its own.
        let answer = get_as(s, behalf.session(), "/order");
        assert!(answer.contains("set-cookie: pw-session=s-"), "{answer}");
        assert!(
            visible(&answer).contains("You have no order yet."),
            "{answer}"
        );
        // The control: the reader's own cookie is the reader's order.
        let own = get_as(s, "a", "/order");
        assert!(visible(&own).contains("Awaiting your payment"), "{own}");
    });
}
