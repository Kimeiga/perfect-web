//! `pw` — the developer command surface (charter §14 M2 task 12).
//!
//! ```text
//! pw check <path>...     parse and report diagnostics
//! pw explain <path>...   print the semantic facts a developer would otherwise infer
//! ```
//!
//! `pw fmt` is deliberately absent. The lexer is lossless so a formatter is
//! *possible*, but an idempotent formatter needs a decision about the
//! declaration layout the corpus should be normalised to, and shipping one that
//! reflows the corpus into a shape nobody agreed on would be worse than not
//! having it. Recorded as open in `docs/milestones/E2.md`.
//!
//! Charter §14 M2 gate: "the compiler can print an effect summary without
//! exposing generated-file paths to the user". `explain` asserts that in tests.

use std::path::Path;
use std::process::ExitCode;

use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};
use pw_core::diagnostics::{Diagnostic, Severity};
use pw_core::rules;
use pw_syntax::ast::{DeclKind, SourceFile, Visibility};
use pw_syntax::parser::{ParseError, parse};

fn render(source: &str, path: &str, errors: &[ParseError], styled: bool) -> String {
    let renderer = if styled {
        Renderer::styled()
    } else {
        Renderer::plain()
    };
    let mut out = String::new();
    for e in errors {
        let snippet = Snippet::source(source).path(path).line_start(1).annotation(
            AnnotationKind::Primary
                .span(e.span.clone())
                .label(&e.message),
        );
        let mut group = Level::ERROR
            .primary_title(format!("[{}] {}", e.code, e.message))
            .element(snippet);
        if let Some(h) = &e.help {
            group = group.element(Level::HELP.message(h.as_str()));
        }
        out.push_str(&renderer.render(&[group]));
        out.push('\n');
    }
    out
}

fn render_findings(source: &str, path: &str, found: &[Diagnostic], styled: bool) -> String {
    let renderer = if styled {
        Renderer::styled()
    } else {
        Renderer::plain()
    };
    let mut out = String::new();
    for f in found {
        let level = if f.severity == Severity::Error {
            Level::ERROR
        } else {
            Level::WARNING
        };
        let mut snippet = Snippet::source(source).path(path).line_start(1).annotation(
            AnnotationKind::Primary
                .span(f.primary_span.clone())
                .label(&f.message),
        );
        // Charter §16.3: name where the conflicting property originated, not
        // only where the rule fired.
        for r in &f.related {
            snippet =
                snippet.annotation(AnnotationKind::Context.span(r.span.clone()).label(&r.label));
        }
        let mut group = level
            .primary_title(format!("[{}] {}", f.code, f.message))
            .element(snippet);
        if let Some(n) = &f.explanation {
            group = group.element(Level::NOTE.message(n.as_str()));
        }
        for r in &f.repairs {
            group = group.element(Level::HELP.message(r.description.as_str()));
        }
        out.push_str(&renderer.render(&[group]));
        out.push('\n');
    }
    out
}

/// The `pw explain` surface: the semantic facts a developer would otherwise
/// have to infer by reading the whole file.
///
/// Nothing here may name a generated artifact — no Koka module, no Marko
/// template, no build directory. That is a charter gate item, and
/// `explain_never_leaks_generated_paths` asserts it.
fn explain(file: &SourceFile, source: &str) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();

    let _ = writeln!(
        s,
        "module       {}",
        file.module
            .as_ref()
            .map(|m| m.name.as_str())
            .unwrap_or("(none)")
    );
    if let Some(id) = file.attr("id") {
        let _ = writeln!(
            s,
            "corpus       {} ({})",
            id,
            file.attr("corpus").unwrap_or("?")
        );
    }
    let _ = writeln!(s);

    for d in &file.decls {
        match &d.kind {
            DeclKind::Module | DeclKind::Import => continue,
            DeclKind::Opaque { representation } => {
                let _ = writeln!(
                    s,
                    "opaque type  {}",
                    d.name.as_ref().map(|n| n.name.as_str()).unwrap_or("?")
                );
                let _ = writeln!(
                    s,
                    "             represented as {} — distinct from it everywhere in the checker",
                    representation
                        .as_ref()
                        .map(|r| r.name.as_str())
                        .unwrap_or("?")
                );
            }
            DeclKind::Union { variants } => {
                let _ = writeln!(
                    s,
                    "union        {} ({} variants)",
                    d.name.as_ref().map(|n| n.name.as_str()).unwrap_or("?"),
                    variants.len()
                );
                for v in variants {
                    let args: Vec<&str> = v.fields.iter().map(|f| f.name.as_str()).collect();
                    let _ = writeln!(
                        s,
                        "             | {}{}",
                        v.name.name,
                        if args.is_empty() {
                            String::new()
                        } else {
                            format!("({})", args.join(", "))
                        }
                    );
                }
                let _ = writeln!(
                    s,
                    "             a match on this must cover every variant, whatever its effect row"
                );
            }
            DeclKind::Function {
                params,
                result,
                effects,
            } => {
                let _ = writeln!(
                    s,
                    "fn           {}",
                    d.name.as_ref().map(|n| n.name.as_str()).unwrap_or("?")
                );
                let ps: Vec<String> = params
                    .iter()
                    .map(|p| {
                        format!(
                            "{}: {}",
                            p.name.name,
                            p.ty.as_ref().map(|t| t.name.as_str()).unwrap_or("?")
                        )
                    })
                    .collect();
                let _ = writeln!(s, "             ({})", ps.join(", "));
                if let Some(r) = result {
                    let _ = writeln!(s, "             -> {}", r.name);
                }
                match effects {
                    None => {
                        let _ = writeln!(s, "             effects: (unannotated — claims nothing)");
                    }
                    Some(row) if row.effects.is_empty() => {
                        let _ =
                            writeln!(s, "             effects: !{{}}  <- explicit purity claim");
                    }
                    Some(row) => {
                        let names: Vec<&str> =
                            row.effects.iter().map(|e| e.name.as_str()).collect();
                        let _ = writeln!(s, "             effects: !{{{}}}", names.join(", "));
                        let _ = writeln!(s, "             placement: {}", derive_placement(&names));
                    }
                }
            }
            DeclKind::Resource {
                noun,
                visibility,
                policies,
                ..
            } => {
                let _ = writeln!(
                    s,
                    "{noun:<12} {}",
                    d.name.as_ref().map(|n| n.name.as_str()).unwrap_or("?")
                );
                let label = match visibility {
                    Visibility::Public => "Public",
                    Visibility::Session => "Session<SessionId>",
                    Visibility::Private => "User<UserId>",
                    Visibility::Unspecified => "(unspecified)",
                };
                let _ = writeln!(s, "             privacy: {label}");
                for p in policies {
                    let _ = writeln!(s, "             {:<14} {}", p.keyword.name, p.value);
                }
                let cache = policies.iter().find(|p| p.keyword.name == "cache");
                if let (Some(c), Visibility::Session | Visibility::Private) = (cache, visibility)
                    && c.value.starts_with("shared")
                {
                    let _ = writeln!(
                        s,
                        "             WARNING: a {label} value in a shared cache is rejected at E5"
                    );
                }
            }
            DeclKind::Ui { noun, effects, .. } => {
                let _ = writeln!(
                    s,
                    "{noun:<12} {}",
                    d.name.as_ref().map(|n| n.name.as_str()).unwrap_or("?")
                );
                if let Some(row) = effects {
                    let names: Vec<&str> = row.effects.iter().map(|e| e.name.as_str()).collect();
                    if names.is_empty() {
                        let _ = writeln!(s, "             effects: !{{}}  <- pure render");
                    } else {
                        let _ = writeln!(s, "             effects: !{{{}}}", names.join(", "));
                    }
                }
            }
            DeclKind::Record { .. } | DeclKind::Unrecognised => {
                if let Some(n) = &d.name {
                    let _ = writeln!(s, "declaration  {}", n.name);
                }
            }
        }
        let _ = writeln!(s, "             at bytes {}..{}", d.span.start, d.span.end);
        let line = source[..d.span.start.min(source.len())]
            .lines()
            .count()
            .max(1);
        let _ = writeln!(s, "             line {line}");
        let _ = writeln!(s);
    }
    s
}

/// Placement derived from effects alone — charter §7.9's claim, in miniature.
fn derive_placement(effects: &[&str]) -> &'static str {
    let has = |p: &str| effects.iter().any(|e| e.starts_with(p));
    if has("secret") || has("database.write") {
        "Origin (secret or write capability required)"
    } else if has("database") {
        "Origin or Edge (database read)"
    } else if has("device") || has("layout") || has("dom") || has("observe") || has("paint") {
        "Browser (browser-only capability required)"
    } else if has("clock.wall") || has("random") {
        "not Build (nondeterministic)"
    } else {
        "unconstrained by effects"
    }
}

/// `pw fmt` — rewrite in place, or with `--check` report and exit non-zero.
///
/// `--check` never writes. It is the CI form: a repository is either canonical
/// or the build fails, with no third state where a tool silently edits code.
fn fmt_command(paths: &[&String], check_only: bool) -> ExitCode {
    let mut changed = Vec::new();
    for path in paths {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pw: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        let out = pw_syntax::format_source(&src);
        if out == src {
            continue;
        }
        changed.push((*path).clone());
        if let Some(Err(e)) = (!check_only).then(|| std::fs::write(path, &out)) {
            eprintln!("pw: cannot write {path}: {e}");
            return ExitCode::from(2);
        }
    }

    if changed.is_empty() {
        println!("pw fmt: {} file(s) already formatted", paths.len());
        return ExitCode::SUCCESS;
    }
    if check_only {
        for c in &changed {
            println!("would reformat: {c}");
        }
        println!(
            "pw fmt: {} of {} file(s) need formatting",
            changed.len(),
            paths.len()
        );
        return ExitCode::FAILURE;
    }
    println!(
        "pw fmt: reformatted {} of {} file(s)",
        changed.len(),
        paths.len()
    );
    ExitCode::SUCCESS
}

/// `pw emit-koka` — print the generated Koka, and every skipped declaration.
///
/// Skips go to stderr rather than being dropped: a backend that covers a subset
/// has to say which subset, or its output looks like a complete translation.
fn emit_koka_command(paths: &[&String]) -> ExitCode {
    for path in paths {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pw: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        let parsed = pw_syntax::parse_tree(&src);
        if !parsed.ok() {
            eprintln!("pw: {path} does not parse; nothing emitted");
            return ExitCode::FAILURE;
        }
        let hir = pw_core::lower::lower_file(&src, &parsed.green);
        let module = hir
            .modules
            .iter()
            .next()
            .map(|(_, m, _)| m.name.clone())
            .unwrap_or_else(|| "main".to_string());
        let out = pw_core::koka::lower_module(&hir, &module);
        print!("{}", out.source);
        for s in &out.skipped {
            eprintln!("skipped {}: {}", s.name, s.reason);
        }
    }
    ExitCode::SUCCESS
}

/// `pw emit-marko` — write generated templates under `--out DIR` (ADR-0017).
///
/// Requires an explicit output directory. Defaulting to the source directory
/// would put build output next to authoring source, which is the exact thing
/// charter §14 M3 task 3 forbids.
fn emit_marko_command(paths: &[&String], out_dir: Option<String>) -> ExitCode {
    let Some(out_dir) = out_dir else {
        eprintln!("pw emit-marko: --out DIR is required");
        eprintln!("  generated templates are build output and must not be written");
        eprintln!("  beside authoring source (charter §14 M3 task 3, ADR-0017)");
        return ExitCode::from(2);
    };
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("pw: cannot create {out_dir}: {e}");
        return ExitCode::from(2);
    }

    let mut written = 0usize;
    let mut skipped = 0usize;
    for path in paths {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pw: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        let parsed = pw_syntax::parse_tree(&src);
        if !parsed.ok() {
            eprintln!("pw: {path} does not parse; nothing emitted");
            return ExitCode::FAILURE;
        }
        let hir = pw_core::lower::lower_file(&src, &parsed.green);
        let name = Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| (*path).clone());
        let out = pw_core::marko::render_module(&hir, &name);

        for (file, text) in &out.files {
            let dest = Path::new(&out_dir).join(file);
            if let Err(e) = std::fs::write(&dest, text) {
                eprintln!("pw: cannot write {}: {e}", dest.display());
                return ExitCode::from(2);
            }
            println!("{}", dest.display());
            written += 1;
        }
        for s in &out.skipped {
            eprintln!("skipped {}: {}", s.name, s.reason);
            skipped += 1;
        }
    }

    println!("pw emit-marko: {written} file(s) written, {skipped} skipped");
    if written == 0 {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// `pw emit-manifest` — the artifact that connects a declaration to the runtime.
///
/// Policies the schema could not interpret go to stderr and set a failing exit
/// code. A manifest with a silently defaulted freshness is worse than none: the
/// runtime would serve stale data and nothing would say why.
fn emit_manifest_command(paths: &[&String]) -> ExitCode {
    let mut all = Vec::new();
    let mut unparsed = 0usize;

    for path in paths {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pw: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        let parsed = pw_syntax::parse_tree(&src);
        if !parsed.ok() {
            eprintln!("pw: {path} does not parse; nothing emitted");
            return ExitCode::FAILURE;
        }
        let hir = pw_core::lower::lower_file(&src, &parsed.green);
        let built = pw_core::manifest::build(&hir);
        for u in &built.unparsed {
            eprintln!(
                "{path}: `{}` declares `{} {}` which is {}",
                u.declaration, u.policy, u.value, u.reason
            );
            unparsed += 1;
        }
        all.extend(built.manifests);
    }

    match serde_json::to_string_pretty(&all) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("pw: cannot serialize manifests: {e}");
            return ExitCode::from(2);
        }
    }
    if unparsed > 0 {
        eprintln!("pw emit-manifest: {unparsed} policy value(s) not understood");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// `pw emit-graph` — the resource dependency graph (charter §14 M6 gate 6).
///
/// Every path at once, never per file. A materialization in one module depends
/// on a query in another, and a graph built one file at a time would report a
/// dangling edge for exactly the case E6 is about.
///
/// `--plain` prints the edges for a person; the default prints JSON. Both, and
/// not one dressed as the other: the gate asks for inspectable AND
/// serializable, and a blob is not inspectable.
fn emit_graph_command(paths: &[&String], plain: bool) -> ExitCode {
    let mut hirs = Vec::new();
    for path in paths {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pw: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        let parsed = pw_syntax::parse_tree(&src);
        if !parsed.ok() {
            eprintln!("pw: {path} does not parse; nothing emitted");
            return ExitCode::FAILURE;
        }
        hirs.push(pw_core::lower::lower_file(&src, &parsed.green));
    }
    let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();
    let ws = pw_core::resolve::Workspace::build(&refs);
    let g = pw_core::graph::Graph::build(&refs, &ws);

    if plain {
        for n in &g.nodes {
            println!("{}  {}", node_label(&n.kind), n.path);
        }
        println!();
        for e in &g.edges {
            let key = if e.key.is_empty() {
                String::new()
            } else {
                format!("({})", e.key.join(", "))
            };
            println!("{}  --{}-->  {}{key}", e.from, edge_label(e.kind), e.to);
        }
        if !g.dangling.is_empty() {
            println!();
            for d in &g.dangling {
                println!(
                    "DANGLING  {} --{}--> {} (nothing declares it)",
                    d.from,
                    edge_label(d.kind),
                    d.name
                );
            }
        }
    } else {
        match serde_json::to_string_pretty(&g) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("pw: cannot serialize the graph: {e}");
                return ExitCode::from(2);
            }
        }
    }

    // A dangling edge is a failing exit code, not a note. A materialization
    // listening for an event nobody declares never regenerates, and the page
    // is not wrong — only permanently stale, which no test of the page finds.
    if !g.dangling.is_empty() {
        eprintln!("pw emit-graph: {} dangling edge(s)", g.dangling.len());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn node_label(k: &pw_core::graph::NodeKind) -> &'static str {
    use pw_core::graph::NodeKind::*;
    match k {
        Resource { .. } => "resource     ",
        Materialization { .. } => "materialize  ",
        Event => "event        ",
        Command => "command      ",
        Page => "page         ",
    }
}

fn edge_label(k: pw_core::graph::EdgeKind) -> &'static str {
    use pw_core::graph::EdgeKind::*;
    match k {
        Reads => "reads",
        InvalidatedBy => "invalidated-by",
        Emits => "emits",
        Invalidates => "invalidates",
    }
}

/// Turn an internal panic into a conspicuous, reproducible report.
///
/// Charter §3.1 and the architect's compiler-robustness gate: for every
/// syntactically representable program the compiler must produce output,
/// ordinary diagnostics, or a **clearly marked internal compiler error**. It
/// must not terminate without a report.
///
/// The reason is not politeness. When `exhaust.rs` panicked on a constructor
/// arity mismatch, every rule in that invocation reported nothing for every
/// file — a compiler that reports nothing is indistinguishable from a compiler
/// that found nothing, and the user's next move is to assume their code is
/// fine.
///
/// Containment is for the user and for the evidence. It is NOT a way of
/// tolerating the defect: the exit code is still a failure, the message says
/// this is a bug in `pw`, and `just ci` runs the robustness suite which fails
/// hard on any panic at all.
fn main() -> ExitCode {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!();
        eprintln!("internal compiler error: this is a bug in `pw`, not in your code.");
        eprintln!("  {info}");
        eprintln!();
        eprintln!("  what to do: the arguments below reproduce it. Please file them");
        eprintln!("  with the source file. Nothing was checked, so the absence of");
        eprintln!("  other diagnostics means nothing.");
        eprintln!(
            "  reproduce: pw {}",
            std::env::args().skip(1).collect::<Vec<_>>().join(" ")
        );
        let _ = &previous;
    }));

    match std::panic::catch_unwind(run) {
        Ok(code) => code,
        // Distinct from an ordinary diagnostic failure, so a script can tell
        // "your program is wrong" from "the compiler is wrong".
        Err(_) => ExitCode::from(101),
    }
}

fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let plain = args.iter().any(|a| a == "--plain");
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    // `--out DIR` takes a value, so DIR must not also be read as a source
    // path. Without this the output directory is opened as a `.pw` file and the
    // command fails after already having written its files.
    let mut paths: Vec<&String> = Vec::new();
    let mut skip_next = false;
    for (i, a) in args.iter().enumerate().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if a == "--out" {
            skip_next = true;
            continue;
        }
        if a.starts_with("--") {
            continue;
        }
        let _ = i;
        paths.push(a);
    }

    if !matches!(
        cmd,
        "check" | "explain" | "fmt" | "emit-koka" | "emit-marko" | "emit-manifest" | "emit-graph"
    ) || paths.is_empty()
    {
        eprintln!(
            "usage: pw <check|explain|fmt|emit-koka|emit-marko|emit-manifest|emit-graph> \
             <path.pw>... [--plain]"
        );
        eprintln!();
        eprintln!("  check          parse and report diagnostics");
        eprintln!("  explain        print types, effects, privacy and derived placement");
        eprintln!("  fmt            rewrite files canonically; --check reports instead");
        eprintln!("  emit-koka      print Koka for the pure subset (ADR-0015)");
        eprintln!("  emit-marko     write Marko templates to --out DIR (ADR-0017)");
        eprintln!("  emit-manifest  print the resource manifests as JSON");
        eprintln!("  emit-graph     print the resource dependency graph (--plain for text)");
        return ExitCode::from(2);
    }

    if cmd == "emit-manifest" {
        return emit_manifest_command(&paths);
    }

    if cmd == "emit-graph" {
        return emit_graph_command(&paths, args.iter().any(|a| a == "--plain"));
    }

    if cmd == "emit-koka" {
        return emit_koka_command(&paths);
    }

    if cmd == "emit-marko" {
        let out_dir = args
            .iter()
            .position(|a| a == "--out")
            .and_then(|i| args.get(i + 1))
            .cloned();
        return emit_marko_command(&paths, out_dir);
    }

    if cmd == "fmt" {
        return fmt_command(&paths, args.iter().any(|a| a == "--check"));
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut files = 0usize;

    // Two passes. Body-level checks need a whole-program type environment: a
    // `match` in one file can be on a type declared in another, so a per-file
    // loop cannot check it at all (assumption A-009).
    struct Input {
        display: String,
        src: String,
        parsed: pw_syntax::Parsed,
    }
    let mut inputs = Vec::new();
    for path in paths {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pw: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        files += 1;
        let display = Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        let parsed = parse(&src);
        inputs.push(Input {
            display,
            src,
            parsed,
        });
    }

    // Only files that parse cleanly enter the program. Recovery invents
    // plausible-looking declarations, and lowering those would put fictional
    // types in the environment for every other file (E0 finding F-4).
    let clean: Vec<(String, String)> = inputs
        .iter()
        .filter(|i| i.parsed.errors.is_empty())
        .map(|i| (i.display.clone(), i.src.clone()))
        .collect();
    let body_diags: std::collections::HashMap<String, Vec<Diagnostic>> =
        pw_core::check::check_sources(&clean).into_iter().collect();

    for input in &inputs {
        let Input {
            display,
            src,
            parsed,
        } = input;

        if !parsed.errors.is_empty() {
            print!("{}", render(src, display, &parsed.errors, !plain));
            errors += parsed.errors.len();
        } else {
            // Semantic rules only run on a clean parse, for the same reason.
            let mut found = rules::check(&parsed.file);
            found.extend(body_diags.get(display).cloned().unwrap_or_default());
            found.sort_by_key(|d| d.primary_span.start);
            if !found.is_empty() {
                print!("{}", render_findings(src, display, &found, !plain));
                errors += found.iter().filter(|f| f.is_error()).count();
                warnings += found.iter().filter(|f| !f.is_error()).count();
            }
        }
        if cmd == "explain" && parsed.errors.is_empty() {
            println!("── {display}");
            print!("{}", explain(&parsed.file, src));
        }
    }

    if cmd == "check" {
        if errors == 0 && warnings == 0 {
            println!("pw check: {files} file(s), no diagnostics");
        } else {
            println!("pw check: {files} file(s), {errors} error(s), {warnings} warning(s)");
        }
    }

    if errors > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explain_never_leaks_generated_paths() {
        // Charter §14 M2 gate: the compiler prints an effect summary "without
        // exposing generated-file paths to the user".
        let src = "module store.pricing\n\npublic query Store(id: StoreId) -> Store\n    cache shared\n{\n    Stores.get(id)\n}\n\nfn subtotal(c: Cart) -> Money !{} { 0 }\n";
        let parsed = parse(src);
        let text = explain(&parsed.file, src);
        for leak in [
            "koka",
            "Koka",
            ".kki",
            "marko",
            "Marko",
            ".pw-build",
            "generated/",
            "target/",
        ] {
            assert!(!text.contains(leak), "explain leaked {leak:?}:\n{text}");
        }
        assert!(text.contains("store.pricing"));
        assert!(text.contains("explicit purity claim"), "{text}");
    }

    #[test]
    fn explain_shows_every_policy_the_compiler_knows_about() {
        // Charter §14 M4 gate: *all* query and command policies are visible in
        // `pw explain`. Checked against HIR rather than against a hand-written
        // list, so a policy the language gains cannot quietly stop being shown.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/accepted");
        let mut checked = 0usize;
        let mut missing = Vec::new();

        for entry in std::fs::read_dir(&root).expect("accepted/") {
            let path = entry.expect("entry").path();
            if path.extension().is_none_or(|e| e != "pw") {
                continue;
            }
            let src = std::fs::read_to_string(&path).expect("read");
            let text = explain(&parse(&src).file, &src);
            let hir = pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green);

            for (_, decl) in hir.all_decls() {
                for policy in &decl.policies {
                    checked += 1;
                    // The name must appear, and so must the value — showing
                    // `retry` without `bounded_exponential(max = 3)` tells a
                    // reader a policy exists but not what it says.
                    let shown = text.contains(&policy.name)
                        && (policy.value.is_empty() || text.contains(&policy.value));
                    if !shown {
                        missing.push(format!(
                            "{}: {} {}",
                            path.file_name().unwrap().to_string_lossy(),
                            policy.name,
                            policy.value
                        ));
                    }
                }
            }
        }

        assert!(
            checked >= 20,
            "expected a real sample, saw {checked} policies"
        );
        assert!(
            missing.is_empty(),
            "{} of {checked} policies are not visible in `pw explain`:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }

    /// The other half of ADR-0018's boundary for E6.
    ///
    /// `runtime/pw-materialize/tests/store-graph.json` is real `pw emit-graph`
    /// output, checked in and read by a crate that shares no type with the
    /// compiler. That is what makes the boundary testable, and it is also how
    /// a checked-in fixture goes stale: rename a field here and the runtime's
    /// tests keep passing against last week's bytes, until an empty list at run
    /// time becomes a fragment nothing ever invalidates.
    ///
    /// So the fixture is regenerated and compared. A rename fails a test.
    #[test]
    fn the_committed_graph_matches_what_the_compiler_emits() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut paths: Vec<std::path::PathBuf> = vec![root.join("examples/domain.pw")];
        for dir in ["examples/lib", "examples/store"] {
            let mut found: Vec<_> = std::fs::read_dir(root.join(dir))
                .unwrap_or_else(|e| panic!("{dir}: {e}"))
                .map(|e| e.expect("entry").path())
                .filter(|p| p.extension().is_some_and(|x| x == "pw"))
                .collect();
            found.sort();
            paths.extend(found);
        }

        let hirs: Vec<pw_core::hir::Hir> = paths
            .iter()
            .map(|p| {
                let src = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
                let parsed = pw_syntax::parse_tree(&src);
                assert!(parsed.ok(), "{p:?} does not parse");
                pw_core::lower::lower_file(&src, &parsed.green)
            })
            .collect();
        let refs: Vec<&pw_core::hir::Hir> = hirs.iter().collect();
        let ws = pw_core::resolve::Workspace::build(&refs);
        let g = pw_core::graph::Graph::build(&refs, &ws);
        let fresh = serde_json::to_string_pretty(&g).expect("serialize");

        let committed =
            std::fs::read_to_string(root.join("runtime/pw-materialize/tests/store-graph.json"))
                .expect("store-graph.json");

        assert_eq!(
            fresh.trim(),
            committed.trim(),
            "the committed graph is stale. Regenerate it with `just materialize` \
             and say in the commit message what changed about the graph's shape."
        );
    }

    #[test]
    fn explain_flags_a_session_value_in_a_shared_cache() {
        // The charter §7.8 rule, surfaced as a forward-looking warning until E5
        // makes it an error.
        let src = "module cart\n\nsession query Cart(s: SessionId) -> Cart\n    cache shared\n{\n    Carts.current(s)\n}\n";
        let parsed = parse(src);
        let text = explain(&parsed.file, src);
        assert!(text.contains("Session<SessionId>"), "{text}");
        assert!(text.contains("rejected at E5"), "{text}");
    }

    #[test]
    fn explain_derives_placement_from_effects_alone() {
        let src = "module m\nfn f() -> () !{ secret<Payments> } { 0 }\nfn g() -> () !{ layout.measure } { 0 }\n";
        let parsed = parse(src);
        let text = explain(&parsed.file, src);
        assert!(text.contains("Origin (secret"), "{text}");
        assert!(text.contains("Browser (browser-only"), "{text}");
    }

    #[test]
    fn check_renders_a_diagnostic_with_a_real_span() {
        let src = "module m\n\n$$$\n";
        let parsed = parse(src);
        assert!(!parsed.ok());
        let text = render(src, "m.pw", &parsed.errors, false);
        // Syntax errors live in PW00xx; PW01xx and above are semantic
        // invariants. They shared a range until the collision surfaced.
        assert!(text.contains("PW0007"), "{text}");
        assert!(text.contains("m.pw"), "{text}");
        assert!(!text.contains('\u{1b}'), "plain output must be ANSI-free");
    }

    #[test]
    fn unannotated_and_empty_effect_rows_read_differently() {
        let src = "module m\nfn a() -> Int { 0 }\nfn b() -> Int !{} { 0 }\n";
        let text = explain(&parse(src).file, src);
        assert!(text.contains("unannotated — claims nothing"), "{text}");
        assert!(text.contains("explicit purity claim"), "{text}");
    }
}
