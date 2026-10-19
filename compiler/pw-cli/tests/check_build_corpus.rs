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

#[test]
fn accepted_corpus_has_exactly_its_known_backend_refusals() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut program = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/accepted",
    ] {
        program.extend(sources(&root, dir));
    }
    program.push(root.join("examples/domain.pw"));
    let files: Vec<_> = program
        .iter()
        .map(|p| (p.display().to_string(), std::fs::read_to_string(p).unwrap()))
        .collect();
    let report = pw_core::program::validate_sources(&files);
    let files_ref = &files;
    let mut got: Vec<_> = report
        .diagnostics
        .iter()
        .flat_map(|(path, ds)| {
            ds.iter().filter(|d| d.is_error()).map(move |d| {
                assert_eq!(d.code, "PW6601");
                let src = &files_ref.iter().find(|(p, _)| p == path).unwrap().1;
                let responsible = match Path::new(path).file_name().unwrap().to_str().unwrap() {
                    "A-007-map-widget-resource-with-cleanup.pw" => "view StoreLocation",
                    "A-008-streamed-public-recommendations.pw" => "query Recommendations",
                    "A-013-build-time-deterministic-page.pw" => "page Terms",
                    "A-018-resize-observation-updates-derived-state.pw" => {
                        "style:grid-template-columns"
                    }
                    other => panic!("unexpected support-refused source: {other}"),
                };
                assert!(src[d.primary_span.clone()].contains(responsible), "{d:?}");
                (
                    Path::new(path)
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                    d.reason,
                    d.message.clone(),
                )
            })
        })
        .collect();
    got.sort();
    let mut expected = vec![
        ("A-007-map-widget-resource-with-cleanup.pw".to_string(), "backend_template", "a template part the renderer cannot render: store.map_widget.StoreLocation: `<map resource={..}>`: an element that mounts a resource is not compiled".to_string()),
        ("A-008-streamed-public-recommendations.pw".to_string(), "backend_wit", "the generated WIT does not parse: duplicate item named `store-recommendations-recommendations-api`".to_string()),
        ("A-013-build-time-deterministic-page.pw".to_string(), "backend_page", "`marketing.terms.Terms`'s values: part 0 computes a value that performs `build.input.read<WorkspaceFile>`, and a host computes only a value that performs nothing".to_string()),
        ("A-018-resize-observation-updates-derived-state.pw".to_string(), "backend_template", "a template part the renderer cannot render: store.responsive_menu.ResponsiveMenu: `style:grid-template-columns`: `style:grid-template-columns` is a directive, and only `on:` directives are compiled".to_string()),
    ];
    expected.sort();
    assert_eq!(
        got, expected,
        "pin each exact source, stage/subreason and refusal; do not widen an allowlist"
    );
    assert!(report.into_build().is_err());
    let out = std::env::temp_dir().join(format!(
        "pw-accepted-corpus-no-artifacts-{}",
        std::process::id()
    ));
    assert!(!out.exists());
    for command in ["check", "build"] {
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
        assert!(!result.status.success(), "{command}: {text}");
        assert_eq!(codes(&text), BTreeSet::from(["PW6601".to_string()]));
        assert_eq!(text.matches("[PW6601]").count(), 4, "{command}: {text}");
        for (_, _, message) in &expected {
            assert!(text.contains(message), "{command}: {text}");
        }
        assert!(!out.exists());
    }
    println!(
        "accepted corpus: semantic language remains accepted; complete check/build agree on exactly four backend gaps"
    );
}

struct Artifacts(PathBuf);
impl Drop for Artifacts {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn reference_program_roots_stay_executable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut std = sources(&root, "packages/pw-std");
    std.extend(sources(&root, "packages/pw-platform-web"));
    let mut base = std.clone();
    base.extend(sources(&root, "examples/lib"));
    base.push(root.join("examples/domain.pw"));
    let mut programs = Vec::new();
    for name in [
        "store",
        "demo",
        "guestbook",
        "hello-static",
        "koka",
        "render",
    ] {
        let mut files = base.clone();
        files.extend(sources(&root, &format!("examples/{name}")));
        programs.push((name, files));
    }
    for name in ["feed", "kiokun"] {
        let mut files = std.clone();
        files.extend(sources(&root, &format!("examples/{name}")));
        programs.push((name, files));
    }
    let mut site = std;
    site.extend([
        root.join("examples/kiokun/Shards.pw"),
        root.join("examples/kiokun/dictionary.pw"),
    ]);
    site.extend(sources(&root, "examples/kiokun-site"));
    programs.push(("kiokun-site", site));
    let output =
        Artifacts(std::env::temp_dir().join(format!("pw-reference-roots-{}", std::process::id())));
    assert!(!output.0.exists());
    for (name, files) in &programs {
        let dest = output.0.join(name);
        for command in ["check", "build"] {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_pw"));
            cmd.arg(command).args(files);
            if command == "check" {
                cmd.arg("--plain");
            } else {
                cmd.arg("--out").arg(&dest);
            }
            let result = cmd.output().unwrap();
            assert!(
                result.status.success(),
                "{name} {command}: {}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            if command == "check" {
                assert!(!dest.exists());
            }
        }
        for artifact in ["templates.json", "contracts.json", "app.wit", "build-id"] {
            assert!(dest.join(artifact).is_file(), "{name}: {artifact}");
        }
    }
    println!(
        "reference program roots: {} complete checks and builds",
        programs.len()
    );
}
