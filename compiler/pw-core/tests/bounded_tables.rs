//! Generated table reads obey the same loop and source-cost rules as
//! handwritten reads. Use D1's real list/item declarations and nominal keys.
use pw_core::check::check_sources;
use pw_core::diagnostics::Diagnostic;
use std::path::Path;

fn diagnostics(declarations: &str, sets: Option<&str>) -> Vec<Diagnostic> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for directory in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<_> = std::fs::read_dir(root.join(directory))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "pw"))
            .collect();
        paths.sort();
        for path in paths {
            files.push((
                path.display().to_string(),
                std::fs::read_to_string(path).unwrap(),
            ));
        }
    }
    for name in ["Lists", "Items", "ListData"] {
        let mut text =
            std::fs::read_to_string(root.join(format!("examples/tables/{name}.pw"))).unwrap();
        if name == "Items" {
            // A composite index beside the real one-field index.
            text.push_str("    index by_list_done on list, done\n");
        }
        if name == "ListData" {
            text = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("sets "))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            if let Some(sets) = sets {
                text.push_str(&format!("    sets {sets}\n"));
            }
        }
        files.push((format!("{name}.pw"), text));
    }
    files.push(("Probe.pw".into(), format!("{IMPORTS}\n{declarations}")));
    let results = check_sources(&files);
    for (path, ds) in &results {
        if path != "Probe.pw" {
            assert!(ds.is_empty(), "fixture {path}: {ds:#?}");
        }
    }
    results
        .into_iter()
        .filter(|(p, _)| p == "Probe.pw")
        .flat_map(|(_, ds)| ds)
        .collect()
}

const IMPORTS: &str = "module Probe\n\nimport List\nimport Lists\nimport Lists.{ ListId, Slug, TodoList }\nimport Items\nimport Items.{ ItemId, Item, ItemsListId, ItemsListDone }\n";

#[test]
fn generated_key_prefix_and_index_repairs_name_the_matching_many_read() {
    for (parameters, answer, call, many, fields) in [
        (
            "keys: List<ListId>",
            "List<Option<TodoList>>",
            "List.map(keys, key => Lists.get(key))",
            "Lists.get_many",
            vec!["id"],
        ),
        (
            "keys: List<ItemsListId>",
            "List<Option<Item>>",
            "List.map(keys, key => Items.get(key.list, key.id))",
            "Items.get_many",
            vec!["list", "id"],
        ),
        (
            "keys: List<ListId>",
            "List<List<Item>>",
            "List.map(keys, key => Items.of_list(key))",
            "Items.of_list_many",
            vec!["list"],
        ),
        (
            "keys: List<Bool>",
            "List<List<Item>>",
            "List.map(keys, key => Items.by_done(key))",
            "Items.by_done_many",
            vec!["done"],
        ),
        (
            "keys: List<Slug>",
            "List<Option<TodoList>>",
            "List.map(keys, key => Lists.by_slug(key))",
            "Lists.by_slug_many",
            vec!["slug"],
        ),
        (
            "keys: List<ItemsListDone>",
            "List<List<Item>>",
            "List.map(keys, key => Items.by_list_done(key.list, key.done))",
            "Items.by_list_done_many",
            vec!["list", "done"],
        ),
    ] {
        let src = format!(
            "fn per({parameters}) -> {answer} !{{ database.read<Lists>, database.read<Items> }} {{\n    {call}\n}}\n"
        );
        let ds = diagnostics(&src, Some("one_call"));
        assert_eq!(
            ds.iter().map(|d| d.code).collect::<Vec<_>>(),
            ["PW0405"],
            "{many}: {ds:#?}"
        );
        let help = &ds[0].repairs[0].description;
        assert!(help.contains(many), "{many}: {help}");
        assert!(help.contains("before the loop"), "{help}");
        assert!(help.contains("matching rows"), "{help}");
        if fields.len() > 1 {
            assert!(help.contains("key records of"), "{many}: {help}");
            let record = if many == "Items.get_many" {
                "Items.ItemsListId"
            } else {
                "Items.ItemsListDone"
            };
            assert!(help.contains(record), "{many}: {help}");
        } else {
            assert!(help.contains("scalar values"), "{many}: {help}");
        }
        for field in fields {
            assert!(help.contains(&format!("`{field}`")), "{many}: {help}");
        }
        assert!(
            !help.contains("List.map("),
            "a descriptive repair must not invent a scalar composite key: {help}"
        );
    }
}

const BATCHED: &str = r#"
fn get_lists(keys: List<ListId>) -> List<TodoList> !{ database.read<Lists> } {
    Lists.get_many(keys)
}
fn get_items(keys: List<ItemsListId>) -> List<Item> !{ database.read<Items> } {
    Items.get_many(keys)
}
fn prefix(keys: List<ListId>) -> List<Item> !{ database.read<Items> } {
    Items.of_list_many(keys)
}
fn index(keys: List<Bool>) -> List<Item> !{ database.read<Items> } {
    Items.by_done_many(keys)
}
fn unique_index(keys: List<Slug>) -> List<TodoList> !{ database.read<Lists> } {
    Lists.by_slug_many(keys)
}
fn composite_index(keys: List<ItemsListDone>) -> List<Item> !{ database.read<Items> } {
    Items.by_list_done_many(keys)
}
"#;

#[test]
fn generated_many_reads_warn_for_default_and_per_key_sources() {
    for sets in [None, Some("per_key")] {
        let ds = diagnostics(BATCHED, sets);
        assert_eq!(ds.len(), 6, "{sets:?}: {ds:#?}");
        assert!(ds.iter().all(|d| d.code == "PW0406"), "{sets:?}: {ds:#?}");
    }
}

#[test]
fn generated_many_reads_accept_one_call_and_chunked_sources() {
    for sets in ["one_call", "chunked(50)"] {
        assert!(diagnostics(BATCHED, Some(sets)).is_empty(), "{sets}");
    }
}

#[test]
fn a_generated_many_read_before_the_loop_selects_each_turns_matching_rows() {
    let src = r#"
fn together(keys: List<ListId>) -> List<List<Item>> !{ database.read<Items> } {
    let found = Items.of_list_many(keys)
    List.map(keys, key => List.filter(found, row => row.list == key))
}
"#;
    assert!(diagnostics(src, Some("one_call")).is_empty());
}

#[test]
fn a_generated_read_named_as_the_callback_has_the_same_matching_row_repair() {
    let src = r#"
fn per(keys: List<ListId>) -> List<Option<TodoList>> !{ database.read<Lists> } {
    List.map(keys, Lists.get)
}
"#;
    let ds = diagnostics(src, Some("one_call"));
    assert_eq!(
        ds.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["PW0405"],
        "{ds:#?}"
    );
    let help = &ds[0].repairs[0].description;
    assert!(help.contains("Lists.get_many"), "{help}");
    assert!(help.contains("matching rows"), "{help}");
    assert!(help.contains("`id`"), "{help}");
}

// Integrator draft: append to bounded_tables.rs only after the generic merge.
// These use that suite's real D1 fixtures and canonical checker.
#[test]
fn a_generated_read_in_a_generic_result_callback_remains_a_read_per_row() {
    let src = r#"
fn per(given: List<Item>) -> List<List<Item>> !{ database.read<Items> } {
    List.map(List.reverse(given), row => Items.of_list(row.list))
}
"#;
    let ds = diagnostics(src, Some("one_call"));
    assert_eq!(
        ds.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["PW0405"],
        "{ds:#?}"
    );
    assert!(ds[0].repairs[0].description.contains("Items.of_list_many"));
}

#[test]
fn a_generated_batch_before_generic_row_selection_keeps_its_source_cost() {
    let src = r#"
fn together(keys: List<ListId>) -> List<List<Item>> !{ database.read<Items> } {
    let found = Items.of_list_many(keys)
    List.map(List.reverse(keys), key => List.filter(found, row => row.list == key))
}
"#;
    assert!(diagnostics(src, Some("one_call")).is_empty());
    assert!(diagnostics(src, Some("chunked(50)")).is_empty());
    let ds = diagnostics(src, Some("per_key"));
    assert_eq!(
        ds.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["PW0406"],
        "{ds:#?}"
    );
}

#[test]
fn a_generated_read_through_a_member_stays_a_read_per_row() {
    let results: Vec<_> = ["rows(row)", "row.rows", "row.rows()"]
        .into_iter()
        .map(|call| {
            let src = format!(
                "fn rows(row: Item) -> List<Item> !{{ database.read<Items> }} {{\n    Items.of_list(row.list)\n}}\n\nfn per(given: List<Item>) -> List<List<Item>> !{{ database.read<Items> }} {{\n    List.map(List.reverse(given), row => {call})\n}}\n"
            );
            let ds = diagnostics(&src, Some("one_call"));
            let codes: Vec<_> = ds.iter().map(|d| d.code).collect();
            let pure = src.replace(
                "fn rows(row: Item) -> List<Item> !{ database.read<Items> } {\n    Items.of_list(row.list)\n}",
                "fn rows(row: Item) -> List<Item> !{} { [] }",
            );
            assert_ne!(pure, src);
            assert!(diagnostics(&pure, Some("one_call")).is_empty(), "{pure}");
            (call, codes)
        })
        .collect();
    assert_eq!(
        results,
        [
            ("rows(row)", vec!["PW0405"]),
            ("row.rows", vec!["PW0405"]),
            ("row.rows()", vec!["PW0405"]),
        ]
    );
}
