//! **The slice, served**: the compiled queries run through the E8 host with
//! the shard as their data layer, and the compiled pages render the result.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pw_host::engine::{HostFn, Prepared, Val};
use pw_host::{ComponentContract, Granted, Limits, Node, Topology, admit};
use pw_render::{Env, Template, Value};

use crate::data::Index;

/// One compiled query, admitted and ready to call.
struct Query {
    contract: ComponentContract,
    granted: Granted,
    prepared: Prepared,
    export: [String; 2],
}

impl Query {
    fn load(build: &Path, contracts: &[ComponentContract], id: &str) -> Result<Query, String> {
        let path = build.join("components").join(format!("{id}.wasm"));
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let contract = contracts
            .iter()
            .find(|c| c.component_id == id)
            .cloned()
            .ok_or_else(|| format!("no contract for `{id}`"))?;
        // The node grants what the contract requires, and nothing else: the
        // slice's host is a read-only dictionary.
        let node = Topology {
            nodes: vec![Node {
                name: "kiokun-origin".into(),
                world: "origin".into(),
                grants: contract
                    .required_capabilities
                    .iter()
                    .map(|c| c.name())
                    .collect::<BTreeSet<_>>(),
            }],
        };
        let actual = pw_host::engine::imports_of(&bytes)?;
        let admission = admit(&contract, &node, "kiokun-origin", &actual);
        let granted = Granted::from(&admission, &BTreeMap::new())
            .ok_or_else(|| format!("`{id}` is not admitted: {admission:?}"))?;
        let export = contract.exports[0]
            .component
            .clone()
            .ok_or_else(|| format!("`{id}`'s contract does not locate its export"))?;
        Ok(Query {
            prepared: Prepared::compile(&bytes)?,
            contract,
            granted,
            export: [export.interface, export.function],
        })
    }

    fn call(&self, ops: &BTreeMap<String, HostFn>, args: &[Val]) -> Result<Val, String> {
        self.measured(ops, args).map(|(v, _)| v)
    }

    /// The call, and what it cost (ADR-0046).
    fn measured(
        &self,
        ops: &BTreeMap<String, HostFn>,
        args: &[Val],
    ) -> Result<(Val, pw_host::engine::Usage), String> {
        let (out, usage) = self.prepared.call_measured(
            &self.contract,
            &self.granted,
            &Limits {
                fuel: Some(100_000_000),
                memory_bytes: Some(64 * 1024 * 1024),
                table_elements: Some(1_000),
            },
            ops,
            &[&self.export[0], &self.export[1]],
            args,
        )?;
        let v = out
            .into_iter()
            .next()
            .ok_or_else(|| format!("`{}` returned nothing", self.contract.component_id))?;
        Ok((v, usage))
    }
}

/// The slice: its shard, its compiled queries, its three pages.
pub struct App {
    pub index: Arc<Index>,
    templates: Vec<Template>,
    lookup: Query,
    search: Query,
    place: Arc<Query>,
    #[cfg(test)]
    places: Query,
    /// Where kiokun's files are: any word's is found here by the compiled
    /// rule, whichever shard it is in.
    dir: PathBuf,
    /// Entries read on demand, outside the loaded shard.
    read: Arc<Mutex<BTreeMap<String, Option<Val>>>>,
}

/// The compiled `shards.Place`'s answer: the shard and the subdirectory.
fn placement(place: &Query, word: &str) -> Result<(String, String), String> {
    located(place.call(&BTreeMap::new(), &[Val::String(word.to_string())])?)
}

/// The compiled `shards.Places`, a thousand words a call: a call is a fresh
/// instance, and a word each made loading a shard three times slower
/// (ADR-0041).
fn placements(places: &Query, words: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::with_capacity(words.len());
    for chunk in words.chunks(1000) {
        let list = chunk.iter().map(|w| Val::String(w.clone())).collect();
        match places.call(&BTreeMap::new(), &[Val::List(list)])? {
            Val::List(found) if found.len() == chunk.len() => {
                for f in found {
                    out.push(located(f)?);
                }
            }
            other => return Err(format!("Places returned {other:?}")),
        }
    }
    Ok(out)
}

fn located(v: Val) -> Result<(String, String), String> {
    match v {
        Val::Record(fields) => {
            let get = |name: &str| match fields.iter().find(|(k, _)| k == name) {
                Some((_, Val::String(s))) => Ok(s.clone()),
                other => Err(format!("a placement's `{name}` is {other:?}")),
            };
            Ok((get("shard")?, get("subdirectory")?))
        }
        other => Err(format!("a placement is {other:?}")),
    }
}

/// A page to send: its status and its HTML.
pub struct Page {
    pub status: u16,
    pub html: String,
}

impl App {
    /// `build` is a `pw build` output directory for `examples/kiokun/`, and
    /// `dir` holds kiokun's files: `shard` is loaded from it and searched, and
    /// any other word's file is read from it on demand.
    pub fn load(build: &Path, dir: &Path, shard: &str) -> Result<App, String> {
        let read = |name: &str| {
            let path = build.join(name);
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        let contracts = ComponentContract::from_json(&read("contracts.json")?)?;
        let templates: Vec<Template> =
            serde_json::from_str(&read("templates.json")?).map_err(|e| e.to_string())?;
        let place = Arc::new(Query::load(build, &contracts, "shards.Place")?);
        let places = Query::load(build, &contracts, "shards.Places")?;
        let shard = crate::shard::Shard::load(dir, shard, &|ws| placements(&places, ws))?;
        Ok(App {
            index: Arc::new(Index::build(shard)),
            lookup: Query::load(build, &contracts, "kiokun.page.Lookup")?,
            search: Query::load(build, &contracts, "kiokun.page.Search")?,
            place,
            #[cfg(test)]
            places,
            dir: dir.to_path_buf(),
            read: Arc::new(Mutex::new(BTreeMap::new())),
            templates,
        })
    }

    /// What one call of each compiled query costs, by name (ADR-0046).
    #[cfg(test)]
    pub fn usage(&self, query: &str, args: &[Val]) -> Result<pw_host::engine::Usage, String> {
        let q = match query {
            "Place" => self.place.as_ref(),
            "Places" => &self.places,
            "Lookup" => &self.lookup,
            "Search" => &self.search,
            other => return Err(format!("no query `{other}`")),
        };
        q.measured(&self.data_layer(), args).map(|(_, u)| u)
    }

    /// Where a word's file is, by the compiled rule.
    #[cfg(test)]
    pub fn place(&self, word: &str) -> Result<(String, String), String> {
        placement(&self.place, word)
    }

    /// Where each word's file is, by the compiled rule, in few calls.
    #[cfg(test)]
    pub fn places(&self, words: &[String]) -> Result<Vec<(String, String)>, String> {
        placements(&self.places, words)
    }

    /// The deployment's data layer, as host operations.
    fn data_layer(&self) -> BTreeMap<String, HostFn> {
        let (index, place, dir, read) = (
            self.index.clone(),
            self.place.clone(),
            self.dir.clone(),
            self.read.clone(),
        );
        let get: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(word)] => match index.get(word) {
                Val::Option(None) => {
                    // Outside the loaded shard: its file, where the compiled
                    // rule says it is, if kiokun has one.
                    if let Some(known) = read.lock().expect("cache").get(word) {
                        return Ok(vec![Val::Option(known.clone().map(Box::new))]);
                    }
                    let (_, sub) = placement(&place, word)?;
                    let path = dir.join(&sub).join(crate::shard::file_name(word));
                    // kiokun's escape gives several words one file name:
                    // the file is this word's only if it records this word.
                    let found = match path.exists() {
                        true => {
                            let raw = crate::shard::read_entry(&path)?;
                            crate::shard::key_of(&raw)
                                .is_none_or(|k| &k == word)
                                .then(|| crate::data::entry(&raw))
                        }
                        false => None,
                    };
                    read.lock()
                        .expect("cache")
                        .insert(word.clone(), found.clone());
                    Ok(vec![Val::Option(found.map(Box::new))])
                }
                known => Ok(vec![known]),
            },
            other => Err(format!("entries#get received {other:?}")),
        });
        let index = self.index.clone();
        let candidates: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(q)] => Ok(vec![Val::List(index.candidates(q))]),
            other => Err(format!("index#candidates received {other:?}")),
        });
        let index = self.index.clone();
        let fold: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(q)] => Ok(vec![Val::String(index.fold(q))]),
            other => Err(format!("index#fold received {other:?}")),
        });
        let index = self.index.clone();
        let alias: HostFn = Arc::new(move |args: &[Val]| match args {
            [Val::String(q)] => Ok(vec![Val::Option(
                index.alias(q).map(|a| Box::new(Val::String(a))),
            )]),
            other => Err(format!("index#alias received {other:?}")),
        });
        BTreeMap::from([
            ("kiokun:data/entries#get".to_string(), get),
            ("kiokun:data/index#candidates".to_string(), candidates),
            ("kiokun:data/index#fold".to_string(), fold),
            ("kiokun:data/index#alias".to_string(), alias),
        ])
    }

    /// The compiled `Lookup`: the entry a word opens, following one redirect.
    pub fn lookup(&self, word: &str) -> Result<Option<Val>, String> {
        match self
            .lookup
            .call(&self.data_layer(), &[Val::String(word.to_string())])?
        {
            Val::Option(found) => Ok(found.map(|b| *b)),
            other => Err(format!("Lookup returned {other:?}")),
        }
    }

    /// The compiled `Search`.
    pub fn search(&self, q: &str) -> Result<Vec<Val>, String> {
        match self
            .search
            .call(&self.data_layer(), &[Val::String(q.to_string())])?
        {
            Val::List(hits) => Ok(hits),
            other => Err(format!("Search returned {other:?}")),
        }
    }

    fn render(&self, name: &str, title: &str, env: Env) -> Result<String, String> {
        let t = self
            .templates
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("no template `{name}`"))?;
        let body = pw_render::render(t, &env, &self.templates).map_err(|e| e.to_string())?;
        Ok(document(title, &body))
    }

    /// `/<word>`: `WordPage`, which takes the compiled `Lookup`'s `Option`
    /// apart itself (ADR-0042); 404 when it is `None`.
    pub fn entry_page(&self, word: &str) -> Result<Page, String> {
        let found = self.lookup(word)?;
        let key = match found.as_ref() {
            Some(Val::Record(fields)) => fields.iter().find_map(|(k, v)| match (k.as_str(), v) {
                ("key", Val::String(key)) => Some(key.clone()),
                _ => None,
            }),
            _ => None,
        };
        let status = if found.is_some() { 200 } else { 404 };
        let entry = value(&Val::Option(found.map(Box::new))).ok_or("the entry has no value")?;
        Ok(Page {
            status,
            html: self.render(
                "WordPage",
                key.as_deref().unwrap_or(word),
                Env::new()
                    .set("word", Value::Text(word.to_string()))
                    .set("entry", entry),
            )?,
        })
    }

    /// `/search?q=`: the compiled `Search`'s hits.
    pub fn search_page(&self, q: &str) -> Result<Page, String> {
        let hits = self.search(q)?;
        let hits: Vec<Value> = hits.iter().filter_map(value).collect();
        Ok(Page {
            status: 200,
            html: self.render(
                "SearchPage",
                &format!("{q} — search"),
                Env::new()
                    .set("q", Value::Text(q.to_string()))
                    .set("hits", Value::List(hits)),
            )?,
        })
    }
}

/// A component value as the renderer reads it. A WIT record field is named in
/// kebab-case and a template reads the Pleris name, so `unit-price` becomes
/// `unit_price`. An `Option` or a `Result` is a variant, which a template
/// takes apart with `{#match}` (ADR-0042); so is a declared sum type's case,
/// by its WIT name, its fields a list when it has several (ADR-0061).
pub fn value(v: &Val) -> Option<Value> {
    let case = |case: &str, payload: Option<&Val>| {
        Some(Value::Variant {
            case: case.to_string(),
            payload: match payload {
                Some(p) => Some(Box::new(value(p)?)),
                None => None,
            },
        })
    };
    Some(match v {
        Val::String(s) => Value::Text(s.clone()),
        Val::Bool(b) => Value::Bool(*b),
        Val::S64(n) => Value::Int(*n),
        Val::List(items) => Value::List(items.iter().filter_map(value).collect()),
        Val::Record(fields) => Value::Record(
            fields
                .iter()
                .filter_map(|(k, v)| Some((k.replace('-', "_"), value(v)?)))
                .collect(),
        ),
        Val::Option(Some(inner)) => case("Some", Some(inner))?,
        Val::Option(None) => case("None", None)?,
        Val::Result(Ok(ok)) => case("Ok", ok.as_deref())?,
        Val::Result(Err(err)) => case("Err", err.as_deref())?,
        Val::Variant(name, payload) => case(name, payload.as_deref())?,
        // A case's several fields (ADR-0061).
        Val::Tuple(parts) => Value::List(parts.iter().map(value).collect::<Option<_>>()?),
        _ => return None,
    })
}

/// The HTML shell. No script: these pages are static, as kiokun.com's
/// entry pages are readable without JavaScript.
fn document(title: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"zh-Hant\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{} — kiokun slice</title>\n</head>\n<body>\n{body}\n</body>\n</html>\n",
        pw_render::escape::text(title)
    )
}
