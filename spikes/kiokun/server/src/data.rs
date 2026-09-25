//! **The deployment's data layer**: `kiokun:data/entries#get`, and the index
//! the compiled `Search` ranks from: `index#candidates`, `index#fold` and
//! `index#alias`, over one shard.
//!
//! An entry is converted from kiokun's JSON to the slice's `dictionary.Entry`,
//! as the component's world types it. The index is what kiokun.com's SQLite
//! FTS5 does: it finds the rows a query could match, and its tokenizer folds
//! case and splits definitions into words. The ranking is Pleris since
//! 2026-09-25 (ADR-0041). [`Index::search`] is kiokun.com's ranking in Rust
//! (sveltekit-app `src/routes/api/search/+server.ts`, as recorded in
//! `docs/evidence/E10/kiokun-2026-09-25.md`), kept as the reference the
//! compiled one is held to. Parity with FTS5's tokenizer, and with kiokun's
//! alias table beyond simplified-form stubs, is not claimed.

use std::collections::BTreeMap;

use pw_host::engine::Val;

use crate::shard::Shard;
#[cfg(test)]
use crate::shard::is_han;

fn text(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string()
}

fn list<'a>(v: &'a serde_json::Value, key: &str) -> impl Iterator<Item = &'a serde_json::Value> {
    v.get(key).and_then(|l| l.as_array()).into_iter().flatten()
}

/// Display pinyin: kiokun separates syllables with U+200B.
fn pinyin(word: &serde_json::Value) -> String {
    list(word, "items")
        .next()
        .map(|i| text(i, "pinyin").replace('\u{200b}', " "))
        .unwrap_or_default()
}

fn definitions(word: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in list(word, "items") {
        for d in list(item, "definitions").filter_map(|d| d.as_str()) {
            if !out.iter().any(|seen| seen == d) {
                out.push(d.to_string());
            }
        }
    }
    out
}

/// One sense's English glosses.
fn glosses(sense: &serde_json::Value) -> String {
    list(sense, "gloss")
        .filter(|g| g.get("lang").and_then(|l| l.as_str()).unwrap_or("eng") == "eng")
        .map(|g| text(g, "text"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn forms(word: &serde_json::Value, key: &str) -> Vec<String> {
    list(word, key).map(|k| text(k, "text")).collect()
}

fn common(word: &serde_json::Value) -> bool {
    list(word, "kanji")
        .chain(list(word, "kana"))
        .any(|k| k.get("common").and_then(|c| c.as_bool()) == Some(true))
}

fn record(fields: Vec<(&str, Val)>) -> Val {
    Val::Record(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

/// Each item's `field`'s strings, joined: `definitions[].text`.
fn joined(items: &serde_json::Value, key: &str, field: &str, by: &str) -> String {
    list(items, key)
        .map(|d| text(d, field))
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(by)
}

/// Records keyed by their `id`, once each. kiokun's build can list one
/// record twice: JMnedict's name 5345360 is in 釋 under both 釈 and 釋, and
/// a keyed list refuses two items with one key. An exact repeat is dropped; a
/// different record with a repeated id is keyed `<id>:<n>`.
fn distinct<'a>(
    records: impl Iterator<Item = &'a serde_json::Value>,
) -> impl Iterator<Item = (String, &'a serde_json::Value)> {
    let mut seen: Vec<(String, &serde_json::Value)> = Vec::new();
    records.filter_map(move |r| {
        let id = text(r, "id");
        if seen.iter().any(|(i, s)| *i == id && *s == r) {
            return None;
        }
        let n = seen.iter().filter(|(i, _)| *i == id).count();
        seen.push((id.clone(), r));
        Some((if n == 0 { id } else { format!("{id}:{n}") }, r))
    })
}

/// A number the data may not have, as the world's `option<s64>`.
fn maybe_int(v: Option<&serde_json::Value>) -> Val {
    Val::Option(v.and_then(|n| n.as_i64()).map(|n| Box::new(Val::S64(n))))
}

/// The character an entry is, from kiokun's `chinese_char`, `japanese_char`
/// and `korean_char`, or `None` for a word that is not one character.
fn character(json: &serde_json::Value) -> Val {
    let (zh, jp, kr) = (
        json.get("chinese_char"),
        json.get("japanese_char"),
        json.get("korean_char"),
    );
    if zh.is_none() && jp.is_none() && kr.is_none() {
        return Val::Option(None);
    }
    let misc = jp.and_then(|j| j.get("misc"));
    let strokes = zh
        .and_then(|z| z.get("strokeCount"))
        .or_else(|| {
            misc.and_then(|m| m.get("strokeCounts"))
                .and_then(|s| s.get(0))
        })
        .or_else(|| kr.and_then(|k| k.get("strokes")));
    let meanings = kr
        .map(|k| {
            list(k, "meaningsEn")
                .filter_map(|m| m.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default();
    Val::Option(Some(Box::new(record(vec![
        ("strokes", maybe_int(strokes)),
        ("grade", maybe_int(misc.and_then(|m| m.get("grade")))),
        ("jlpt", maybe_int(misc.and_then(|m| m.get("jlptLevel")))),
        (
            "frequency",
            maybe_int(misc.and_then(|m| m.get("frequency"))),
        ),
        (
            "hangul",
            Val::String(
                kr.map(|k| joined(k, "readings", "hangul", "、"))
                    .unwrap_or_default(),
            ),
        ),
        ("meanings", Val::String(meanings)),
    ]))))
}

/// kiokun's entry, as the world's `entry` record, in the declaration's order:
/// `key`, `redirect`, `chinese`, `japanese`, `korean`, `names`, `character`.
pub fn entry(json: &serde_json::Value) -> Val {
    let chinese = list(json, "chinese_words")
        .map(|w| {
            record(vec![
                ("id", Val::String(text(w, "_id"))),
                ("traditional", Val::String(text(w, "trad"))),
                ("simplified", Val::String(text(w, "simp"))),
                ("pinyin", Val::String(pinyin(w))),
                ("definitions", Val::String(definitions(w).join("; "))),
            ])
        })
        .collect();
    let japanese = list(json, "japanese_words")
        .map(|w| {
            let id = text(w, "id");
            let senses = list(w, "sense")
                .enumerate()
                .map(|(i, s)| (i, glosses(s)))
                .filter(|(_, g)| !g.is_empty())
                .map(|(i, g)| {
                    record(vec![
                        ("id", Val::String(format!("{id}:{i}"))),
                        ("glosses", Val::String(g)),
                    ])
                })
                .collect();
            record(vec![
                ("id", Val::String(id.clone())),
                ("written", Val::String(forms(w, "kanji").join("、"))),
                ("readings", Val::String(forms(w, "kana").join("、"))),
                ("senses", Val::List(senses)),
                ("common", Val::Bool(common(w))),
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
        (
            "korean",
            Val::List(
                list(json, "korean_words")
                    .map(|w| {
                        record(vec![
                            ("id", Val::String(text(w, "id"))),
                            ("hangul", Val::String(text(w, "hangul"))),
                            ("hanja", Val::String(text(w, "hanja"))),
                            ("pronunciation", Val::String(text(w, "pronunciation"))),
                            (
                                "definitions",
                                Val::String(joined(w, "definitions", "text", "; ")),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "names",
            Val::List(
                distinct(list(json, "japanese_names"))
                    .map(|(id, n)| {
                        let kinds: Vec<String> = list(n, "translation")
                            .flat_map(|t| list(t, "type"))
                            .filter_map(|k| k.as_str().map(str::to_string))
                            .collect();
                        let translations: Vec<String> = list(n, "translation")
                            .map(|t| joined(t, "translation", "text", "; "))
                            .filter(|t| !t.is_empty())
                            .collect();
                        record(vec![
                            ("id", Val::String(id)),
                            ("written", Val::String(forms(n, "kanji").join("、"))),
                            ("readings", Val::String(forms(n, "kana").join("、"))),
                            ("kinds", Val::String(kinds.join(", "))),
                            ("translations", Val::String(translations.join("; "))),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("character", character(json)),
    ])
}

/// One searchable row: one definition of one word, as kiokun indexes it.
#[derive(Debug, Clone)]
pub struct Row {
    pub word: String,
    pub target: String,
    pub language: &'static str,
    pub pronunciation: String,
    /// What a Latin query is compared with: pinyin without tones, or romaji.
    /// kiokun stores romaji for Japanese; this shard has kana, so a Japanese
    /// row's reading is its hiragana and a Latin query does not match it.
    pub reading: String,
    pub definition: String,
    /// The definition case-folded, and split into words, as the index's
    /// tokenizer reads it.
    pub folded: String,
    pub words: Vec<String>,
    pub common: bool,
}

/// The tokenizer's words: the folded text split at every character that is
/// not alphanumeric, as kiokun's whole-word match splits it.
fn words_of(folded: &str) -> Vec<String> {
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

impl Row {
    fn new(
        word: String,
        target: String,
        language: &'static str,
        pronunciation: String,
        reading: String,
        definition: String,
        common: bool,
    ) -> Row {
        let folded = definition.to_lowercase();
        Row {
            words: words_of(&folded),
            folded,
            word,
            target,
            language,
            pronunciation,
            reading,
            definition,
            common,
        }
    }

    /// The row as the world's `row` record, in `dictionary.Row`'s order.
    fn val(&self) -> Val {
        record(vec![
            ("word", Val::String(self.word.clone())),
            ("target", Val::String(self.target.clone())),
            ("language", Val::String(self.language.to_string())),
            ("pronunciation", Val::String(self.pronunciation.clone())),
            ("reading", Val::String(self.reading.clone())),
            ("definition", Val::String(self.definition.clone())),
            ("folded", Val::String(self.folded.clone())),
            (
                "words",
                Val::List(self.words.iter().map(|w| Val::String(w.clone())).collect()),
            ),
            ("common", Val::Bool(self.common)),
        ])
    }
}

/// The shard's entries and its search rows.
pub struct Index {
    pub shard: Shard,
    pub rows: Vec<Row>,
    /// A stub's key and the entry it names: a search alias.
    pub aliases: BTreeMap<String, String>,
}

/// Katakana to hiragana, as kiokun compares kana.
pub fn hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match u32::from(c) {
            k @ 0x30A1..=0x30F6 => char::from_u32(k - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// Hiragana (U+3040–309F) and katakana (U+30A0–30FF), adjacent blocks.
#[cfg(test)]
fn is_kana(c: char) -> bool {
    matches!(u32::from(c), 0x3040..=0x30FF)
}

impl Index {
    pub fn build(shard: Shard) -> Index {
        let mut rows = Vec::new();
        let mut aliases = BTreeMap::new();
        for (key, json) in &shard.entries {
            if let Some(target) = json.get("redirect").and_then(|r| r.as_str()) {
                aliases.insert(key.clone(), target.to_string());
                continue;
            }
            for w in list(json, "chinese_words") {
                // The plain reading is the last form of `pinyinSearchString`:
                // "rén ren2 ren".
                let plain = text(w, "pinyinSearchString")
                    .split_whitespace()
                    .last()
                    .unwrap_or_default()
                    .to_lowercase();
                for d in definitions(w) {
                    rows.push(Row::new(
                        text(w, "trad"),
                        key.clone(),
                        "chinese",
                        pinyin(w),
                        plain.clone(),
                        d,
                        false,
                    ));
                }
            }
            for w in list(json, "japanese_words") {
                let written = forms(w, "kanji");
                let kana = forms(w, "kana");
                let word = written
                    .first()
                    .or(kana.first())
                    .cloned()
                    .unwrap_or_default();
                let pronunciation = kana.first().cloned().unwrap_or_default();
                for s in list(w, "sense") {
                    let g = glosses(s);
                    if g.is_empty() {
                        continue;
                    }
                    rows.push(Row::new(
                        word.clone(),
                        key.clone(),
                        "japanese",
                        pronunciation.clone(),
                        hiragana(&pronunciation),
                        g,
                        common(w),
                    ));
                }
            }
        }
        Index {
            shard,
            rows,
            aliases,
        }
    }

    /// `entries#get`: the entry, or `None`.
    pub fn get(&self, word: &str) -> Val {
        Val::Option(self.shard.entries.get(word).map(|e| Box::new(entry(e))))
    }

    /// `index#candidates`: every row the query could match, in the index's
    /// order. A row matches only if its word starts with the query or with the
    /// entry the query's stub names, its reading starts with the query's
    /// hiragana or its folded form, or its folded definition starts with that
    /// form or holds it as a word; so this is a superset of every row the
    /// compiled ranking scores above zero. The query arrives trimmed.
    pub fn candidates(&self, query: &str) -> Vec<Val> {
        if query.is_empty() {
            return Vec::new();
        }
        let lower = self.fold(query);
        let reading = hiragana(query);
        let alias = self.alias(query);
        self.rows
            .iter()
            .filter(|r| {
                r.word.starts_with(query)
                    || alias.as_deref().is_some_and(|a| r.word.starts_with(a))
                    || r.reading.starts_with(&reading)
                    || r.reading.starts_with(&lower)
                    || r.folded.starts_with(&lower)
                    || r.words.contains(&lower)
            })
            .map(Row::val)
            .collect()
    }

    /// `index#fold`: the tokenizer's case folding.
    pub fn fold(&self, query: &str) -> String {
        query.to_lowercase()
    }

    /// `index#alias`: the entry a stub names.
    pub fn alias(&self, query: &str) -> Option<String> {
        self.aliases.get(query).cloned()
    }

    /// **kiokun's ranking, in Rust**: the reference `kiokun.page.Search` is
    /// held to (ADR-0041). It was `index#search` until 2026-09-25, when the
    /// ranking moved into Pleris.
    #[cfg(test)]
    pub fn search(&self, query: &str, limit: usize) -> Vec<Val> {
        let q = query.trim();
        if q.is_empty() {
            return Vec::new();
        }
        let cjk = q.chars().any(|c| is_han(c) || is_kana(c));
        let kana_only = q.chars().all(is_kana);
        let alias = self.aliases.get(q).cloned();
        let lower = q.to_lowercase();
        let whole_word = |text: &str| {
            text.to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .any(|w| w == lower)
        };

        let score = |r: &Row| -> Option<u32> {
            if cjk {
                let reading = hiragana(q);
                if r.word == q {
                    Some(1000)
                } else if alias.as_deref() == Some(r.word.as_str()) {
                    Some(900)
                } else if kana_only && r.reading == reading {
                    Some(875)
                } else if r.word.starts_with(q) {
                    Some(500)
                } else if alias.as_deref().is_some_and(|a| r.word.starts_with(a))
                    || (kana_only && r.reading.starts_with(&reading))
                {
                    Some(450)
                } else {
                    None
                }
            } else if r.definition.to_lowercase() == lower {
                Some(1000)
            } else if r.reading == lower {
                Some(900)
            } else if !r.reading.is_empty() && r.reading.starts_with(&lower) {
                Some(600)
            } else if r.definition.to_lowercase().starts_with(&lower) {
                Some(500)
            } else if whole_word(&r.definition) {
                Some(100)
            } else {
                None
            }
        };

        // One hit per entry: kiokun groups results by the entry they open, and
        // shows the languages it matched in. The entry's row is its best by
        // score, then a common word's before a rare one's.
        let mut best: BTreeMap<String, (u32, &Row, Vec<&'static str>, bool)> = BTreeMap::new();
        for r in &self.rows {
            let Some(s) = score(r) else { continue };
            let slot = best
                .entry(r.target.clone())
                .or_insert((s, r, Vec::new(), false));
            if (s, r.common) > (slot.0, slot.1.common) {
                slot.0 = s;
                slot.1 = r;
            }
            if !slot.2.contains(&r.language) {
                slot.2.push(r.language);
            }
            slot.3 |= r.common;
        }
        let mut hits: Vec<(u32, &Row, Vec<&'static str>, bool)> = best.into_values().collect();
        hits.sort_by(|(sa, a, _, ca), (sb, b, _, cb)| {
            sb.cmp(sa)
                .then(cb.cmp(ca))
                .then(a.word.chars().count().cmp(&b.word.chars().count()))
                .then(a.target.cmp(&b.target))
        });
        hits.into_iter()
            .take(limit)
            .map(|(_, r, mut languages, common)| {
                languages.sort();
                record(vec![
                    ("id", Val::String(r.target.clone())),
                    ("word", Val::String(r.word.clone())),
                    ("target", Val::String(r.target.clone())),
                    ("language", Val::String(languages.join(" · "))),
                    ("pronunciation", Val::String(r.pronunciation.clone())),
                    ("definition", Val::String(r.definition.clone())),
                    ("common", Val::Bool(common)),
                ])
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    /// An exact repeat is dropped, as 釋 lists JMnedict's name 5345360
    /// twice; a different record with a repeated id is kept and keyed apart.
    #[test]
    fn a_record_listed_twice_is_read_once() {
        let json: serde_json::Value = serde_json::json!({ "names": [
            { "id": "1", "text": "a" },
            { "id": "1", "text": "a" },
            { "id": "1", "text": "b" },
            { "id": "2", "text": "c" },
        ]});
        let keys: Vec<(String, String)> = super::distinct(super::list(&json, "names"))
            .map(|(k, r)| (k, super::text(r, "text")))
            .collect();
        assert_eq!(
            keys,
            [
                ("1".to_string(), "a".to_string()),
                ("1:1".to_string(), "b".to_string()),
                ("2".to_string(), "c".to_string()),
            ]
        );
    }
}
