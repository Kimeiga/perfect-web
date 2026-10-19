//! **A read as of an epoch** (`Resources::fetch_cancellable_as_of`; NEXT 4c,
//! track `render-merge`): a reader of a snapshot taken after `epoch()` reads
//! as any reader does, from the cache or a request running, where nothing
//! invalidated its resource since; where something did, its snapshot may be
//! older than the change, and it reads alone: from no cache, joining no
//! request and joined by none, its value not kept. So a value read in a
//! snapshot older than an invalidation is never kept or handed to another
//! reader after it. Channels order the work; the wall-clock deadlines only
//! detect a stuck test.
use pw_resource::*;
use std::sync::mpsc;
use std::time::Duration;

fn runtime() -> (Resources, Manifest, Key) {
    (
        Resources::new(Clock::new()),
        Manifest::new("items"),
        Key::new("items", "1"),
    )
}

const WAIT: Duration = Duration::from_secs(5);

/// The four ways a resource's values are invalidated, each by its name.
type Invalidation = (&'static str, fn(&Resources, &Key));

const INVALIDATIONS: [Invalidation; 4] = [
    ("invalidate_key", |rt, key| rt.invalidate_key(key)),
    ("invalidate_where", |rt, key| {
        rt.invalidate_where(&key.resource, |k| k == key.key);
    }),
    ("invalidate", |rt, key| rt.invalidate(&key.resource)),
    ("expire", |rt, key| rt.expire(&key.resource)),
];

/// **Where nothing invalidated its resource since its epoch, a read as of it
/// is an ordinary one**: it is served what is kept, and joins a request
/// running.
#[test]
fn a_read_as_of_an_epoch_nothing_invalidated_since_is_an_ordinary_one() {
    let (rt, manifest, key) = runtime();
    let as_of = rt.epoch();
    assert_eq!(
        rt.fetch(&manifest, &key, |_| Ok("kept".into())),
        Fetched::Fresh("kept".into())
    );
    assert_eq!(
        rt.fetch_cancellable_as_of(&manifest, &key, as_of, |_, _| panic!("read again")),
        Fetched::FromCache("kept".into()),
        "what is kept"
    );
    let other = Key::new("items", "2");
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel::<()>();
    let (first, second) = std::thread::scope(|scope| {
        let (rt, manifest, other) = (&rt, &manifest, &other);
        let first = scope.spawn(move || {
            rt.fetch(manifest, other, |_| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok("the request's".into())
            })
        });
        let begun = ready.recv_timeout(WAIT);
        let second = scope.spawn(move || {
            rt.fetch_cancellable_as_of(manifest, other, as_of, |_, _| Ok("a second load".into()))
        });
        let deadline = std::time::Instant::now() + WAIT;
        while begun.is_ok()
            && !rt
                .trace()
                .iter()
                .any(|t| matches!(t, Trace::RequestDeduplicated { .. }))
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }
        release.send(()).unwrap();
        begun.expect("the request began");
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_eq!(first, Fetched::Fresh("the request's".into()));
    assert_eq!(
        second,
        Fetched::Deduplicated("the request's".into()),
        "joined"
    );
}

/// **Where its resource was invalidated since its epoch, a read as of it
/// reads alone**, by each of the four invalidations, nothing kept when it
/// came: it is not served what a reader after the invalidation kept, and
/// what it reads is not kept over it.
#[test]
fn a_read_as_of_an_epoch_its_resource_was_invalidated_since_reads_alone() {
    for (how, invalidated) in INVALIDATIONS {
        let (rt, manifest, key) = runtime();
        let as_of = rt.epoch();
        invalidated(&rt, &key);
        assert_eq!(
            rt.fetch(&manifest, &key, |_| Ok("after".into())),
            Fetched::Fresh("after".into()),
            "{how}: a reader after it"
        );
        assert_eq!(
            rt.fetch_cancellable_as_of(&manifest, &key, as_of, |_, _| Ok("mine".into())),
            Fetched::Fresh("mine".into()),
            "{how}: read alone, not served what is kept"
        );
        assert_eq!(
            rt.fetch(&manifest, &key, |_| panic!("kept")),
            Fetched::FromCache("after".into()),
            "{how}: what is kept is not the read alone's"
        );
        assert!(
            rt.trace()
                .iter()
                .any(|t| matches!(t, Trace::RequestAlone { .. })),
            "{how}"
        );
    }
}

/// **A read alone neither joins a request nor is joined, and is not kept**:
/// while it reads, the key has no request of its own, so a reader after it
/// reads for itself and keeps its value; the read alone answers its own.
#[test]
fn a_read_alone_neither_joins_nor_is_joined_nor_kept() {
    let (rt, manifest, key) = runtime();
    let as_of = rt.epoch();
    rt.invalidate_key(&key);
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel::<()>();
    let (alone, ordinary, registered) = std::thread::scope(|scope| {
        let (rt, manifest, key) = (&rt, &manifest, &key);
        let alone = scope.spawn(move || {
            rt.fetch_cancellable_as_of(manifest, key, as_of, |_, _| {
                started.send(()).unwrap();
                resume.recv().unwrap();
                Ok("mine".into())
            })
        });
        let begun = ready.recv_timeout(WAIT);
        let registered = rt.in_flight(key);
        let (tx, rx) = mpsc::channel();
        scope.spawn(move || tx.send(rt.fetch(manifest, key, |_| Ok("ordinary".into()))));
        let ordinary = rx.recv_timeout(WAIT);
        release.send(()).unwrap();
        begun.expect("the read alone began");
        (alone.join().unwrap(), ordinary, registered)
    });
    assert!(!registered, "a read alone is not the key's request");
    assert_eq!(
        ordinary,
        Ok(Fetched::Fresh("ordinary".into())),
        "a reader after it reads for itself, not waiting"
    );
    assert_eq!(alone, Fetched::Fresh("mine".into()));
    assert_eq!(
        rt.fetch(&manifest, &key, |_| panic!("kept")),
        Fetched::FromCache("ordinary".into()),
        "the read alone's value is not kept"
    );
}

/// **The controls**: a read as of an epoch taken after its resource's last
/// invalidation, and one whose epoch only another resource's invalidation
/// passed, are ordinary ones, served what is kept.
#[test]
fn a_read_as_of_an_epoch_after_its_resources_last_invalidation_is_an_ordinary_one() {
    let (rt, manifest, key) = runtime();
    rt.invalidate_key(&key);
    let as_of = rt.epoch();
    rt.invalidate(&Key::new("others", "1").resource);
    assert_eq!(
        rt.fetch(&manifest, &key, |_| Ok("kept".into())),
        Fetched::Fresh("kept".into())
    );
    assert_eq!(
        rt.fetch_cancellable_as_of(&manifest, &key, as_of, |_, _| panic!("read alone")),
        Fetched::FromCache("kept".into())
    );
}
