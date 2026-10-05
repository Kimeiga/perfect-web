//! **Every expression in a file that parses is one the compiler reads**
//! (ADR-0200).
//!
//! The lowering gives an expression it has no meaning for an error node, and
//! the checker types an error node as anything, since the parser has said
//! what is wrong. Where the parser accepted the text, nothing had: `()`, the
//! unit value, was such a node until ADR-0200, and `fn f() -> Int !{} { () }`
//! checked. This holds every `.pw` file in the repository to it.

use std::path::{Path, PathBuf};

fn pw_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.map(|e| e.expect("entry").path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if p.is_dir() {
            if !matches!(name, "node_modules" | "target" | "dist" | ".git") {
                pw_files(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "pw") {
            out.push(p);
        }
    }
}

#[test]
fn no_file_that_parses_holds_an_expression_the_compiler_cannot_read() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["examples", "packages", "spikes", "compiler"] {
        pw_files(&root.join(dir), &mut files);
    }
    assert!(files.len() > 400, "{} files", files.len());
    let mut parsed_clean = 0;
    let mut unread = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path).expect("read");
        let parsed = pw_syntax::parse_tree(&src);
        if !parsed.ok() {
            continue;
        }
        parsed_clean += 1;
        let hir = pw_core::lower::lower_file(&src, &parsed.green);
        for (_, decl) in hir.all_decls() {
            let Some(b) = decl.body else { continue };
            let body = hir.body(b);
            for i in 0..body.exprs.len() {
                if matches!(body.exprs.get(i), Some(pw_core::hir::Expr::Error)) {
                    let span = body.expr_span(pw_core::hir::ExprId(i as u32));
                    unread.push(format!(
                        "{}: expression `{}`",
                        path.strip_prefix(&root).unwrap_or(path).display(),
                        &src[span]
                    ));
                }
            }
            for i in 0..body.pats.len() {
                if matches!(body.pats.get(i), Some(pw_core::hir::Pattern::Error)) {
                    let span = body.pat_span(pw_core::hir::PatternId(i as u32));
                    unread.push(format!(
                        "{}: pattern `{}`",
                        path.strip_prefix(&root).unwrap_or(path).display(),
                        &src[span]
                    ));
                }
            }
        }
    }
    assert!(parsed_clean > 400, "{parsed_clean} files parse");
    assert!(
        unread.is_empty(),
        "{} unread:\n{}",
        unread.len(),
        unread.join("\n")
    );
}
