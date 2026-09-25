//! **One shard loaded in kiokun's own format, and kiokun's shard rule in
//! Rust.**
//!
//! The rule is kiokun's (kiokun-data `src/main.rs`, the SvelteKit app's
//! `src/lib/shard-utils.ts`): count the word's Han characters; hash every
//! character, `h = h * 31 + codepoint`, wrapping; zero Han characters is
//! `non-han-{h % 4 + 1}`, one is `han-1char-{h % 8 + 1}`, two `han-2char-..`,
//! three or more `han-3plus-..`; the file sits in the subdirectory `h & 0xFF`,
//! two hex digits, as `<word>.json.deflate`, raw DEFLATE with no zlib header.
//!
//! Since 2026-09-25 the rule is Pleris, `examples/kiokun/Shards.pw`, and the
//! host asks the compiled `shards.Place` (ADR-0041). The Rust functions here
//! are the reference the compiled rule is held to, over every word the data
//! has.

use std::collections::BTreeMap;
use std::path::Path;

/// CJK Unified Ideographs, Extensions A–I, and the compatibility blocks.
#[cfg(test)]
pub fn is_han(c: char) -> bool {
    matches!(u32::from(c),
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF | 0x2A700..=0x2B73F
        | 0x2B740..=0x2B81F | 0x2B820..=0x2CEAF | 0x2CEB0..=0x2EBEF | 0x30000..=0x3134F
        | 0x31350..=0x323AF | 0x2EBF0..=0x2EE5F | 0xF900..=0xFAFF | 0x2F800..=0x2FA1F)
}

/// kiokun's hash. Only its low bits are ever read, so a 32-bit wrap agrees
/// with the Rust build's `usize` and the app's `u32`.
#[cfg(test)]
pub fn hash(word: &str) -> u32 {
    word.chars()
        .fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(u32::from(c)))
}

/// The shard a word's file is in.
#[cfg(test)]
pub fn shard_of(word: &str) -> String {
    let h = hash(word);
    match word.chars().filter(|c| is_han(*c)).count() {
        0 => format!("non-han-{}", h % 4 + 1),
        1 => format!("han-1char-{}", h % 8 + 1),
        2 => format!("han-2char-{}", h % 8 + 1),
        _ => format!("han-3plus-{}", h % 8 + 1),
    }
}

/// The subdirectory a word's file is in.
#[cfg(test)]
pub fn subdirectory(word: &str) -> String {
    format!("{:02x}", hash(word) & 0xFF)
}

/// One shard's entries, as kiokun wrote them: the word, and its JSON.
pub struct Shard {
    pub name: String,
    pub entries: BTreeMap<String, serde_json::Value>,
}

/// Which shard each word belongs to and where its file is: what the compiled
/// `shards.Places` answers.
pub type Placer<'a> = dyn Fn(&[String]) -> Result<Vec<(String, String)>, String> + 'a;

/// **A word's file name**, as kiokun's `create_safe_filename` writes it
/// (`kiokun-data/src/main.rs`): `/ \ : * ? " < > |` and control characters
/// become `_`. The subdirectory is still the word's hash, so
/// `길거리로/에 나앉다` is `길거리로_에 나앉다.json.deflate` in the
/// subdirectory `길거리로/에 나앉다` names. The escape loses the word: a name
/// holding `_` is read by the `key` its file records.
pub fn file_name(word: &str) -> String {
    let safe: String = word
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    format!("{safe}.json.deflate")
}

/// The word a file holds: its name, unless the name has `_`, which may be an
/// escape; then the `key` in the file. `Ok(None)` is a file that is not an
/// entry.
pub fn word_of(path: &Path) -> Result<Option<String>, String> {
    let Some(name) = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".json.deflate"))
    else {
        return Ok(None);
    };
    if !name.contains('_') {
        return Ok(Some(name.to_string()));
    }
    let key = key_of(&read_entry(path)?)
        .ok_or_else(|| format!("{}: a name with `_` and no `key`", path.display()))?;
    match file_name(&key) == format!("{name}.json.deflate") {
        true => Ok(Some(key)),
        false => Err(format!("{}: holds `{key}`", path.display())),
    }
}

/// The word an entry records.
pub fn key_of(entry: &serde_json::Value) -> Option<String> {
    entry.get("key")?.as_str().map(str::to_string)
}

/// One file of kiokun's: raw DEFLATE, then JSON.
pub fn read_entry(path: &Path) -> Result<serde_json::Value, String> {
    let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json = miniz_oxide::inflate::decompress_to_vec(&raw)
        .map_err(|e| format!("{}: does not inflate: {e:?}", path.display()))?;
    serde_json::from_slice(&json).map_err(|e| format!("{}: {e}", path.display()))
}

impl Shard {
    /// Every `<sub>/<word>.json.deflate` under `dir` whose word is in `name`,
    /// by `place`: the compiled rule. A file in the shard that does not
    /// inflate or parse is an error, not a skipped word: a dictionary quietly
    /// missing entries looks complete.
    pub fn load(dir: &Path, name: &str, place: &Placer<'_>) -> Result<Shard, String> {
        let mut entries = BTreeMap::new();
        let mut subs: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .collect();
        subs.sort_by_key(|e| e.file_name());
        for sub in subs {
            let here = sub.file_name().to_string_lossy().to_string();
            let mut files = Vec::new();
            for file in std::fs::read_dir(sub.path()).map_err(|e| e.to_string())? {
                let path = file.map_err(|e| e.to_string())?.path();
                if let Some(word) = word_of(&path)? {
                    files.push((path, word));
                }
            }
            let words: Vec<String> = files.iter().map(|(_, w)| w.clone()).collect();
            for ((path, word), (shard, subdirectory)) in files.into_iter().zip(place(&words)?) {
                if shard != name {
                    continue;
                }
                // kiokun's build puts every word in the subdirectory its hash
                // names. One that is elsewhere is a layout this loader does not
                // understand, and serving it would hide that.
                if here != subdirectory {
                    return Err(format!(
                        "{}: `{word}` belongs in `{subdirectory}/`",
                        path.display()
                    ));
                }
                let entry = read_entry(&path)?;
                // A file records its word; a name that is not it is a layout
                // this loader does not understand either.
                if key_of(&entry).is_some_and(|k| k != word) {
                    return Err(format!("{}: is not `{word}`'s", path.display()));
                }
                entries.insert(word, entry);
            }
        }
        Ok(Shard {
            name: name.to_string(),
            entries,
        })
    }
}
