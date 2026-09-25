//! **kiokun's shard rule**, and one shard loaded in kiokun's own format.
//!
//! The rule is kiokun's (kiokun-data `src/main.rs`, the SvelteKit app's
//! `src/lib/shard-utils.ts`): count the word's Han characters; hash every
//! character, `h = h * 31 + codepoint`, wrapping; zero Han characters is
//! `non-han-{h % 4 + 1}`, one is `han-1char-{h % 8 + 1}`, two `han-2char-..`,
//! three or more `han-3plus-..`; the file sits in the subdirectory `h & 0xFF`,
//! two hex digits, as `<word>.json.deflate`, raw DEFLATE with no zlib header.

use std::collections::BTreeMap;
use std::path::Path;

/// CJK Unified Ideographs, Extensions A–I, and the compatibility blocks.
pub fn is_han(c: char) -> bool {
    matches!(u32::from(c),
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF | 0x2A700..=0x2B73F
        | 0x2B740..=0x2B81F | 0x2B820..=0x2CEAF | 0x2CEB0..=0x2EBEF | 0x30000..=0x3134F
        | 0x31350..=0x323AF | 0x2EBF0..=0x2EE5F | 0xF900..=0xFAFF | 0x2F800..=0x2FA1F)
}

/// kiokun's hash. Only its low bits are ever read, so a 32-bit wrap agrees
/// with the Rust build's `usize` and the app's `u32`.
pub fn hash(word: &str) -> u32 {
    word.chars()
        .fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(u32::from(c)))
}

/// The shard a word's file is in.
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
pub fn subdirectory(word: &str) -> String {
    format!("{:02x}", hash(word) & 0xFF)
}

/// One shard's entries, as kiokun wrote them: the word, and its JSON.
pub struct Shard {
    pub name: String,
    pub entries: BTreeMap<String, serde_json::Value>,
}

impl Shard {
    /// Every `<sub>/<word>.json.deflate` under `dir` whose word is in `name`.
    /// A file in the shard that does not inflate or parse is an error, not a
    /// skipped word: a dictionary quietly missing entries looks complete.
    pub fn load(dir: &Path, name: &str) -> Result<Shard, String> {
        let mut entries = BTreeMap::new();
        let mut subs: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .collect();
        subs.sort_by_key(|e| e.file_name());
        for sub in subs {
            for file in std::fs::read_dir(sub.path()).map_err(|e| e.to_string())? {
                let path = file.map_err(|e| e.to_string())?.path();
                let Some(word) = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(".json.deflate"))
                else {
                    continue;
                };
                if shard_of(word) != name {
                    continue;
                }
                // kiokun's build puts every word in the subdirectory its hash
                // names. One that is elsewhere is a layout this loader does not
                // understand, and serving it would hide that.
                let here = sub.file_name().to_string_lossy().to_string();
                if here != subdirectory(word) {
                    return Err(format!(
                        "{}: `{word}` belongs in `{}/`",
                        path.display(),
                        subdirectory(word)
                    ));
                }
                let raw = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let json = miniz_oxide::inflate::decompress_to_vec(&raw)
                    .map_err(|e| format!("{}: does not inflate: {e:?}", path.display()))?;
                let entry: serde_json::Value = serde_json::from_slice(&json)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                entries.insert(word.to_string(), entry);
            }
        }
        Ok(Shard {
            name: name.to_string(),
            entries,
        })
    }
}
