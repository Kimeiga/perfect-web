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

fn render(source: &str, path: &str, errors: &[pw_syntax::SyntaxError], styled: bool) -> String {
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
///
/// E6F: reads the HIR. It used to read a second declaration tree from a second
/// parser, and E6 showed what two parsers cost — a `materialize` block had
/// policies to one of them and none to the other, in the same build, for the
/// same file. There is now one parser that decides what a `.pw` program means.
/// `explain`, with the whole-program graph when the caller has one.
///
/// A materialization's key audit is a whole-program question — what a fragment
/// separates depends on what its dependencies separate — so a single-file
/// `explain` cannot answer it and says so rather than printing a smaller
/// answer that looks complete.
fn explain_with(
    hir: &pw_core::hir::Hir,
    source: &str,
    graph: Option<&pw_core::graph::Graph>,
) -> String {
    use pw_core::hir::DeclKind;
    use std::fmt::Write as _;
    let mut s = String::new();

    let module = hir
        .modules
        .iter()
        .next()
        .map(|(_, m, _)| m.name.clone())
        .unwrap_or_else(|| "(none)".to_string());
    let _ = writeln!(s, "module       {module}");

    // `@id:` and `@corpus:` are doc attributes in comments. Read from the
    // source, because neither tree carries trivia into its declarations.
    let attr = |k: &str| {
        source
            .lines()
            .find_map(|l| l.trim().strip_prefix(&format!("// @{k}:")))
            .map(str::trim)
    };
    if let Some(id) = attr("id") {
        let _ = writeln!(s, "corpus       {} ({})", id, attr("corpus").unwrap_or("?"));
    }
    let _ = writeln!(s);

    for (id, d) in hir.all_decls() {
        let name = if d.name.is_empty() { "?" } else { &d.name };
        let effects: Option<Vec<&str>> = d
            .declared_effects
            .as_ref()
            .map(|row| row.iter().map(|e| e.written.as_str()).collect());

        match d.kind {
            DeclKind::Import => continue,
            DeclKind::Effect => {
                let _ = writeln!(s, "effect       {name}");
            }
            DeclKind::Opaque => {
                let _ = writeln!(s, "opaque type  {name}");
                let _ = writeln!(
                    s,
                    "             represented as {} — distinct from it everywhere in the checker",
                    d.opaque_of.as_deref().unwrap_or("?")
                );
            }
            DeclKind::Type if d.variants.is_some() => {
                let variants = d.variants.as_deref().unwrap_or_default();
                let _ = writeln!(s, "union        {name} ({} variants)", variants.len());
                for v in variants {
                    let _ = writeln!(
                        s,
                        "             | {}{}",
                        v.name,
                        if v.fields.is_empty() {
                            String::new()
                        } else {
                            format!("({})", v.fields.join(", "))
                        }
                    );
                }
                let _ = writeln!(
                    s,
                    "             a match on this must cover every variant, whatever its effect row"
                );
            }
            DeclKind::Type => {
                let _ = writeln!(s, "record       {name}");
                for f in d.fields.as_deref().unwrap_or_default() {
                    let _ = writeln!(
                        s,
                        "             {}: {}",
                        f.name,
                        f.ty.as_ref().map(|t| t.written()).unwrap_or("?".into())
                    );
                }
            }
            DeclKind::Fn => {
                let _ = writeln!(s, "fn           {name}");
                let ps: Vec<String> = d
                    .params
                    .iter()
                    .map(|p| {
                        let ty = p.ty.as_ref().map(|t| t.written());
                        format!("{}: {}", p.name, ty.as_deref().unwrap_or("?"))
                    })
                    .collect();
                let _ = writeln!(s, "             ({})", ps.join(", "));
                if let Some(r) = &d.ret {
                    let _ = writeln!(s, "             -> {r}");
                }
                match &effects {
                    None => {
                        let _ = writeln!(s, "             effects: (unannotated — claims nothing)");
                    }
                    Some(row) if row.is_empty() => {
                        let _ =
                            writeln!(s, "             effects: !{{}}  <- explicit purity claim");
                    }
                    Some(row) => {
                        let _ = writeln!(s, "             effects: !{{{}}}", row.join(", "));
                        let _ = writeln!(s, "             placement: {}", derive_placement(row));
                    }
                }
            }
            DeclKind::Query
            | DeclKind::Command
            | DeclKind::Subscription
            | DeclKind::Resource
            | DeclKind::Materialize
            | DeclKind::Event
            | DeclKind::Task => {
                let noun = noun_of(d.kind);
                let _ = writeln!(s, "{noun:<12} {name}");
                let label = match d.visibility.as_deref() {
                    Some("public") => "Public",
                    Some("session") => "Session<SessionId>",
                    Some("private") => "User<UserId>",
                    _ => "(unspecified)",
                };
                let _ = writeln!(s, "             privacy: {label}");
                for p in &d.policies {
                    let _ = writeln!(s, "             {:<14} {}", p.name, p.value);
                }
                if let Some(c) = d.policies.iter().find(|p| p.name == "cache")
                    && c.value.starts_with("shared")
                    && matches!(d.visibility.as_deref(), Some("session") | Some("private"))
                {
                    let _ = writeln!(
                        s,
                        "             WARNING: a {label} value in a shared cache is rejected at E5"
                    );
                }
                if d.kind == DeclKind::Materialize {
                    let module = hir.module_of(id).unwrap_or_default();
                    let path = if module.is_empty() {
                        name.to_string()
                    } else {
                        format!("{module}.{name}")
                    };
                    s.push_str(&key_audit(graph, &path));
                }
            }
            DeclKind::View | DeclKind::Component | DeclKind::Page => {
                let noun = noun_of(d.kind);
                let _ = writeln!(s, "{noun:<12} {name}");
                for p in &d.policies {
                    let _ = writeln!(s, "             {:<14} {}", p.name, p.value);
                }
                if let Some(row) = &effects {
                    if row.is_empty() {
                        let _ = writeln!(s, "             effects: !{{}}  <- pure render");
                    } else {
                        let _ = writeln!(s, "             effects: !{{{}}}", row.join(", "));
                    }
                }
            }
            // `prelude Effect` — what this package makes ambient. Shown,
            // because a reader asking "why does this file resolve
            // `database.read` without importing anything" needs the answer to
            // be visible somewhere.
            DeclKind::Prelude => {
                let _ = writeln!(s, "prelude      {name} namespace exported by this module");
            }
            DeclKind::Let | DeclKind::Other => {
                let _ = writeln!(s, "declaration  {name}");
            }
        }

        // **Every policy, whatever the declaration kind.**
        //
        // Charter §14 M4's gate is that a policy the compiler knows about is
        // visible here, and the arms above cover the kinds that had policies
        // when it was written. `replicated` and `paint` gained theirs on
        // 2026-08-11, when data-operation declarations began parsing their
        // clauses inside their braces — and `A-011`'s six and `A-021`'s two
        // became invisible the moment they became real.
        //
        // Kept as a sweep after the match rather than a ninth arm: a new
        // declaration kind should not be able to hide its policies by being
        // new. The `printed` guard is what stops the arms above double-printing.
        if !matches!(
            d.kind,
            DeclKind::Query
                | DeclKind::Command
                | DeclKind::Subscription
                | DeclKind::Resource
                | DeclKind::Materialize
                | DeclKind::Task
                | DeclKind::View
                | DeclKind::Component
                | DeclKind::Page
        ) {
            for p in &d.policies {
                let _ = writeln!(s, "             {:<14} {}", p.name, p.value);
            }
        }

        let span = hir.decl_span(id);
        let _ = writeln!(s, "             at bytes {}..{}", span.start, span.end);
        let line = source[..span.start.min(source.len())]
            .lines()
            .count()
            .max(1);
        let _ = writeln!(s, "             line {line}");
        let _ = writeln!(s);
    }
    s
}

/// Charter §14 M6 task 9 — cache-key auditing.
///
/// Reported as advice, not as a diagnostic. The compiler cannot always tell a
/// deliberate omission from a mistake: two readers sharing an entry is
/// sometimes exactly what a cache is for. What it CAN do is say which
/// dimensions the key separates and which a dependency separates that this key
/// does not, and let a reader decide.
///
/// The build that produced the artifact is deliberately absent from the
/// application section. `PW5102` used to ask the author for it and was retired
/// — injecting a compatibility generation is platform mechanism.
fn key_audit(graph: Option<&pw_core::graph::Graph>, path: &str) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let Some(g) = graph else {
        // Said rather than omitted. A smaller answer that looks complete is
        // the failure mode this whole milestone was about.
        let _ = writeln!(
            s,
            "             key audit: needs the whole program; pass every file"
        );
        return s;
    };
    let Some(a) = g.key_audit(path) else {
        return s;
    };

    let _ = writeln!(s, "             logical key");
    if a.logical_key.is_empty() {
        let _ = writeln!(s, "               (none — one entry for every reader)");
    }
    for k in &a.logical_key {
        let _ = writeln!(s, "               {k}");
    }

    let _ = writeln!(s, "             automatic platform dimensions");
    let _ = writeln!(s, "               build generation    included");
    let _ = writeln!(
        s,
        "               privacy partition   {}",
        a.partition.as_deref().unwrap_or("NOT DECLARED")
    );

    let _ = writeln!(s, "             application dimensions");
    for (d, present) in &a.application {
        let _ = writeln!(
            s,
            "               {:<19} {}",
            d.keyword(),
            if *present { "included" } else { "absent" }
        );
    }

    if !a.gaps.is_empty() {
        let _ = writeln!(s, "             POTENTIAL KEY GAP");
        for gap in &a.gaps {
            let _ = writeln!(
                s,
                "               `{}` separates by `{}`, this key does not",
                gap.resource, gap.component
            );
            let _ = writeln!(s, "                 {}", gap.why);
        }
    }
    s
}

/// The declaration keyword, so `explain` names what the author wrote.
fn noun_of(kind: pw_core::hir::DeclKind) -> &'static str {
    use pw_core::hir::DeclKind::*;
    match kind {
        Query => "query",
        Command => "command",
        Subscription => "subscription",
        Resource => "resource",
        Materialize => "materialize",
        Event => "event",
        Task => "task",
        View => "view",
        Component => "component",
        Page => "page",
        _ => "declaration",
    }
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
    let mut refused: Vec<String> = Vec::new();
    for path in paths {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pw: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        // **Never overwrite source the parser has said it does not
        // understand.** Architect ruling, 2026-08-07:
        //
        // > Formatting to stdout for inspection is one thing; overwriting
        // > source whose meaning the parser has explicitly said it does not
        // > understand is another.
        //
        // A file containing an `UnknownPolicy` or an error node has been
        // rejected, and rewriting it in place would canonicalise a program
        // whose semantics the compiler never established — turning "I could not
        // read this" into "here is my version of it".
        //
        // `--check` still reports, because reading is safe and a formatting
        // difference is worth knowing about either way.
        let parsed = pw_syntax::parse_tree(&src);
        if !check_only && !parsed.ok() {
            eprintln!(
                "pw fmt: refusing to rewrite {path}: it does not parse cleanly,                  and formatting it would canonicalise a program whose meaning                  this compiler has not established. Run `pw check {path}` first."
            );
            refused.push((*path).clone());
            continue;
        }
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

    if !refused.is_empty() {
        println!(
            "pw fmt: refused {} file(s) that do not parse cleanly",
            refused.len()
        );
        return ExitCode::FAILURE;
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

/// `pw emit-template` — the checked template IR (charter §14 M7 task 2).
///
/// The artifact the E7 renderer consumes. Printed as JSON for the same reason
/// the manifest and the graph are: a runtime that the compiler depends on is
/// the only runtime there can ever be (ADR-0018).
fn emit_template_command(paths: &[&String], plain: bool) -> ExitCode {
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

    // Handler identities, derived once by the resume artifacts and read by the
    // template IR. Per file, because `located` walks one HIR against the whole
    // program's signatures.
    let ws = pw_core::resolve::Workspace::build(&refs);
    let sigs = pw_core::signatures::Signatures::build(&ws, &refs);
    let mut handlers = pw_core::template_ir::Handlers::new();
    for (path, hir) in paths.iter().zip(&hirs) {
        let src = std::fs::read_to_string(path).unwrap_or_default();
        for (decl, lambda, m, _) in
            pw_core::resume_artifacts::located(&src, hir, &sigs, pw_core::resume_artifacts::BUILD)
        {
            handlers.insert((decl, lambda), m.handler);
        }
    }
    let templates = pw_core::template_ir::build_with(&refs, &handlers);

    let mut blocked = 0usize;
    for t in &templates {
        for b in t.blocked() {
            if let pw_core::template_ir::Part::Blocked { reason, at } = b {
                eprintln!("{}: blocked at `{at}`: {reason}", t.path);
                blocked += 1;
            }
        }
    }

    if plain {
        for t in &templates {
            println!("{}  ({})  schema {}", t.path, t.params.join(", "), t.schema);
            let manifest = t.manifest();
            if !manifest.is_empty() {
                println!("  parts manifest — dynamic regions only:");
                for e in &manifest {
                    let owner = e.owner.map(|o| format!(" element {o}")).unwrap_or_default();
                    println!(
                        "    {:>3}  {:<18} {:?}{owner}  {}",
                        e.id.0, e.kind, e.anchor, e.value
                    );
                }
            }
            for c in &t.chunks {
                match c {
                    pw_core::template_ir::Chunk::Static(s) => {
                        println!("    static  {:?}", s);
                    }
                    pw_core::template_ir::Chunk::Dynamic(p) => {
                        println!("    part    {p:?}");
                    }
                }
            }
            println!();
        }
    } else {
        match serde_json::to_string_pretty(&templates) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("pw: cannot serialize the template IR: {e}");
                return ExitCode::from(2);
            }
        }
    }

    // A blocked part is a failing exit code. The renderer refuses it too; a
    // build that emitted it as a warning would let a page ship with a region
    // silently missing.
    if blocked > 0 {
        eprintln!("pw emit-template: {blocked} blocked part(s)");
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
/// E8-0 — the compiler→host `ComponentContract`, as JSON.
///
/// A DATA artifact (ADR-0018). The compiler emits it; the host deserializes it
/// and mirrors the types by field name. Neither links the other, which is what
/// keeps "the compiler decides what authority code needs, the host decides
/// whether it exists" a boundary rather than a slogan.
/// **The WIT package for a checked program: one world per contract.**
///
/// Refuses rather than emits when a signature names a type it cannot map — a
/// world containing a guessed type parses, links, and decodes a value into
/// something it never was.
fn emit_wit_command(paths: &[&String]) -> ExitCode {
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
    let sigs = pw_core::signatures::Signatures::build(&ws, &refs);
    let contracts = pw_core::contract::contracts(&refs, &sigs, &ws);
    match pw_core::wit::package(&refs, &ws, &contracts) {
        Ok((text, _)) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("pw: cannot generate WIT: {e}");
            ExitCode::FAILURE
        }
    }
}

fn emit_contracts_command(paths: &[&String], plain: bool) -> ExitCode {
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
    let sigs = pw_core::signatures::Signatures::build(&ws, &refs);
    let contracts = pw_core::contract::contracts(&refs, &sigs, &ws);

    if plain {
        for c in &contracts {
            println!("{}  abi {}", c.component_id, c.abi_schema);
            println!("  runs at   {}", c.allowed_placements.join(", "));
            for cap in &c.required_capabilities {
                println!("  needs     {}", cap.name());
            }
            for i in &c.imports {
                match i.kind {
                    pw_core::contract::ImportKind::HostCapability => {
                        println!("  host      {}  ({})", i.key(), i.capability)
                    }
                    pw_core::contract::ImportKind::Component => {
                        println!("  component {}", i.key())
                    }
                }
            }
            for e in &c.exports {
                println!("  exports   {} {}", e.kind, e.name);
            }
            println!();
        }
    } else {
        match serde_json::to_string_pretty(&contracts) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("pw: cannot serialize contracts: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

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
        "check"
            | "explain"
            | "fmt"
            | "emit-koka"
            | "emit-marko"
            | "emit-manifest"
            | "emit-graph"
            | "emit-contracts"
            | "emit-template"
            | "emit-wit"
    ) || paths.is_empty()
    {
        eprintln!(
            "usage: pw <check|explain|fmt|emit-koka|emit-marko|emit-manifest|emit-graph|\
             emit-contracts|emit-template|emit-wit> <path.pw>... [--plain]"
        );
        eprintln!();
        eprintln!("  check          parse and report diagnostics");
        eprintln!("  explain        print types, effects, privacy and derived placement");
        eprintln!("  fmt            rewrite files canonically; --check reports instead");
        eprintln!("  emit-koka      print Koka for the pure subset (ADR-0015)");
        eprintln!("  emit-marko     write Marko templates to --out DIR (ADR-0017)");
        eprintln!("  emit-manifest  print the resource manifests as JSON");
        eprintln!("  emit-graph     print the resource dependency graph (--plain for text)");
        eprintln!("  emit-template  print the checked template IR (--plain for text)");
        eprintln!("  emit-wit       print a WIT world per component contract");
        return ExitCode::from(2);
    }

    if cmd == "emit-manifest" {
        return emit_manifest_command(&paths);
    }

    if cmd == "emit-graph" {
        return emit_graph_command(&paths, args.iter().any(|a| a == "--plain"));
    }

    if cmd == "emit-contracts" {
        return emit_contracts_command(&paths, args.iter().any(|a| a == "--plain"));
    }

    if cmd == "emit-template" {
        return emit_template_command(&paths, args.iter().any(|a| a == "--plain"));
    }

    if cmd == "emit-wit" {
        return emit_wit_command(&paths);
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
        parsed: pw_syntax::Parse,
        /// `None` when the file did not parse cleanly. Recovery invents
        /// plausible declarations, and lowering those would put fictional types
        /// in the environment for every other file (E0 finding F-4).
        hir: Option<pw_core::hir::Hir>,
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
        let parsed = pw_syntax::parse_tree(&src);
        let hir = parsed
            .errors
            .is_empty()
            .then(|| pw_core::lower::lower_file(&src, &parsed.green));
        inputs.push(Input {
            display,
            src,
            parsed,
            hir,
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

    // One graph for every file given, so `explain`'s key audit is a
    // whole-program answer. Built once rather than per file: a fragment's key
    // is only auditable against the resources it reads, and those are usually
    // in other files.
    let program_hirs: Vec<&pw_core::hir::Hir> =
        inputs.iter().filter_map(|i| i.hir.as_ref()).collect();
    let program_ws = pw_core::resolve::Workspace::build(&program_hirs);
    let program_graph = pw_core::graph::Graph::build(&program_hirs, &program_ws);

    for input in &inputs {
        let Input {
            display,
            src,
            parsed,
            hir,
        } = input;

        if !parsed.errors.is_empty() {
            print!("{}", render(src, display, &parsed.errors, !plain));
            errors += parsed.errors.len();
        } else {
            // Semantic rules only run on a clean parse, for the same reason.
            // `hir` is `Some` exactly when `parsed.errors` is empty, three
            // lines up. The `else` branch is unreachable and says so rather
            // than silently reporting a clean file.
            let mut found = hir.as_ref().map(rules::check).unwrap_or_default();
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
            if let Some(hir) = hir {
                print!("{}", explain_with(hir, src, Some(&program_graph)));
            }
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

    /// Parse and lower, the one way the compiler does it.
    fn hir_of(src: &str) -> pw_core::hir::Hir {
        let p = pw_syntax::parse_tree(src);
        assert!(p.ok(), "parse errors: {:?}", p.errors);
        pw_core::lower::lower_file(src, &p.green)
    }

    #[test]
    fn explain_never_leaks_generated_paths() {
        // Charter §14 M2 gate: the compiler prints an effect summary "without
        // exposing generated-file paths to the user".
        let src = "module store.pricing\n\npublic query Store(id: StoreId) -> Store\n    cache shared\n{\n    Stores.get(id)\n}\n\nfn subtotal(c: Cart) -> Money !{} { 0 }\n";
        let text = explain_with(&hir_of(src), src, None);
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
            let hir = hir_of(&src);
            let text = explain_with(&hir, &src, None);

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
        let text = explain_with(&hir_of(src), src, None);
        assert!(text.contains("Session<SessionId>"), "{text}");
        assert!(text.contains("rejected at E5"), "{text}");
    }

    #[test]
    fn explain_derives_placement_from_effects_alone() {
        let src = "module m\nfn f() -> () !{ secret<Payments> } { 0 }\nfn g() -> () !{ layout.measure } { 0 }\n";
        let text = explain_with(&hir_of(src), src, None);
        assert!(text.contains("Origin (secret"), "{text}");
        assert!(text.contains("Browser (browser-only"), "{text}");
    }

    #[test]
    fn check_renders_a_diagnostic_with_a_real_span() {
        let src = "module m\n\n$$$\n";
        let parsed = pw_syntax::parse_tree(src);
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
        let text = explain_with(&hir_of(src), src, None);
        assert!(text.contains("unannotated — claims nothing"), "{text}");
        assert!(text.contains("explicit purity claim"), "{text}");
    }
}
