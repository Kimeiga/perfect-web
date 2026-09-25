//! **The slice, served**: the compiled queries run through the E8 host with
//! the shard as their data layer, and the compiled pages render the result.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

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
        let out = self.prepared.call_within(
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
        out.into_iter()
            .next()
            .ok_or_else(|| format!("`{}` returned nothing", self.contract.component_id))
    }
}

/// The slice: its shard, its two compiled queries, its three pages.
pub struct App {
    pub index: Arc<Index>,
    templates: Vec<Template>,
    lookup: Query,
    search: Query,
}

/// A page to send: its status and its HTML.
pub struct Page {
    pub status: u16,
    pub html: String,
}

impl App {
    /// `build` is a `pw build` output directory for `examples/kiokun/`.
    pub fn load(build: &Path, index: Index) -> Result<App, String> {
        let read = |name: &str| {
            let path = build.join(name);
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        let contracts = ComponentContract::from_json(&read("contracts.json")?)?;
        let templates: Vec<Template> =
            serde_json::from_str(&read("templates.json")?).map_err(|e| e.to_string())?;
        Ok(App {
            index: Arc::new(index),
            lookup: Query::load(build, &contracts, "kiokun.page.Lookup")?,
            search: Query::load(build, &contracts, "kiokun.page.Search")?,
            templates,
        })
    }

    /// The deployment's data layer, as host operations.
    fn data_layer(&self) -> BTreeMap<String, HostFn> {
        let get = self.index.clone();
        let search = self.index.clone();
        BTreeMap::from([
            (
                "kiokun:data/entries#get".to_string(),
                Arc::new(move |args: &[Val]| match args {
                    [Val::String(word)] => Ok(vec![get.get(word)]),
                    other => Err(format!("entries#get received {other:?}")),
                }) as HostFn,
            ),
            (
                "kiokun:data/index#search".to_string(),
                Arc::new(move |args: &[Val]| match args {
                    [Val::String(q), Val::S64(limit)] => Ok(vec![Val::List(
                        search.search(q, (*limit).clamp(0, 100) as usize),
                    )]),
                    other => Err(format!("index#search received {other:?}")),
                }) as HostFn,
            ),
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

    /// `/<word>`: the entry, or 404 with the not-found page.
    pub fn entry_page(&self, word: &str) -> Result<Page, String> {
        match self.lookup(word)? {
            Some(entry) => {
                let value = value(&entry).ok_or("the entry has no value")?;
                let key = match &value {
                    Value::Record(fields) => match fields.get("key") {
                        Some(Value::Text(k)) => k.clone(),
                        _ => word.to_string(),
                    },
                    _ => word.to_string(),
                };
                Ok(Page {
                    status: 200,
                    html: self.render("EntryPage", &key, Env::new().set("entry", value))?,
                })
            }
            None => Ok(Page {
                status: 404,
                html: self.render(
                    "NotFound",
                    word,
                    Env::new().set("word", Value::Text(word.to_string())),
                )?,
            }),
        }
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
/// `unit_price`. `None` is absent: a template that reads it is refused by the
/// renderer rather than shown an invented empty value.
pub fn value(v: &Val) -> Option<Value> {
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
        Val::Option(Some(inner)) => value(inner)?,
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
