//! **Direct messages** (track `messages`, ADR-XXXX; docs/PARALLEL.md, the
//! integrator's rulings of 2026-10-08).
//!
//! A message is a row of its own, written in its command's transaction: its
//! sender, its recipient, its text and its time. A read mark per reader and
//! other user says how far the reader has read. A conversation is read from
//! one side: each participant reads the same rows through a query keyed by
//! their own handle and the other's id, private to them.
//!
//! **Who may message whom is X's rule**: the recipient follows the sender,
//! or has sent the sender a message before; no one messages themselves.
//! `send` requires `MayMessage(to)`, which the identity's `requires`
//! evaluates through the command's own operations (`feed:data/messages#may`),
//! in its transaction, as `OwnsPost(post)` is.

use super::*;
use crate::identity::Principal;
use std::collections::BTreeSet;

/// **The operation `MayMessage` reads through**: whether the first user may
/// message the second, an operation of the feed's data layer.
pub(crate) const MAY: &str = "feed:data/messages#may";

/// **One message, as kept**: its place in every message ever written, who
/// sent it to whom, what it says, and when, in seconds since the epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sent {
    pub seq: u64,
    pub from: String,
    pub to: String,
    pub text: String,
    /// Kept as the row's time; no page shows it yet.
    #[allow(dead_code)]
    pub at: u64,
}

impl Sent {
    /// Its id, which no other message has had.
    pub(crate) fn id(&self) -> String {
        format!("m{}", self.seq)
    }
}

/// **The messages in memory**: every one, oldest first; each reader's mark
/// in each conversation, the last message they have read; and how many were
/// ever written.
#[derive(Debug, Clone, Default)]
pub(crate) struct Messages {
    pub sent: Vec<Sent>,
    pub marks: BTreeMap<(String, String), u64>,
    pub written: u64,
}

/// **Whether `from` may message `to`** (X's rule): never oneself; and `to`
/// follows `from`, or `to` has messaged `from` before. `follows` holds
/// (follower, followee).
pub(crate) fn may(
    follows: &BTreeSet<(String, String)>,
    sent: &[Sent],
    from: &str,
    to: &str,
) -> bool {
    from != to
        && (follows.contains(&(to.to_string(), from.to_string()))
            || sent.iter().any(|m| m.from == to && m.to == from))
}

impl Messages {
    /// The messages between `a` and `b`, either way, oldest first.
    fn between<'a>(&'a self, a: &'a str, b: &'a str) -> impl Iterator<Item = &'a Sent> + 'a {
        self.sent
            .iter()
            .filter(move |m| (m.from == a && m.to == b) || (m.from == b && m.to == a))
    }

    /// The last message `reader` has read of their conversation with `with`.
    fn mark(&self, reader: &str, with: &str) -> u64 {
        self.marks
            .get(&(reader.to_string(), with.to_string()))
            .copied()
            .unwrap_or(0)
    }

    /// Whether `reader` has yet to read `m`: sent to them, past their mark.
    fn unread_by(&self, reader: &str, m: &Sent) -> bool {
        m.to == reader && m.seq > self.mark(reader, &m.from)
    }

    /// **Whether `with` is anyone `reader` has messaged or been messaged by.**
    pub(crate) fn involves(&self, user: &str) -> bool {
        self.sent.iter().any(|m| m.from == user || m.to == user)
    }

    /// **How many messages `reader` has yet to read**, in every
    /// conversation.
    pub(crate) fn unread(&self, reader: &str) -> i64 {
        self.sent
            .iter()
            .filter(|m| self.unread_by(reader, m))
            .count() as i64
    }

    /// **`reader`'s conversation with `with`**, the program's
    /// `Conversation`: the newest `limit` messages, oldest first.
    pub(crate) fn conversation(
        &self,
        reader: &str,
        with: &str,
        limit: i64,
        may_send: bool,
        user: &dyn Fn(&str) -> Val,
    ) -> Val {
        let all: Vec<&Sent> = self.between(reader, with).collect();
        let shown = &all[all.len().saturating_sub(limit.max(0) as usize)..];
        conversation_val(
            user(with),
            user(reader),
            shown
                .iter()
                .map(|m| message_val(&m.id(), user(&m.from), &m.text, self.unread_by(reader, m)))
                .collect(),
            may_send,
            reader == with,
        )
    }

    /// **`reader`'s conversations**, newest first, the program's
    /// `List<ConversationItem>`.
    pub(crate) fn listed(&self, reader: &str, user: &dyn Fn(&str) -> Val) -> Val {
        let mut seen = BTreeSet::new();
        let items = self
            .sent
            .iter()
            .rev()
            .filter(|m| m.from == reader || m.to == reader)
            .filter_map(|m| {
                let other = if m.from == reader { &m.to } else { &m.from };
                seen.insert(other.clone()).then(|| {
                    let unread = self
                        .between(reader, other)
                        .filter(|x| self.unread_by(reader, x))
                        .count() as i64;
                    item_val(user(other), &m.text, unread)
                })
            })
            .collect();
        Val::List(items)
    }

    /// **A message staged**: its place, past every one written and staged.
    pub(crate) fn next_seq(&self, staged: u64) -> u64 {
        self.written + staged + 1
    }

    /// **A message committed**, and its sender's side read to it: sending
    /// marks the conversation read for its sender.
    pub(crate) fn commit(&mut self, sent: Sent) {
        self.written = self.written.max(sent.seq);
        self.marks
            .insert((sent.from.clone(), sent.to.clone()), sent.seq);
        self.sent.push(sent);
    }

    /// **`reader`'s conversation with `with`, read**: their mark at its last
    /// message. How many were unread.
    pub(crate) fn read(&mut self, reader: &str, with: &str) -> i64 {
        let unread = self.unread_from(reader, with);
        if let Some(last) = self.between(reader, with).map(|m| m.seq).max() {
            let mark = self
                .marks
                .entry((reader.to_string(), with.to_string()))
                .or_default();
            *mark = (*mark).max(last);
        }
        unread
    }

    /// How many messages from `with` `reader` has yet to read.
    pub(crate) fn unread_from(&self, reader: &str, with: &str) -> i64 {
        self.sent
            .iter()
            .filter(|m| m.from == with && self.unread_by(reader, m))
            .count() as i64
    }
}

/// **A message as the program's `Message` is.**
pub(crate) fn message_val(id: &str, from: Val, text: &str, unread: bool) -> Val {
    Val::Record(vec![
        ("id".into(), Val::String(id.to_string())),
        ("from".into(), from),
        ("text".into(), Val::String(text.to_string())),
        ("unread".into(), Val::Bool(unread)),
    ])
}

/// **A conversation as the program's `Conversation` is**: `closed` where the
/// reader may not send.
pub(crate) fn conversation_val(
    with: Val,
    me: Val,
    messages: Vec<Val>,
    may_send: bool,
    yourself: bool,
) -> Val {
    Val::Record(vec![
        ("with".into(), with),
        ("me".into(), me),
        ("messages".into(), Val::List(messages)),
        ("closed".into(), Val::Bool(!may_send)),
        ("yourself".into(), Val::Bool(yourself)),
    ])
}

/// **A conversation as the reader's list shows it**, `ConversationItem`.
pub(crate) fn item_val(with: Val, last: &str, unread: i64) -> Val {
    Val::Record(vec![
        ("with".into(), with),
        ("last".into(), Val::String(last.to_string())),
        ("unread".into(), Val::S64(unread)),
    ])
}

/// Now, in seconds since the epoch: a message's time.
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// **`MayMessage(to)`, for the session's principal**: read through the
/// command's own operations (`feed:data/messages#may`), so within its
/// transaction. No principal may message no one.
pub(crate) fn may_message(
    principal: Option<Principal>,
    bound: &[&Val],
    host: &BTreeMap<String, HostFn>,
) -> Result<bool, String> {
    let Some(principal) = principal else {
        return Ok(false);
    };
    let [Val::String(to)] = bound else {
        return Err(format!(
            "MayMessage takes one user id, and was given {bound:?}"
        ));
    };
    let may = host
        .get(MAY)
        .ok_or("MayMessage reads who may message whom through feed:data/messages#may")?;
    match may(&[Val::String(principal.user), Val::String(to.clone())])?.as_slice() {
        [Val::Bool(held)] => Ok(*held),
        other => Err(format!("messages#may answered {other:?}")),
    }
}

/// **The in-memory layer's message reads**: a conversation, the list, the
/// count and who may message whom, over `messages` and `follows` as one
/// snapshot read them; `user` a user's record, `known` whether the feed
/// knows a user.
pub(crate) fn reads(
    ops: &mut crate::data::Ops,
    messages: Arc<Messages>,
    follows: Arc<BTreeSet<(String, String)>>,
    user: Arc<dyn Fn(&str) -> Val + Send + Sync>,
    known: Arc<dyn Fn(&str) -> bool + Send + Sync>,
) {
    let (m, f, u) = (messages.clone(), follows.clone(), user.clone());
    ops.insert(
        "feed:data/messages#conversation".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader), Val::String(with), Val::S64(limit)] => {
                if !known(with) && !m.involves(with) {
                    return Ok(vec![crate::feed::not_found()]);
                }
                let may_send = may(&f, &m.sent, reader, with);
                Ok(vec![crate::feed::ok(
                    m.conversation(reader, with, *limit, may_send, &*u),
                )])
            }
            other => Err(format!("messages#conversation received {other:?}")),
        }),
    );
    let (m, u) = (messages.clone(), user);
    ops.insert(
        "feed:data/messages#list".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader)] => Ok(vec![m.listed(reader, &*u)]),
            other => Err(format!("messages#list received {other:?}")),
        }),
    );
    let m = messages.clone();
    ops.insert(
        "feed:data/messages#unread".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(reader)] => Ok(vec![Val::S64(m.unread(reader))]),
            other => Err(format!("messages#unread received {other:?}")),
        }),
    );
    ops.insert(
        MAY.to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(from), Val::String(to)] => {
                Ok(vec![Val::Bool(may(&follows, &messages.sent, from, to))])
            }
            other => Err(format!("messages#may received {other:?}")),
        }),
    );
}

/// **Messages in PostgreSQL** (migration `0007_messages`): a row each,
/// written in its command's transaction, and a read mark per reader and
/// other user; each read in one statement.
pub(crate) mod pg {
    use super::*;
    use postgres::Client;

    /// Whether `$1` may message `$2`, in SQL: X's rule, as [`may`] is.
    const MAY_SQL: &str = "($1 <> $2 AND (\
         EXISTS (SELECT 1 FROM follows WHERE follower = $2 AND followee = $1) \
         OR EXISTS (SELECT 1 FROM messages WHERE sender = $2 AND recipient = $1)))";

    /// **Whether `from` may message `to`.**
    pub(crate) fn may(c: &mut Client, from: &str, to: &str) -> Result<bool, postgres::Error> {
        Ok(c.query_one(&format!("SELECT {MAY_SQL}"), &[&from, &to])?
            .get(0))
    }

    /// **`reader`'s conversation with `with`**, read in one statement, so one
    /// snapshot: the two users, whether the feed knows `with`, whether the
    /// reader may send, and the newest `limit` messages, oldest first.
    pub(crate) fn conversation(
        c: &mut Client,
        reader: &str,
        with: &str,
        limit: i64,
    ) -> Result<Val, postgres::Error> {
        let row = c.query_one(
            &format!(
                "SELECT (SELECT handle FROM users WHERE id = $2), \
                        (SELECT name FROM users WHERE id = $2), \
                        (SELECT handle FROM users WHERE id = $1), \
                        (SELECT name FROM users WHERE id = $1), \
                        (EXISTS (SELECT 1 FROM users WHERE id = $2) \
                         OR EXISTS (SELECT 1 FROM posts WHERE author = $2) \
                         OR EXISTS (SELECT 1 FROM follows WHERE follower = $2 OR followee = $2) \
                         OR EXISTS (SELECT 1 FROM messages WHERE sender = $2 OR recipient = $2)), \
                        {MAY_SQL_READER}, \
                        (SELECT coalesce(json_agg(json_build_array(m.id, m.sender, u.handle, \
                             u.name, m.text, m.sender = $2 AND m.seq > coalesce(r.seq, 0)) \
                             ORDER BY m.seq), '[]')::text \
                         FROM (SELECT * FROM messages \
                               WHERE (sender = $1 AND recipient = $2) \
                                  OR (sender = $2 AND recipient = $1) \
                               ORDER BY seq DESC LIMIT $3) m \
                         LEFT JOIN users u ON u.id = m.sender \
                         LEFT JOIN message_reads r ON r.reader = $1 AND r.other = $2)",
                MAY_SQL_READER = MAY_SQL,
            ),
            &[&reader, &with, &limit.max(0)],
        )?;
        let known: bool = row.get(4);
        if !known {
            return Ok(crate::feed::not_found());
        }
        let them: (Option<String>, Option<String>) = (row.get(0), row.get(1));
        let me: (Option<String>, Option<String>) = (row.get(2), row.get(3));
        let may_send: bool = row.get(5);
        type Shown = (String, String, Option<String>, Option<String>, String, bool);
        let shown: Vec<Shown> = serde_json::from_str(&row.get::<_, String>(6)).unwrap_or_default();
        Ok(crate::feed::ok(conversation_val(
            crate::feed::user_record(with, them.0.zip(them.1)),
            crate::feed::user_record(reader, me.0.zip(me.1)),
            shown
                .into_iter()
                .map(|(id, from, handle, name, text, unread)| {
                    message_val(
                        &id,
                        crate::feed::user_record(&from, handle.zip(name)),
                        &text,
                        unread,
                    )
                })
                .collect(),
            may_send,
            reader == with,
        )))
    }

    /// **`reader`'s conversations**, newest first: each with the other
    /// user, the last message, and how many from them are unread.
    pub(crate) fn list(c: &mut Client, reader: &str) -> Result<Val, postgres::Error> {
        let rows = c.query(
            "WITH mine AS ( \
                 SELECT seq, sender, text, \
                        CASE WHEN sender = $1 THEN recipient ELSE sender END AS other \
                 FROM messages WHERE sender = $1 OR recipient = $1), \
             last AS (SELECT DISTINCT ON (other) other, seq, text FROM mine \
                      ORDER BY other, seq DESC) \
             SELECT l.other, u.handle, u.name, l.text, \
                    (SELECT count(*) FROM mine x \
                     WHERE x.other = l.other AND x.sender = l.other \
                       AND x.seq > coalesce((SELECT r.seq FROM message_reads r \
                                             WHERE r.reader = $1 AND r.other = l.other), 0)) \
             FROM last l LEFT JOIN users u ON u.id = l.other \
             ORDER BY l.seq DESC",
            &[&reader],
        )?;
        Ok(Val::List(
            rows.iter()
                .map(|r| {
                    let (handle, name): (Option<String>, Option<String>) = (r.get(1), r.get(2));
                    let other: String = r.get(0);
                    item_val(
                        crate::feed::user_record(&other, handle.zip(name)),
                        r.get::<_, &str>(3),
                        r.get(4),
                    )
                })
                .collect(),
        ))
    }

    /// **How many messages `reader` has yet to read**, in every
    /// conversation.
    pub(crate) fn unread(c: &mut Client, reader: &str) -> Result<i64, postgres::Error> {
        Ok(c.query_one(
            "SELECT count(*) FROM messages m \
             LEFT JOIN message_reads r ON r.reader = $1 AND r.other = m.sender \
             WHERE m.recipient = $1 AND m.seq > coalesce(r.seq, 0)",
            &[&reader],
        )?
        .get(0))
    }

    /// **A message from `from` to `to`**, and the sender's side read to it,
    /// in the command's transaction: the program's `Message`.
    pub(crate) fn send(
        c: &mut Client,
        from: &str,
        to: &str,
        text: &str,
    ) -> Result<Val, postgres::Error> {
        let row = c.query_one(
            "WITH sent AS (INSERT INTO messages (sender, recipient, text) VALUES ($1, $2, $3) \
                           RETURNING seq, id), \
                  marked AS (INSERT INTO message_reads (reader, other, seq) \
                             SELECT $1, $2, seq FROM sent \
                             ON CONFLICT (reader, other) DO UPDATE \
                             SET seq = greatest(message_reads.seq, EXCLUDED.seq), \
                                 committed_in = pg_current_xact_id()) \
             SELECT id, (SELECT handle FROM users WHERE id = $1), \
                        (SELECT name FROM users WHERE id = $1) FROM sent",
            &[&from, &to, &text],
        )?;
        let (handle, name): (Option<String>, Option<String>) = (row.get(1), row.get(2));
        Ok(message_val(
            row.get::<_, &str>(0),
            crate::feed::user_record(from, handle.zip(name)),
            text,
            false,
        ))
    }

    /// **`reader`'s conversation with `with`, read**: their mark at its last
    /// message. How many were unread.
    pub(crate) fn read(c: &mut Client, reader: &str, with: &str) -> Result<i64, postgres::Error> {
        let unread: i64 = c
            .query_one(
                "SELECT count(*) FROM messages m \
                 LEFT JOIN message_reads r ON r.reader = $1 AND r.other = $2 \
                 WHERE m.sender = $2 AND m.recipient = $1 AND m.seq > coalesce(r.seq, 0)",
                &[&reader, &with],
            )?
            .get(0);
        c.execute(
            "INSERT INTO message_reads (reader, other, seq) \
             SELECT $1, $2, max(seq) FROM messages \
             WHERE (sender = $1 AND recipient = $2) OR (sender = $2 AND recipient = $1) \
             HAVING max(seq) IS NOT NULL \
             ON CONFLICT (reader, other) DO UPDATE \
             SET seq = greatest(message_reads.seq, EXCLUDED.seq), \
                 committed_in = pg_current_xact_id()",
            &[&reader, &with],
        )?;
        Ok(unread)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent(seq: u64, from: &str, to: &str) -> Sent {
        Sent {
            seq,
            from: from.into(),
            to: to.into(),
            text: format!("{from} to {to}, {seq}"),
            at: 0,
        }
    }

    #[test]
    fn who_may_message_whom_is_xs_rule() {
        let none = BTreeSet::new();
        // No one messages themselves, whoever follows them.
        let selfish = BTreeSet::from([("a".to_string(), "a".to_string())]);
        assert!(!may(&selfish, &[], "a", "a"));
        // A stranger may not; one the recipient follows may.
        assert!(!may(&none, &[], "a", "b"));
        let b_follows_a = BTreeSet::from([("b".to_string(), "a".to_string())]);
        assert!(may(&b_follows_a, &[], "a", "b"));
        // Following someone is not leave to message them: it is the other
        // way about.
        assert!(!may(&b_follows_a, &[], "b", "a"));
        // One who has messaged you, you may answer; they may not again
        // unless you answer or follow them.
        let b_wrote = [sent(1, "b", "a")];
        assert!(may(&none, &b_wrote, "a", "b"));
        assert!(!may(&none, &b_wrote, "b", "a"));
        // A message between two others is no leave.
        assert!(!may(&none, &[sent(1, "b", "c")], "a", "b"));
    }

    #[test]
    fn sending_marks_the_senders_side_read_and_not_the_recipients() {
        let mut m = Messages::default();
        m.commit(sent(m.next_seq(0), "a", "b"));
        m.commit(sent(m.next_seq(0), "b", "a"));
        m.commit(sent(m.next_seq(0), "b", "a"));
        assert_eq!((m.unread("a"), m.unread("b")), (2, 0));
        // `a` answers: their side is read to their own message.
        m.commit(sent(m.next_seq(0), "a", "b"));
        assert_eq!((m.unread("a"), m.unread("b")), (0, 1));
        assert_eq!(m.read("b", "a"), 1);
        assert_eq!(m.unread("b"), 0);
        // Read again, nothing was unread.
        assert_eq!(m.read("b", "a"), 0);
    }

    #[test]
    fn a_readers_list_is_theirs_newest_first_and_counts_each_conversation() {
        let mut m = Messages::default();
        m.commit(sent(1, "b", "a"));
        m.commit(sent(2, "c", "a"));
        m.commit(sent(3, "b", "a"));
        m.commit(sent(4, "c", "d"));
        let user = |id: &str| Val::String(id.to_string());
        let Val::List(items) = m.listed("a", &user) else {
            panic!("a list")
        };
        let summary: Vec<(Val, Val)> = items
            .iter()
            .map(|i| match i {
                Val::Record(f) => (f[0].1.clone(), f[2].1.clone()),
                _ => panic!("a record"),
            })
            .collect();
        assert_eq!(
            summary,
            [
                (Val::String("b".into()), Val::S64(2)),
                (Val::String("c".into()), Val::S64(1)),
            ]
        );
        // `d`'s is between others: not `a`'s.
        let Val::List(ds) = m.listed("d", &user) else {
            panic!("a list")
        };
        assert_eq!(ds.len(), 1);
    }

    #[test]
    fn a_conversation_shows_the_newest_messages_oldest_first() {
        let mut m = Messages::default();
        for seq in 1..=5 {
            m.commit(sent(
                seq,
                if seq % 2 == 0 { "a" } else { "b" },
                if seq % 2 == 0 { "b" } else { "a" },
            ));
        }
        m.commit(sent(6, "c", "a"));
        let user = |id: &str| Val::String(id.to_string());
        let Val::Record(fields) = m.conversation("a", "b", 3, true, &user) else {
            panic!("a record")
        };
        let Val::List(shown) = &fields[2].1 else {
            panic!("its messages")
        };
        let ids: Vec<&Val> = shown
            .iter()
            .map(|v| match v {
                Val::Record(f) => &f[0].1,
                _ => panic!("a message"),
            })
            .collect();
        assert_eq!(
            ids,
            [
                &Val::String("m3".into()),
                &Val::String("m4".into()),
                &Val::String("m5".into())
            ]
        );
        assert_eq!(fields[3], ("closed".into(), Val::Bool(false)));
    }
}
