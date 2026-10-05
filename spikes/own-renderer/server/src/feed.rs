//! **The feed's data** (ADR-0218): the posts and users of the feed reference
//! app, a Twitter-shaped program (`examples/feed`), kept in memory.
//!
//! What its contracts import, `feed:data/posts#…` and `feed:data/users#…`,
//! is this. A query reads it; a command stages its writes here, and they are
//! the feed's once the command's transaction commits.

use super::*;

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
}

/// What a command changes, staged until it commits.
#[derive(Debug, Clone)]
enum Change {
    Post(Row),
    Like(String),
}

pub(crate) struct FeedData {
    state: Mutex<State>,
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
        FeedData {
            state: Mutex::new(state),
        }
    }
}

/// The user a session posts as: one of its own, named for it.
fn user_of(session: &str) -> String {
    format!("u-{session}")
}

fn user_val(state: &State, id: &str) -> Val {
    // A session's own user, which signed up as no one: a guest named for its
    // session, the same to every reader. Until ADR-0220 it was "You" to
    // every reader.
    let (handle, name) = state.users.get(id).cloned().unwrap_or_else(|| {
        let session = id.strip_prefix("u-").unwrap_or(id);
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
/// wrote it, what it says, and its likes.
fn item_val(state: &State, row: &Row) -> Val {
    Val::Record(vec![
        ("id".into(), Val::String(row.id.clone())),
        ("author".into(), user_val(state, &row.author)),
        ("text".into(), Val::String(row.text.clone())),
        ("likes".into(), Val::S64(row.likes)),
    ])
}

fn not_found() -> Val {
    Val::Result(Err(Some(Box::new(Val::Variant("not-found".into(), None)))))
}

fn ok(v: Val) -> Val {
    Val::Result(Ok(Some(Box::new(v))))
}

/// The reads of `state`: the timeline, a thread, a session's user.
fn reads_of(state: Arc<State>) -> crate::data::Ops {
    let mut ops: crate::data::Ops = BTreeMap::new();
    let s = state.clone();
    ops.insert(
        "feed:data/posts#timeline".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(_), Val::S64(limit)] => {
                // Each an `Item`, a post as a timeline shows it (ADR-0222):
                // its replies are its thread's.
                let shown: Vec<Val> = s
                    .posts
                    .iter()
                    .rev()
                    .filter(|r| r.reply_to.is_none())
                    .take((*limit).max(0) as usize)
                    .map(|r| item_val(&s, r))
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
    ops.insert(
        "feed:data/users#of-session".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(session)] => Ok(vec![Val::String(user_of(session))]),
            other => Err(format!("users#of-session received {other:?}")),
        }),
    );
    ops
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
                (self.state.posts.len() + staged.len()).to_string(),
            )]
        })
    }

    fn publish(&mut self) {
        for change in self.staged.lock().expect("staged").drain(..) {
            match change {
                Change::Post(row) => self.state.posts.push(row),
                Change::Like(id) => {
                    if let Some(row) = self.state.posts.iter_mut().find(|r| r.id == id) {
                        row.likes += 1;
                    }
                }
            }
        }
    }
}

impl crate::data::DataLayer for FeedData {
    fn reads(&self, _session: &str, _stopped: Option<Stopped>) -> crate::data::Ops {
        reads_of(Arc::new(self.state.lock().expect("feed").clone()))
    }

    fn begin<'a>(&'a self, _session: &str) -> Box<dyn crate::data::Staged + 'a> {
        let state = self.state.lock().expect("feed");
        let seen = Arc::new(state.clone());
        let staged: Arc<Mutex<Vec<Change>>> = Arc::default();
        let mut ops = reads_of(seen.clone());
        let (s, into) = (seen.clone(), staged.clone());
        ops.insert(
            "feed:data/posts#publish".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(session), Val::String(text)] => {
                    let mut staged = into.lock().expect("staged");
                    let row = Row {
                        id: format!("p{}", s.posts.len() + staged.len() + 1),
                        author: user_of(session),
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
        vec![
            "database.read<Post>",
            "database.write<Post>",
            "database.read<User>",
        ]
    }

    fn default_page(&self) -> Option<&'static str> {
        None
    }
}
