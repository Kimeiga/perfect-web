//! The compatibility decision, compiled for the browser.
//!
//! One implementation, not two. `runtime/pw-resume` decides; this exposes that
//! decision to a page as `wasm32-unknown-unknown` with a hand-rolled ABI — no
//! `wasm-bindgen`, because the project pins its dependency set and the
//! interface is four integers and two byte buffers.
//!
//! # Why not a JavaScript port
//!
//! A second implementation of a security decision is two things that can
//! disagree, and the disagreement is silent: both sides return a boolean and
//! only one of them is right. The whole reason `decide` returns a typed
//! `Decision` is so the answer carries its reason, and a port would carry a
//! different one.
//!
//! # The ABI
//!
//! ```text
//! alloc(len)            -> ptr        the caller writes the manifest here
//! decide(ptr, len)      -> code       0 resume, 1 migrate, 2.. a refusal
//! last_recovery()       -> code       which recovery the refusal chose
//! last_trace_ptr/len()               the causal trace, UTF-8
//! ```
//!
//! The manifest is the same `|`-separated encoding the fuzzer's harness uses,
//! so a browser input and a fuzz input are the same bytes.

use std::sync::Mutex;

use pw_resume::*;

static TRACE: Mutex<String> = Mutex::new(String::new());
static RECOVERY: Mutex<u32> = Mutex::new(0);

/// Bytes the caller may write a manifest into. Leaked deliberately: a page
/// makes a handful of decisions, and a free() across the ABI is more ways to
/// be wrong than it is worth.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = vec![0u8; len];
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// 0 resume · 1 migrate · 2 unsupported scheme · 3 unsupported ABI ·
/// 4 unknown handler · 5 privacy widened · 6 document schema · 7 no migration ·
/// 8 migration failed · 9 malformed · 10 mixed build
/// # Safety
///
/// `ptr` must point at `len` bytes the caller obtained from [`alloc`]. The ABI
/// is two integers wide on purpose — there is nothing here for a caller to get
/// subtly wrong except the length, and a wrong length is a wrong manifest,
/// which the decision refuses rather than trusts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn decide_manifest(ptr: *const u8, len: usize) -> u32 {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let text = String::from_utf8_lossy(bytes).to_string();
    let (entry, rt, construct) = parse(&text);

    let d = decide(&entry, &rt, construct);

    // The authorisation is taken here, inside the same call, so a page cannot
    // attach without it: the ABI has no way to produce an `Authorised` and no
    // way to attach except through a `Resume`/`Migrate` answer.
    let code = match &d {
        Decision::Resume => {
            let _ = authorise(&entry, &d).map(attach);
            0
        }
        Decision::Migrate { .. } => {
            let _ = authorise(&entry, &d).map(attach);
            1
        }
        Decision::Refuse {
            why,
            recovery,
            trace,
        } => {
            *TRACE.lock().unwrap() = trace.join("\n");
            *RECOVERY.lock().unwrap() = match recovery {
                Recovery::RefetchRegion => 0,
                Recovery::RerenderPrivateSlot => 1,
                Recovery::ReloadDocument => 2,
                Recovery::RetryInteraction => 3,
                Recovery::RequireUserConfirmation => 4,
                Recovery::RejectIrrecoverable => 5,
            };
            match why {
                Refusal::UnsupportedHashScheme(_) => 2,
                Refusal::UnsupportedAbi(_) => 3,
                Refusal::UnknownHandler(_) => 4,
                Refusal::PrivacyWidened { .. } => 5,
                Refusal::DocumentSchemaMismatch => 6,
                Refusal::NoMigration { .. } => 7,
                Refusal::MigrationFailed(_) => 8,
                Refusal::MalformedManifest(_) => 9,
                Refusal::MixedBuild { .. } => 10,
                Refusal::CaptureSchemaMismatch { .. } => 7,
            }
        }
    };
    if d.attaches() {
        TRACE.lock().unwrap().clear();
        *RECOVERY.lock().unwrap() = u32::MAX;
    }
    code
}

#[unsafe(no_mangle)]
pub extern "C" fn last_recovery() -> u32 {
    *RECOVERY.lock().unwrap()
}

#[unsafe(no_mangle)]
pub extern "C" fn last_trace_ptr() -> *const u8 {
    TRACE.lock().unwrap().as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn last_trace_len() -> usize {
    TRACE.lock().unwrap().len()
}

/// `scheme|abi|build|handler|capture|document|scope|captures|construct`
fn parse(text: &str) -> (ResumeEntry, Runtime, Construct) {
    let f: Vec<&str> = text.split('|').collect();
    let field = |i: usize| f.get(i).copied().unwrap_or_default();
    // The schema of NOTHING is the schema of no fields, not the schema of one
    // field named "". `decide` refuses a manifest that carries no capture
    // bytes under a non-empty schema — correctly — so a handler that captures
    // nothing was malformed by construction, and the only symptom was a button
    // that never attached.
    let schema = |s: &str| {
        if s.is_empty() {
            SchemaHash::of_fields(&[])
        } else {
            SchemaHash::of_fields(&[(s, "T")])
        }
    };

    let scheme = HashScheme(field(0).parse().unwrap_or(0));
    let abi = PlatformAbi(field(1).parse().unwrap_or(0));
    let scope = match field(6) {
        "public" => PrivacyScope::Public,
        s if s.starts_with("session:") => PrivacyScope::Session(s[8..].to_string()),
        s if s.starts_with("user:") => PrivacyScope::User(s[5..].to_string()),
        s => PrivacyScope::Organization(s.to_string()),
    };
    let handler = HandlerId::derive(
        &ImplementationHash::new(scheme, field(3)),
        &DependencySet::of(&[["app", "store", "Term", field(3), "r1"]]),
        &schema(field(4)),
        &abi,
    );
    // What this build knows: the store page's TWO handlers, at scheme 2.
    //
    // Two, because E7-L's claim is that the exact handler is loaded — and with
    // one known handler, "the right one was authorised" is satisfied by
    // authorising anything. `clear_cart` captures nothing, so its capture
    // schema is the schema of nothing, which is still a schema.
    let known = |name: &str, capture: &str| {
        (
            HandlerId::derive(
                &ImplementationHash::new(CURRENT_SCHEME, name),
                &DependencySet::of(&[["app", "store", "Term", name, "r1"]]),
                &schema(capture),
                &PlatformAbi(1),
            ),
            schema(capture),
        )
    };
    let entry = ResumeEntry {
        hash_scheme: scheme,
        platform_abi: abi,
        application_build: BuildId(field(2).to_string()),
        handler,
        capture_schema: schema(field(4)),
        document_schema: schema(field(5)),
        privacy_scope: scope,
        captures: field(7).as_bytes().to_vec(),
    };
    let rt = Runtime {
        abi: vec![PlatformAbi(1)],
        build: Some(BuildId("B1".into())),
        handlers: [known("add_to_cart", "cart"), known("clear_cart", "")]
            .into_iter()
            .collect(),
        document_schema: Some(schema("cart-doc")),
        scope: Some(PrivacyScope::Public),
        migrations: Vec::new(),
    };
    let construct = match field(8) {
        "private" => Construct::PrivateSlot,
        "command" => Construct::PendingCommand,
        "resource" => Construct::OpenResource,
        _ => Construct::PublicRegion,
    };
    (entry, rt, construct)
}
