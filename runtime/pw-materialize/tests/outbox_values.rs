//! **An event's values come out of the outbox as they went in** (ADR-0208).
//!
//! They were joined by a separator, and one empty value was read back as
//! none: an event carrying `""` named no entry and reached every one. A
//! command computes its events' values now, any `String` among them.

use pw_materialize::{Clock, Event, Materializer};

#[test]
fn an_events_values_are_read_back_as_written() {
    let m = Materializer::new(Clock::new(), "test");
    let written = [
        Event::new("Events.Named", &[""]),
        Event::new("Events.Named", &["", ""]),
        Event::new("Events.Moved", &["a\u{1}b", "47"]),
        Event::new("Events.Cleared", &[]),
    ];
    m.command(|_| Ok::<_, String>(written.to_vec()))
        .expect("commits");
    let read: Vec<Event> = m.pending().into_iter().map(|c| c.event).collect();
    assert_eq!(read, written);
}
