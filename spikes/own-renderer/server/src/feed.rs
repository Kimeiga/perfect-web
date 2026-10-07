//! **The feed's data** (ADR-0218): the posts and users of the feed reference
//! app, a Twitter-shaped program (`examples/feed`), kept in memory.
//!
//! What its contracts import, `feed:data/posts#…` and `feed:data/users#…`,
//! is this. A query reads it; a command stages its writes here, and they are
//! the feed's once the command's transaction commits.

use super::*;
use crate::identity::{Principal, Principals};

/// **What a node grants the feed's data**, wherever it is kept.
pub(crate) const GRANTS: &[&str] = &[
    "database.read<Post>",
    "database.write<Post>",
    "database.read<User>",
];

/// One post as kept: its author by id, and what it replies to.
#[derive(Debug, Clone)]
struct Row {
    id: String,
    author: String,
    text: String,
    likes: i64,
    reply_to: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct State {
    /// Oldest first.
    posts: Vec<Row>,
    /// A user's handle and name, by id.
    users: BTreeMap<String, (String, String)>,
    /// Posts ever written, so that a post's id is never one a deleted post
    /// had.
    written: usize,
    /// Commits, which the materializer's row for the feed counts.
    commits: usize,
}

/// What a command changes, staged until it commits.
#[derive(Debug, Clone)]
enum Change {
    Post(Row),
    Like(String),
    /// A post and every reply under it (track `identity`, ADR-XXXX).
    Delete(String),
    /// A signed-in author's handle and name, as their provider gave them:
    /// the user the feed shows for them.
    User(String, (String, String)),
}

pub(crate) struct FeedData {
    state: Mutex<State>,
    /// Each session's principal (track `identity`): whom a session posts
    /// as. Until the server hands its own, every session is its own guest.
    principals: std::sync::RwLock<Principals>,
}

impl FeedData {
    pub(crate) fn new() -> FeedData {
        let mut state = State::default();
        for (id, handle, name) in [("u-ada", "@ada", "Ada"), ("u-grace", "@grace", "Grace")] {
            state
                .users
                .insert(id.to_string(), (handle.to_string(), name.to_string()));
        }
        state.posts = vec![
            Row {
                id: "p1".into(),
                author: "u-ada".into(),
                text: "Hello, feed.".into(),
                likes: 2,
                reply_to: None,
            },
            Row {
                id: "p2".into(),
                author: "u-grace".into(),
                text: "Hello, Ada.".into(),
                likes: 0,
                reply_to: Some("p1".into()),
            },
            Row {
                id: "p3".into(),
                author: "u-ada".into(),
                text: "And a reply to the reply.".into(),
                likes: 1,
                reply_to: Some("p2".into()),
            },
        ];
        state.written = state.posts.len();
        FeedData {
            state: Mutex::new(state),
            principals: std::sync::RwLock::new(Principals::default()),
        }
    }
}

/// The user a session posts as where it has no principal: one of its own,
/// named for it.
pub(crate) fn user_of(session: &str) -> String {
    format!("u-{session}")
}

/// **Who `session` reads as** (track `identity`, ADR-XXXX), the program's
/// `Viewer`: signed in or not, its user, and its name.
pub(crate) fn viewer_val(principal: Option<Principal>) -> Val {
    let (signed_in, id, name) = match principal {
        Some(p) => (true, p.user, p.name),
        None => (false, String::new(), String::new()),
    };
    Val::Record(vec![
        ("signed-in".into(), Val::Bool(signed_in)),
        ("id".into(), Val::String(id)),
        ("name".into(), Val::String(name)),
    ])
}

/// **A signed-in author, as the feed shows them**: the handle and name their
/// provider vouched for, written with their post. A guest has none, and is
/// named for their session.
pub(crate) fn profile_of(principal: &Principal) -> Option<(String, String)> {
    (!principal.is_guest()).then(|| (principal.handle.clone(), principal.name.clone()))
}

fn user_val(state: &State, id: &str) -> Val {
    user_record(id, state.users.get(id).cloned())
}

/// **A user as the program's `User` is**: its handle and name where it has
/// them. A session's own user, which signed up as no one, is a guest named
/// for its session, the same to every reader. Until ADR-0220 it was "You" to
/// every reader.
pub(crate) fn user_record(id: &str, known: Option<(String, String)>) -> Val {
    let (handle, name) = known.unwrap_or_else(|| {
        // A session's id is long since it is random (track `identity`): a
        // guest is named for its first ten characters.
        let session: String = id
            .strip_prefix("u-")
            .unwrap_or(id)
            .chars()
            .take(10)
            .collect();
        (format!("@{session}"), format!("Guest {session}"))
    });
    Val::Record(vec![
        ("id".into(), Val::String(id.to_string())),
        ("handle".into(), Val::String(handle)),
        ("name".into(), Val::String(name)),
    ])
}

/// A post as the program's `Post` is: its replies inside it, as deep as they
/// go, where `replies` asks for them.
fn post_val(state: &State, row: &Row, replies: bool) -> Val {
    let inner = if replies {
        state
            .posts
            .iter()
            .filter(|r| r.reply_to.as_deref() == Some(row.id.as_str()))
            .map(|r| post_val(state, r, true))
            .collect()
    } else {
        Vec::new()
    };
    Val::Record(vec![
        ("id".into(), Val::String(row.id.clone())),
        ("author".into(), user_val(state, &row.author)),
        ("text".into(), Val::String(row.text.clone())),
        ("likes".into(), Val::S64(row.likes)),
        ("replies".into(), Val::List(inner)),
    ])
}

/// **A post as a timeline shows it** (ADR-0222), the program's `Item`: who
/// wrote it, what it says, its likes, and whether the reader wrote it
/// (track `identity`), which the timeline is private to.
fn item_val(state: &State, row: &Row, reader: &str) -> Val {
    Val::Record(vec![
        ("id".into(), Val::String(row.id.clone())),
        ("author".into(), user_val(state, &row.author)),
        ("text".into(), Val::String(row.text.clone())),
        ("likes".into(), Val::S64(row.likes)),
        ("mine".into(), Val::Bool(row.author == reader)),
    ])
}

pub(crate) fn not_found() -> Val {
    Val::Result(Err(Some(Box::new(Val::Variant("not-found".into(), None)))))
}

pub(crate) fn ok(v: Val) -> Val {
    Val::Result(Ok(Some(Box::new(v))))
}

/// The reads of `state`: the timeline, a thread, a session's user and who
/// it reads as.
fn reads_of(state: Arc<State>, principals: Principals) -> crate::data::Ops {
    let mut ops: crate::data::Ops = BTreeMap::new();
    let (s, p) = (state.clone(), principals.clone());
    ops.insert(
        "feed:data/posts#timeline".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(session), Val::S64(limit)] => {
                let reader = p.user_of(session);
                // Each an `Item`, a post as a timeline shows it (ADR-0222):
                // its replies are its thread's.
                let shown: Vec<Val> = s
                    .posts
                    .iter()
                    .rev()
                    .filter(|r| r.reply_to.is_none())
                    .take((*limit).max(0) as usize)
                    .map(|r| item_val(&s, r, &reader))
                    .collect();
                Ok(vec![Val::List(shown)])
            }
            other => Err(format!("posts#timeline received {other:?}")),
        }),
    );
    let s = state.clone();
    ops.insert(
        "feed:data/posts#thread".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(id)] => Ok(vec![match s.posts.iter().find(|r| r.id == *id) {
                Some(row) => ok(post_val(&s, row, true)),
                None => not_found(),
            }]),
            other => Err(format!("posts#thread received {other:?}")),
        }),
    );
    let p = principals.clone();
    ops.insert(
        "feed:data/users#of-session".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(session)] => Ok(vec![Val::String(p.user_of(session))]),
            other => Err(format!("users#of-session received {other:?}")),
        }),
    );
    ops.insert(
        "feed:data/users#viewer".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(session)] => Ok(vec![viewer_val(principals.of(session))]),
            other => Err(format!("users#viewer received {other:?}")),
        }),
    );
    ops
}

/// **A post and every reply under it**, by id.
fn under(posts: &[Row], id: &str) -> std::collections::BTreeSet<String> {
    let mut gone = std::collections::BTreeSet::from([id.to_string()]);
    loop {
        let more: Vec<String> = posts
            .iter()
            .filter(|r| r.reply_to.as_ref().is_some_and(|to| gone.contains(to)))
            .filter(|r| !gone.contains(&r.id))
            .map(|r| r.id.clone())
            .collect();
        if more.is_empty() {
            return gone;
        }
        gone.extend(more);
    }
}

/// **A command's writes to the feed, staged** (ADR-0218): under the feed's
/// lock from the call to the commit, as the store's cart is.
pub(crate) struct Staging<'a> {
    state: std::sync::MutexGuard<'a, State>,
    staged: Arc<Mutex<Vec<Change>>>,
    ops: crate::data::Ops,
}

impl crate::data::Staged for Staging<'_> {
    fn ops(&self) -> crate::data::Ops {
        self.ops.clone()
    }

    fn rows(&self) -> Option<Vec<(String, String)>> {
        let staged = self.staged.lock().expect("staged");
        (!staged.is_empty()).then(|| {
            vec![(
                "feed:posts".to_string(),
                (self.state.commits + 1).to_string(),
            )]
        })
    }

    fn publish(&mut self) {
        let staged: Vec<Change> = self.staged.lock().expect("staged").drain(..).collect();
        if !staged.is_empty() {
            self.state.commits += 1;
        }
        for change in staged {
            match change {
                Change::Post(row) => {
                    self.state.written += 1;
                    self.state.posts.push(row);
                }
                Change::Delete(id) => {
                    let gone = under(&self.state.posts, &id);
                    self.state.posts.retain(|r| !gone.contains(&r.id));
                }
                Change::User(id, profile) => {
                    self.state.users.insert(id, profile);
                }
                Change::Like(id) => {
                    if let Some(row) = self.state.posts.iter_mut().find(|r| r.id == id) {
                        row.likes += 1;
                    }
                }
            }
        }
    }
}

impl FeedData {
    fn principals(&self) -> Principals {
        self.principals.read().expect("principals").clone()
    }
}

/// **The author `session` posts as, staged with their post**: the
/// principal's user, and, for a signed-in one, the handle and name the feed
/// shows for them.
fn author(principals: &Principals, session: &str, staged: &mut Vec<Change>) -> String {
    match principals.of(session) {
        Some(p) => {
            if let Some(profile) = profile_of(&p) {
                staged.push(Change::User(p.user.clone(), profile));
            }
            p.user
        }
        None => user_of(session),
    }
}

/// A post's id: `p` and how many were ever written, so never a deleted one's.
fn next_id(state: &State, staged: &[Change]) -> String {
    let posted = staged
        .iter()
        .filter(|c| matches!(c, Change::Post(_)))
        .count();
    format!("p{}", state.written + posted + 1)
}

impl crate::data::DataLayer for FeedData {
    fn reads(&self, _session: &str, _stopped: Option<Stopped>) -> crate::data::Ops {
        reads_of(
            Arc::new(self.state.lock().expect("feed").clone()),
            self.principals(),
        )
    }

    fn identified_by(&self, principals: Principals) {
        *self.principals.write().expect("principals") = principals;
    }

    fn begin<'a>(&'a self, _session: &str) -> Box<dyn crate::data::Staged + 'a> {
        let state = self.state.lock().expect("feed");
        let seen = Arc::new(state.clone());
        let staged: Arc<Mutex<Vec<Change>>> = Arc::default();
        let principals = self.principals();
        let mut ops = reads_of(seen.clone(), principals.clone());
        let (s, into, p) = (seen.clone(), staged.clone(), principals.clone());
        ops.insert(
            "feed:data/posts#publish".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(session), Val::String(text)] => {
                    let mut staged = into.lock().expect("staged");
                    let row = Row {
                        id: next_id(&s, &staged),
                        author: author(&p, session, &mut staged),
                        text: text.clone(),
                        likes: 0,
                        reply_to: None,
                    };
                    let answer = ok(post_val(&s, &row, false));
                    staged.push(Change::Post(row));
                    Ok(vec![answer])
                }
                other => Err(format!("posts#publish received {other:?}")),
            }),
        );
        // **A reply** (ADR-0231): a post that replies to `to`, shown in its
        // thread and in no timeline. To a post that is not there, none.
        let (s, into, p) = (seen.clone(), staged.clone(), principals.clone());
        ops.insert(
            "feed:data/posts#reply".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(session), Val::String(to), Val::String(text)] => {
                    if !s.posts.iter().any(|r| r.id == *to) {
                        return Ok(vec![not_found()]);
                    }
                    let mut staged = into.lock().expect("staged");
                    let row = Row {
                        id: next_id(&s, &staged),
                        author: author(&p, session, &mut staged),
                        text: text.clone(),
                        likes: 0,
                        reply_to: Some(to.clone()),
                    };
                    let answer = ok(post_val(&s, &row, false));
                    staged.push(Change::Post(row));
                    Ok(vec![answer])
                }
                other => Err(format!("posts#reply received {other:?}")),
            }),
        );
        // **A post deleted** (track `identity`, ADR-XXXX): it and every reply
        // under it. Who may is `requires OwnsPost(post)`'s, evaluated before
        // the command runs; a post that is not there is not found.
        let (s, into) = (seen.clone(), staged.clone());
        ops.insert(
            "feed:data/posts#delete".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(_), Val::String(id)] => {
                    if !s.posts.iter().any(|r| r.id == *id) {
                        return Ok(vec![not_found()]);
                    }
                    into.lock()
                        .expect("staged")
                        .push(Change::Delete(id.clone()));
                    Ok(vec![ok(Val::String(id.clone()))])
                }
                other => Err(format!("posts#delete received {other:?}")),
            }),
        );
        let (s, into) = (seen, staged.clone());
        ops.insert(
            "feed:data/posts#like".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(_), Val::String(id)] => match s.posts.iter().find(|r| r.id == *id) {
                    Some(row) => {
                        let mut liked = row.clone();
                        liked.likes += 1;
                        into.lock().expect("staged").push(Change::Like(id.clone()));
                        Ok(vec![ok(post_val(&s, &liked, false))])
                    }
                    None => Ok(vec![not_found()]),
                },
                other => Err(format!("posts#like received {other:?}")),
            }),
        );
        Box::new(Staging { state, staged, ops })
    }

    fn grants(&self) -> Vec<&'static str> {
        GRANTS.to_vec()
    }

    fn default_page(&self) -> Option<&'static str> {
        None
    }
}
