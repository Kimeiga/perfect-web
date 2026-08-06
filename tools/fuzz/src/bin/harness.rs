//! Runs one fuzz input against one target, under coverage instrumentation.
//!
//! A separate binary from the driver so the driver's own execution does not
//! pollute the coverage signal, and so a crash kills only this process.
//!
//! Exit codes: 0 survived, 101 panicked. Anything else is an abort the runtime
//! could not catch, which the driver reports differently because it means the
//! failure escaped even `catch_unwind`.

use std::panic::{AssertUnwindSafe, catch_unwind};

fn main() {
    let mut args = std::env::args().skip(1);
    let target = args.next().unwrap_or_default();
    let path = args.next().unwrap_or_default();
    let bytes = std::fs::read(&path).unwrap_or_default();
    // Every target takes bytes. Lossy, because a fuzzer's job is to hand the
    // compiler things a person would not type.
    let text = String::from_utf8_lossy(&bytes).to_string();

    let outcome = catch_unwind(AssertUnwindSafe(|| match target.as_str() {
        "parser-to-hir" => {
            let parsed = pw_syntax::parse_tree(&text);
            let _ = pw_core::lower::lower_file(&text, &parsed.green);
        }
        "resolution-to-typing" | "labels-and-effects" | "exhaustiveness" => {
            let _ = pw_core::rules::check(&pw_syntax::parse(&text).file);
            let _ = pw_core::check::check_sources(&[("fuzz.pw".to_string(), text.clone())]);
        }
        "manifest-decoding" | "resume-compatibility" => resume(&text),
        _ => {}
    }));

    if outcome.is_err() {
        eprintln!("panic in target {target}");
        std::process::exit(101);
    }
}

/// Decode a manifest from bytes and run it through the compatibility decision.
///
/// The wire format is deliberately simple and deliberately UNVALIDATED before
/// `decide` sees it: that is the surface being fuzzed.
fn resume(text: &str) {
    use pw_resume::*;

    let f: Vec<&str> = text.split('|').collect();
    let field = |i: usize| f.get(i).copied().unwrap_or_default();

    let scheme = HashScheme(field(0).parse().unwrap_or(0));
    let abi = PlatformAbi(field(1).parse().unwrap_or(0));
    let schema = |s: &str| SchemaHash::of_fields(&[(s, "T")]);
    let scope = match field(6) {
        "public" => PrivacyScope::Public,
        s if s.starts_with("session:") => PrivacyScope::Session(s[8..].to_string()),
        s if s.starts_with("user:") => PrivacyScope::User(s[5..].to_string()),
        s => PrivacyScope::Organization(s.to_string()),
    };
    let handler = HandlerId::derive(
        &ImplementationHash::new(scheme, field(3)),
        &DependencySet::of(&[["a", "b", "c", field(3), "r"]]),
        &schema(field(4)),
        &abi,
    );
    let entry = ResumeEntry {
        hash_scheme: scheme,
        platform_abi: abi,
        application_build: BuildId(field(2).to_string()),
        handler: handler.clone(),
        capture_schema: schema(field(4)),
        document_schema: schema(field(5)),
        privacy_scope: scope,
        captures: field(7).as_bytes().to_vec(),
    };
    let rt = Runtime {
        abi: vec![PlatformAbi(1)],
        build: Some(BuildId("B1".into())),
        handlers: [(handler, schema("s1"))].into_iter().collect(),
        document_schema: Some(schema("d1")),
        scope: Some(PrivacyScope::Public),
        migrations: Vec::new(),
    };

    for c in [
        Construct::PublicRegion,
        Construct::PrivateSlot,
        Construct::PendingCommand,
        Construct::OpenResource,
    ] {
        let d = decide(&entry, &rt, c);
        // The properties, checked here so the fuzzer's finding is a property
        // violation and not only a crash.
        if let Some(proof) = authorise(&entry, &d) {
            assert!(d.attaches(), "authorised a decision that did not attach");
            assert!(
                entry.privacy_scope.can_flow_to(rt.scope.as_ref().unwrap()),
                "authorised across a privacy boundary"
            );
            assert_eq!(
                entry.hash_scheme, CURRENT_SCHEME,
                "authorised under an unknown hash scheme"
            );
            let _ = attach(proof);
        }
        if let Decision::Refuse { recovery, .. } = &d {
            assert!(
                !(c == Construct::OpenResource && *recovery != Recovery::RejectIrrecoverable),
                "an open resource was offered a recovery"
            );
        }
    }
    let _ = patch_applies(&BuildId(field(2).to_string()), &BuildId("B1".into()));
}
