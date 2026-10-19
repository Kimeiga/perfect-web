//! A disk cache is shared by real readers and builders, not just one
//! Search's OnceLock. All inputs/cache roots below are private synthetic
//! files above the production 4 MiB threshold; no deployment data is read.

use super::*;
use std::os::unix::fs::MetadataExt;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

fn csv(root: &Path, label: &str) -> PathBuf {
    let mut text = format!(
        "word,language,definition,pronunciation,reading_search,is_common\n\
         {label},synthetic,needle,pron,read,1\n"
    );
    let filler = format!("filler,synthetic,{},pron,read,0\n", "pad".repeat(1024));
    for _ in 0..2048 {
        text.push_str(&filler);
    }
    assert!(text.len() as u64 > IN_MEMORY);
    let path = root.join(format!("{label}.csv"));
    std::fs::write(&path, text).expect("private synthetic CSV");
    path
}

fn words(index: &Index) -> Vec<String> {
    index
        .latin("needle", 10)
        .expect("the actual SQLite FTS query")
        .into_iter()
        .map(|row| row.word)
        .collect()
}

fn backing_file(index: &Index) -> PathBuf {
    let connection = index.conn.lock().expect("the SQLite connection");
    PathBuf::from(connection.path().expect("the cached database has a file"))
}

fn identity(path: &Path) -> (u64, u64) {
    let metadata = std::fs::metadata(path).expect("the published SQLite path remains live");
    (metadata.dev(), metadata.ino())
}

/// The owning test always resumes and joins its builder, including when an
/// assertion unwinds. No global environment or process-wide hook is used.
struct Held {
    path: PathBuf,
    release: Option<Sender<()>>,
    run: Option<JoinHandle<Result<Index, String>>>,
}

impl Held {
    fn at(csv: PathBuf, cache: PathBuf, phase: CachePhase) -> Held {
        let (ready, wait): (Sender<PathBuf>, Receiver<PathBuf>) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let run = thread::spawn(move || {
            Index::open_inner(&csv, &cache, |event, path| {
                if event == phase {
                    ready
                        .send(path.to_path_buf())
                        .expect("the owning test waits");
                    resume
                        .recv_timeout(WAIT)
                        .expect("the owning test resumes its builder");
                }
            })
        });
        let path = wait
            .recv_timeout(WAIT)
            .expect("the real SQLite builder reaches its barrier");
        Held {
            path,
            release: Some(release),
            run: Some(run),
        }
    }

    fn finish(mut self) -> Result<Index, String> {
        self.release
            .take()
            .unwrap()
            .send(())
            .expect("the builder is waiting");
        self.run
            .take()
            .unwrap()
            .join()
            .expect("the builder did not panic")
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(run) = self.run.take() {
            let _ = run.join();
        }
    }
}

#[test]
fn builders_of_one_digest_do_not_unlink_an_open_sqlite() {
    let root = tempfile::tempdir().unwrap();
    let source = csv(root.path(), "alpha");
    let cache = root.path().join("cache");
    let a = Held::at(source.clone(), cache.clone(), CachePhase::Opened);
    let b = Index::open(&source, &cache);
    let a = a.finish();
    assert!(
        a.is_ok(),
        "first real SQLite builder: {}",
        a.as_ref().err().unwrap()
    );
    assert!(
        b.is_ok(),
        "second real SQLite builder: {}",
        b.as_ref().err().unwrap()
    );
    assert_eq!(words(&a.unwrap()), ["alpha"]);
    assert_eq!(words(&b.unwrap()), ["alpha"]);
}

#[test]
fn a_delayed_builder_reuses_the_same_digest_winners_file() {
    let root = tempfile::tempdir().unwrap();
    let source = csv(root.path(), "alpha");
    let cache = root.path().join("cache");
    let a = Held::at(source.clone(), cache.clone(), CachePhase::Ready);
    let b = Index::open(&source, &cache).expect("the second SQLite builder publishes");
    let file = backing_file(&b);
    let winner = identity(&file);
    let a = a
        .finish()
        .expect("the delayed builder reuses the published winner");
    assert_eq!(identity(&backing_file(&a)), winner);
    assert_eq!(
        identity(&file),
        winner,
        "publishing never replaces the winner"
    );
    assert_eq!(words(&a), ["alpha"]);
    assert_eq!(words(&b), ["alpha"]);
}

#[test]
fn publishing_another_digest_does_not_unlink_a_live_builder() {
    let root = tempfile::tempdir().unwrap();
    let alpha = csv(root.path(), "alpha");
    let beta = csv(root.path(), "beta");
    let cache = root.path().join("cache");
    let a = Held::at(alpha, cache.clone(), CachePhase::Opened);
    let b = Index::open(&beta, &cache);
    let a = a.finish();
    assert!(
        a.is_ok(),
        "other digest's real SQLite builder: {}",
        a.as_ref().err().unwrap()
    );
    assert!(
        b.is_ok(),
        "new digest's real SQLite builder: {}",
        b.as_ref().err().unwrap()
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(words(&a), ["alpha"]);
    assert_eq!(words(&b), ["beta"]);
    assert!(backing_file(&a).exists() && backing_file(&b).exists());
}

#[test]
fn an_open_reader_keeps_its_path_and_identity_after_another_version_publishes() {
    let root = tempfile::tempdir().unwrap();
    let alpha = csv(root.path(), "alpha");
    let beta = csv(root.path(), "beta");
    let cache = root.path().join("cache");
    let a = Index::open(&alpha, &cache).unwrap();
    let file = backing_file(&a);
    let before = identity(&file);
    assert_eq!(words(&a), ["alpha"]);
    let b = Index::open(&beta, &cache).unwrap();
    // A read may happen to work from an unlinked inode on Unix. Its path
    // must still name that same DB, so another opener never recreates it.
    assert_eq!(words(&a), ["alpha"]);
    assert_eq!(identity(&file), before);
    let again = Index::open(&alpha, &cache).unwrap();
    assert_eq!(identity(&backing_file(&again)), before);
    assert_eq!(words(&again), ["alpha"]);
    assert_eq!(words(&b), ["beta"]);
}

#[test]
fn a_failed_builder_cleans_itself_without_touching_another_builders_stage() {
    let root = tempfile::tempdir().unwrap();
    let alpha = csv(root.path(), "alpha");
    let broken = csv(root.path(), "broken");
    let text = std::fs::read_to_string(&broken)
        .unwrap()
        .replacen("word,", "unknown,", 1);
    std::fs::write(&broken, text).unwrap();
    let cache = root.path().join("cache");
    let a = Held::at(alpha, cache.clone(), CachePhase::Opened);
    let held_path = a.path.clone();
    let mut before: Vec<PathBuf> = std::fs::read_dir(&cache)
        .unwrap()
        .map(|x| x.unwrap().path())
        .collect();
    before.sort();
    let failed = Index::open(&broken, &cache);
    assert!(
        failed.is_err(),
        "malformed private input must fail to build"
    );
    assert!(
        held_path.exists(),
        "the other builder's SQLite stays linked"
    );
    let mut after: Vec<PathBuf> = std::fs::read_dir(&cache)
        .unwrap()
        .map(|x| x.unwrap().path())
        .collect();
    after.sort();
    assert_eq!(
        after, before,
        "a failed builder leaves none of its own files"
    );
    let a = a
        .finish()
        .expect("the independent builder can still publish");
    assert_eq!(words(&a), ["alpha"]);
}

#[test]
fn an_incomplete_concurrent_winner_is_refused_without_replacement() {
    let root = tempfile::tempdir().unwrap();
    let source = csv(root.path(), "alpha");
    let cache = root.path().join("cache");
    let a = Held::at(source.clone(), cache.clone(), CachePhase::Ready);
    let directory = cache_of(&cache, &fingerprint(&std::fs::read(&source).unwrap()));
    std::fs::create_dir(&directory).unwrap();
    let file = directory.join("index.db");
    let unfinished = Connection::open(&file).unwrap();
    unfinished
        .execute_batch("CREATE TABLE unfinished (n INT)")
        .unwrap();
    drop(unfinished);
    let before = identity(&file);
    let result = a.finish();
    assert!(
        result.is_err(),
        "a loser must validate the actual SQLite winner"
    );
    assert_eq!(
        identity(&file),
        before,
        "invalid winners are never replaced"
    );
    let entries: Vec<_> = std::fs::read_dir(&cache).unwrap().collect();
    assert_eq!(entries.len(), 1, "the losing private stage was cleaned");
}

#[test]
fn a_cached_wrong_fingerprint_is_refused_without_deleting_its_file() {
    let root = tempfile::tempdir().unwrap();
    let source = csv(root.path(), "alpha");
    let cache = root.path().join("cache");
    let index = Index::open(&source, &cache).unwrap();
    let file = backing_file(&index);
    drop(index);
    let before = identity(&file);
    let conn = Connection::open(&file).unwrap();
    conn.execute("UPDATE kiokun_cache SET fingerprint = 'another input'", [])
        .unwrap();
    drop(conn);
    let result = Index::open(&source, &cache);
    assert!(
        result.is_err(),
        "a stored fingerprint must match the actual CSV/schema"
    );
    assert_eq!(identity(&file), before);
}

#[test]
fn a_cached_wrong_fts_schema_is_refused_without_deleting_its_file() {
    let root = tempfile::tempdir().unwrap();
    let source = csv(root.path(), "alpha");
    let bytes = std::fs::read(&source).unwrap();
    let cache = root.path().join("cache");
    let directory = cache_of(&cache, &fingerprint(&bytes));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("index.db");
    let conn = Connection::open(&file).unwrap();
    conn.execute_batch("CREATE TABLE dictionary_search (word TEXT); CREATE TABLE kiokun_cache (fingerprint TEXT NOT NULL)").unwrap();
    conn.execute(
        "INSERT INTO kiokun_cache VALUES (?1)",
        [fingerprint(&bytes)],
    )
    .unwrap();
    drop(conn);
    let before = identity(&file);
    let result = Index::open(&source, &cache);
    assert!(
        result.is_err(),
        "a matching identity cannot stand in for the actual FTS schema"
    );
    assert_eq!(identity(&file), before);
}

#[test]
fn private_abandoned_stages_and_old_flat_sqlite_files_remain_untouched() {
    let root = tempfile::tempdir().unwrap();
    let source = csv(root.path(), "alpha");
    let cache = root.path().join("cache");
    let stage = cache.join("kiokun-search-building-interrupted");
    std::fs::create_dir_all(&stage).unwrap();
    let abandoned = stage.join("index.db");
    let mut abandoned_conn = Connection::open(&abandoned).unwrap();
    fill(&mut abandoned_conn, "word,definition\nprivate,needle\n").unwrap();
    let legacy = cache.join("kiokun-search-old.db");
    let mut legacy_conn = Connection::open(&legacy).unwrap();
    fill(&mut legacy_conn, "word,definition\nlegacy,needle\n").unwrap();
    let before = (identity(&abandoned), identity(&legacy));
    let index = Index::open(&source, &cache).unwrap();
    assert_eq!(words(&index), ["alpha"]);
    assert_eq!((identity(&abandoned), identity(&legacy)), before);
    let count: i64 = legacy_conn
        .query_row(
            "SELECT count(*) FROM dictionary_search WHERE dictionary_search MATCH 'needle'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 1,
        "the legacy SQLite remains usable under its original name"
    );
    let count: i64 = abandoned_conn
        .query_row(
            "SELECT count(*) FROM dictionary_search WHERE dictionary_search MATCH 'needle'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 1,
        "unowned stages are ignored even while SQLite is open"
    );
}

#[test]
fn a_small_index_stays_in_memory_and_a_published_index_is_read_only() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("small.csv");
    std::fs::write(&source, "word,definition\nsmall,needle\n").unwrap();
    let cache = root.path().join("cache");
    let small = Index::open(&source, &cache).unwrap();
    assert_eq!(words(&small), ["small"]);
    assert!(
        !cache.exists(),
        "small inputs create no disk-cache artifacts"
    );
    let source = csv(root.path(), "alpha");
    let disk = Index::open(&source, &cache).unwrap();
    let connection = disk.conn.lock().unwrap();
    let write = connection.execute(
        "INSERT INTO dictionary_search (word) VALUES ('forbidden')",
        [],
    );
    assert!(
        write.is_err(),
        "published SQLite connections must be read only"
    );
}
