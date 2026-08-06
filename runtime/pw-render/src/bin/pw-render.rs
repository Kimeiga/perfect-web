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

use pw_render::{Env, Template, Value, render};

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(out_dir) = flag("--out") else {
        eprintln!("usage: pw-render --out DIR [--values FILE] [--wrap TITLE] < template-ir.json");
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

    let env = match flag("--values") {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(s) => match values_from_json(&s) {
                Ok(env) => env,
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
        None => Env::new(),
    };

    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("pw-render: cannot create {out_dir}: {e}");
        return std::process::ExitCode::from(2);
    }

    let mut written = 0usize;
    for t in &templates {
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
        let title = flag("--wrap").unwrap_or_else(|| t.name.clone());
        let page = document(&title, &body);
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
/// Minimal on purpose: a doctype, a charset, a language and a title. Anything
/// more would be this binary deciding what a page contains, and the page is the
/// `.pw` file's business.
///
/// The doctype matters and is not decoration — without it the browser parses in
/// quirks mode, where the tree and the layout both differ, so gate 4 would be
/// measuring a document nobody ships.
fn document(title: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>{}</title>\n</head>\n<body>\n{body}\n</body>\n</html>\n",
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
            let mut m = BTreeMap::new();
            for (k, v) in o {
                m.insert(k.clone(), convert(v)?);
            }
            Value::Record(m)
        }
        serde_json::Value::Null => return Err("null has no rendering".into()),
    })
}
