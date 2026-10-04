//! **Last-known-good, for declared public data only** (ADR-0177, charter
//! §15.6 test 18: "Origin failure follows last-known-good policy only for
//! declared public data").
//!
//! A public read declared `last_known_good` whose origin fails is answered
//! with the last value kept, past its freshness. Nothing else is: a read
//! that declares none, a private read whatever it declares, a read with
//! nothing kept, and a read stopped rather than failed.

use pw_resource::*;

fn runtime() -> (Resources, Clock) {
    let clock = Clock::new();
    (Resources::new(clock.clone()), clock)
}

/// The kept value, read once, then expired past its freshness.
fn kept(rt: &Resources, clock: &Clock, m: &Manifest, key: &Key) {
    assert!(matches!(
        rt.fetch(m, key, |_| Ok("Blue Bottle".to_string())),
        Fetched::Fresh(_)
    ));
    clock.advance(30_001);
}

fn down(_: u32) -> Result<String, String> {
    Err("the origin is down".to_string())
}

#[test]
fn a_public_read_declared_last_known_good_is_answered_with_the_last_value_kept() {
    let (rt, clock) = runtime();
    let m = Manifest::new("store").freshness(30_000).last_known_good();
    let key = Key::new("store", "47");
    kept(&rt, &clock, &m, &key);
    assert!(matches!(
        rt.fetch(&m, &key, down),
        Fetched::LastKnownGood(ref v) if v == "Blue Bottle"
    ));
    assert!(
        rt.trace()
            .iter()
            .any(|t| matches!(t, Trace::ServedLastKnownGood { .. }))
    );
    // The origin back: the next read is its, and is kept.
    assert!(matches!(
        rt.fetch(&m, &key, |_| Ok("Blue Bottle Coffee".to_string())),
        Fetched::Fresh(ref v) if v == "Blue Bottle Coffee"
    ));
}

#[test]
fn nothing_else_is() {
    // Declared none.
    let (rt, clock) = runtime();
    let m = Manifest::new("store").freshness(30_000);
    let key = Key::new("store", "47");
    kept(&rt, &clock, &m, &key);
    assert!(matches!(rt.fetch(&m, &key, down), Fetched::Failed(_)));

    // Private, whatever it declares.
    let (rt, clock) = runtime();
    let m = Manifest::new("cart")
        .freshness(30_000)
        .private()
        .last_known_good();
    let key = Key::new("cart", "session=a");
    kept(&rt, &clock, &m, &key);
    assert!(matches!(rt.fetch(&m, &key, down), Fetched::Failed(_)));

    // A private value kept under the key a public read asks for: the read
    // fails on the privacy that does not match, and is not answered with it.
    let (rt, _) = runtime();
    let private = Manifest::new("store").freshness(30_000).private();
    let public = Manifest::new("store").freshness(30_000).last_known_good();
    let key = Key::new("store", "47");
    assert!(matches!(
        rt.fetch(&private, &key, |_| Ok("someone's".to_string())),
        Fetched::Fresh(_)
    ));
    assert!(
        matches!(rt.fetch(&public, &key, down), Fetched::Failed(_)),
        "a private value is never another reader's last known good"
    );

    // And the other way: a public value kept under the key a private read
    // asks for is not its answer either, though the private read declares
    // the fallback.
    let (rt, _) = runtime();
    let public = Manifest::new("store").freshness(30_000);
    let private = Manifest::new("store")
        .freshness(30_000)
        .private()
        .last_known_good();
    let key = Key::new("store", "47");
    assert!(matches!(
        rt.fetch(&public, &key, |_| Ok("everyone's".to_string())),
        Fetched::Fresh(_)
    ));
    assert!(matches!(rt.fetch(&private, &key, down), Fetched::Failed(_)));

    // Nothing kept.
    let (rt, _) = runtime();
    let m = Manifest::new("store").freshness(30_000).last_known_good();
    assert!(matches!(
        rt.fetch(&m, &Key::new("store", "48"), down),
        Fetched::Failed(_)
    ));
}

#[test]
fn an_expired_value_is_read_again_and_kept_for_its_fallback() {
    // `expire` is a test's way past freshness without a clock that moves.
    let (rt, _) = runtime();
    let m = Manifest::new("store").freshness(30_000).last_known_good();
    let key = Key::new("store", "47");
    assert!(matches!(
        rt.fetch(&m, &key, |_| Ok("Blue Bottle".to_string())),
        Fetched::Fresh(_)
    ));
    assert!(matches!(rt.fetch(&m, &key, down), Fetched::FromCache(_)));
    rt.expire("store");
    assert!(matches!(
        rt.fetch(&m, &key, down),
        Fetched::LastKnownGood(ref v) if v == "Blue Bottle"
    ));
    // And another resource's was not expired.
    let other = Manifest::new("menu").freshness(30_000);
    let menu = Key::new("menu", "47");
    assert!(matches!(
        rt.fetch(&other, &menu, |_| Ok("menu".to_string())),
        Fetched::Fresh(_)
    ));
    rt.expire("store");
    assert!(matches!(
        rt.fetch(&other, &menu, down),
        Fetched::FromCache(_)
    ));
}
