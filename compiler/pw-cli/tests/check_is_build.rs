//! Full-program check/build parity, through the executable users run.
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Program(PathBuf);
impl Program {
    fn new(src: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "pw-check-is-build-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.pw"), src).unwrap();
        Self(dir)
    }
    fn run(&self, command: &str) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_pw"));
        cmd.arg(command).arg(self.0.join("app.pw"));
        if command == "check" {
            cmd.arg("--plain");
        }
        if command == "build" {
            cmd.arg("--out").arg(self.0.join("dist"));
        }
        cmd.output().unwrap()
    }
}
impl Drop for Program {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}
fn refused(src: &str, why: &str) {
    let program = Program::new(src);
    let build = program.run("build");
    assert!(
        !build.status.success(),
        "witness must be build-refused: {}",
        text(&build)
    );
    assert!(text(&build).contains(why), "{}", text(&build));
    let check = program.run("check");
    assert!(
        !check.status.success(),
        "check accepted a build-refused program: {}",
        text(&check)
    );
    assert!(text(&check).contains("PW6601"), "{}", text(&check));
    assert!(text(&check).contains(why), "{}", text(&check));
    assert!(text(&build).contains("PW6601"), "{}", text(&build));
    assert!(
        !program.0.join("dist").exists(),
        "a refused program writes no artifacts"
    );
}
#[test]
fn a_page_cannot_depend_on_an_unwritten_query() {
    refused(
        "module shop.ui\nquery Count() -> Int { todo }\npage Home() {\n route \"/\"\n let count = query Count()\n view { <title>Shop</title><p>{count}</p> }\n}\n",
        "whose body is a placeholder",
    );
}
#[test]
fn a_signal_in_a_title_is_refused_during_check() {
    refused(
        "module shop.titled\npage Counter() {\n route \"/counter\"\n cache private\n signal count: Int = 0\n view { <title>Pressed {count}</title><button type=\"button\" on:press={() => count = count + 1}>Press</button> }\n}\n",
        "title part",
    );
}

#[test]
fn a_part_cannot_compute_from_two_signals() {
    refused(
        "module shop.sum\npage Sum() {\n route \"/sum\"\n cache private\n signal a: Int = 1\n signal b: Int = 2\n view { <title>Sum</title><p>{a + b}</p><button type=\"button\" on:press={() => a = a + 1}>Add</button><button type=\"button\" on:press={() => b = b + 1}>Add</button> }\n}\n",
        "computes a value from `a` and `b`",
    );
}
#[test]
fn a_pure_computed_each_head_is_refused() {
    refused(
        "module shop.head\nfn items() -> List<Int> !{} { [1, 2] }\nview Menu() !{} { <ul>{#each items() as item (item)}<li>{item}</li>{/each}</ul> }\n",
        "reads its list by path",
    );
}
#[test]
fn wit_name_collisions_are_check_refusals() {
    refused(
        "module shop.wit\nquery Foo() -> Int { 1 }\nquery foo() -> Int { 2 }\n",
        "both mangle",
    );
}
#[test]
fn a_supported_neighbor_checks_and_builds() {
    let program = Program::new(
        "module shop.ui\nquery Count() -> Int { 3 }\npage Home() {\n route \"/\"\n let count = query Count()\n view { <title>Shop</title><p>{count}</p> }\n}\n",
    );
    for command in ["check", "build"] {
        let out = program.run(command);
        assert!(out.status.success(), "{command}: {}", text(&out));
    }
    assert!(program.0.join("dist/build-id").is_file());
}
