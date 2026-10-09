//! **kiokun's dictionary, read** (track `kiokun`, W6): the data layer the
//! development server gives `examples/kiokun-site`, chosen because its
//! contracts import `kiokun:data/…` (docs/PARALLEL.md, "W6's inventory,
//! answered", Q2).
//!
//! It reads kiokun's build output as the build wrote it: one raw-DEFLATE
//! JSON file a word, at `<subdirectory>/<file name>.json.deflate`. Where the
//! file is, and whether it is the word's, the program decides (`shards`,
//! `kiokun.site`); this reads the one file it names and hands back the
//! entry's fields as the program's `Entry` declares them, each list as the
//! file lists it. What a page shows of them is the program's.
//!
//! The files are `KIOKUN_DATA`, a kiokun-data checkout's `output_dictionary`
//! (read-only: nothing here writes), or else the sample the repository
//! carries, `examples/kiokun/data/han-1char-3` (ADR-0037).
//!
//! It is read-only: it stages nothing, and a program that names a write is
//! refused at start, since no operation of this layer's is one.

use super::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// What a node grants the program: reading entries, and nothing else.
pub(crate) const GRANTS: &[&str] = &["database.read<Entry>"];

/// The sample the repository carries (ADR-0037): 28 files of one shard.
fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/kiokun/data/han-1char-3")
}

pub(crate) struct KiokunData {
    root: PathBuf,
}

impl KiokunData {
    /// The files `KIOKUN_DATA` names, or the repository's sample.
    pub(crate) fn new() -> KiokunData {
        let root = std::env::var_os("KIOKUN_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(sample);
        KiokunData { root }
    }

    #[cfg(test)]
    pub(crate) fn at(root: PathBuf) -> KiokunData {
        KiokunData { root }
    }
}

/// The longest file name, in bytes, on macOS (APFS) and Linux (ext4).
const NAME_MAX: usize = 255;

/// **Is `subdirectory/file` one of kiokun's files, and nothing else?** Two
/// lowercase hexadecimal digits, as the shard rule writes them, and one path
/// segment: kiokun's file names escape every separator
/// (`create_safe_filename`), so a name that holds one, or is `.` or `..`,
/// names no file of kiokun's and could name one outside its directory.
pub(crate) fn place(subdirectory: &str, file: &str) -> Option<PathBuf> {
    let hex = subdirectory.len() == 2
        && subdirectory
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let segment =
        !file.is_empty() && file != "." && file != ".." && !file.contains(['/', '\\', '\0']);
    let name = format!("{file}.json.deflate");
    // A file name is at most `NAME_MAX` bytes, 255 on macOS and Linux:
    // kiokun's build wrote none longer, and reading one is the file system's
    // refusal (ENAMETOOLONG), not a word kiokun lacks.
    let fits = name.len() <= NAME_MAX;
    (hex && segment && fits).then(|| PathBuf::from(subdirectory).join(name))
}

fn text(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string()
}

fn list<'a>(v: &'a serde_json::Value, key: &str) -> impl Iterator<Item = &'a serde_json::Value> {
    v.get(key).and_then(|l| l.as_array()).into_iter().flatten()
}

/// A list of strings, as the file lists them.
fn strings(v: &serde_json::Value, key: &str) -> Val {
    Val::List(
        list(v, key)
            .filter_map(|s| s.as_str())
            .map(|s| Val::String(s.to_string()))
            .collect(),
    )
}

/// Each item's `field`, as the file lists them.
fn texts(v: &serde_json::Value, key: &str, field: &str) -> Val {
    Val::List(list(v, key).map(|d| Val::String(text(d, field))).collect())
}

fn record(fields: Vec<(&str, Val)>) -> Val {
    Val::Record(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

/// **Each record's id, once in its list.** A page keys a list by it, and a
/// keyed list refuses two items with one key; kiokun's build can list one
/// record twice (JMnedict's name 5345360 is in 釋 under both 釈 and 釋,
/// ADR-0037). Every record is kept, as kiokun.com shows each, and a repeated
/// id is `<id>:<n>`, its `n`th repeat.
fn keyed<'a>(
    records: impl Iterator<Item = &'a serde_json::Value>,
    field: &'a str,
) -> impl Iterator<Item = (String, &'a serde_json::Value)> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    records.map(move |r| {
        let id = text(r, field);
        let n = seen.entry(id.clone()).or_default();
        let key = if *n == 0 {
            id.clone()
        } else {
            format!("{id}:{n}")
        };
        *n += 1;
        (key, r)
    })
}

/// A Japanese word's `kanji` or `kana`: `Headword { text, tags, common }`.
fn headwords(word: &serde_json::Value, key: &str) -> Val {
    Val::List(
        list(word, key)
            .map(|h| {
                record(vec![
                    ("text", Val::String(text(h, "text"))),
                    ("tags", strings(h, "tags")),
                    (
                        "common",
                        Val::Bool(h.get("common").and_then(|c| c.as_bool()) == Some(true)),
                    ),
                ])
            })
            .collect(),
    )
}

/// A sense: `Sense { glosses, info, pos, field, misc, dialect }`.
fn sense(s: &serde_json::Value) -> Val {
    record(vec![
        (
            "glosses",
            Val::List(
                list(s, "gloss")
                    .map(|g| {
                        record(vec![
                            ("text", Val::String(text(g, "text"))),
                            ("kind", Val::String(text(g, "type"))),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("info", strings(s, "info")),
        ("pos", strings(s, "partOfSpeech")),
        ("field", strings(s, "field")),
        ("misc", strings(s, "misc")),
        ("dialect", strings(s, "dialect")),
    ])
}

/// **kiokun's entry, as the program's `Entry`**, its fields in the
/// declaration's order: `key`, `redirect`, `chinese`, `japanese`, `korean`,
/// `names`.
pub(crate) fn entry(json: &serde_json::Value) -> Val {
    let chinese = keyed(list(json, "chinese_words"), "_id")
        .map(|(id, w)| {
            record(vec![
                ("id", Val::String(id)),
                ("simp", Val::String(text(w, "simp"))),
                ("trad", Val::String(text(w, "trad"))),
                (
                    "items",
                    Val::List(
                        list(w, "items")
                            .map(|i| {
                                record(vec![
                                    ("pinyin", Val::String(text(i, "pinyin"))),
                                    ("jyutping", Val::String(text(i, "jyutping"))),
                                    ("definitions", strings(i, "definitions")),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();
    let japanese = keyed(list(json, "japanese_words"), "id")
        .map(|(id, w)| {
            record(vec![
                ("id", Val::String(id)),
                ("kanji", headwords(w, "kanji")),
                ("kana", headwords(w, "kana")),
                ("senses", Val::List(list(w, "sense").map(sense).collect())),
            ])
        })
        .collect();
    let korean = keyed(list(json, "korean_words"), "id")
        .map(|(id, w)| {
            record(vec![
                ("id", Val::String(id)),
                ("hangul", Val::String(text(w, "hangul"))),
                ("hanja", Val::String(text(w, "hanja"))),
                ("pos", Val::String(text(w, "pos"))),
                ("definitions", texts(w, "definitions", "text")),
            ])
        })
        .collect();
    let names = keyed(list(json, "japanese_names"), "id")
        .map(|(id, n)| {
            record(vec![
                ("id", Val::String(id)),
                ("kanji", texts(n, "kanji", "text")),
                ("kana", texts(n, "kana", "text")),
                (
                    "translations",
                    Val::List(
                        list(n, "translation")
                            .map(|t| {
                                record(vec![
                                    ("kinds", strings(t, "type")),
                                    ("texts", texts(t, "translation", "text")),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();
    record(vec![
        ("key", Val::String(text(json, "key"))),
        (
            "redirect",
            Val::Option(
                json.get("redirect")
                    .and_then(|r| r.as_str())
                    .map(|r| Box::new(Val::String(r.to_string()))),
            ),
        ),
        ("chinese", Val::List(chinese)),
        ("japanese", Val::List(japanese)),
        ("korean", Val::List(korean)),
        ("names", Val::List(names)),
    ])
}

/// **The file `subdirectory/file` names, read**: its JSON, or `None` where
/// there is no such file. An error is a file that is not kiokun's: not raw
/// DEFLATE, or not JSON.
pub(crate) fn read(
    root: &Path,
    subdirectory: &str,
    file: &str,
) -> Result<Option<serde_json::Value>, String> {
    let Some(rel) = place(subdirectory, file) else {
        return Ok(None);
    };
    let raw = match std::fs::read(root.join(&rel)) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", rel.display())),
    };
    let json = miniz_oxide::inflate::decompress_to_vec(&raw)
        .map_err(|e| format!("{}: not raw DEFLATE: {e:?}", rel.display()))?;
    serde_json::from_slice(&json)
        .map(Some)
        .map_err(|e| format!("{}: not JSON: {e}", rel.display()))
}

fn reads_of(root: PathBuf) -> data::Ops {
    let mut ops: data::Ops = BTreeMap::new();
    ops.insert(
        "kiokun:data/entries#read".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::String(subdirectory), Val::String(file)] => {
                let found = read(&root, subdirectory, file)?;
                Ok(vec![Val::Option(found.map(|j| Box::new(entry(&j))))])
            }
            other => Err(format!("entries#read received {other:?}")),
        }),
    );
    ops
}

/// A command's staging, which stages nothing: kiokun's files are read.
struct Nothing(data::Ops);

impl data::Staged for Nothing {
    fn ops(&self) -> data::Ops {
        self.0.clone()
    }
    fn rows(&self) -> Option<Vec<(String, String)>> {
        None
    }
    fn publish(&mut self) {}
}

impl data::DataLayer for KiokunData {
    fn reads(&self, _session: &str, _stopped: Option<Stopped>) -> data::Ops {
        reads_of(self.root.clone())
    }

    fn begin<'a>(&'a self, _session: &str) -> Box<dyn data::Staged + 'a> {
        Box::new(Nothing(reads_of(self.root.clone())))
    }

    fn grants(&self) -> Vec<&'static str> {
        GRANTS.to_vec()
    }

    fn default_page(&self) -> Option<&'static str> {
        None
    }

    /// **What reading kiokun's files gives** (ADR-0207): every read sees the
    /// one version the deployment opened, so reads are strong; nothing
    /// commits, so there are no transactions; nothing changes, so there is
    /// nothing to tell.
    fn provides(&self) -> Result<data::Provided, String> {
        Ok(data::Provided {
            transactions: "none".to_string(),
            reads: BTreeSet::from(["strong".to_string()]),
            feed: false,
        })
    }
}
