//! **The guide's examples compile** (ADR-0343, a guide whose examples
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
//!   refused example just before it, less its closing count;
//! - ```` ```pw gap=build ```` is a program `pw check` passes and `pw build`
//!   refuses, and ```` ```pw gap=check ```` one `pw check` passes that the
//!   page says it should refuse: each a gap in the checks that the page says
//!   so about, held so that the page cannot go on saying it once the
//!   compiler closes the gap. ```` ```text pw-build ```` after a build gap is
//!   what `pw build` prints for it.
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

/// **A gap in the checks a page says so about**: a program the compiler is
/// held to refuse, or that the build refuses, and the check passes. The
/// guide holds each so that a page cannot go on saying "not yet" once the
/// compiler has closed the gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gap {
    /// `pw check` passes it and `pw build` refuses it.
    Build,
    /// `pw check` passes it, though the page says it should not.
    Check,
}

/// What a block is, read from its info string.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// A whole program, checked clean.
    Clean,
    /// A whole program the compiler refuses, with exactly these codes.
    Fails(BTreeSet<String>),
    /// A whole program the checks let through, as the page says.
    Gap(Gap),
    /// Part of the page's program: this file's next piece.
    File(String),
    /// What `pw check --plain` prints for the refused example before it.
    Output,
    /// What `pw build` prints for the gap example before it.
    BuildOutput,
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
        let mut gap = None;
        for a in &attributes {
            match a.split_once('=') {
                Some(("gap", "build")) if gap.is_none() => gap = Some(Gap::Build),
                Some(("gap", "check")) if gap.is_none() => gap = Some(Gap::Check),
                Some(("gap", _)) => {
                    return Err(format!(
                        "`{a}`: the gaps a page may show are `gap=build` and `gap=check`"
                    ));
                }
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
        return match (fails, file, gap) {
            (None, None, None) => Ok(Kind::Clean),
            (Some(codes), None, None) => Ok(Kind::Fails(codes)),
            (None, Some(file), None) => Ok(Kind::File(file)),
            (None, None, Some(gap)) => Ok(Kind::Gap(gap)),
            _ => Err("a block is one of `fails=..`, `file=..` and `gap=..`".into()),
        };
    }
    if language == "text" && attributes == ["pw-check"] {
        return Ok(Kind::Output);
    }
    if language == "text" && attributes == ["pw-build"] {
        return Ok(Kind::BuildOutput);
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
    /// `pw check` passes it. A `Build` gap is one `pw build` refuses,
    /// printing, where the guide shows it, this.
    Gap(Gap, Option<String>),
}

/// **A page's examples, and the program its `file=` blocks make**, or every
/// way the page is malformed.
fn examples_of(page: &str, md: &str) -> Result<Vec<Example>, Vec<String>> {
    let found = blocks(md).map_err(|e| vec![format!("{page}: {e}")])?;
    let mut problems = Vec::new();
    let mut out: Vec<Example> = Vec::new();
    let mut program: BTreeMap<String, String> = BTreeMap::new();
    let mut first_line = None;
    // The example whose output a `text` block after it may show.
    let mut last_refused: Option<usize> = None;
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
                last_refused = Some(out.len());
                out.push(Example {
                    place,
                    files: vec![(file_name_of(&b.text), b.text.clone())],
                    expect: Expect::Fails(codes, None),
                });
            }
            Ok(Kind::Gap(gap)) => {
                last_refused = Some(out.len());
                out.push(Example {
                    place,
                    files: vec![(file_name_of(&b.text), b.text.clone())],
                    expect: Expect::Gap(gap, None),
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
                match (follows, last_refused) {
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
            Ok(Kind::BuildOutput) => {
                // The output of the build gap just before it.
                let follows =
                    i > 0 && matches!(kind(&found[i - 1].info), Ok(Kind::Gap(Gap::Build)));
                match (follows, last_refused) {
                    (true, Some(at)) => {
                        if let Expect::Gap(Gap::Build, shown) = &mut out[at].expect {
                            *shown = Some(b.text.clone());
                        }
                    }
                    _ => problems.push(format!(
                        "{place}: `text pw-build` follows a `pw gap=build` block, and this one does not"
                    )),
                }
            }
            Ok(Kind::Other) => {}
        }
    }
    // A build gap shows what `pw build` prints, or the page claims nothing
    // but that the build refuses it, whatever it refuses it for.
    for e in &out {
        if matches!(e.expect, Expect::Gap(Gap::Build, None)) {
            problems.push(format!(
                "{}: a `pw gap=build` example shows what `pw build` prints, in a `text pw-build` block after it",
                e.place
            ));
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

/// **`pw build` of an example's files**, with the standard packages, into
/// the example's own directory: its status, and what it printed.
fn build(
    names: &[&std::ffi::OsStr],
    packages: &[PathBuf],
    scratch: &Path,
) -> (Option<i32>, String, String) {
    let out_dir = scratch.join("build-out");
    let mut args: Vec<&std::ffi::OsStr> =
        vec!["build".as_ref(), "--out".as_ref(), out_dir.as_os_str()];
    args.extend(packages.iter().map(|p| p.as_os_str()));
    args.extend(names.iter().copied());
    pw(&args, scratch)
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
    // Every example is as `pw fmt` writes it, but one that shows a syntax
    // error: `pw fmt` formats a tree, and a text that does not parse has
    // none to write.
    let parses = example
        .files
        .iter()
        .all(|(_, src)| pw_syntax::parse_tree(src).ok());
    if fmt_code != Some(0) && (parses || matches!(example.expect, Expect::Clean)) {
        return Some(format!("not as `pw fmt` writes it:\n{fmt_out}"));
    }
    match &example.expect {
        Expect::Clean => {
            if code != Some(0) || !out.contains("no diagnostics") {
                return Some(format!("expected no diagnostics:\n{said}"));
            }
            // And it builds: what `pw check` passes the backend can still
            // refuse (a template value computed from two values, a title
            // that reads a signal), and the guide teaches nothing that
            // does not build, but what it says it does not.
            let (built, build_out, build_err) = build(&names, &packages, scratch);
            if built != Some(0) {
                return Some(format!(
                    "checks clean, and `pw build` refuses it:\n{build_out}{build_err}"
                ));
            }
        }
        Expect::Gap(gap, shown) => {
            // Either gap is a program the check lets through: when `pw
            // check` refuses it, the gap is closed.
            if code != Some(0) || !out.contains("no diagnostics") {
                return Some(format!(
                    "a gap is a program `pw check` passes, and `pw check` does not: the gap is \
                     closed. Show it as a refusal (`pw fails=..`), and take back what the page \
                     says of it:\n{said}"
                ));
            }
            // A build gap is one the build refuses.
            if *gap == Gap::Build {
                let (built, build_out, build_err) = build(&names, &packages, scratch);
                if built == Some(0) {
                    return Some(
                        "`pw build` accepts it: the gap is closed. Show it as a plain `pw` \
                         example, and take back what the page says of it"
                            .to_string(),
                    );
                }
                if let Some(shown) = shown {
                    let printed = format!("{build_out}{build_err}");
                    if printed.trim_end() != shown.trim_end() {
                        return Some(format!(
                            "the page shows output `pw build` no longer prints. It prints:\n{printed}"
                        ));
                    }
                }
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

/// Whether an example is its page's program, made of its `file=` blocks.
fn is_program(example: &Example) -> bool {
    example.place.ends_with("(the page's program)")
}

/// **Every example of some pages, and all that is wrong with how they are
/// written**: a page with no Pleris example (the index and the generated
/// reference of codes aside), a ```` ```pw ```` block the reader of the
/// pages did not see, and whatever `examples_of` refuses.
fn survey(pages: &[(String, String)]) -> (Vec<Example>, Vec<String>) {
    let mut examples = Vec::new();
    let mut problems = Vec::new();
    for (page, md) in pages {
        // A plain count of the opening ```pw lines, read apart from the
        // extractor: a block it skipped would be a block nobody checked.
        let opening = md.lines().filter(|l| l.starts_with("```pw")).count();
        // A page teaches by example: all but the index and the generated
        // reference of codes show Pleris.
        if opening == 0 && !UNEXAMPLED.contains(&page.as_str()) {
            problems.push(format!(
                "{page}: a page of the guide with no Pleris example"
            ));
        }
        match examples_of(page, md) {
            Ok(found) => {
                // Every ```pw block is one example, or one piece of its
                // page's program.
                let program = found.iter().any(is_program);
                let pieces = found.iter().filter(|e| !is_program(e)).count()
                    + if program { file_blocks(md) } else { 0 };
                if pieces != opening {
                    problems.push(format!(
                        "{page}: {opening} lines open a ```pw block and {pieces} were read: \
                         every block is one example, or one piece of its page's program"
                    ));
                }
                examples.extend(found);
            }
            Err(e) => problems.extend(e),
        }
    }
    (examples, problems)
}

/// **Every example in the guide compiles as the guide says.** All are run,
/// on several threads, and every failure is reported, so one run says all
/// it found.
#[test]
fn every_example_in_the_guide_compiles_as_it_says() {
    let (examples, mut problems) = survey(&pages());
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

/// How many `file=` blocks a page's program was assembled from: the blocks',
/// not the files'.
fn file_blocks(md: &str) -> usize {
    blocks(md)
        .map(|found| {
            found
                .iter()
                .filter(|b| matches!(kind(&b.info), Ok(Kind::File(_))))
                .count()
        })
        .unwrap_or(0)
}

/// The directory a page says it builds, in `<!-- guide-program: DIR -->`.
fn named_directory(md: &str) -> Option<String> {
    md.lines().find_map(|l| {
        l.trim()
            .strip_prefix("<!-- guide-program:")
            .and_then(|r| r.strip_suffix("-->"))
            .map(|dir| dir.trim().to_string())
    })
}

/// **How a page's program differs from the files of the directory it
/// names**, or nothing: its `file=` blocks assemble to exactly those files,
/// byte for byte.
fn program_problem(page: &str, md: &str, on_disk: &BTreeMap<String, String>) -> Option<String> {
    let examples = match examples_of(page, md) {
        Ok(found) => found,
        Err(problems) => return Some(problems.join("; ")),
    };
    let Some(program) = examples.iter().find(|e| is_program(e)) else {
        return Some(format!("{page} names a directory and has no `file=` block"));
    };
    let assembled: BTreeMap<String, String> = program
        .files
        .iter()
        .map(|(name, text)| {
            let base = Path::new(name).file_name().expect("a file name");
            (base.to_string_lossy().to_string(), text.clone())
        })
        .collect();
    differs(&assembled, on_disk).map(|why| format!("{page} and its directory: {why}"))
}

/// **The tutorial is the app the server runs**: a page that names a
/// directory with `<!-- guide-program: DIR -->` assembles, from its `file=`
/// blocks, exactly the `.pw` files in that directory, byte for byte.
#[test]
fn a_pages_program_is_the_directory_it_names() {
    let mut named = 0;
    for (page, md) in pages() {
        let Some(dir) = named_directory(&md) else {
            continue;
        };
        named += 1;
        let mut on_disk = BTreeMap::new();
        for e in std::fs::read_dir(repo().join(&dir)).unwrap_or_else(|e| panic!("{dir}: {e}")) {
            let p = e.expect("entry").path();
            if p.extension().is_some_and(|x| x == "pw") {
                let name = p.file_name().unwrap().to_string_lossy().to_string();
                on_disk.insert(name, std::fs::read_to_string(&p).expect("read"));
            }
        }
        if let Some(why) = program_problem(&page, &md, &on_disk) {
            panic!("{why}");
        }
    }
    assert!(named >= 1, "the tutorial names the app it builds");
}

/// **How a page's program differs from a directory's files**, or nothing
/// where they are the same files, byte for byte.
fn differs(
    assembled: &BTreeMap<String, String>,
    on_disk: &BTreeMap<String, String>,
) -> Option<String> {
    let (page, dir): (Vec<_>, Vec<_>) = (assembled.keys().collect(), on_disk.keys().collect());
    if page != dir {
        return Some(format!(
            "the page assembles {page:?}, and the directory holds {dir:?}"
        ));
    }
    assembled
        .iter()
        .find(|(name, text)| **text != on_disk[*name])
        .map(|(name, _)| format!("`{name}` differs: the tutorial and the app differ"))
}

#[test]
fn a_program_differs_from_a_directory_by_a_file_or_a_byte() {
    let files = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(n, s)| (n.to_string(), s.to_string()))
            .collect()
    };
    let app = files(&[("book.pw", "module a\n"), ("pages.pw", "module b\n")]);
    assert_eq!(differs(&app, &app), None);
    assert!(differs(&app, &files(&[("book.pw", "module a\n")])).is_some());
    assert!(
        differs(
            &app,
            &files(&[("book.pw", "module a\n"), ("pages.pw", "module c\n")])
        )
        .is_some()
    );
    assert!(
        differs(
            &app,
            &files(&[("book.pw", "module a \n"), ("pages.pw", "module b\n")])
        )
        .is_some()
    );
}

/// **The diagnostic reference lists every code the compiler has**, each
/// with the invariant the registry gives it: read from the registry as the
/// compiler sees it, apart from the script that writes the page.
#[test]
fn the_diagnostic_reference_lists_every_registered_code() {
    let page = std::fs::read_to_string(repo().join("docs/guide/diagnostics.md"))
        .expect("docs/guide/diagnostics.md");
    let codes: Vec<(&str, &str)> = pw_core::codes::ALL
        .iter()
        .map(|code| (code.id, code.invariant))
        .collect();
    // One row a code, a retired code and an old number.
    let rows = codes.len()
        + pw_core::codes::RETIRED.len()
        + pw_core::diagnostics::DEPRECATED_ALIASES.len();
    let problems = reference_problems(&page, &codes, rows);
    assert!(
        problems.is_empty(),
        "docs/guide/diagnostics.md is not the registry; run `python3 scripts/guide_diagnostics.py`:\n{}",
        problems.join("\n")
    );
}

/// **What is wrong with the reference of codes**: a code it has no row for,
/// a row without the invariant the registry gives, or rows beyond the
/// `rows` the registry accounts for (a code the compiler lacks).
fn reference_problems(page: &str, codes: &[(&str, &str)], rows: usize) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, invariant) in codes {
        let row = page.lines().find(|l| l.starts_with(&format!("| {id} |")));
        match row {
            Some(row) if row.contains(&escaped(invariant)) => {}
            Some(row) => problems.push(format!("{id}: its invariant is not in `{row}`")),
            None => problems.push(format!("{id}: no row")),
        }
    }
    let found = page.lines().filter(|l| l.starts_with("| PW")).count();
    if found != rows {
        problems.push(format!(
            "{found} rows, and the registry accounts for {rows}: a row for a code the compiler lacks"
        ));
    }
    problems
}

/// A sentence as a Markdown table cell writes it.
fn escaped(text: &str) -> String {
    text.replace('|', "\\|")
}

// --- the verdict, held on its own --------------------------------------------

/// One whole-program example, as a page would give it.
fn example(src: &str, expect: Expect) -> Example {
    Example {
        place: "t.md:1".into(),
        files: vec![(file_name_of(src), src.to_string())],
        expect,
    }
}

/// **A wrong example is reported, and a right one is not**: each way an
/// example can be wrong, given to the verdict alone, so a verdict that
/// stopped looking would be seen even while every page is right.
#[test]
fn a_wrong_example_is_reported_and_a_right_one_is_not() {
    let scratch = std::env::temp_dir().join(format!("pw-guide-verdict-{}", std::process::id()));
    let fails = |codes: &[&str], shown: Option<&str>| {
        Expect::Fails(
            codes.iter().map(|c| c.to_string()).collect(),
            shown.map(str::to_string),
        )
    };
    let right = "module ok\n\nfn one() -> Int !{} {\n    1\n}\n";
    let mistyped = "module bad\n\nfn one() -> String !{} {\n    1\n}\n";
    let unformatted = "module ok\n\nfn one() -> Int !{} {\n  1\n}\n";
    // Checks clean, and `pw build` refuses it (ADR-0137).
    let unbuilt = "module titled\n\npage Counter() {\n    route \"/counter\"\n    cache private\n\n    \
                   signal count: Int = 0\n\n    view {\n        <title>Pressed {count}</title>\n        \
                   <button type=\"button\" on:press={() => count = count + 1}>Press</button>\n    }\n}\n";
    let unparsed = "module logic\n\nfn both(a: Bool, b: Bool) -> Bool !{} {\n    a && b\n}\n";
    // Two mistakes, so two codes: PW0606 and PW0021.
    let twice = "module twice\n\nfn one() -> String !{} {\n    1\n}\n\n\
                 fn two(names: List<String>) -> Int !{} {\n    List.length(names)\n}\n";
    // The same page, a line of it not as `pw fmt` writes it.
    let unbuilt_unformatted = unbuilt.replace("    route", "  route");
    let shown_wrongly = Some("pw: not what it says\n".to_string());
    // Each case says what is wrong with it, by a sentence the report holds,
    // or that nothing is: a report for another reason is a wrong report.
    let cases: Vec<(&str, Example, Option<&str>)> = vec![
        ("a right example", example(right, Expect::Clean), None),
        (
            "a mistyped one",
            example(mistyped, Expect::Clean),
            Some("expected no diagnostics"),
        ),
        (
            "an unformatted one",
            example(unformatted, Expect::Clean),
            Some("not as `pw fmt` writes it"),
        ),
        (
            "one that checks and does not build",
            example(unbuilt, Expect::Clean),
            Some("`pw build` refuses it"),
        ),
        (
            "a refusal by its code",
            example(mistyped, fails(&["PW0606"], None)),
            None,
        ),
        (
            "a refusal by another code",
            example(mistyped, fails(&["PW0021"], None)),
            Some("expected exactly"),
        ),
        (
            "a refusal that names a code too many",
            example(mistyped, fails(&["PW0606", "PW0021"], None)),
            Some("expected exactly"),
        ),
        (
            "a refusal that names a code too few",
            example(twice, fails(&["PW0606"], None)),
            Some("expected exactly"),
        ),
        (
            "a refusal of two mistakes, each named",
            example(twice, fails(&["PW0606", "PW0021"], None)),
            None,
        ),
        (
            "a refusal of a right program",
            example(right, fails(&["PW0606"], None)),
            Some("expected `pw check` to refuse it"),
        ),
        (
            "a refusal shown wrongly",
            example(
                mistyped,
                fails(&["PW0606"], Some("error: [PW0606] not what it says\n")),
            ),
            Some("output `pw check` no longer prints"),
        ),
        // A text that does not parse has no tree for `pw fmt` to write.
        (
            "a refusal that does not parse",
            example(unparsed, fails(&["PW0009"], None)),
            None,
        ),
        (
            "a build gap: checks, and does not build",
            example(unbuilt, Expect::Gap(Gap::Build, None)),
            None,
        ),
        (
            "a build gap that builds",
            example(right, Expect::Gap(Gap::Build, None)),
            Some("`pw build` accepts it: the gap is closed"),
        ),
        (
            "a build gap that does not check",
            example(mistyped, Expect::Gap(Gap::Build, None)),
            Some("a gap is a program `pw check` passes"),
        ),
        (
            "a build gap not as `pw fmt` writes it",
            example(&unbuilt_unformatted, Expect::Gap(Gap::Build, None)),
            Some("not as `pw fmt` writes it"),
        ),
        (
            "a build gap whose build output is shown wrongly",
            example(unbuilt, Expect::Gap(Gap::Build, shown_wrongly)),
            Some("output `pw build` no longer prints"),
        ),
        (
            "a check gap: checks, whatever the build does",
            example(right, Expect::Gap(Gap::Check, None)),
            None,
        ),
        (
            "a check gap that the build refuses says nothing of the build",
            example(unbuilt, Expect::Gap(Gap::Check, None)),
            None,
        ),
        (
            "a check gap that the check refuses",
            example(mistyped, Expect::Gap(Gap::Check, None)),
            Some("a gap is a program `pw check` passes"),
        ),
        (
            "a check gap not as `pw fmt` writes it",
            example(unformatted, Expect::Gap(Gap::Check, None)),
            Some("not as `pw fmt` writes it"),
        ),
    ];
    let mut wrong = Vec::new();
    for (i, (what, e, expected)) in cases.iter().enumerate() {
        let said = verdict(e, &scratch.join(i.to_string()));
        let as_expected = match (&said, expected) {
            (None, None) => true,
            (Some(said), Some(sentence)) => said.contains(sentence),
            _ => false,
        };
        if !as_expected {
            wrong.push(format!("{what}: {said:?}"));
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(
        wrong.is_empty(),
        "the verdict is wrong about:\n{}",
        wrong.join("\n")
    );
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
    // Two backticks are inline code, not a fence.
    assert_eq!(blocks("``code`` at a line's start\n"), Ok(Vec::new()));
}

#[test]
fn an_unclosed_or_indented_fence_is_refused() {
    assert!(blocks("```pw\nmodule a\n").is_err());
    assert!(blocks("- a list\n  ```pw\n  module a\n  ```\n").is_err());
    // Refused for being indented, not only for never closing: this one is
    // closed, at the line's start.
    assert!(blocks("  ```pw\nmodule a\n```\n").is_err());
    // A closing fence carries nothing after it.
    assert!(blocks("```pw\nmodule a\n``` pw\n").is_err());
    // A fence of the other character closes nothing.
    assert!(blocks("```pw\nmodule a\n~~~\n").is_err());
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
    assert_eq!(kind("pw gap=build"), Ok(Kind::Gap(Gap::Build)));
    assert_eq!(kind("pw gap=check"), Ok(Kind::Gap(Gap::Check)));
    assert_eq!(kind("text pw-check"), Ok(Kind::Output));
    assert_eq!(kind("text pw-build"), Ok(Kind::BuildOutput));
    assert_eq!(kind("sh"), Ok(Kind::Other));
    for refused in [
        "",
        "pw fails=",
        "pw fails=E0606",
        "pw fails=PW0606 fails=PW0021",
        "pw file=app.rs",
        "pw file=../app.pw",
        "pw file=/app.pw",
        "pw file=a.pw file=b.pw",
        "pw fails=PW0606 file=a.pw",
        "pw gap=",
        "pw gap=lint",
        "pw gap=build gap=build",
        "pw gap=build gap=check",
        "pw gap=build fails=PW0606",
        "pw gap=check fails=PW0606",
        "pw gap=build file=a.pw",
        "pw gap=check file=a.pw",
        "pw strict",
        "rust",
        "sh strict",
        "text output",
        "text pw-check pw-build",
        "text pw-build strict",
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
    // Nor one that is not just after it, whatever refusal comes earlier.
    assert!(
        examples_of(
            "t.md",
            "```pw fails=PW0606\nmodule a\n```\n\n```sh\nls\n```\n```text pw-check\nx\n```\n"
        )
        .is_err()
    );
    // Not after a gap: that block's output is the build's.
    assert!(
        examples_of(
            "t.md",
            "```pw gap=build\nmodule a\n```\n```text pw-check\nx\n```\n"
        )
        .is_err()
    );
}

#[test]
fn a_gaps_build_output_belongs_to_the_gap_before_it() {
    let md = "```pw gap=build\nmodule bad\n```\n```text pw-build\npw: refused\n```\n";
    let found = examples_of("t.md", md).expect("well formed");
    match &found[0].expect {
        Expect::Gap(Gap::Build, Some(shown)) => assert_eq!(shown, "pw: refused\n"),
        other => panic!("{other:?}"),
    }
    // A build gap shows what the build prints.
    assert!(examples_of("t.md", "```pw gap=build\nmodule a\n```\n").is_err());
    // A check gap has no build to show, and a build's output is not its.
    assert!(examples_of("t.md", "```pw gap=check\nmodule a\n```\n").is_ok());
    assert!(
        examples_of(
            "t.md",
            "```pw gap=check\nmodule a\n```\n```text pw-build\nx\n```\n"
        )
        .is_err()
    );
    // And only a gap's build prints it: not a clean example's, not a refusal's.
    assert!(examples_of("t.md", "```pw\nmodule a\n```\n```text pw-build\nx\n```\n").is_err());
    assert!(
        examples_of(
            "t.md",
            "```pw fails=PW0606\nmodule a\n```\n```text pw-build\nx\n```\n"
        )
        .is_err()
    );
    // Nor one that is not just after it.
    assert!(
        examples_of(
            "t.md",
            "```pw gap=build\nmodule a\n```\n\n```sh\nls\n```\n```text pw-build\nx\n```\n"
        )
        .is_err()
    );
}

/// A page, as `survey` takes it.
fn page(name: &str, md: &str) -> (String, String) {
    (name.to_string(), md.to_string())
}

#[test]
fn a_page_with_no_example_or_a_block_unseen_is_reported() {
    let right = page("a.md", "```pw\nmodule a\n```\n");
    let (examples, problems) = survey(std::slice::from_ref(&right));
    assert_eq!((examples.len(), problems), (1, Vec::new()));
    // A page teaches by example: the index and the reference of codes aside.
    let prose = page("b.md", "No example here.\n");
    let (_, problems) = survey(&[right.clone(), prose]);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("no Pleris example"), "{problems:?}");
    for excused in UNEXAMPLED {
        let (_, problems) = survey(&[page(excused, "No example here.\n")]);
        assert!(problems.is_empty(), "{excused}: {problems:?}");
    }
    // A ```pw line the reader of pages found inside another block, which
    // is no block of its own: a plain count sees it, the extractor does not.
    let hidden = page("c.md", "````text\n```pw\nmodule x\n```\n````\n");
    let (_, problems) = survey(&[hidden]);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("lines open a ```pw block"),
        "{problems:?}"
    );
    // A page's `file=` blocks are pieces of one program, and each is counted.
    let pieces = page(
        "d.md",
        "```pw file=g/a.pw\nmodule g.a\n```\n```pw file=g/a.pw\nfn f() {}\n```\n```pw file=g/b.pw\nmodule g.b\n```\n",
    );
    let (examples, problems) = survey(&[pieces]);
    assert_eq!((examples.len(), problems), (1, Vec::new()));
    // What the extractor refuses is reported, whatever the page.
    let (_, problems) = survey(&[page("e.md", "```pw\nmodule a\n")]);
    assert!(!problems.is_empty());
}

#[test]
fn a_page_is_held_to_the_directory_it_names() {
    let md = "<!-- guide-program: app/guestbook -->\n\n```pw file=guestbook/book.pw\nmodule a\n```\n\n\
              ```pw file=guestbook/pages.pw\nmodule b\n```\n";
    assert_eq!(named_directory(md).as_deref(), Some("app/guestbook"));
    assert_eq!(named_directory("No comment.\n"), None);
    let disk = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(n, s)| (n.to_string(), s.to_string()))
            .collect()
    };
    let app = disk(&[("book.pw", "module a\n"), ("pages.pw", "module b\n")]);
    assert_eq!(program_problem("t.md", md, &app), None);
    // A file the directory has and the page lacks, and a byte apart.
    let more = disk(&[
        ("book.pw", "module a\n"),
        ("pages.pw", "module b\n"),
        ("extra.pw", "module c\n"),
    ]);
    assert!(program_problem("t.md", md, &more).is_some());
    let apart = disk(&[("book.pw", "module a\n"), ("pages.pw", "module b \n")]);
    assert!(program_problem("t.md", md, &apart).is_some());
    // A page that names a directory and builds nothing in it.
    let none = "<!-- guide-program: app/guestbook -->\n\n```pw\nmodule a\n```\n";
    assert!(program_problem("t.md", none, &app).is_some());
    assert!(
        program_problem("t.md", "<!-- guide-program: x -->\n```pw\nmodule a\n", &app).is_some()
    );
}

#[test]
fn the_reference_is_held_to_the_registry() {
    let page = "| PW0001 | `a` | one | — |\n| PW0002 | `b` | two \\| three | [1](x) |\n";
    let codes = [("PW0001", "one"), ("PW0002", "two | three")];
    assert_eq!(reference_problems(page, &codes, 2), Vec::<String>::new());
    // A code with no row.
    let more = [codes[0], codes[1], ("PW0003", "three")];
    let problems = reference_problems(page, &more, 3);
    assert!(
        problems.iter().any(|p| p.contains("PW0003: no row")),
        "{problems:?}"
    );
    // A row whose invariant is not the registry's.
    let other = [codes[0], ("PW0002", "another")];
    let problems = reference_problems(page, &other, 2);
    assert!(
        problems.iter().any(|p| p.contains("PW0002: its invariant")),
        "{problems:?}"
    );
    // A row for a code the compiler lacks.
    let problems = reference_problems(page, &codes[..1], 1);
    assert!(
        problems.iter().any(|p| p.contains("2 rows")),
        "{problems:?}"
    );
}

#[test]
fn a_refused_examples_codes_are_read_from_its_output() {
    let out = "error: [PW0606] a\n --> bad.pw:4:5\nwarning: [PW0402] b\nnote: [PW9999] c\n";
    assert_eq!(
        codes_in(out),
        BTreeSet::from(["PW0606".to_string(), "PW0402".to_string()])
    );
}
