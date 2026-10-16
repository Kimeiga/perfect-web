//! All rejected corpus programs are refused by both public commands.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
fn sources(root: &Path, dir: &str) -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(root.join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "pw"))
        .collect();
    paths.sort();
    paths
}
fn codes(text: &str) -> BTreeSet<String> {
    text.split("[PW")
        .skip(1)
        .filter_map(|tail| {
            let code = tail.get(..4)?;
            (code.bytes().all(|b| b.is_ascii_digit()) && tail.as_bytes().get(4) == Some(&b']'))
                .then(|| format!("PW{code}"))
        })
        .collect()
}
#[test]
fn every_rejected_corpus_program_has_the_same_check_and_build_codes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut base = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
    ] {
        base.extend(sources(&root, dir));
    }
    base.push(root.join("examples/domain.pw"));
    let accepted = sources(&root, "examples/accepted");
    let fixtures = sources(&root, "examples/rejected");
    let out = std::env::temp_dir().join(format!("pw-corpus-no-artifacts-{}", std::process::id()));
    assert!(!out.exists());
    for fixture in &fixtures {
        let src = std::fs::read_to_string(fixture).unwrap();
        let imports: Vec<_> = src
            .lines()
            .filter_map(|l| l.trim().strip_prefix("import "))
            .map(|m| m.split(['.', ' ']).next().unwrap_or(""))
            .collect();
        let mut program = base.clone();
        for path in &accepted {
            let src = std::fs::read_to_string(path).unwrap();
            let module = src
                .lines()
                .find_map(|l| l.trim().strip_prefix("module "))
                .unwrap();
            if imports.contains(&module.trim().split('.').next().unwrap()) {
                program.push(path.clone());
            }
        }
        program.push(fixture.clone());
        let run = |command: &str| {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_pw"));
            cmd.arg(command).args(&program);
            if command == "check" {
                cmd.arg("--plain");
            } else {
                cmd.arg("--out").arg(&out);
            }
            let result = cmd.output().unwrap();
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(
                !result.status.success(),
                "{} {command}: {text}",
                fixture.display()
            );
            let found = codes(&text);
            assert!(!found.is_empty(), "{} {command}: {text}", fixture.display());
            found
        };
        assert_eq!(run("check"), run("build"), "{}", fixture.display());
        assert!(!out.exists(), "a refused program wrote artifacts");
    }
    println!(
        "check/build corpus parity: {} rejected programs",
        fixtures.len()
    );
}
