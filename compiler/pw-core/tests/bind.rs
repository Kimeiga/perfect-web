//! **An input bound to a signal** (ADR-0142).
//!
//! `bind:value={s}` is lowered to `value={s}` and an `on:input` handler
//! setting `s` to what was typed. The browser sets an attribute a signal
//! decides in place, and a block a signal decides is rendered again only for
//! what it cannot set in place. Until 2026-10-02 `bind:value` was blocked at
//! build, an attribute a signal decides was refused, and a block rendered
//! again for every signal read anywhere in it, so a field bound inside one
//! would have been replaced at each key pressed.
//!
//! Each test states one part, with controls.

use pw_core::backend::wasm::Encoding;
use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn library() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out
}

fn reported(src: &str) -> Vec<String> {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    check_sources(&sources)
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| ds.into_iter().map(|d| format!("{} {}", d.code, d.message)))
        .collect()
}

fn build(src: &str) -> pw_core::build::Build {
    let mut sources = library();
    sources.push(("t.pw".to_string(), src.to_string()));
    let units: Vec<Unit> = sources
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    pw_core::build::build(&units).expect("builds")
}

/// A page of signals whose view holds `markup`.
fn page(markup: &str) -> String {
    format!(
        "module t\n\npage P() {{\n    cache private\n\n    signal name: String = \"Ada\"\n    \
         signal count: Int = 0\n    signal editing: Bool = false\n    let fixed = \"x\"\n\n    \
         view {{\n        <main><button type=\"button\" on:press={{() => editing = true}}>Edit</button>\
         {markup}<p>{{name}} {{count}}</p></main>\n    }}\n}}\n"
    )
}

const FIELD: &str = "<input type=\"text\" aria-label=\"Name\" bind:value={name} />";

fn plan(b: &pw_core::build::Build) -> pw_core::page_values::PageValues {
    b.pages
        .iter()
        .find(|p| p.page == "t.P")
        .expect("a plan")
        .plan
        .clone()
        .expect("planned")
}

#[test]
fn a_binding_is_a_value_and_a_handler() {
    let src = page(FIELD);
    assert_eq!(reported(&src), Vec::<String>::new());
    let b = build(&src);
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
    let t = b
        .templates
        .iter()
        .find(|t| t.path == "t.P")
        .expect("the page");
    let manifest = t.manifest();
    assert!(
        manifest
            .iter()
            .any(|e| e.kind == "attribute" && e.value == "name"),
        "{manifest:#?}"
    );
    assert!(
        manifest
            .iter()
            .any(|e| e.kind == "event" && e.event == "input"),
        "{manifest:#?}"
    );
    // The browser sets the field's value in place.
    let live: Vec<(String, String)> = plan(&b)
        .live
        .iter()
        .filter(|l| l.kind == "attribute")
        .map(|l| (l.attribute.clone(), l.signal.clone()))
        .collect();
    assert_eq!(live, [("value".to_string(), "name".to_string())]);
    // And the handler sets the signal to what was typed, under Node.
    let module = b
        .handlers
        .iter()
        .find_map(|h| match &h.module {
            Encoding::Encoded(m) if m.source.contains("context.event") => Some(m.clone()),
            _ => None,
        })
        .expect("the binding's handler");
    let temp = tempfile::TempDir::with_prefix("pw-bind-").expect("a temporary directory");
    let dir = temp.path().to_path_buf();
    std::fs::write(dir.join("handler.mjs"), &module.source).expect("write");
    std::fs::write(
        dir.join("run.mjs"),
        "import { run } from \"./handler.mjs\";\nconst sets = [];\n\
         await run({ captures: {}, event: { value: \"Grace\" }, get: () => undefined,\n  \
         set: (n, v) => sets.push([n, v]), command: async () => ({ $case: \"ok\" }) });\n\
         console.log(JSON.stringify(sets));\n",
    )
    .expect("write");
    let out = std::process::Command::new("node")
        .arg(dir.join("run.mjs"))
        .output()
        .expect("node");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        r#"[["name","Grace"]]"#,
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_binding_binds_a_signal_of_string_by_its_name() {
    for (markup, expected) in [
        (
            "<input type=\"text\" aria-label=\"N\" bind:value={count} />",
            "PW5304 `bind:value` binds `count`, a signal of `Int`",
        ),
        (
            "<input type=\"text\" aria-label=\"N\" bind:value={fixed} />",
            "PW5304 `bind:value` binds `fixed`, which is not a signal",
        ),
        (
            "<p bind:value={name}>x</p>",
            "PW5304 `<p>` has no value a person types",
        ),
        (
            "<input type=\"checkbox\" aria-label=\"N\" bind:checked={name} />",
            "PW5304 `bind:checked` binds `value`, to a signal's name",
        ),
    ] {
        let found = reported(&page(markup));
        assert_eq!(found.len(), 1, "{markup}: {found:#?}");
        assert!(found[0].starts_with(expected), "{markup}: {found:#?}");
    }
}

#[test]
fn a_block_is_rendered_again_only_for_what_it_cannot_set_in_place() {
    let src = page(&format!(
        "{{#if editing}}<section>{FIELD}<p>{{name}}</p></section>{{/if}}"
    ));
    assert_eq!(reported(&src), Vec::<String>::new());
    let planned = plan(&build(&src));
    let block = planned
        .live
        .iter()
        .find(|l| l.kind == "conditional")
        .expect("the block");
    // Typing sets `name`, which the block reads only where the browser sets
    // it in place: the block is not rendered again, and its field stays.
    assert_eq!(block.reads, ["editing"]);
    assert!(
        planned
            .live
            .iter()
            .any(|l| l.kind == "text" && l.signal == "name" && l.part > block.part),
        "{:#?}",
        planned.live
    );
    // Control: a list of the signal inside the block is the block's to
    // render again.
    let list = page("{#if editing}<ul>{#each tags as t (t)}<li>{t}</li>{/each}</ul>{/if}").replace(
        "    let fixed = \"x\"",
        "    let fixed = \"x\"\n    signal tags: List<String> = [\"a\"]",
    );
    assert_eq!(reported(&list), Vec::<String>::new());
    let planned = plan(&build(&list));
    let block = planned
        .live
        .iter()
        .find(|l| l.kind == "conditional")
        .expect("the block");
    assert_eq!(block.reads, ["editing", "tags"]);
}
