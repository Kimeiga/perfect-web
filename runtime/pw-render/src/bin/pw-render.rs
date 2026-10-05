//! Render checked template IR to HTML files.
//!
//! ```text
//! pw emit-template page.pw | pw-render --out DIR --values values.json
//! ```
//!
//! The E7 renderer's command-line face, and the thing charter §14 M7 gate 1
//! asks for: a `.pw` page becomes bytes with no Marko anywhere in the pipeline.
//!
//! Values arrive as JSON rather than being evaluated here. Evaluation is the
//! program's job and it has already happened; this turns values plus IR into
//! bytes, which is the one thing task 2 is about.

use std::collections::BTreeMap;
use std::io::Read;

use pw_render::{Env, IdentityDomain, Partition, Template, Value, render};

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(out_dir) = flag("--out") else {
        eprintln!(
            "usage: pw-render --out DIR [--values FILE] [--plan FILE] [--resume FILE] \
             [--runtime SRC] [--document KEY] [--partition P] \
             [--compatibility GEN] [--identity-key K] [--wrap TITLE] \
             < template-ir.json"
        );
        return std::process::ExitCode::from(2);
    };

    let mut ir = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut ir) {
        eprintln!("pw-render: cannot read the template IR: {e}");
        return std::process::ExitCode::from(2);
    }
    let templates: Vec<Template> = match serde_json::from_str(&ir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("pw-render: the template IR does not parse: {e}");
            return std::process::ExitCode::from(2);
        }
    };

    // Who shares an identity with whom. E6 already decided it — the
    // materialization's physical key, or the document's route identity and its
    // partition — so this reads that answer rather than inventing one.
    //
    // `--partition public|session:ID|user:ID`. A caller cannot pass a session
    // as the domain of a public fragment by accident, because the partition is
    // its own field and a public one names no principal.
    let domain = IdentityDomain::document(
        &flag("--document").unwrap_or_else(|| "document".to_string()),
        match flag("--partition").as_deref() {
            Some(p) if p.starts_with("session:") => Partition::Session {
                id: p[8..].to_string(),
            },
            Some(p) if p.starts_with("user:") => Partition::User {
                id: p[5..].to_string(),
            },
            _ => Partition::Public,
        },
        &flag("--compatibility").unwrap_or_else(|| "dev".to_string()),
    );
    let domain = match flag("--identity-key") {
        Some(k) => domain.keyed(&k),
        // Left at the development key, and the token contract says what that
        // costs: tokens are enumerable by anyone who knows the document and can
        // guess the application keys.
        None => domain,
    };

    // The names the values give: a private list they do not give is empty
    // in a render no session asked for (ADR-0145).
    let mut given = std::collections::BTreeSet::new();
    let env = match flag("--values") {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(s) => match values_from_json(&s) {
                Ok(env) => {
                    if let Ok(serde_json::Value::Object(o)) = serde_json::from_str(&s) {
                        given.extend(o.keys().cloned());
                    }
                    env.in_domain(domain.clone())
                }
                Err(e) => {
                    eprintln!("pw-render: {path}: {e}");
                    return std::process::ExitCode::from(2);
                }
            },
            Err(e) => {
                eprintln!("pw-render: cannot read {path}: {e}");
                return std::process::ExitCode::from(2);
            }
        },
        None => Env::new().in_domain(domain.clone()),
    };

    // The page's signals, at their first values (ADR-0140): what `pw build`'s
    // plan for the page says, so a page that holds UI state renders here as
    // the server renders it.
    // The page the plan is for, and its blocks a session's query decides
    // that the values do not give: this render is no session's, and renders
    // nothing there (ADR-0146).
    let mut unasked: Vec<(String, u32)> = Vec::new();
    let env =
        match flag("--plan") {
            Some(path) => {
                let plan = std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|s| {
                        serde_json::from_str::<serde_json::Value>(&s).map_err(|e| e.to_string())
                    });
                match plan {
                    Ok(plan) => {
                        let mut env = env;
                        for s in plan["signals"].as_array().into_iter().flatten() {
                            let name = s["name"].as_str().unwrap_or_default();
                            match Value::from_wire(&s["initial"]) {
                                Ok(v) => env = env.set(name, v),
                                Err(e) => {
                                    eprintln!("pw-render: {path}: the signal `{name}`: {e}");
                                    return std::process::ExitCode::from(2);
                                }
                            }
                        }
                        // A list a session's query fills, the values not giving
                        // it: this render is no session's, and no session's list
                        // is empty, as its cart's count is 0 (ADR-0145).
                        for c in plan["collections"].as_array().into_iter().flatten() {
                            let Some(name) = c.as_str() else { continue };
                            let private =
                                plan["bindings"].as_array().into_iter().flatten().any(|b| {
                                    b["binding"] == name && b["policy"]["cache"] == "private"
                                });
                            if private && !given.contains(name) {
                                env = env.set(name, Value::List(Vec::new()));
                            }
                        }
                        // And a text part a session's query decides, which
                        // this render, no session's, shows nothing of: as its
                        // list is empty and its block shows no arm. Found by
                        // T10, whose store shows a session's estimate as text,
                        // and failed to build here.
                        for part in plan["parts"].as_array().into_iter().flatten() {
                            let path = part["path"].as_str().unwrap_or_default();
                            let binding = part["binding"].as_str().unwrap_or_default();
                            let private =
                                plan["bindings"].as_array().into_iter().flatten().any(|b| {
                                    b["binding"] == binding && b["policy"]["cache"] == "private"
                                });
                            if private && !given.contains(path) {
                                env = env.set(path, Value::Text(String::new()));
                            }
                        }
                        let page = plan["page"].as_str().unwrap_or_default();
                        for b in plan["blocks"].as_array().into_iter().flatten() {
                            let id = b.as_u64().unwrap_or_default() as u32;
                            let root = templates
                                .iter()
                                .filter(|t| t.path == page)
                                .find_map(|t| subject_of(&t.chunks, id));
                            let private = |name: &str| {
                                plan["bindings"].as_array().into_iter().flatten().any(|b| {
                                    b["binding"] == name && b["policy"]["cache"] == "private"
                                })
                            };
                            if let Some(root) = root
                                && private(&root)
                                && !given.contains(&root)
                            {
                                unasked.push((page.to_string(), id));
                            }
                        }
                        // A region whose query the page waits for, which this
                        // render ran for no request: rendered as nothing, as a
                        // block a session's query decides is. A streamed one
                        // shows its placeholder, as a document does before its
                        // query settles (ADR-0148).
                        for s in plan["streams"].as_array().into_iter().flatten() {
                            if s["streamed"] != true {
                                let id = s["part"].as_u64().unwrap_or_default() as u32;
                                unasked.push((page.to_string(), id));
                            }
                        }
                        env
                    }
                    Err(e) => {
                        eprintln!("pw-render: {path}: {e}");
                        return std::process::ExitCode::from(2);
                    }
                }
            }
            None => env,
        };

    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("pw-render: cannot create {out_dir}: {e}");
        return std::process::ExitCode::from(2);
    }

    // The resume metadata the page presents to `decide`, as data the server
    // supplies rather than as a string the client composes. A test that wants
    // an INCOMPATIBLE manifest changes this file; it does not edit the runtime,
    // which is what makes the refusal path testable at all.
    let resume = match flag("--resume") {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("pw-render: cannot read {path}: {e}");
                return std::process::ExitCode::from(2);
            }
        },
        None => None,
    };

    let mut written = 0usize;
    for t in &templates {
        let mut env = env.clone();
        for (page, id) in &unasked {
            if *page == t.path {
                env = env.materialized(pw_render::PartId(*id), "");
            }
        }
        let body = match render(t, &env, &templates) {
            Ok(b) => b,
            Err(e) => {
                // Refused, not skipped. A renderer that wrote the pages it
                // could and stayed quiet about the rest would produce a site
                // that looks complete.
                eprintln!("pw-render: {}: {e}", t.path);
                return std::process::ExitCode::FAILURE;
            }
        };
        // The page's own title, where it states one (ADR-0183). A view
        // rendered as a document is titled by `--wrap`, or by its name.
        let title = match pw_render::title_text(t, &env) {
            Ok(Some(title)) => title,
            Ok(None) => flag("--wrap").unwrap_or_else(|| t.name.clone()),
            Err(e) => {
                eprintln!("pw-render: {}: its title: {e}", t.path);
                return std::process::ExitCode::FAILURE;
            }
        };
        let manifest = t.manifest();
        // And what it says of itself to what reads it unshown (ADR-0186).
        let metadata = match pw_render::head_metadata(t, &env) {
            Ok(metadata) => metadata,
            Err(e) => {
                eprintln!("pw-render: {}: its metadata: {e}", t.path);
                return std::process::ExitCode::FAILURE;
            }
        };
        let page = document(
            &title,
            &metadata,
            &body,
            t,
            &manifest,
            flag("--runtime").as_deref(),
            resume.as_deref(),
        );
        let path = format!("{out_dir}/{}.html", t.name);
        if let Err(e) = std::fs::write(&path, page) {
            eprintln!("pw-render: cannot write {path}: {e}");
            return std::process::ExitCode::from(2);
        }
        written += 1;
    }
    eprintln!("pw-render: {written} file(s) written to {out_dir}");
    std::process::ExitCode::SUCCESS
}

/// The document shell.
///
/// Minimal on purpose: a doctype, a charset, a language, the viewport and a
/// title. Anything more would be this binary deciding what a page contains,
/// and the page is the `.pw` file's business.
///
/// The doctype matters and is not decoration — without it the browser parses in
/// quirks mode, where the tree and the layout both differ, so gate 4 would be
/// measuring a document nobody ships. The viewport is the same for a phone
/// (ADR-0182): without it a phone lays the page out 980 CSS pixels wide and
/// shows it shrunk, where text is too small to read until it is zoomed, and
/// zoomed it scrolls sideways.
fn document(
    title: &str,
    metadata: &str,
    body: &str,
    t: &Template,
    manifest: &[pw_render::PartEntry],
    runtime: Option<&str>,
    resume: Option<&str>,
) -> String {
    // The parts manifest travels with the document, as data. A page with no
    // dynamic part gets neither the manifest nor the runtime — charter §14 M7
    // gate 2: a static route ships no browser runtime, and "no dynamic parts"
    // is exactly when that is true.
    //
    // Its title and metadata are parts of its head, written as it is served,
    // and alone they leave the runtime nothing to do: the runtime keeps a
    // title current only as the parts in the body it changes with are.
    // Until 2026-10-04 a static page that stated its title shipped both (a
    // correction to ADR-0183).
    let active = manifest
        .iter()
        .any(|e| e.anchor != pw_render::Anchor::Document);
    let mut tail = String::new();
    if let Some(src) = runtime.filter(|_| active) {
        let resume: serde_json::Value = resume
            .and_then(|r| serde_json::from_str(r).ok())
            .unwrap_or(serde_json::Value::Null);
        let json = serde_json::json!({
            "template": t.path,
            "schema": t.schema,
            "parts": manifest,
            "resume": resume,
        });
        // Escaping the JSON as HTML would corrupt it; `json_in_script` keeps
        // the element's text free of `<`, which is what ends it (ADR-0097).
        let json =
            pw_render::escape::json_in_script(&serde_json::to_string(&json).unwrap_or_default());
        tail.push_str(&format!(
            "<script type=\"application/json\" id=\"pw-parts\">{json}</script>\n"
        ));
        tail.push_str(&format!(
            "<script type=\"module\" src=\"{}\"></script>\n",
            pw_render::escape::attribute(src)
        ));
    }
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n{metadata}</head>\n<body>\n{body}\n{tail}</body>\n</html>\n",
        pw_render::escape::text(title)
    )
}

fn values_from_json(s: &str) -> Result<Env, String> {
    let raw: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
    let obj = raw.as_object().ok_or("values must be a JSON object")?;
    let mut env = Env::new();
    for (k, v) in obj {
        if k == "$grants" {
            for g in v.as_array().ok_or("$grants must be an array")? {
                env = env.grant(g.as_str().ok_or("a grant must be a string")?);
            }
            continue;
        }
        env = env.set(k, convert(v)?);
    }
    Ok(env)
}

fn convert(v: &serde_json::Value) -> Result<Value, String> {
    Ok(match v {
        serde_json::Value::String(s) => Value::Text(s.clone()),
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            Value::Int(n.as_i64().ok_or("only integer numbers are supported")?)
        }
        serde_json::Value::Array(a) => {
            Value::List(a.iter().map(convert).collect::<Result<_, _>>()?)
        }
        serde_json::Value::Object(o) => {
            // `{"$raw": "...", "$capability": "..."}` is the only way to build
            // a raw value from JSON, and it must name its capability. A boolean
            // flag would be something a caller passes by accident.
            if let Some(html) = o.get("$raw") {
                let capability = o
                    .get("$capability")
                    .and_then(|c| c.as_str())
                    .ok_or("a $raw value must name its $capability")?;
                return Ok(Value::Raw {
                    html: html.as_str().ok_or("$raw must be a string")?.to_string(),
                    capability: capability.to_string(),
                });
            }
            // A case, in the wire form a page's values travel in
            // (`Value::from_wire`): `{"$case": "None"}`, or `{"$case": "Some",
            // "value": ...}`. Until 2026-10-04 a values file could state no
            // case, and a page that matches on an option could not be
            // rendered from one (ADR-0193).
            if let Some(case) = o.get("$case") {
                let case = case.as_str().ok_or("$case must be a string")?;
                return Ok(Value::Variant {
                    case: case.to_string(),
                    payload: o.get("value").map(convert).transpose()?.map(Box::new),
                });
            }
            let mut m = BTreeMap::new();
            for (k, v) in o {
                m.insert(k.clone(), convert(v)?);
            }
            Value::Record(m)
        }
        serde_json::Value::Null => return Err("null has no rendering".into()),
    })
}

/// The root of the value the block numbered `id` decides, where it is.
fn subject_of(chunks: &[pw_render::Chunk], id: u32) -> Option<String> {
    use pw_render::{Chunk, Part};
    for c in chunks {
        let Chunk::Dynamic(p) = c else { continue };
        match p {
            Part::Conditional { id: at, value, .. } | Part::Match { id: at, value, .. }
                if at.0 == id =>
            {
                return value.split('.').next().map(str::to_string);
            }
            _ => {}
        }
        for region in p.nested() {
            if let Some(found) = subject_of(region, id) {
                return Some(found);
            }
        }
    }
    None
}
