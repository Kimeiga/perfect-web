//! **The deployment's data layer**: `kiokun:data/entries#get` and
//! `kiokun:data/index#search`, over one shard.
//!
//! An entry is converted from kiokun's JSON to the slice's `dictionary.Entry`,
//! as the component's world types it. Search reimplements kiokun.com's rules
//! (sveltekit-app `src/routes/api/search/+server.ts`, as recorded in
//! `docs/evidence/E10/kiokun-2026-09-25.md`) over this shard's entries, in
//! memory, where kiokun.com runs them in SQLite FTS5. It is a reimplementation
//! with the same scores and tie-breaks. Parity with FTS5's tokenizer, and with
//! kiokun's alias table beyond simplified-form stubs, is not claimed.

use std::collections::BTreeMap;

use pw_host::engine::Val;

use crate::shard::{Shard, is_han};

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

/// kiokun's entry, as the world's `entry` record: `key`, `redirect`,
/// `chinese`, `japanese`, in the declaration's order.
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
    pub common: bool,
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
                    rows.push(Row {
                        word: text(w, "trad"),
                        target: key.clone(),
                        language: "chinese",
                        pronunciation: pinyin(w),
                        reading: plain.clone(),
                        definition: d,
                        common: false,
                    });
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
                    rows.push(Row {
                        word: word.clone(),
                        target: key.clone(),
                        language: "japanese",
                        reading: hiragana(&pronunciation),
                        pronunciation: pronunciation.clone(),
                        definition: g,
                        common: common(w),
                    });
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

    /// `index#search`: kiokun's matching and ranking over this shard.
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
