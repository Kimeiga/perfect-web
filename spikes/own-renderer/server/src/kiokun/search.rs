//! **kiokun.com's search index, read** (track `kiokun`, W6; the integrator's
//! ruling of 2026-10-10 on search, B): migration 0005's FTS5 table
//! (kiokun-data `sveltekit-app/migrations/0005_search_index_with_jyutping.sql`
//! at `abf71a89`), filled from the builder's search index CSV, and the two
//! statements `api/search/+server.ts` runs over it, each as it writes them,
//! with the parameters the program gives. Ranking is FTS5's and SQLite's, as
//! on kiokun.com's D1: its `porter unicode61` tokenizer is not reimplemented.
//!
//! The index is built once per process from the CSV, into a file under
//! `target/` keyed by the CSV's and the schema's digests (gone with `cargo
//! clean`; a stale one is removed when a new one is built), or in memory for
//! a small CSV. Nothing is written in either repository.

use rusqlite::Connection;
use rusqlite::types::Value as Sql;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Migration 0005's table, ported: seven columns, Porter stemming over
/// Unicode 6.1 word breaks.
pub(crate) const SCHEMA: &str = "CREATE VIRTUAL TABLE dictionary_search USING fts5(\
    word, language, definition, pronunciation, reading_search, jyutping_search, is_common, \
    tokenize = 'porter unicode61')";

/// A CSV smaller than this is indexed in memory, not cached.
const IN_MEMORY: u64 = 4 << 20;

/// One row the statements return.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) word: String,
    pub(crate) language: String,
    pub(crate) definition: String,
    pub(crate) pronunciation: String,
    pub(crate) reading_search: String,
    pub(crate) is_common: i64,
}

/// The index, opened.
pub(crate) struct Index {
    conn: Mutex<Connection>,
}

/// A CSV file's records, as the builder's `escape_csv_field` writes them:
/// a field holding a comma, a quote or a line break is quoted, a quote
/// inside doubled.
pub(crate) fn records(text: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                c => field.push(c),
            }
        } else {
            match c {
                '"' => quoted = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    out.push(std::mem::take(&mut row));
                }
                '\r' => {}
                c => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        out.push(row);
    }
    out
}

/// The table, filled from `csv`: its header names the columns it holds; each
/// field as text, but `is_common` an integer, as `output_search_index.sql`
/// writes it (an assumption the step's ADR names: loaded as text, kiokun.com's
/// `Boolean(row.is_common)` would read `'0'` as true).
fn fill(conn: &mut Connection, csv: &str) -> Result<usize, String> {
    conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
    let mut all = records(csv).into_iter();
    let header = all.next().ok_or("the search index is empty")?;
    for column in &header {
        if ![
            "word",
            "language",
            "definition",
            "pronunciation",
            "reading_search",
            "jyutping_search",
            "is_common",
        ]
        .contains(&column.as_str())
        {
            return Err(format!(
                "the search index has a column {column} the table has not"
            ));
        }
    }
    let common = header.iter().position(|c| c == "is_common");
    let sql = format!(
        "INSERT INTO dictionary_search ({}) VALUES ({})",
        header.join(", "),
        vec!["?"; header.len()].join(", ")
    );
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut n = 0;
    {
        let mut insert = tx.prepare(&sql).map_err(|e| e.to_string())?;
        for record in all {
            if record.len() != header.len() {
                return Err(format!(
                    "a search index row of {} fields: {record:?}",
                    record.len()
                ));
            }
            let values: Vec<Sql> = record
                .into_iter()
                .enumerate()
                .map(|(i, v)| match (Some(i) == common, v.parse::<i64>()) {
                    (true, Ok(n)) => Sql::Integer(n),
                    _ => Sql::Text(v),
                })
                .collect();
            insert
                .execute(rusqlite::params_from_iter(values))
                .map_err(|e| e.to_string())?;
            n += 1;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(n)
}

/// The index's cache file for `csv` under `dir`: named by the digests of the
/// CSV's bytes and of the schema.
fn cache_of(dir: &Path, csv: &[u8]) -> PathBuf {
    let mut h = Sha256::new();
    h.update(csv);
    h.update(SCHEMA.as_bytes());
    let digest = h.finalize();
    let name: String = digest.iter().take(12).map(|b| format!("{b:02x}")).collect();
    dir.join(format!("kiokun-search-{name}.db"))
}

impl Index {
    /// The index of `csv`: in memory where the CSV is small; else the file
    /// under `cache` its digests name, built where it is not there, a stale
    /// one removed.
    pub(crate) fn open(csv: &Path, cache: &Path) -> Result<Index, String> {
        let bytes = std::fs::read(csv).map_err(|e| format!("{}: {e}", csv.display()))?;
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("{}: {e}", csv.display()))?;
        if (bytes.len() as u64) < IN_MEMORY {
            let mut conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
            fill(&mut conn, text)?;
            return Ok(Index {
                conn: Mutex::new(conn),
            });
        }
        std::fs::create_dir_all(cache).map_err(|e| format!("{}: {e}", cache.display()))?;
        let file = cache_of(cache, &bytes);
        if !file.exists() {
            let building = file.with_extension("building");
            let _ = std::fs::remove_file(&building);
            let mut conn = Connection::open(&building).map_err(|e| e.to_string())?;
            fill(&mut conn, text)?;
            drop(conn);
            std::fs::rename(&building, &file).map_err(|e| e.to_string())?;
            for stale in std::fs::read_dir(cache)
                .map_err(|e| e.to_string())?
                .flatten()
            {
                let path = stale.path();
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if name.starts_with("kiokun-search-") && path != file {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        let conn = Connection::open_with_flags(&file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
        Ok(Index {
            conn: Mutex::new(conn),
        })
    }

    fn rows(&self, sql: &str, binds: Vec<Sql>) -> Result<Vec<Row>, String> {
        let conn = self.conn.lock().expect("the index");
        let mut statement = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(binds), |r| {
                let text = |i: usize| -> rusqlite::Result<String> {
                    Ok(match r.get::<_, Sql>(i)? {
                        Sql::Text(t) => t,
                        Sql::Integer(n) => n.to_string(),
                        Sql::Real(f) => f.to_string(),
                        Sql::Null | Sql::Blob(_) => String::new(),
                    })
                };
                Ok(Row {
                    word: text(0)?,
                    language: text(1)?,
                    definition: text(2)?,
                    pronunciation: text(3)?,
                    reading_search: text(4)?,
                    is_common: match r.get::<_, Sql>(5)? {
                        Sql::Integer(n) => n,
                        Sql::Text(t) => t.parse().unwrap_or(0),
                        _ => 0,
                    },
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// **The CJK statement** (`buildCjkSearchSql`, `buildCjkMatchQuery`):
    /// each term a word prefix, the first exact at 1000 and the rest at 900,
    /// a Japanese reading at 875 where the program gives one, the first's
    /// prefix at 500 and the rest's at 450; then common, shorter, earlier.
    pub(crate) fn cjk(
        &self,
        terms: &[String],
        reading: &str,
        limit: i64,
    ) -> Result<Vec<Row>, String> {
        let phrase = |v: &str| format!("\"{}\"", v.replace('"', "\"\""));
        let mut clauses: Vec<String> = terms
            .iter()
            .map(|t| format!("word : {}*", phrase(t)))
            .collect();
        if !reading.is_empty() {
            clauses.push(format!("pronunciation : {}*", phrase(reading)));
        }
        let exact: Vec<String> = (0..terms.len())
            .map(|i| format!("WHEN word = ? THEN {}", if i == 0 { 1000 } else { 900 }))
            .collect();
        let prefix: Vec<String> = (0..terms.len())
            .map(|i| {
                format!(
                    "WHEN word LIKE ? || '%' THEN {}",
                    if i == 0 { 500 } else { 450 }
                )
            })
            .collect();
        let reading_rank = if reading.is_empty() {
            String::new()
        } else {
            "WHEN language = 'japanese' AND pronunciation = ? THEN 875".to_string()
        };
        let sql = format!(
            "SELECT word, language, definition, pronunciation, reading_search, is_common, \
             CASE {} {} {} ELSE 0 END as custom_rank \
             FROM dictionary_search WHERE dictionary_search MATCH ? \
             ORDER BY custom_rank DESC, is_common DESC, LENGTH(word) ASC, rowid ASC LIMIT ?",
            exact.join(" "),
            reading_rank,
            prefix.join(" ")
        );
        let mut binds: Vec<Sql> = terms.iter().map(|t| Sql::Text(t.clone())).collect();
        if !reading.is_empty() {
            binds.push(Sql::Text(reading.to_string()));
        }
        binds.extend(terms.iter().map(|t| Sql::Text(t.clone())));
        binds.push(Sql::Text(clauses.join(" OR ")));
        binds.push(Sql::Integer(limit));
        self.rows(&sql, binds)
    }

    /// **The Latin statement** (`api/search/+server.ts`): the query matched
    /// over every column as FTS5 reads it; an exact definition at 1000, an
    /// exact reading at 900, a reading's prefix at 600, a definition's at
    /// 500, a whole word of a definition at 100; then common, earlier.
    pub(crate) fn latin(&self, query: &str, limit: i64) -> Result<Vec<Row>, String> {
        let sql = "SELECT word, language, definition, pronunciation, reading_search, is_common, \
             CASE \
             WHEN LOWER(definition) = LOWER(?) THEN 1000 \
             WHEN LOWER(reading_search) = LOWER(?) THEN 900 \
             WHEN LOWER(reading_search) LIKE LOWER(? || '%') THEN 600 \
             WHEN LOWER(definition) LIKE LOWER(? || '%') THEN 500 \
             WHEN LOWER(definition) LIKE LOWER('% ' || ? || ' %') \
               OR LOWER(definition) LIKE LOWER(? || ' %') \
               OR LOWER(definition) LIKE LOWER('% ' || ?) THEN 100 \
             ELSE 0 END as custom_rank \
             FROM dictionary_search WHERE dictionary_search MATCH ? \
             ORDER BY custom_rank DESC, is_common DESC, rowid ASC LIMIT ?";
        let mut binds: Vec<Sql> = (0..8).map(|_| Sql::Text(query.to_string())).collect();
        binds.push(Sql::Integer(limit));
        self.rows(sql, binds)
    }
}

/// **kiokun.com's search aliases** (`src/lib/generated/search-aliases.json`):
/// a Japanese form's canonical page, a written form's whole-word aliases,
/// and each character's script variants.
#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Aliases {
    #[serde(default)]
    pub(crate) japanese_to_canonical: HashMap<String, String>,
    #[serde(default)]
    pub(crate) aliases: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub(crate) character_variants: HashMap<String, Vec<String>>,
}

impl Aliases {
    pub(crate) fn read(path: &Path) -> Result<Aliases, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_csv_record_holds_quotes_commas_and_line_breaks() {
        let rows = records("word,definition\n\"a, b\",\"say \"\"hi\"\"\nthen\"\nc,d");
        assert_eq!(
            rows,
            vec![
                vec!["word".to_string(), "definition".to_string()],
                vec!["a, b".to_string(), "say \"hi\"\nthen".to_string()],
                vec!["c".to_string(), "d".to_string()],
            ]
        );
    }
}
