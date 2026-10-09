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
pub(crate) const GRANTS: &[&str] = &[
    "database.read<Entry>",
    "database.read<Label>",
    "database.read<CharGloss>",
];

/// The sample the repository carries (ADR-0037): 28 files of one shard.
fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/kiokun/data/han-1char-3")
}

pub(crate) struct KiokunData {
    root: PathBuf,
    app: Arc<App>,
}

/// **kiokun.com's own data, as its app keeps it**, read once: its labels for
/// JMdict's codes, each a `Label`, and its component glosses by character.
#[derive(Default)]
pub(crate) struct App {
    labels: Vec<Val>,
    glosses: BTreeMap<String, String>,
}

/// The app's data at `app`, the checkout's `sveltekit-app`: an error where a
/// table is missing, not an empty table.
pub(crate) fn app_of(app: &Path) -> Result<App, String> {
    Ok(App {
        labels: labels_of(app)?,
        glosses: glosses_of(app)?,
    })
}

/// **kiokun.com's component glosses** (`static/game_data/component_glosses.json`):
/// a character's learner gloss, by character.
pub(crate) fn glosses_of(app: &Path) -> Result<BTreeMap<String, String>, String> {
    let path = app.join("static/game_data/component_glosses.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

impl KiokunData {
    /// The files `KIOKUN_DATA` names, or the repository's sample; and the
    /// labels of the app `KIOKUN_APP` names, the checkout's `sveltekit-app`
    /// (docs/PARALLEL.md, "W6's inventory, answered", Q3), or none. An app
    /// named without its tables is an error, not empty tables.
    pub(crate) fn new() -> Result<KiokunData, String> {
        let root = std::env::var_os("KIOKUN_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(sample);
        let app = match std::env::var_os("KIOKUN_APP") {
            Some(app) => app_of(&PathBuf::from(app))?,
            None => App::default(),
        };
        Ok(KiokunData {
            root,
            app: Arc::new(app),
        })
    }

    #[cfg(test)]
    pub(crate) fn at(root: PathBuf, app: Option<&Path>) -> KiokunData {
        let app = app
            .map(|a| app_of(a).expect("the app's data"))
            .unwrap_or_default();
        KiokunData {
            root,
            app: Arc::new(app),
        }
    }
}

/// The longest file name, in bytes, on macOS (APFS) and Linux (ext4).
const NAME_MAX: usize = 255;

/// The kinds of kiokun.com's label table, as `japaneseLabels.ts` reads them.
const LABEL_KINDS: [&str; 5] = ["pos", "misc", "field", "dial", "head_info"];

/// **kiokun.com's labels** (`src/lib/japanese-labels.json`, its `labels`):
/// each kind's codes and texts, as the program's `Label`. A lookup reads a
/// code's text, so their order is not kept.
pub(crate) fn labels_of(app: &Path) -> Result<Vec<Val>, String> {
    let path = app.join("src/lib/japanese-labels.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for kind in LABEL_KINDS {
        let Some(table) = json
            .get("labels")
            .and_then(|l| l.get(kind))
            .and_then(|t| t.as_object())
        else {
            continue;
        };
        for (code, label) in table {
            out.push(record(vec![
                ("kind", Val::String(kind.to_string())),
                ("code", Val::String(code.clone())),
                (
                    "text",
                    Val::String(label.as_str().unwrap_or_default().to_string()),
                ),
            ]));
        }
    }
    Ok(out)
}

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
        (
            "chinese-char",
            optional(json.get("chinese_char"), chinese_char),
        ),
        (
            "japanese-char",
            optional(json.get("japanese_char"), japanese_char),
        ),
        (
            "korean-char",
            optional(json.get("korean_char"), korean_char),
        ),
        (
            "mnemonic",
            optional(json.get("semantic_mnemonic"), mnemonic),
        ),
        (
            "variants",
            Val::List(
                list(json, "semantic_mnemonic_variants")
                    .filter(|c| !c.is_null())
                    .map(mnemonic)
                    .collect(),
            ),
        ),
        (
            "simplified-form-of",
            Val::String(text(json, "simplified_form_of")),
        ),
    ])
}

/// A field the file may not have, or may have as `null`, as an `Option`.
fn optional(v: Option<&serde_json::Value>, f: fn(&serde_json::Value) -> Val) -> Val {
    Val::Option(v.filter(|v| !v.is_null()).map(|v| Box::new(f(v))))
}

/// A number the file may not have, as an `Option<Int>`.
fn maybe_int(v: Option<&serde_json::Value>) -> Val {
    Val::Option(v.and_then(|n| n.as_i64()).map(|n| Box::new(Val::S64(n))))
}

/// `ChineseChar { gloss, frequencies, old, cantonese, hsk }`.
fn chinese_char(c: &serde_json::Value) -> Val {
    record(vec![
        ("character", Val::String(text(c, "char"))),
        ("simplified", strings(c, "simpVariants")),
        ("traditional", strings(c, "tradVariants")),
        ("hong-kong", Val::String(text(c, "hkChar"))),
        ("gloss", Val::String(text(c, "gloss"))),
        ("frequencies", texts(c, "pinyinFrequencies", "pinyin")),
        ("old", texts(c, "oldPronunciations", "pinyin")),
        ("cantonese", strings(c, "cantonese")),
        (
            "hsk",
            maybe_int(c.get("statistics").and_then(|s| s.get("hskLevel"))),
        ),
    ])
}

/// KANJIDIC's readings, each `KanjiReading { kind, value }`.
fn kanji_readings(v: &serde_json::Value, key: &str) -> Val {
    Val::List(
        list(v, key)
            .map(|r| {
                record(vec![
                    ("kind", Val::String(text(r, "type"))),
                    ("value", Val::String(text(r, "value"))),
                ])
            })
            .collect(),
    )
}

/// `JapaneseChar { jlpt, grouped, groups, readings }`: `grouped` where the
/// file's `readingMeaning` has `groups`, whatever they hold.
fn japanese_char(j: &serde_json::Value) -> Val {
    let meaning = j.get("readingMeaning").cloned().unwrap_or_default();
    let groups = meaning.get("groups").filter(|g| g.is_array());
    record(vec![
        ("literal", Val::String(text(j, "literal"))),
        (
            "jlpt",
            maybe_int(j.get("misc").and_then(|m| m.get("jlptLevel"))),
        ),
        ("grouped", Val::Bool(groups.is_some())),
        (
            "groups",
            Val::List(
                list(&meaning, "groups")
                    .map(|g| record(vec![("readings", kanji_readings(g, "readings"))]))
                    .collect(),
            ),
        ),
        ("readings", kanji_readings(&meaning, "readings")),
    ])
}

/// `KoreanChar { readings }`: its readings' hangul.
fn korean_char(k: &serde_json::Value) -> Val {
    record(vec![
        ("character", Val::String(text(k, "character"))),
        ("hanja", Val::String(text(k, "hanjaForm"))),
        ("readings", texts(k, "readings", "hangul")),
    ])
}

/// A card: `Card { character, keyword, meaning, lexical }`.
fn mnemonic(m: &serde_json::Value) -> Val {
    record(vec![
        ("character", Val::String(text(m, "character"))),
        ("keyword", Val::String(text(m, "mnemonic_keyword"))),
        ("meaning", Val::String(text(m, "meaning"))),
        ("lexical", Val::String(text(m, "lexical_gloss"))),
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

/// One file read, as `entries#read` answers it.
fn read_one(root: &Path, subdirectory: &str, file: &str) -> Result<Val, String> {
    let found = read(root, subdirectory, file)?;
    Ok(Val::Option(found.map(|j| Box::new(entry(&j)))))
}

/// A `Place { subdirectory, file }`'s two fields.
fn place_fields(v: &Val) -> Result<(String, String), String> {
    let Val::Record(fields) = v else {
        return Err(format!("a place is a record, not {v:?}"));
    };
    let field = |name: &str| match fields.iter().find(|(k, _)| k == name) {
        Some((_, Val::String(s))) => Ok(s.clone()),
        other => Err(format!("a place's {name} is {other:?}")),
    };
    Ok((field("subdirectory")?, field("file")?))
}

fn reads_of(root: PathBuf, app: Arc<App>) -> data::Ops {
    let mut ops: data::Ops = BTreeMap::new();
    let a = app.clone();
    ops.insert(
        "kiokun:data/support#glosses".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::List(characters)] => Ok(vec![Val::List(
                characters
                    .iter()
                    .filter_map(|c| match c {
                        Val::String(c) => a.glosses.get(c).map(|g| {
                            record(vec![
                                ("character", Val::String(c.clone())),
                                ("gloss", Val::String(g.clone())),
                            ])
                        }),
                        _ => None,
                    })
                    .collect(),
            )]),
            other => Err(format!("support#glosses received {other:?}")),
        }),
    );
    let r = root.clone();
    ops.insert(
        "kiokun:data/entries#read-all".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [Val::List(places)] => {
                let mut out = Vec::with_capacity(places.len());
                for p in places {
                    let (subdirectory, file) = place_fields(p)?;
                    out.push(read_one(&r, &subdirectory, &file)?);
                }
                Ok(vec![Val::List(out)])
            }
            other => Err(format!("entries#read-all received {other:?}")),
        }),
    );
    ops.insert(
        "kiokun:data/labels#japanese".to_string(),
        Arc::new(move |args: &[Val]| match args {
            [] => Ok(vec![Val::List(app.labels.clone())]),
            other => Err(format!("labels#japanese received {other:?}")),
        }),
    );
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
        reads_of(self.root.clone(), self.app.clone())
    }

    fn begin<'a>(&'a self, _session: &str) -> Box<dyn data::Staged + 'a> {
        Box::new(Nothing(reads_of(self.root.clone(), self.app.clone())))
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
