//! **Compile a declaration, run it through the E8 host, report what it did.**
//!
//! The helpers every conformance test uses: the standard packages plus a
//! program, one declaration compiled through the production backend
//! (`pw_core::backend::component::compile`), and a call through the same host
//! API the development server uses (`pw_host::engine::Prepared::call_within`),
//! with the host's operations recording every call the component makes.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use pw_core::backend::component::Compiled;
use pw_core::check::Unit;
use pw_host::engine::{HostFn, Prepared, Val};
use pw_host::{Granted, Limits, Node, Topology, admit};

/// The standard packages, then `program`'s files: a checkable program.
pub fn units(program: &[(&str, &str)]) -> Vec<Unit> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in ["packages/pw-std", "packages/pw-platform-web"] {
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        paths.sort();
        for p in paths {
            let src = std::fs::read_to_string(&p).expect("read");
            out.push(unit(&p.display().to_string(), &src));
        }
    }
    for (path, src) in program {
        out.push(unit(path, src));
    }
    out
}

/// Files of a program on disk, relative to the repository root.
pub fn files(paths: &[&str]) -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    paths
        .iter()
        .map(|p| {
            let src = std::fs::read_to_string(root.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
            (p.to_string(), src)
        })
        .collect()
}

fn unit(path: &str, src: &str) -> Unit {
    Unit {
        path: path.to_string(),
        hir: pw_core::lower::lower_file(src, &pw_syntax::parse_tree(src).green),
        src: src.to_string(),
    }
}

/// One declaration, compiled and audited, or the reason it was not.
pub fn compile(units: &[Unit], component_id: &str) -> Compiled {
    let compiled = pw_core::backend::component::compile(units, component_id)
        .unwrap_or_else(|e| panic!("{component_id} does not compile: {e}"));
    pw_core::backend::component::audit(
        &compiled.component.bytes,
        &compiled.wit,
        &compiled.component.world,
    )
    .unwrap_or_else(|wrong| {
        panic!("{component_id}'s artifact disagrees with its world: {wrong:?}")
    });
    compiled
}

/// Every call the component made to a host operation, in order.
pub type Calls = Arc<Mutex<Vec<(String, Vec<Val>)>>>;

/// A host operation that records its call and answers with `answer(args)`.
pub fn operation(
    calls: &Calls,
    name: &str,
    answer: impl Fn(&[Val]) -> Val + Send + Sync + 'static,
) -> (String, HostFn) {
    let calls = calls.clone();
    let recorded = name.to_string();
    let f: HostFn = Arc::new(move |args: &[Val]| {
        calls
            .lock()
            .expect("calls")
            .push((recorded.clone(), args.to_vec()));
        Ok(vec![answer(args)])
    });
    (name.to_string(), f)
}

/// A compiled component, ready to call many times: compiled by the engine
/// once, admitted on a node that grants exactly what its contract requires.
pub struct Runnable {
    compiled: Compiled,
    /// The contract as the host reads it: the compiler's, serialized and
    /// parsed back, the way a deployment receives it (ADR-0020).
    contract: pw_host::ComponentContract,
    prepared: Prepared,
    granted: Granted,
}

impl Runnable {
    pub fn new(compiled: Compiled) -> Runnable {
        let prepared =
            Prepared::compile(&compiled.component.bytes).expect("the engine compiles it");
        let actual =
            pw_host::engine::imports_of(&compiled.component.bytes).expect("its imports read");
        let grants: BTreeSet<String> = compiled
            .contract
            .required_capabilities
            .iter()
            .map(|c| c.name())
            .collect();
        let node = Topology {
            nodes: vec![Node {
                name: "conformance".into(),
                world: "origin".into(),
                grants,
            }],
        };
        let json = serde_json::to_string(&[&compiled.contract]).expect("the contract serializes");
        let contract = pw_host::ComponentContract::from_json(&json)
            .expect("the host reads the compiler's contract")
            .remove(0);
        let admission = admit(&contract, &node, "conformance", &actual);
        let granted = Granted::from(&admission, &BTreeMap::new())
            .unwrap_or_else(|| panic!("not admitted: {admission:?}"));
        Runnable {
            compiled,
            contract,
            prepared,
            granted,
        }
    }

    /// Call the component's export with `args`, the host supplying `ops`.
    pub fn call(&self, ops: &BTreeMap<String, HostFn>, args: &[Val]) -> Result<Vec<Val>, String> {
        let export = self.contract.exports[0]
            .component
            .clone()
            .expect("the contract locates its export");
        self.prepared.call_within(
            &self.contract,
            &self.granted,
            &Limits {
                fuel: Some(50_000_000),
                memory_bytes: Some(64 * 1024 * 1024),
                table_elements: Some(1_000),
            },
            ops,
            &[&export.interface, &export.function],
            args,
        )
    }

    pub fn compiled(&self) -> &Compiled {
        &self.compiled
    }
}
