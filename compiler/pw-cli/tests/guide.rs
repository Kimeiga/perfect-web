//! **The guide's examples compile** (ADR-XXXX, a guide whose examples
//! compile).
//!
//! Every fenced block of `docs/guide/*.md` says what it is in its info
//! string, and this reads every one:
//!
//! - ```` ```pw ```` is a whole program, run through `pw check --plain` with
//!   the standard packages: it reports nothing, and `pw fmt --check` passes;
//! - ```` ```pw fails=PW0606 ```` is a program the compiler refuses, with
//!   exactly the codes named;
//! - ```` ```pw file=guestbook/app.pw ```` is part of the page's program,
//!   each file's blocks in order, checked clean; and where the page says
//!   `<!-- guide-program: examples/guestbook -->`, those files are, byte for
//!   byte, the directory's;
//! - ```` ```text pw-check ```` is what `pw check --plain` prints for the
//!   refused example just before it, less its closing count.
//!
//! And `docs/guide/diagnostics.md`, which `scripts/guide_diagnostics.py`
//! writes from the registry, lists every code `pw_core::codes::ALL` holds,
//! with its invariant.
//!
//! The binary is run as a user runs it, as `related_files.rs` runs it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The languages a block may be written in besides Pleris: commands,
/// output, and other languages' code for comparison. None is compiled.
const OTHER_LANGUAGES: &[&str] = &["sh", "text", "ts", "tsx", "svelte", "json"];

/// The pages that show no Pleris: the index, and the reference of codes
/// `scripts/guide_diagnostics.py` writes.
const UNEXAMPLED: &[&str] = &["README.md", "diagnostics.md"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

/// One fenced block: its info string, its text, and the line its fence
/// opens on (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Block {
    info: String,
    text: String,
    line: usize,
}

/// **The fenced blocks of a page**, as CommonMark reads a fence: three or
/// more backticks or tildes, closed by the same character, at least as many,
/// with nothing after. A fence is at the line's start: one indented is
/// refused rather than guessed at, since a list's indentation would make it
/// another block. An unclosed fence is refused too.
fn blocks(md: &str) -> Result<Vec<Block>, String> {
    let mut out = Vec::new();
    let mut open: Option<(char, usize, String, usize, Vec<&str>)> = None;
    for (i, line) in md.lines().enumerate() {
        let n = i + 1;
        match &mut open {
            None => {
                let trimmed = line.trim_start();
                let fence = fence_of(trimmed);
                if let Some((c, len)) = fence {
                    if trimmed.len() != line.len() {
                        return Err(format!("line {n}: an indented fence"));
                    }
                    let info = trimmed[len..].trim().to_string();
                    open = Some((c, len, info, n, Vec::new()));
                }
            }
            Some((c, len, _, _, lines)) => {
                let closes = fence_of(line)
                    .is_some_and(|(d, l)| d == *c && l >= *len && line[l..].trim().is_empty());
                if closes {
                    let (_, _, info, at, lines) = open.take().expect("open");
                    let mut text = lines.join("\n");
                    text.push('\n');
                    out.push(Block {
                        info,
                        text,
                        line: at,
                    });
                } else {
                    lines.push(line);
                }
            }
        }
    }
    if let Some((_, _, _, at, _)) = open {
        return Err(format!("line {at}: a fence never closed"));
    }
    Ok(out)
}

/// A fence's character and length, where `line` begins with one.
fn fence_of(line: &str) -> Option<(char, usize)> {
    let c = line.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let len = line.chars().take_while(|x| *x == c).count();
    (len >= 3).then_some((c, len))
}

/// What a block is, read from its info string.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// A whole program, checked clean.
    Clean,
    /// A whole program the compiler refuses, with exactly these codes.
    Fails(BTreeSet<String>),
    /// Part of the page's program: this file's next piece.
    File(String),
    /// What `pw check --plain` prints for the refused example before it.
    Output,
    /// Commands, output, or another language: not compiled.
    Other,
}

/// **A block's kind**, or why its info string is refused.
fn kind(info: &str) -> Result<Kind, String> {
    let mut words = info.split_whitespace();
    let Some(language) = words.next() else {
        return Err("a block with no info string: say what it is (`pw`, `text`, `sh`, ..)".into());
    };
    let attributes: Vec<&str> = words.collect();
    if language == "pw" {
        let mut fails = None;
        let mut file = None;
        for a in &attributes {
            match a.split_once('=') {
                Some(("fails", codes)) if fails.is_none() => {
                    let codes: BTreeSet<String> = codes.split(',').map(str::to_string).collect();
                    if codes.is_empty() || codes.iter().any(|c| !is_code(c)) {
                        return Err(format!("`{a}`: codes are `PW` and four digits"));
                    }
                    fails = Some(codes);
                }
                Some(("file", name)) if file.is_none() => {
                    if !name.ends_with(".pw") || name.starts_with('/') || name.contains("..") {
                        return Err(format!("`{a}`: a file is a relative `.pw` path"));
                    }
                    file = Some(name.to_string());
                }
                _ => return Err(format!("`{a}`: not an attribute a `pw` block takes")),
            }
        }
        return match (fails, file) {
            (None, None) => Ok(Kind::Clean),
            (Some(codes), None) => Ok(Kind::Fails(codes)),
            (None, Some(file)) => Ok(Kind::File(file)),
            (Some(_), Some(_)) => Err("a page's program is clean: `fails` with `file`".into()),
        };
    }
    if language == "text" && attributes == ["pw-check"] {
        return Ok(Kind::Output);
    }
    if !OTHER_LANGUAGES.contains(&language) {
        return Err(format!(
            "`{language}`: not a language the guide is written in"
        ));
    }
    if !attributes.is_empty() {
        return Err(format!(
            "`{info}`: a `{language}` block takes no attributes"
        ));
    }
    Ok(Kind::Other)
}

fn is_code(c: &str) -> bool {
    c.len() == 6 && c.starts_with("PW") && c[2..].chars().all(|d| d.is_ascii_digit())
}

/// **One program to check**: where it is in the guide, its files, and what
/// it must give.
#[derive(Debug, Clone)]
struct Example {
    place: String,
    files: Vec<(String, String)>,
    expect: Expect,
}

#[derive(Debug, Clone)]
enum Expect {
    Clean,
    /// Exactly these codes, and, where the guide shows it, this output.
    Fails(BTreeSet<String>, Option<String>),
}

/// **A page's examples, and the program its `file=` blocks make**, or every
/// way the page is malformed.
fn examples_of(page: &str, md: &str) -> Result<Vec<Example>, Vec<String>> {
    let found = blocks(md).map_err(|e| vec![format!("{page}: {e}")])?;
    let mut problems = Vec::new();
    let mut out: Vec<Example> = Vec::new();
    let mut program: BTreeMap<String, String> = BTreeMap::new();
    let mut first_line = None;
    let mut last_fails: Option<usize> = None;
    for (i, b) in found.iter().enumerate() {
        let place = format!("{page}:{}", b.line);
        match kind(&b.info) {
            Err(e) => problems.push(format!("{place}: {e}")),
            Ok(Kind::Clean) => out.push(Example {
                place,
                files: vec![(file_name_of(&b.text), b.text.clone())],
                expect: Expect::Clean,
            }),
            Ok(Kind::Fails(codes)) => {
                last_fails = Some(out.len());
                out.push(Example {
                    place,
                    files: vec![(file_name_of(&b.text), b.text.clone())],
                    expect: Expect::Fails(codes, None),
                });
            }
            Ok(Kind::File(name)) => {
                first_line.get_or_insert(b.line);
                let piece = program.entry(name).or_default();
                if !piece.is_empty() {
                    piece.push('\n');
                }
                piece.push_str(&b.text);
            }
            Ok(Kind::Output) => {
                // The output of the refused example just before it.
                let follows = i > 0 && matches!(kind(&found[i - 1].info), Ok(Kind::Fails(_)));
                match (follows, last_fails) {
                    (true, Some(at)) => {
                        if let Expect::Fails(_, shown) = &mut out[at].expect {
                            *shown = Some(b.text.clone());
                        }
                    }
                    _ => problems.push(format!(
                        "{place}: `text pw-check` follows a `pw fails=` block, and this one does not"
                    )),
                }
            }
            Ok(Kind::Other) => {}
        }
    }
    if let Some(line) = first_line {
        out.push(Example {
            place: format!("{page}:{line} (the page's program)"),
            files: program.into_iter().collect(),
            expect: Expect::Clean,
        });
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

/// **The file a whole-program example is checked as**: its module's last
/// segment, `module guide.price` in `price.pw`, so the output a page shows
/// names the file a reader would save.
fn file_name_of(src: &str) -> String {
    src.lines()
        .find_map(|l| l.strip_prefix("module "))
        .and_then(|m| m.trim().rsplit('.').next())
        .filter(|m| !m.is_empty())
        .map(|m| format!("{m}.pw"))
        .unwrap_or_else(|| "example.pw".to_string())
}

/// The standard packages a program is checked with, in a stable order.
fn packages() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        for e in std::fs::read_dir(repo().join(dir)).expect(dir) {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn pw(args: &[&std::ffi::OsStr], dir: &Path) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_pw"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("pw runs");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// The codes `pw check --plain` reported, from its `error:` and `warning:`
/// lines.
fn codes_in(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .filter(|l| l.starts_with("error: [") || l.starts_with("warning: ["))
        .filter_map(|l| l.split_once('[').and_then(|(_, r)| r.split_once(']')))
        .map(|(code, _)| code.to_string())
        .collect()
}

/// **What is wrong with one example**, or nothing: checked in a directory
/// of its own, its files written as the guide gives them.
fn verdict(example: &Example, scratch: &Path) -> Option<String> {
    let _ = std::fs::remove_dir_all(scratch);
    for (name, src) in &example.files {
        let path = scratch.join(name);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("scratch");
        std::fs::write(&path, src).expect("write");
    }
    let names: Vec<&std::ffi::OsStr> = example
        .files
        .iter()
        .map(|(n, _)| std::ffi::OsStr::new(n.as_str()))
        .collect();
    let packages = packages();
    let mut args: Vec<&std::ffi::OsStr> = vec!["check".as_ref(), "--plain".as_ref()];
    args.extend(packages.iter().map(|p| p.as_os_str()));
    args.extend(names.iter().copied());
    let (code, out, err) = pw(&args, scratch);
    let mut fmt: Vec<&std::ffi::OsStr> = vec!["fmt".as_ref(), "--check".as_ref()];
    fmt.extend(names.iter().copied());
    let (fmt_code, fmt_out, _) = pw(&fmt, scratch);
    let said = format!("{out}{err}");
    if err.contains("panicked") {
        return Some(format!("`pw check` stopped:\n{said}"));
    }
    if fmt_code != Some(0) {
        return Some(format!("not as `pw fmt` writes it:\n{fmt_out}"));
    }
    match &example.expect {
        Expect::Clean => {
            if code != Some(0) || !out.contains("no diagnostics") {
                return Some(format!("expected no diagnostics:\n{said}"));
            }
        }
        Expect::Fails(codes, shown) => {
            if code != Some(1) {
                return Some(format!("expected `pw check` to refuse it:\n{said}"));
            }
            let found = codes_in(&out);
            if &found != codes {
                return Some(format!(
                    "expected exactly {codes:?}, and `pw check` reported {found:?}:\n{said}"
                ));
            }
            if let Some(shown) = shown {
                let printed = without_count(&out);
                if printed.trim_end() != shown.trim_end() {
                    return Some(format!(
                        "the page shows output `pw check` no longer prints. It prints:\n{printed}"
                    ));
                }
            }
        }
    }
    None
}

/// `pw check`'s output less its closing line, which counts the standard
/// packages' files too.
fn without_count(out: &str) -> String {
    out.lines()
        .filter(|l| !l.starts_with("pw check: "))
        .map(|l| format!("{l}\n"))
        .collect()
}

/// The guide's pages, by name.
fn pages() -> Vec<(String, String)> {
    let dir = repo().join("docs/guide");
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).expect("docs/guide") {
        let p = e.expect("entry").path();
        if p.extension().is_some_and(|x| x == "md") {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            out.push((name, std::fs::read_to_string(&p).expect("read")));
        }
    }
    out.sort();
    out
}

/// **Every example in the guide compiles as the guide says.** All are run,
/// on several threads, and every failure is reported, so one run says all
/// it found.
#[test]
fn every_example_in_the_guide_compiles_as_it_says() {
    let mut examples = Vec::new();
    let mut problems = Vec::new();
    let mut opening = 0;
    for (page, md) in pages() {
        // A plain count of the opening ```pw lines, read apart from the
        // extractor: a block it skipped would be a block nobody checked.
        let here = md.lines().filter(|l| l.starts_with("```pw")).count();
        opening += here;
        // A page teaches by example: all but the index and the generated
        // reference of codes show Pleris.
        if here == 0 && !UNEXAMPLED.contains(&page.as_str()) {
            problems.push(format!(
                "{page}: a page of the guide with no Pleris example"
            ));
        }
        match examples_of(&page, &md) {
            Ok(found) => examples.extend(found),
            Err(e) => problems.extend(e),
        }
    }
    let pieces: usize = examples
        .iter()
        .map(|e| match e.place.ends_with("(the page's program)") {
            true => count_pieces(e),
            false => 1,
        })
        .sum();
    assert_eq!(
        pieces, opening,
        "every ```pw block is one example, or one piece of its page's program"
    );
    assert!(!examples.is_empty(), "the guide has examples");

    let threads = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4);
    let base = std::env::temp_dir().join(format!("pw-guide-{}", std::process::id()));
    let failures: Vec<String> = std::thread::scope(|s| {
        let chunks: Vec<Vec<(usize, &Example)>> = (0..threads)
            .map(|t| {
                examples
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % threads == t)
                    .collect()
            })
            .collect();
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                let base = base.clone();
                s.spawn(move || {
                    chunk
                        .into_iter()
                        .filter_map(|(i, e)| {
                            verdict(e, &base.join(i.to_string()))
                                .map(|why| format!("{}: {why}", e.place))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a checking thread"))
            .collect()
    });
    let _ = std::fs::remove_dir_all(&base);
    problems.extend(failures);
    assert!(
        problems.is_empty(),
        "{} of the guide's examples are wrong:\n\n{}",
        problems.len(),
        problems.join("\n\n")
    );
}

/// How many `file=` blocks a page's program was assembled from: the page is
/// read again for it, so the count is the blocks', not the files'.
fn count_pieces(program: &Example) -> usize {
    let page = program.place.split(':').next().unwrap_or_default();
    let md = std::fs::read_to_string(repo().join("docs/guide").join(page)).expect("the page");
    blocks(&md)
        .expect("read once already")
        .iter()
        .filter(|b| matches!(kind(&b.info), Ok(Kind::File(_))))
        .count()
}

/// **The tutorial is the app the server runs**: a page that names a
/// directory with `<!-- guide-program: DIR -->` assembles, from its `file=`
/// blocks, exactly the `.pw` files in that directory, byte for byte.
#[test]
fn a_pages_program_is_the_directory_it_names() {
    let mut named = 0;
    for (page, md) in pages() {
        let Some(dir) = md.lines().find_map(|l| {
            l.trim()
                .strip_prefix("<!-- guide-program:")
                .and_then(|r| r.strip_suffix("-->"))
                .map(str::trim)
        }) else {
            continue;
        };
        named += 1;
        let examples = examples_of(&page, &md).unwrap_or_else(|e| panic!("{e:?}"));
        let program = examples
            .iter()
            .find(|e| e.place.ends_with("(the page's program)"))
            .unwrap_or_else(|| panic!("{page} names {dir} and has no `file=` block"));
        let assembled: BTreeMap<String, String> = program
            .files
            .iter()
            .map(|(n, s)| {
                let base = Path::new(n)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                (base, s.clone())
            })
            .collect();
        let mut on_disk = BTreeMap::new();
        for e in std::fs::read_dir(repo().join(dir)).unwrap_or_else(|e| panic!("{dir}: {e}")) {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                let name = p.file_name().unwrap().to_string_lossy().to_string();
                on_disk.insert(name, std::fs::read_to_string(&p).expect("read"));
            }
        }
        assert_eq!(
            assembled.keys().collect::<Vec<_>>(),
            on_disk.keys().collect::<Vec<_>>(),
            "{page}'s program and {dir} hold the same files"
        );
        for (name, text) in &assembled {
            assert!(
                *text == on_disk[name],
                "{page}'s `{name}` is not {dir}/{name}: the tutorial and the app differ"
            );
        }
    }
    assert!(named >= 1, "the tutorial names the app it builds");
}

/// **The diagnostic reference lists every code the compiler has**, each
/// with the invariant the registry gives it: read from the registry as the
/// compiler sees it, apart from the script that writes the page.
#[test]
fn the_diagnostic_reference_lists_every_registered_code() {
    let page = std::fs::read_to_string(repo().join("docs/guide/diagnostics.md"))
        .expect("docs/guide/diagnostics.md");
    let mut missing = Vec::new();
    for code in pw_core::codes::ALL {
        let row = page
            .lines()
            .find(|l| l.starts_with(&format!("| {} |", code.id)));
        match row {
            Some(row) if row.contains(&escaped(code.invariant)) => {}
            Some(row) => missing.push(format!("{}: its invariant is not in `{row}`", code.id)),
            None => missing.push(format!("{}: no row", code.id)),
        }
    }
    let rows = page.lines().filter(|l| l.starts_with("| PW")).count();
    assert!(
        missing.is_empty(),
        "docs/guide/diagnostics.md is not the registry; run `python3 scripts/guide_diagnostics.py`:\n{}",
        missing.join("\n")
    );
    assert_eq!(
        rows,
        pw_core::codes::ALL.len()
            + pw_core::codes::RETIRED.len()
            + pw_core::diagnostics::DEPRECATED_ALIASES.len(),
        "one row a code, a retired code and an old number: no row for a code the compiler lacks"
    );
}

/// A sentence as a Markdown table cell writes it.
fn escaped(text: &str) -> String {
    text.replace('|', "\\|")
}

// --- the extractor, held on its own ----------------------------------------

#[test]
fn a_fence_is_read_as_commonmark_reads_it() {
    let md = "intro\n```pw\nmodule a\n```\n\n~~~~text\n```\nstill text\n~~~~\n````sh\nls\n````\n";
    let found = blocks(md).expect("well formed");
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(found[0].info, "pw");
    assert_eq!(found[0].text, "module a\n");
    assert_eq!(found[0].line, 2);
    // A shorter fence of another character does not close a block.
    assert_eq!(found[1].text, "```\nstill text\n");
    assert_eq!(found[2].info, "sh");
}

#[test]
fn an_unclosed_or_indented_fence_is_refused() {
    assert!(blocks("```pw\nmodule a\n").is_err());
    assert!(blocks("- a list\n  ```pw\n  module a\n  ```\n").is_err());
    // A closing fence carries nothing after it.
    assert!(blocks("```pw\nmodule a\n``` pw\n").is_err());
}

#[test]
fn an_info_string_says_what_a_block_is() {
    assert_eq!(kind("pw"), Ok(Kind::Clean));
    assert_eq!(
        kind("pw fails=PW0606,PW0021"),
        Ok(Kind::Fails(BTreeSet::from([
            "PW0606".to_string(),
            "PW0021".to_string()
        ])))
    );
    assert_eq!(
        kind("pw file=guestbook/app.pw"),
        Ok(Kind::File("guestbook/app.pw".into()))
    );
    assert_eq!(kind("text pw-check"), Ok(Kind::Output));
    assert_eq!(kind("sh"), Ok(Kind::Other));
    for refused in [
        "",
        "pw fails=",
        "pw fails=E0606",
        "pw file=app.rs",
        "pw file=../app.pw",
        "pw fails=PW0606 file=a.pw",
        "pw strict",
        "rust",
        "text output",
    ] {
        assert!(kind(refused).is_err(), "{refused:?} is refused");
    }
}

#[test]
fn a_pages_file_blocks_are_its_program_in_order() {
    let md = "```pw file=g/app.pw\nmodule g.app\n```\n\n```pw\nmodule alone\n```\n\n\
              ```pw file=g/app.pw\nfn f() -> Int !{} { 1 }\n```\n";
    let found = examples_of("t.md", md).expect("well formed");
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0].files,
        vec![("alone.pw".into(), "module alone\n".into())]
    );
    assert_eq!(
        found[1].files,
        vec![(
            "g/app.pw".into(),
            "module g.app\n\nfn f() -> Int !{} { 1 }\n".into()
        )]
    );
}

#[test]
fn shown_output_belongs_to_the_refused_example_before_it() {
    let md = "```pw fails=PW0606\nmodule bad\n```\n```text pw-check\nerror: x\n```\n";
    let found = examples_of("t.md", md).expect("well formed");
    match &found[0].expect {
        Expect::Fails(_, Some(shown)) => assert_eq!(shown, "error: x\n"),
        other => panic!("{other:?}"),
    }
    assert!(examples_of("t.md", "```pw\nmodule a\n```\n```text pw-check\nx\n```\n").is_err());
}

#[test]
fn a_refused_examples_codes_are_read_from_its_output() {
    let out = "error: [PW0606] a\n --> bad.pw:4:5\nwarning: [PW0402] b\nnote: [PW9999] c\n";
    assert_eq!(
        codes_in(out),
        BTreeSet::from(["PW0606".to_string(), "PW0402".to_string()])
    );
}
