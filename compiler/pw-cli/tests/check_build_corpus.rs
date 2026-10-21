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
        let schema_lock = src
            .lines()
            .find_map(|line| line.trim().strip_prefix("// @schema-lock:"))
            .map(|name| {
                let path = fixture.with_file_name(name.trim());
                assert!(
                    path.is_file(),
                    "declared schema lock is missing: {}",
                    path.display()
                );
                path
            });
        let schema_errors = schema_lock.as_ref().map(|_| {
            let id = src
                .lines()
                .find_map(|line| line.trim().strip_prefix("// @id:"))
                .expect("a schema fixture has an id")
                .trim();
            let count = match id {
                "R-106" => 1,
                "R-107" => 2,
                _ => panic!("pin the exact released-schema refusal count for {id}"),
            };
            let reasons: Vec<_> = src
                .lines()
                .filter_map(|line| line.trim().strip_prefix("// @expect-error:"))
                .map(str::trim)
                .collect();
            assert_eq!(reasons.len(), count, "{id}: exact schema refusal reasons");
            (count, reasons)
        });
        let run = |command: &str| {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_pw"));
            cmd.arg(command).args(&program);
            if let Some(lock) = &schema_lock {
                cmd.arg("--schema-lock").arg(lock);
            }
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
            assert_eq!(
                result.status.code(),
                Some(1),
                "{} {command}: {text}",
                fixture.display()
            );
            let found = codes(&text);
            assert!(!found.is_empty(), "{} {command}: {text}", fixture.display());
            if let Some((count, reasons)) = &schema_errors {
                assert_eq!(
                    found,
                    BTreeSet::from(["PW0366".to_string()]),
                    "{} {command}: {text}",
                    fixture.display()
                );
                let header = if command == "check" {
                    "error: [PW0366]"
                } else {
                    "[PW0366]"
                };
                assert_eq!(
                    text.matches(header).count(),
                    *count,
                    "{} {command}: {text}",
                    fixture.display()
                );
                for reason in reasons {
                    assert!(
                        text.contains(reason),
                        "{} {command} omitted {reason}: {text}",
                        fixture.display()
                    );
                }
                if command == "check" {
                    assert!(
                        text.contains(&format!(
                            "pw check: {} file(s), {count} error(s), 0 warning(s)",
                            program.len()
                        )),
                        "{} {command}: {text}",
                        fixture.display()
                    );
                }
            }
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
    let list: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("examples/accepted/build-refusals.json")).unwrap(),
    )
    .unwrap();
    let listed = list["refused"].as_array().unwrap().clone();
    let listed_ref = &listed;
    assert!(
        listed.iter().all(|entry| entry["code"] == "PW6601"),
        "{listed:?}"
    );
    let files_ref = &files;
    // D1's accepted cross-source reference is a warning, separate from the
    // executable backend's listed errors. Pin its native structured identity.
    let expected_warning = "`Comments.author` refers to `Members` in `MemberData`, and `Comments` is in `CommentData`: no database holds the reference, so the referred row may be missing";
    let warnings: Vec<_> = report
        .diagnostics
        .iter()
        .flat_map(|(path, ds)| ds.iter().filter(|d| !d.is_error()).map(move |d| (path, d)))
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let (warning_path, warning) = warnings[0];
    assert_eq!(warning.code, "PW0365");
    assert_eq!(warning.severity, pw_core::diagnostics::Severity::Warning);
    assert_eq!(
        warning.detector,
        pw_core::diagnostics::Detector::DeclarationRule
    );
    assert_eq!(warning.reason, "refers_across_sources");
    assert_eq!(warning.message, expected_warning);
    assert_eq!(
        Path::new(warning_path).file_name().unwrap().to_str(),
        Some("A-052-a-reference-across-sources-that-may-name-no-row.pw")
    );
    let warning_source = &files.iter().find(|(p, _)| p == warning_path).unwrap().1;
    let warning_start = warning_source.find("refers author to Members").unwrap();
    assert_eq!(
        warning.primary_span,
        warning_start..warning_start + "refers author to Members".len()
    );
    assert_eq!(warning.related.len(), 1);
    let target_unit = files
        .iter()
        .position(|(p, _)| {
            Path::new(p).file_name().unwrap().to_str()
                == Some("A-051-a-reference-within-one-source.pw")
        })
        .unwrap();
    let target_source = &files[target_unit].1;
    let target_start = target_source.find("table Members").unwrap() + "table ".len();
    // Related.unit is private; its resolved public file names this exact unit.
    assert_eq!(
        warning.related[0].file.as_deref(),
        Some(files[target_unit].0.as_str())
    );
    assert_eq!(
        warning.related[0].span,
        target_start..target_start + "Members".len()
    );
    assert_eq!(
        warning.related[0].label,
        "`Members` is held by `MemberData`"
    );
    let mut got: Vec<_> = report
        .diagnostics
        .iter()
        .flat_map(|(path, ds)| {
            ds.iter().filter(|d| d.is_error()).map(move |d| {
                assert_eq!(d.code, "PW6601");
                let src = &files_ref.iter().find(|(p, _)| p == path).unwrap().1;
                let name = Path::new(path).file_name().unwrap().to_str().unwrap();
                let responsible = listed_ref
                    .iter()
                    .find(|entry| entry["file"] == name)
                    .unwrap_or_else(|| panic!("unexpected support-refused source: {name}"))
                    ["responsible"]
                    .as_str()
                    .unwrap();
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
    // The one list every corpus check reads (examples/accepted/build-refusals.json, the
    // coordinator's ruling of 2026-10-20): each refused program, its stage and its exact message.
    let mut expected: Vec<_> = listed
        .iter()
        .map(|entry| {
            (
                entry["file"].as_str().unwrap().to_string(),
                entry["stage"].as_str().unwrap(),
                entry["message"].as_str().unwrap().to_string(),
            )
        })
        .collect();
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
        assert_eq!(result.status.code(), Some(1), "{command}: {text}");
        if command == "check" {
            assert_eq!(
                codes(&text),
                BTreeSet::from(["PW0365".to_string(), "PW6601".to_string()])
            );
            let errors: Vec<_> = text
                .lines()
                .filter(|line| line.starts_with("error: ["))
                .collect();
            assert_eq!(errors.len(), listed.len(), "{text}");
            assert!(
                errors
                    .iter()
                    .all(|line| line.starts_with("error: [PW6601]")),
                "{text}"
            );
            let warnings: Vec<_> = text
                .lines()
                .filter(|line| line.starts_with("warning: ["))
                .collect();
            let warning_header = format!("warning: [PW0365] {expected_warning}");
            assert_eq!(warnings, vec![warning_header.as_str()], "{text}");
            let warning_block = text.split_once(&warning_header).unwrap().1;
            assert!(
                warning_block.starts_with(
                    "\n  --> A-052-a-reference-across-sources-that-may-name-no-row.pw:24:5\n"
                ),
                "{text}"
            );
            assert!(
                text.contains(&format!(
                    "pw check: {} file(s), {} error(s), 1 warning(s)",
                    program.len(),
                    listed.len()
                )),
                "{text}"
            );
        } else {
            assert_eq!(codes(&text), BTreeSet::from(["PW6601".to_string()]));
            assert!(!text.contains("[PW0365]"), "{text}");
        }
        assert_eq!(
            text.matches("[PW6601]").count(),
            listed.len(),
            "{command}: {text}"
        );
        for (_, _, message) in &expected {
            assert!(text.contains(message), "{command}: {text}");
        }
        assert!(!out.exists());
    }
    println!(
        "accepted corpus: semantic language remains accepted; complete check/build agree on exactly {} listed backend refusals",
        listed.len()
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
