//! **TodoMVC's data** (track `todomvc`, docs/PARALLEL.md, W7): a session's
//! todos, kept in memory, for `examples/todomvc`.
//!
//! What its contracts import, `todomvc:data/todos#…`, is this. A query reads
//! it; a command stages its writes here, and they are the session's once the
//! command's transaction commits.
//!
//! **Whose todos these are is the session's, and the layer holds a command
//! to it**: every operation names the session it acts for, and one that
//! names any session but the command's own is refused, so a program (or a
//! host bug) cannot read or write another session's list.
//!
//! **This is the framework's own persistence, in the place of the spec's
//! `localStorage`** (TodoMVC's `app-spec.md` allows it: "if the framework has
//! capabilities for persisting data, use that"). It is the session's, not the
//! device's: the todos outlive a reload and not a restart of this host, and a
//! device that clears its cookies has a new session and an empty list. The
//! version on a device's own storage waits for charter §9.1's fourth kind of
//! state.
//!
//! **Bounded**, as every layer of a host that guests write to must be: a
//! session holds at most [`MAX_TODOS`] todos (a further one is `full`), and
//! the program's `Title` holds a title to its length before it reaches here.

use super::*;

/// What a node grants the todos' data, wherever it is kept.
pub(crate) const GRANTS: &[&str] = &["database.read<Todo>", "database.write<Todo>"];

/// The most todos one session holds. TodoMVC's spec states none; a list a
/// person reads and edits by hand has no need of more, and a host that
/// guests write to needs one.
pub(crate) const MAX_TODOS: usize = 500;

/// One todo as kept: its id, its title as the program's `Title` held it,
/// and whether it is completed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    id: String,
    title: String,
    completed: bool,
}

/// One session's todos, oldest first, and how many it ever wrote, so that
/// an id is never one a destroyed todo had.
#[derive(Debug, Clone, Default)]
struct List {
    rows: Vec<Row>,
    written: usize,
}

#[derive(Debug, Default)]
struct State {
    sessions: BTreeMap<String, List>,
    /// Commits, which the materializer's row for the todos counts.
    commits: usize,
}

/// What a command changes, staged until it commits.
#[derive(Debug, Clone)]
enum Change {
    Add(Row),
    Complete(String, bool),
    CompleteAll(bool),
    Retitle(String, String),
    Remove(String),
    RemoveCompleted,
}

pub(crate) struct TodoData {
    state: Mutex<State>,
}

impl TodoData {
    pub(crate) fn new() -> TodoData {
        TodoData {
            state: Mutex::new(State::default()),
        }
    }
}

/// A todo as the program's `Todo` is.
fn todo_val(row: &Row) -> Val {
    Val::Record(vec![
        ("id".into(), Val::String(row.id.clone())),
        ("title".into(), Val::String(row.title.clone())),
        ("completed".into(), Val::Bool(row.completed)),
    ])
}

fn ok(v: Val) -> Val {
    Val::Result(Ok(Some(Box::new(v))))
}

/// The program's `TodoError`, a case with no payload.
fn refused(case: &str) -> Val {
    Val::Result(Err(Some(Box::new(Val::Variant(case.into(), None)))))
}

/// The session an operation acts for, which must be the command's or the
/// query's own.
fn own<'a>(called: &'a str, session: &'a str, op: &str) -> Result<&'a str, String> {
    if called == session {
        Ok(called)
    } else {
        Err(format!(
            "{op} acts for another session than the one it runs for"
        ))
    }
}

/// The reads of one session's list: all its todos, oldest first.
fn reads_of(session: &str, list: Arc<List>) -> crate::data::Ops {
    let mut ops: crate::data::Ops = BTreeMap::new();
    let session = session.to_string();
    ops.insert(
        "todomvc:data/todos#all".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(called)] => {
                own(called, &session, "todos#all")?;
                Ok(vec![Val::List(list.rows.iter().map(todo_val).collect())])
            }
            other => Err(format!("todos#all received {other:?}")),
        }),
    );
    ops
}

/// **A command's writes to a session's todos, staged**: under the todos'
/// lock from the call to the commit, as the feed's are.
pub(crate) struct Staging<'a> {
    state: std::sync::MutexGuard<'a, State>,
    session: String,
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
                "todomvc:todos".to_string(),
                (self.state.commits + 1).to_string(),
            )]
        })
    }

    fn publish(&mut self) {
        let staged: Vec<Change> = self.staged.lock().expect("staged").drain(..).collect();
        if staged.is_empty() {
            return;
        }
        self.state.commits += 1;
        let list = self.state.sessions.entry(self.session.clone()).or_default();
        for change in staged {
            match change {
                Change::Add(row) => {
                    list.written += 1;
                    list.rows.push(row);
                }
                Change::Complete(id, done) => {
                    if let Some(row) = list.rows.iter_mut().find(|r| r.id == id) {
                        row.completed = done;
                    }
                }
                Change::CompleteAll(done) => {
                    for row in &mut list.rows {
                        row.completed = done;
                    }
                }
                Change::Retitle(id, title) => {
                    if let Some(row) = list.rows.iter_mut().find(|r| r.id == id) {
                        row.title = title;
                    }
                }
                Change::Remove(id) => list.rows.retain(|r| r.id != id),
                Change::RemoveCompleted => list.rows.retain(|r| !r.completed),
            }
        }
    }
}

impl crate::data::DataLayer for TodoData {
    fn reads(&self, session: &str, _stopped: Option<Stopped>) -> crate::data::Ops {
        let list = self
            .state
            .lock()
            .expect("todos")
            .sessions
            .get(session)
            .cloned()
            .unwrap_or_default();
        reads_of(session, Arc::new(list))
    }

    fn begin<'a>(&'a self, session: &str) -> Box<dyn crate::data::Staged + 'a> {
        let state = self.state.lock().expect("todos");
        let seen = Arc::new(state.sessions.get(session).cloned().unwrap_or_default());
        let staged: Arc<Mutex<Vec<Change>>> = Arc::default();
        let mut ops = reads_of(session, seen.clone());
        let me = session.to_string();

        // **A todo, added**: a further one past the most a session holds is
        // `full`. Its id is `t` and how many the session ever wrote, so
        // never a destroyed todo's.
        let (s, into, mine) = (seen.clone(), staged.clone(), me.clone());
        ops.insert(
            "todomvc:data/todos#add".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(called), Val::String(title)] => {
                    own(called, &mine, "todos#add")?;
                    let mut staged = into.lock().expect("staged");
                    let added = staged
                        .iter()
                        .filter(|c| matches!(c, Change::Add(_)))
                        .count();
                    if s.rows.len() + added >= MAX_TODOS {
                        return Ok(vec![refused("full")]);
                    }
                    let row = Row {
                        id: format!("t{}", s.written + added + 1),
                        title: title.clone(),
                        completed: false,
                    };
                    let answer = ok(todo_val(&row));
                    staged.push(Change::Add(row));
                    Ok(vec![answer])
                }
                other => Err(format!("todos#add received {other:?}")),
            }),
        );

        // **One todo completed, or not**: one that is not there is not found.
        let (s, into, mine) = (seen.clone(), staged.clone(), me.clone());
        ops.insert(
            "todomvc:data/todos#complete".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(called), Val::String(id), Val::Bool(done)] => {
                    own(called, &mine, "todos#complete")?;
                    let Some(row) = s.rows.iter().find(|r| r.id == *id) else {
                        return Ok(vec![refused("not-found")]);
                    };
                    let answer = ok(todo_val(&Row {
                        completed: *done,
                        ..row.clone()
                    }));
                    into.lock()
                        .expect("staged")
                        .push(Change::Complete(id.clone(), *done));
                    Ok(vec![answer])
                }
                other => Err(format!("todos#complete received {other:?}")),
            }),
        );

        // **Every todo completed, or not**: how many it changed.
        let (s, into, mine) = (seen.clone(), staged.clone(), me.clone());
        ops.insert(
            "todomvc:data/todos#complete-all".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(called), Val::Bool(done)] => {
                    own(called, &mine, "todos#complete-all")?;
                    let changed = s.rows.iter().filter(|r| r.completed != *done).count();
                    into.lock()
                        .expect("staged")
                        .push(Change::CompleteAll(*done));
                    Ok(vec![Val::S64(changed as i64)])
                }
                other => Err(format!("todos#complete-all received {other:?}")),
            }),
        );

        // **A todo's title changed**: one that is not there is not found.
        let (s, into, mine) = (seen.clone(), staged.clone(), me.clone());
        ops.insert(
            "todomvc:data/todos#retitle".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(called), Val::String(id), Val::String(title)] => {
                    own(called, &mine, "todos#retitle")?;
                    let Some(row) = s.rows.iter().find(|r| r.id == *id) else {
                        return Ok(vec![refused("not-found")]);
                    };
                    let answer = ok(todo_val(&Row {
                        title: title.clone(),
                        ..row.clone()
                    }));
                    into.lock()
                        .expect("staged")
                        .push(Change::Retitle(id.clone(), title.clone()));
                    Ok(vec![answer])
                }
                other => Err(format!("todos#retitle received {other:?}")),
            }),
        );

        // **A todo destroyed**: one that is not there is not found.
        let (s, into, mine) = (seen.clone(), staged.clone(), me.clone());
        ops.insert(
            "todomvc:data/todos#remove".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(called), Val::String(id)] => {
                    own(called, &mine, "todos#remove")?;
                    if !s.rows.iter().any(|r| r.id == *id) {
                        return Ok(vec![refused("not-found")]);
                    }
                    into.lock()
                        .expect("staged")
                        .push(Change::Remove(id.clone()));
                    Ok(vec![ok(Val::String(id.clone()))])
                }
                other => Err(format!("todos#remove received {other:?}")),
            }),
        );

        // **The completed todos destroyed**: how many went.
        let (s, into, mine) = (seen, staged.clone(), me);
        ops.insert(
            "todomvc:data/todos#remove-completed".to_string(),
            Arc::new(move |args: &[Val]| match args {
                [Val::String(called)] => {
                    own(called, &mine, "todos#remove-completed")?;
                    let gone = s.rows.iter().filter(|r| r.completed).count();
                    into.lock().expect("staged").push(Change::RemoveCompleted);
                    Ok(vec![Val::S64(gone as i64)])
                }
                other => Err(format!("todos#remove-completed received {other:?}")),
            }),
        );

        Box::new(Staging {
            state,
            session: session.to_string(),
            staged,
            ops,
        })
    }

    fn grants(&self) -> Vec<&'static str> {
        GRANTS.to_vec()
    }

    fn default_page(&self) -> Option<&'static str> {
        None
    }
}
