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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let plain = args.iter().any(|a| a == "--plain");
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    let paths: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect();

    if !matches!(cmd, "check" | "explain") || paths.is_empty() {
        eprintln!("usage: pw <check|explain> <path.pw>... [--plain]");
        eprintln!();
        eprintln!("  check    parse and report diagnostics");
        eprintln!("  explain  print types, effects, privacy and derived placement");
        return ExitCode::from(2);
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut files = 0usize;

    for path in paths {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pw: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        files += 1;
        let parsed = parse(&src);
        let display = Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());

        if !parsed.errors.is_empty() {
            print!("{}", render(&src, &display, &parsed.errors, !plain));
            errors += parsed.errors.len();
        } else {
            // Semantic rules only run on a clean parse — recovery invents
            // plausible-looking declarations, and checking those reports on the
            // recovery rather than on the user's code (E0 finding F-4).
            let found = rules::check(&parsed.file);
            if !found.is_empty() {
                print!("{}", render_findings(&src, &display, &found, !plain));
                errors += found.iter().filter(|f| f.is_error()).count();
                warnings += found.iter().filter(|f| !f.is_error()).count();
            }
        }
        if cmd == "explain" && parsed.errors.is_empty() {
            println!("── {display}");
            print!("{}", explain(&parsed.file, &src));
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
        assert!(text.contains("PW0106"), "{text}");
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
