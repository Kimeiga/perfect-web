//! **A page's commands run in the order the page sent them** (ADR-XXXX).
//!
//! Each command a page sends names the one it sent before it and has not
//! been answered (`pw-after`, that one's interaction). The network brings
//! the two in either order: under load, "Clear", pressed after "Place
//! order", arrived first, the cart was emptied, and the order was refused as
//! nothing to order. A command runs here only once the one it names has:
//! Bayou's monotonic writes, "If Write W1 precedes Write W2 in a session,
//! then, for any server S2, if W2 in DB(S2) then W1 is also in DB(S2) and
//! WriteOrder(W1,W2)" (Terry et al., PDIS 1994).
//!
//! One that names a command not yet here waits for it, a while ([`WAIT`]);
//! past that it is answered early and runs nothing, and its page sends it
//! again once the one before it is answered. One that names a command
//! answered early is answered early too: what follows a command never runs
//! before it.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long a command waits for the one it names to arrive: as long as a
/// page's first resend of a request that found no server may take (a second,
/// ADR-0173), and as long again for its way. A request sent before another
/// and later than this behind it was lost, not overtaken.
pub const WAIT: Duration = Duration::from_secs(2);

/// How many commands' fates one session keeps, as many as it keeps outcomes
/// (ADR-0121). One still here is never let go: it is a request in flight.
const KEPT: usize = 64;

/// What became of a command the server was sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fate {
    /// Here: waiting for the one before it, or running.
    Here,
    /// Answered: it ran, or was refused for good. What follows it may run.
    Answered,
    /// Answered early: the one before it did not come, and its page sends it
    /// again.
    Early,
}

/// Whether a command may run, the one it names having run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    Run,
    Early,
}

/// Each session's commands, by interaction, the latest change last.
pub struct Order {
    fates: Mutex<BTreeMap<String, VecDeque<(String, Fate)>>>,
    changed: Condvar,
    wait: Duration,
}

impl Default for Order {
    fn default() -> Order {
        Order::waiting(WAIT)
    }
}

impl Order {
    /// An order whose commands wait `wait` for the one they name to arrive.
    pub fn waiting(wait: Duration) -> Order {
        Order {
            fates: Mutex::new(BTreeMap::new()),
            changed: Condvar::new(),
            wait,
        }
    }

    /// `interaction` is here for `session`. What became of it is told when
    /// the [`Arrival`] ends: answered, unless it says otherwise.
    pub fn arrive<'a>(&'a self, session: &str, interaction: &str) -> Arrival<'a> {
        self.set(session, interaction, Some(Fate::Here));
        Arrival {
            order: self,
            session: session.to_string(),
            interaction: interaction.to_string(),
            fate: Some(Fate::Answered),
        }
    }

    /// Once `after`, the command `session`'s page sent before this one, has
    /// run: [`Turn::Run`]. [`Turn::Early`] where it was answered early, or
    /// did not arrive within the wait.
    pub fn follow(&self, session: &str, after: &str) -> Turn {
        let deadline = Instant::now() + self.wait;
        let mut fates = self.fates.lock().expect("order");
        loop {
            let fate = fates
                .get(session)
                .and_then(|kept| kept.iter().find(|(i, _)| i == after))
                .map(|(_, fate)| *fate);
            match fate {
                Some(Fate::Answered) => return Turn::Run,
                Some(Fate::Early) => return Turn::Early,
                // Running, or waiting for its own: either ends.
                Some(Fate::Here) => fates = self.changed.wait(fates).expect("order"),
                None => {
                    let now = Instant::now();
                    if now >= deadline {
                        return Turn::Early;
                    }
                    fates = self
                        .changed
                        .wait_timeout(fates, deadline - now)
                        .expect("order")
                        .0;
                }
            }
        }
    }

    /// A session forgotten, its commands' fates with it.
    pub fn forget(&self, session: &str) {
        self.fates.lock().expect("order").remove(session);
    }

    fn set(&self, session: &str, interaction: &str, fate: Option<Fate>) {
        {
            let mut fates = self.fates.lock().expect("order");
            let kept = fates.entry(session.to_string()).or_default();
            kept.retain(|(i, _)| i != interaction);
            if let Some(fate) = fate {
                kept.push_back((interaction.to_string(), fate));
            }
            while kept.len() > KEPT {
                match kept.iter().position(|(_, fate)| *fate != Fate::Here) {
                    Some(oldest) => {
                        kept.remove(oldest);
                    }
                    None => break,
                }
            }
            if kept.is_empty() {
                fates.remove(session);
            }
        }
        self.changed.notify_all();
    }
}

/// A command that is here, until it is answered.
pub struct Arrival<'a> {
    order: &'a Order,
    session: String,
    interaction: String,
    fate: Option<Fate>,
}

impl Arrival<'_> {
    /// Answered early: its page sends it again.
    pub fn early(mut self) {
        self.fate = Some(Fate::Early);
    }

    /// Gone unanswered before it ran, as a connection closed: as if it never
    /// came, so what follows it waits for its page to send it again.
    pub fn vanish(mut self) {
        self.fate = None;
    }
}

impl Drop for Arrival<'_> {
    /// Answered as it ends, a panic included: what follows is never held for
    /// a command that will not answer.
    fn drop(&mut self) {
        self.order.set(&self.session, &self.interaction, self.fate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const SHORT: Duration = Duration::from_millis(200);

    #[test]
    fn a_command_whose_named_one_was_answered_runs() {
        let order = Order::waiting(SHORT);
        drop(order.arrive("s", "one"));
        assert_eq!(order.follow("s", "one"), Turn::Run);
    }

    #[test]
    fn a_command_waits_for_the_one_it_names_to_arrive_and_run() {
        let order = Arc::new(Order::waiting(Duration::from_secs(5)));
        let ran = Arc::new(Mutex::new(Vec::new()));
        // The second is here first, and waits.
        let second = {
            let (order, ran) = (order.clone(), ran.clone());
            std::thread::spawn(move || {
                let arrival = order.arrive("s", "two");
                assert_eq!(order.follow("s", "one"), Turn::Run);
                ran.lock().unwrap().push("two");
                drop(arrival);
            })
        };
        std::thread::sleep(Duration::from_millis(100));
        let arrival = order.arrive("s", "one");
        // Here and running: the second still waits.
        std::thread::sleep(Duration::from_millis(100));
        ran.lock().unwrap().push("one");
        drop(arrival);
        second.join().unwrap();
        assert_eq!(*ran.lock().unwrap(), ["one", "two"]);
    }

    #[test]
    fn a_command_whose_named_one_never_comes_is_answered_early_after_the_wait() {
        let order = Arc::new(Order::waiting(SHORT));
        let started = Instant::now();
        let (told, answer) = std::sync::mpsc::channel();
        {
            let order = order.clone();
            std::thread::spawn(move || told.send(order.follow("s", "lost")));
        }
        // Watched from here: a wait with no end fails, and does not hang.
        let turn = answer
            .recv_timeout(SHORT * 5)
            .expect("answered within the wait");
        assert_eq!(turn, Turn::Early);
        let waited = started.elapsed();
        assert!(waited >= SHORT, "answered early after {waited:?}");
    }

    #[test]
    fn a_command_whose_named_one_was_answered_early_is_answered_early() {
        let order = Order::waiting(Duration::from_secs(5));
        order.arrive("s", "one").early();
        let started = Instant::now();
        assert_eq!(order.follow("s", "one"), Turn::Early);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_command_that_vanished_is_waited_for_again() {
        let order = Arc::new(Order::waiting(Duration::from_secs(5)));
        order.arrive("s", "one").vanish();
        let follower = {
            let order = order.clone();
            std::thread::spawn(move || order.follow("s", "one"))
        };
        // Its page sends it again, and it runs.
        std::thread::sleep(Duration::from_millis(100));
        drop(order.arrive("s", "one"));
        assert_eq!(follower.join().unwrap(), Turn::Run);
    }

    #[test]
    fn another_sessions_command_of_the_same_name_is_not_the_one_named() {
        let order = Order::waiting(SHORT);
        drop(order.arrive("other", "one"));
        assert_eq!(order.follow("s", "one"), Turn::Early);
    }

    #[test]
    fn a_command_still_here_is_kept_past_the_bound() {
        let order = Order::waiting(SHORT);
        let here = order.arrive("s", "first");
        for i in 0..KEPT * 2 {
            drop(order.arrive("s", &format!("i{i}")));
        }
        // The oldest answered went; the one in flight stayed, and is waited for.
        assert_eq!(order.follow("s", "i0"), Turn::Early);
        assert_eq!(order.follow("s", &format!("i{}", KEPT * 2 - 1)), Turn::Run);
        let waiting = std::thread::scope(|scope| {
            let follower = scope.spawn(|| order.follow("s", "first"));
            std::thread::sleep(SHORT * 2);
            let finished = follower.is_finished();
            drop(here);
            (finished, follower.join().unwrap())
        });
        assert_eq!(waiting, (false, Turn::Run));
    }

    #[test]
    fn a_session_forgotten_takes_its_fates() {
        let order = Order::waiting(SHORT);
        drop(order.arrive("s", "one"));
        order.forget("s");
        assert_eq!(order.follow("s", "one"), Turn::Early);
    }
}
