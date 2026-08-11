//! **E10-A — the real `add_to_cart` lowers, and everything else says why not.**
//!
//! Architect ruling, 2026-08-09:
//!
//! > Narrow application surface, general backend spine. Compile exactly one
//! > real Pleris command (`add_to_cart`) end-to-end first, but do not build any
//! > stage specifically around `add_to_cart`.
//!
//! So the test that matters is not "the backend handles `add_to_cart`" — it is
//! **the backend handles a set of constructs, and `add_to_cart` stays inside
//! it**. The refusals are as load-bearing as the successes: a construct lowered
//! to nothing produces a component that runs and does the wrong thing, and
//! `nothing_lowers_silently` is what says that cannot happen.

use pw_core::backend::ir::{Instr, Lowering, Type};
use pw_core::backend::lower::{Checked, Context, program};
use pw_core::contract::contracts;
use pw_core::hir::Hir;
use pw_core::lower::lower_file;
use pw_core::resolve::Workspace;
use pw_core::signatures::Signatures;
use pw_syntax::parse_tree;

/// The store demo, as the one program it is.
fn store() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in [
        "packages/pw-std",
        "packages/pw-platform-web",
        "examples/lib",
        "examples/store",
    ] {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        files.sort();
        for p in files {
            out.push((
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).expect("read"),
            ));
        }
    }
    out.push((
        "domain.pw".to_string(),
        std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
    ));
    out
}

struct Built {
    hirs: Vec<Hir>,
    units: Vec<pw_core::check::Unit>,
}

impl Built {
    /// A synthetic program, on top of the platform packages.
    ///
    /// Without them `database.write<Cart>` is `PW5201 unknown effect family`,
    /// and `Checked` refuses — which is the invariant working. These three
    /// controls had been measuring the backend against programs `pw check`
    /// rejects, and nothing said so until the precondition became a parameter.
    fn synthetic(src: &str) -> Built {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = Vec::new();
        for dir in ["packages/pw-std", "packages/pw-platform-web"] {
            let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(dir))
                .unwrap_or_else(|e| panic!("{dir}: {e}"))
                .map(|e| e.expect("entry").path())
                .filter(|p| p.extension().is_some_and(|x| x == "pw"))
                .collect();
            ps.sort();
            for p in ps {
                files.push((
                    p.file_name().unwrap().to_string_lossy().to_string(),
                    std::fs::read_to_string(&p).expect("read"),
                ));
            }
        }
        files.push((
            "domain.pw".to_string(),
            std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain.pw"),
        ));
        files.push(("t.pw".to_string(), src.to_string()));
        Built::new(&files)
    }

    fn new(files: &[(String, String)]) -> Built {
        let units: Vec<pw_core::check::Unit> = files
            .iter()
            .map(|(path, src)| pw_core::check::Unit {
                path: path.clone(),
                src: src.clone(),
                hir: lower_file(src, &parse_tree(src).green),
            })
            .collect();
        Built {
            hirs: files
                .iter()
                .map(|(_, s)| lower_file(s, &parse_tree(s).green))
                .collect(),
            units,
        }
    }
}

/// **Everything the backend is asked to lower has checked first.**
///
/// `Checked::of` runs the checker on the same units and refuses to produce a
/// context if it reported an error, so the tests below cannot accidentally
/// measure the backend against a program `pw check` would reject. That
/// precondition is the architect's step 10, and it is a parameter rather than
/// a convention — see `backend::lower::Checked`.
fn lowered(
    built: &Built,
) -> (
    pw_core::backend::ir::Program,
    Vec<Lowering<pw_core::backend::ir::Function>>,
) {
    let refs: Vec<&Hir> = built.hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let cx = Context {
        hirs: &refs,
        ws: &ws,
        sigs: &sigs,
        contracts: &cs,
    };
    let checked = Checked::of(&built.units, cx).unwrap_or_else(|ds| {
        panic!(
            "the program does not check, so the backend must not see it: {:?}",
            ds.iter().map(|d| (d.code, &d.message)).collect::<Vec<_>>()
        )
    });
    program(&checked)
}

/// **The real `add_to_cart` lowers**, which is E10-A's subject.
///
/// It did not, for one commit, and the reason was a defect in the demo the
/// backend was the first thing to find: `examples/store/app.pw` called
/// `current_session()` and never imported it. Assumption A-009 says Term names
/// are not ambient, so the call resolved to nothing — and `pw check` reported
/// nothing, because `unresolved_uses` only examines dotted paths whose head
/// looks like a module.
///
/// Every analysis upstream can produce an answer for a call it cannot resolve:
/// inference contributes no effects, privacy no label, placement no constraint.
/// Each of those looks exactly like *this call is harmless*. A backend cannot
/// emit a call to nothing, so it was the first consumer for which the absence
/// was fatal rather than quiet.
///
/// The repair was one import, and it moved a documented E8 claim with it —
/// `StorePage` requires `session.read` to render. See
/// `tests/unresolved_provenance.rs::store_page_after_repair`.
///
/// **Two host calls, not one.** The first version of this test expected one and
/// was wrong: the command reads the session and writes the cart, and the
/// backend was right.
#[test]
fn the_real_add_to_cart_lowers_to_two_host_calls() {
    let built = Built::new(&store());
    let (p, refusals) = lowered(&built);

    let f = p
        .functions
        .iter()
        .find(|f| f.export == "add_to_cart")
        .unwrap_or_else(|| {
            panic!(
                "`add_to_cart` did not lower: {:?}",
                refusals.iter().map(|r| r.to_string()).collect::<Vec<_>>()
            )
        });

    let mut host: Vec<String> = f.blocks[0]
        .instrs
        .iter()
        .filter_map(|i| match i {
            Instr::HostCall { capability, .. } => Some(capability.name()),
            _ => None,
        })
        .collect();
    host.sort();
    assert_eq!(host, ["database.write<Carts>", "session.read"]);

    // Named by the CONTRACT's capabilities, not by matching a spelling.
    let mut declared: Vec<String> = f.capabilities.iter().map(|c| c.name()).collect();
    declared.sort();
    assert_eq!(declared, host, "the function carries the contract's set");
}

/// **The lowering itself works**, on a program that does import what it calls.
///
/// The same shape as `add_to_cart` — a command whose body calls a function
/// declaring a capability the contract requires — with nothing missing. This is
/// what says the test above is about the DEMO and not about the backend.
#[test]
fn a_command_that_imports_what_it_calls_lowers_to_a_host_call() {
    let src = "\
module m

opaque type Id = String
type Cart = Cart { n: Int }
type CartError = CartError { why: String }

fn write(id: Id) -> Result<Cart, CartError> !{ database.write<Cart> } { todo }

command Add(id: Id) -> Result<Cart, CartError>
    requires SignedIn
{
    write(id)
}
";
    let built = Built::synthetic(src);
    let (p, refusals) = lowered(&built);
    let f = p
        .functions
        .iter()
        .find(|f| f.export == "Add")
        .unwrap_or_else(|| {
            panic!(
                "{:?}",
                refusals.iter().map(|r| r.to_string()).collect::<Vec<_>>()
            )
        });

    // The parameter and result kept their types, resolved to DefIds.
    assert!(matches!(f.params[0].1, Type::Nominal(_)));
    let Type::Result(ok, err) = &f.ret else {
        panic!("{:?}", f.ret)
    };
    assert!(matches!(**ok, Type::Nominal(_)));
    assert!(matches!(**err, Type::Nominal(_)));

    // **The host call**, named by the capability the CONTRACT derived — not by
    // matching `write` against a list of privileged spellings.
    let host: Vec<String> = f.blocks[0]
        .instrs
        .iter()
        .filter_map(|i| match i {
            Instr::HostCall { capability, .. } => Some(capability.name()),
            _ => None,
        })
        .collect();
    assert_eq!(host, ["database.write<Cart>"]);
    assert!(
        f.capabilities
            .iter()
            .any(|c| c.name() == "database.write<Cart>"),
        "and the function carries the contract's set, not a second derivation"
    );
}

#[test]
fn a_call_that_needs_no_authority_is_an_ordinary_call() {
    // The discriminating half. Without it, `the_real_add_to_cart_lowers_to_a_
    // host_call` would pass for a backend that made EVERY call a host call —
    // which would ask the host to grant things nothing requires, and would make
    // the artifact audit refuse a component for imports it never needed.
    // Synthetic, and it has to be: EVERY call the store's commands and queries
    // make needs authority. That is a fact about the demo — a query exists to
    // reach data — and asserting on it would have measured the fixture rather
    // than the rule.
    let src = "\
module m

opaque type Id = String
type Cart = Cart { n: Int }
type CartError = CartError { why: String }

fn double(n: Int) -> Int { n }

command Pure(id: Id) -> Int
    requires SignedIn
{
    double(1)
}
";
    let built = Built::synthetic(src);
    let (p, refusals) = lowered(&built);
    let f = p
        .functions
        .iter()
        .find(|f| f.export == "Pure")
        .unwrap_or_else(|| panic!("{refusals:?}"));

    assert!(
        f.blocks[0]
            .instrs
            .iter()
            .any(|i| matches!(i, Instr::Call { .. })),
        "a call to a function with no effect row is an ordinary call: {:?}",
        f.blocks[0].instrs
    );
    assert!(
        !f.blocks[0]
            .instrs
            .iter()
            .any(|i| matches!(i, Instr::HostCall { .. })),
        "and nothing here asks the host for anything: {:?}",
        f.blocks[0].instrs
    );
}

#[test]
fn nothing_lowers_silently() {
    // **The rule the three-valued outcome exists for.** Every declaration the
    // backend was asked about either produced a function or produced a REASON.
    // A construct lowered to no instructions is a component that runs and does
    // the wrong thing, and it would look exactly like success from here.
    let built = Built::new(&store());
    let (p, refusals) = lowered(&built);

    for r in &refusals {
        assert!(!r.is_lowered());
        let text = r.to_string();
        assert!(
            text.contains("does not lower") || text.contains("blocked upstream"),
            "a refusal must say which construct or which upstream failure: {text}"
        );
    }

    // And a function that lowered has instructions and a terminator. An empty
    // block would satisfy "lowered" while meaning nothing happened.
    for f in &p.functions {
        assert!(!f.blocks.is_empty(), "{} has no blocks", f.export);
        assert!(
            !f.blocks[0].instrs.is_empty(),
            "{} lowered to no instructions",
            f.export
        );
    }
}

#[test]
fn a_construct_outside_the_supported_set_is_refused_by_name() {
    // E10-A supports what `add_to_cart` needs and nothing more, and the refusal
    // says which construct — so widening the backend is a visible act rather
    // than a fixture that quietly started passing.
    let src = "\
module m

opaque type Id = String
type Cart = Cart { n: Int }
type CartError = CartError { why: String }

command Weird(id: Id) -> Result<Cart, CartError>
    requires SignedIn
{
    id |> todo
}
";
    let built = Built::synthetic(src);
    let (p, refusals) = lowered(&built);
    assert!(
        p.functions.is_empty(),
        "a pipeline is not in E10-A's supported set"
    );
    assert_eq!(refusals.len(), 1);
    let text = refusals[0].to_string();
    assert!(
        text.contains("does not lower") || text.contains("blocked"),
        "{text}"
    );
}

#[test]
fn the_backend_never_decides_from_a_name() {
    // The structural property, as a scan of the module rather than as a
    // behaviour. `docs/RISK_QUEUE.md` is mostly instances of spelling-based
    // resolution, and a backend is the worst place for the next one: its
    // answer becomes machine code, where nothing downstream can notice.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/backend");
    let mut offenders = Vec::new();
    let mut scanned = 0usize;
    for e in std::fs::read_dir(&dir).expect("src/backend") {
        let p = e.expect("entry").path();
        if p.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        scanned += 1;
        let file = p.file_name().unwrap().to_string_lossy().to_string();
        for (n, line) in std::fs::read_to_string(&p)
            .expect("read")
            .lines()
            .enumerate()
        {
            let l = line.trim();
            if l.starts_with("//") {
                continue;
            }
            // A capability or effect spelling compared as a literal. The
            // carriers `Result`/`Option`/`List` are the language's own type
            // constructors and are matched by name deliberately — they are
            // syntax, not declarations, and there is no `DefId` for `Result`.
            if (l.contains("== \"database") || l.contains("== \"secret") || l.contains("== \"dom"))
                || (l.contains("starts_with(\"database") || l.contains("contains(\"database"))
            {
                offenders.push(format!("{file}:{}: {l}", n + 1));
            }
        }
    }
    assert!(scanned >= 3, "the scan read {scanned} files");
    assert!(
        offenders.is_empty(),
        "the backend decides something from a capability spelling:\n  {}\n\n\
         Which calls need authority is the CONTRACT's answer, already derived \
         from inference. Matching a name here would be the fourth place in this \
         project to resolve by spelling, and the first three each cost a \
         milestone.",
        offenders.join("\n  ")
    );
}

/// **The backend cannot be handed a program that did not check.**
///
/// Architect ruling, 2026-08-10, step 10: *enforce resolved/checked program
/// input before E10.*
///
/// `Checked::of` is the only way to obtain what `program` takes, and it runs
/// the checker. This asserts the refusal directly, because the guarantee is
/// otherwise invisible: every other test in this file passes through it and
/// none of them would notice if it stopped refusing.
///
/// It found three on its first run. `a_command_that_imports_what_it_calls…`,
/// `a_call_that_needs_no_authority…` and
/// `a_construct_outside_the_supported_set…` were each built without the
/// platform packages, so `database.write<Cart>` was `PW5201 unknown effect
/// family` — three controls measuring the backend against programs `pw check`
/// rejects. They now build on the real platform.
#[test]
fn a_program_that_does_not_check_never_reaches_the_backend() {
    // A bare call to a name nothing declares — the shape of all four defects
    // this milestone repaired, and `PW0021` since 2026-08-10.
    let src = "\
module m

command Add(id: Int) -> Int
    requires SignedIn
{
    vanished(id)
}
";
    let built = Built::new(&[("t.pw".to_string(), src.to_string())]);
    let refs: Vec<&Hir> = built.hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let cx = Context {
        hirs: &refs,
        ws: &ws,
        sigs: &sigs,
        contracts: &cs,
    };
    let refused = Checked::of(&built.units, cx).err().unwrap_or_else(|| {
        panic!(
            "the backend accepted a program calling a name that does not \
             exist — which is precisely how `add_to_cart` came to be blocked \
             by a defect no earlier stage reported"
        )
    });
    assert!(
        refused.iter().any(|d| d.code == "PW0021"),
        "and it refused for the right reason: {:?}",
        refused.iter().map(|d| d.code).collect::<Vec<_>>()
    );

    // The discriminating half: the same program with the callee declared is
    // accepted, so the gate reacts to the defect and not to the shape.
    let ok = "\
module m

fn vanished(n: Int) -> Int !{} { n }

command Add(id: Int) -> Int
    requires SignedIn
{
    vanished(id)
}
";
    let built = Built::new(&[("t.pw".to_string(), ok.to_string())]);
    let refs: Vec<&Hir> = built.hirs.iter().collect();
    let ws = Workspace::build(&refs);
    let sigs = Signatures::build(&ws, &refs);
    let cs = contracts(&refs, &sigs, &ws);
    let cx = Context {
        hirs: &refs,
        ws: &ws,
        sigs: &sigs,
        contracts: &cs,
    };
    assert!(
        Checked::of(&built.units, cx).is_ok(),
        "declaring the callee did not satisfy the gate"
    );
}
