//! **A template's condition is a `Bool`, or a `List` or a `String` tested
//! non-empty** (ADR-0230, ruling 0071-a).
//!
//! Until 0071-a a number and a record were the renderer's truth, and checked
//! clean: a number tested as one is JSX's `{count && …}`, which shows a `0`
//! where nothing was meant, and a record is always true. Each is PW0609 now,
//! with its repair, which ruling 0073-a's computed condition lets build
//! (ADR-0229).
//!
//! Each test states one part, with controls.

use pw_core::check::{Unit, check_sources};
use pw_core::lower::lower_file;
use pw_syntax::parse_tree;

fn program(src: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for d in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
            .unwrap_or_else(|e| panic!("{d}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            out.push((
                p.display().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push(("t.pw".to_string(), src.to_string()));
    out
}

/// `code message | repair` for each diagnostic on `t.pw`.
fn reported(src: &str) -> Vec<String> {
    check_sources(&program(src))
        .into_iter()
        .filter(|(n, _)| n == "t.pw")
        .flat_map(|(_, ds)| {
            ds.into_iter().map(|d| {
                format!(
                    "{} {} | {}",
                    d.code,
                    d.message,
                    d.repairs
                        .first()
                        .map(|r| r.description.clone())
                        .unwrap_or_default()
                )
            })
        })
        .collect()
}

/// A view of a post, its markup `markup`.
fn view(markup: &str) -> String {
    format!(
        "module t\n\nopaque type Title = String\n\n\
         type Post = Post {{ likes: Int, score: Float, tags: List<String>, title: String, pinned: Bool, name: Title }}\n\n\
         view V(post: Post) !{{}} {{\n    <article>\n        {markup}\n    </article>\n}}\n"
    )
}

#[test]
fn a_number_and_a_record_have_no_truth() {
    for (markup, said) in [
        (
            "{#if post.likes}<p>x</p>{/if}",
            "PW0609 `{#if}` tests an `Int`, a number, which has no truth | say what it tests: \
             `n > 0`",
        ),
        (
            "{#if post.pinned}<p>x</p>{:else if post.score}<p>y</p>{/if}",
            "PW0609 `{:else if}` tests a `Float`, a number, which has no truth | say what it \
             tests: `n > 0`",
        ),
        (
            "<button type=\"button\" disabled={post}>x</button>",
            "PW0609 `disabled` tests a `t.Post`, which has no truth | test a `Bool` it holds, \
             or compare it",
        ),
        // An opaque type is its own, whatever it is over.
        (
            "{#if post.name}<p>x</p>{/if}",
            "PW0609 `{#if}` tests a `t.Title`, which has no truth | test a `Bool` it holds, or \
             compare it",
        ),
    ] {
        assert_eq!(reported(&view(markup)), [said], "{markup}");
    }
}

#[test]
fn a_bool_and_a_list_or_string_tested_non_empty_have() {
    for markup in [
        "{#if post.pinned}<p>x</p>{/if}",
        "{#if post.tags}<p>x</p>{:else if post.title}<p>y</p>{/if}",
        "<button type=\"button\" disabled={post.pinned}>x</button>",
        "<p hidden={post.tags}>x</p>",
        // The repair, which a computed condition builds (ADR-0229).
        "{#if post.likes > 0}<p>x</p>{:else if post.score < 0.5}<p>y</p>{/if}",
    ] {
        assert_eq!(reported(&view(markup)), Vec::<String>::new(), "{markup}");
    }
}

#[test]
fn the_repair_builds() {
    // A page whose condition is its count compared, which ADR-0229 builds.
    let src = "module t\n\ntype Post = Post { likes: Int }\n\n\
               public query Latest() -> Post !{} {\n    Post { likes: 0 }\n}\n\n\
               page P() {\n    route \"/p\"\n    cache private\n\n    let post = query Latest()\n\n    \
               view {\n        <title>P</title>\n        <main>{#if post.likes > 0}<p>Liked</p>{/if}</main>\n    }\n}\n";
    assert_eq!(reported(src), Vec::<String>::new());
    let units: Vec<Unit> = program(src)
        .into_iter()
        .map(|(path, src)| Unit {
            path,
            hir: lower_file(&src, &parse_tree(&src).green),
            src,
        })
        .collect();
    let b = pw_core::build::build(&units).expect("builds");
    assert!(b.refusals().is_empty(), "{:?}", b.refusals());
}
