//! The released-schema boundary exercised through the actual CLI.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pw(command: &str, extra: &[&Path], source: &Path) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pw"));
    cmd.current_dir(root()).arg(command).args(extra).arg(source);
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<_> = std::fs::read_dir(root().join(dir))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "pw"))
            .collect();
        paths.sort();
        cmd.args(paths);
    }
    cmd.output().expect("pw runs")
}

fn shown(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn check_and_build_hold_a_program_to_the_named_release() {
    let dir = std::env::temp_dir().join(format!("pw-cli-tables-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("Notes.pw");
    let released = "module Notes\ntype Note = Note { id: String, text: String }\ntable Notes\n    row Note\n    key id\n";
    std::fs::write(&source, released).unwrap();
    let built = dir.join("built");
    let out = pw("build", &[Path::new("--out"), &built], &source);
    assert!(out.status.success(), "{}", shown(&out));
    assert!(built.join("tables.json").exists());
    let lock = dir.join("released.lock");
    std::fs::copy(built.join("schema.lock"), &lock).unwrap();
    let after = released.replace(", text: String", "");
    std::fs::write(&source, &after).unwrap();
    let out = pw("check", &[], &source);
    assert!(out.status.success(), "{}", shown(&out));
    for command in ["check", "build"] {
        let dest = dir.join(format!("refused-{command}"));
        let mut args = vec![Path::new("--schema-lock"), lock.as_path()];
        if command == "build" {
            args.extend([Path::new("--out"), dest.as_path()]);
        }
        let out = pw(command, &args, &source);
        assert!(!out.status.success(), "{command} accepted a dropped field");
        assert!(shown(&out).contains("PW0366"), "{}", shown(&out));
        assert!(!dest.join("tables.json").exists());
    }
    // The same dropped field, stated, is built with the plan and a new lock.
    std::fs::write(&source, format!("{after}    migrate text dropped\n")).unwrap();
    let out = pw(
        "build",
        &[
            Path::new("--schema-lock"),
            &lock,
            Path::new("--out"),
            &built,
        ],
        &source,
    );
    assert!(out.status.success(), "{}", shown(&out));
    let checked = pw("check", &[Path::new("--schema-lock"), &lock], &source);
    assert!(checked.status.success(), "{}", shown(&checked));
    let tables = std::fs::read_to_string(built.join("tables.json")).unwrap();
    assert!(tables.contains("drop_field"), "{tables}");
    let tables: pw_core::tables::TablesJson = serde_json::from_str(&tables).unwrap();
    let accepted =
        pw_core::tables::Schema::read(&std::fs::read_to_string(built.join("schema.lock")).unwrap())
            .unwrap();
    let previous = pw_core::tables::Schema::read(&std::fs::read_to_string(&lock).unwrap()).unwrap();
    assert_eq!(tables.hash, accepted.hash);
    assert_eq!(
        tables.migration.from.as_deref(),
        Some(previous.hash.as_str())
    );
    assert_eq!(tables.migration.to, accepted.hash);
    assert_eq!(
        tables
            .tables
            .iter()
            .map(|table| table.schema.clone())
            .collect::<Vec<_>>(),
        accepted.tables
    );
    assert_ne!(
        std::fs::read(&lock).unwrap(),
        std::fs::read(built.join("schema.lock")).unwrap()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_invalid_or_missing_release_is_an_error() {
    let source = root().join("examples/tables/Lists.pw");
    let missing = root().join("work/a-schema-that-does-not-exist.lock");
    let out = pw("check", &[Path::new("--schema-lock"), &missing], &source);
    assert_eq!(out.status.code(), Some(2), "{}", shown(&out));
    let out = Command::new(env!("CARGO_BIN_EXE_pw"))
        .arg("check")
        .arg(&source)
        .arg("--schema-lock")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", shown(&out));
    assert!(shown(&out).contains("needs a file"), "{}", shown(&out));
}

#[test]
fn a_released_schema_does_not_skip_executable_support() {
    let dir = std::env::temp_dir().join(format!("pw-cli-schema-support-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("Menu.pw");
    std::fs::write(&source, "module shop.head\nfn items() -> List<Int> !{} { [1, 2] }\nview Menu() !{} { <ul>{#each items() as item (item)}<li>{item}</li>{/each}</ul> }\n").unwrap();
    let lock = dir.join("released.lock");
    std::fs::write(&lock, pw_core::tables::Schema::of(Vec::new()).written()).unwrap();
    for command in ["check", "build"] {
        let dest = dir.join(format!("refused-{command}"));
        let mut args = vec![Path::new("--schema-lock"), lock.as_path()];
        if command == "build" {
            args.extend([Path::new("--out"), dest.as_path()]);
        }
        let out = pw(command, &args, &source);
        let said = shown(&out);
        assert_eq!(out.status.code(), Some(1), "{command}: {said}");
        assert!(
            said.contains("PW6601") && said.contains("reads its list by path"),
            "{command}: {said}"
        );
        assert!(
            !said.contains("PW0366"),
            "a valid empty release was reinterpreted: {said}"
        );
        assert!(!dest.exists(), "refused {command} wrote artifacts");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn migration_refusals_precede_unsupported_backend_diagnostics() {
    let dir = std::env::temp_dir().join(format!("pw-cli-schema-priority-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("Notes.pw");
    let original = "module Notes\ntype Note = Note { id: String, text: String }\ntable Notes\n    row Note\n    key id\n";
    std::fs::write(&source, original).unwrap();
    let first = dir.join("first");
    let out = pw("build", &[Path::new("--out"), &first], &source);
    assert!(out.status.success(), "{}", shown(&out));
    let lock = dir.join("released.lock");
    std::fs::copy(first.join("schema.lock"), &lock).unwrap();
    let unsupported = "\nfn items() -> List<Int> !{} { [1, 2] }\nview Menu() !{} { <ul>{#each items() as item (item)}<li>{item}</li>{/each}</ul> }\n";
    std::fs::write(
        &source,
        original.replace(", text: String", "") + unsupported,
    )
    .unwrap();
    for command in ["check", "build"] {
        let dest = dir.join(format!("refused-{command}"));
        let mut args = vec![Path::new("--schema-lock"), lock.as_path()];
        if command == "build" {
            args.extend([Path::new("--out"), dest.as_path()]);
        }
        let out = pw(command, &args, &source);
        let said = shown(&out);
        assert_eq!(out.status.code(), Some(1), "{command}: {said}");
        assert!(
            said.contains("PW0366"),
            "native migration identity was lost: {command}: {said}"
        );
        assert!(
            !said.contains("PW6601"),
            "backend ran after migration refusal: {command}: {said}"
        );
        assert!(!dest.exists(), "refused {command} wrote artifacts");
    }
    std::fs::remove_dir_all(dir).unwrap();
}
