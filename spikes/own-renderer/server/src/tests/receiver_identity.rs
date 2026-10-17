//! A checked member receiver keeps its identity through actual compiled code.
use super::*;

#[test]
fn a_local_receiver_getter_runs_instead_of_a_namesake_type_constructor() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut sources = Vec::new();
    for directory in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<_> = std::fs::read_dir(root.join(directory))
            .expect("packages")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "pw"))
            .collect();
        paths.sort();
        for path in paths {
            sources.push((
                path.display().to_string(),
                std::fs::read_to_string(path).expect("source"),
            ));
        }
    }
    sources.push((
        "receiver.pw".to_string(),
        r#"
module receiver

type Shape = Circle | Empty
type Item = Item { name: String }

fn Empty(item: Item) -> Shape !{} { Shape.Circle }
fn shadow(Shape: Item) -> Shape !{} { Shape.Empty }

query LocalAnswer() -> Bool {
    match shadow(Item { name: "x" }) {
        Circle => true,
        Empty => false,
    }
}

query GenuineAnswer() -> Bool {
    match Shape.Empty {
        Circle => false,
        Empty => true,
    }
}

page Local {
    route "/local"
    cache private
    let answer = query LocalAnswer()
    view {
        <title>Local receiver</title>
        <main><output>Local={answer}</output></main>
    }
}

page Genuine {
    route "/genuine"
    cache private
    let answer = query GenuineAnswer()
    view {
        <title>Genuine constructor</title>
        <main><output>Genuine={answer}</output></main>
    }
}
"#
        .to_string(),
    ));
    let units: Vec<_> = sources
        .into_iter()
        .map(|(path, src)| pw_core::check::Unit {
            hir: pw_core::lower::lower_file(&src, &pw_syntax::parse_tree(&src).green),
            path,
            src,
        })
        .collect();
    let build = pw_core::build::build(&units).expect("checked program builds");
    assert!(build.refusals().is_empty(), "{:?}", build.refusals());
    let directory = tempfile::TempDir::with_prefix("pw-receiver-identity-").expect("private build");
    let out = directory.path().join("build");
    build.write(&out).expect("write actual components");
    let server = Server::from_build(out.clone(), out).expect("serve actual components");
    let genuine = get_as(&server, "reader", "/genuine");
    assert!(genuine.starts_with("HTTP/1.1 200"), "{genuine}");
    assert!(visible(&genuine).contains("Genuine=true"), "{genuine}");
    let local = get_as(&server, "reader", "/local");
    assert!(local.starts_with("HTTP/1.1 200"), "{local}");
    assert!(visible(&local).contains("Local=true"), "{local}");
}
