//! E7V — resume-version compatibility.
//!
//! Charter §8.5. A resume manifest attaches saved state to executable code.
//! `resume.rs` in the compiler already answers two of the three questions that
//! must hold before it may:
//!
//! ```text
//! 1. serializability   can this value be encoded?          compiler
//! 2. authority         may it cross this boundary?         compiler
//! 3. version           can THIS code safely interpret it?  here
//! ```
//!
//! Passing one does not imply passing the others. A `Cart` is perfectly
//! serializable and may not be in a public manifest; a public string is
//! serializable and unrestricted and may still be handed to a handler that no
//! longer means what it meant when the document was rendered.
//!
//! # The invariant
//!
//! > A resume manifest may attach state to executable code only when the
//! > platform has verified that the handler, captured data, document parts,
//! > privacy scope and runtime ABI are mutually compatible.
//!
//! # Why identity is content, not a path
//!
//! `checkout/add_to_cart` is a name. The name survives a rewrite of the
//! function it points at, so a manifest that trusts it will hand last week's
//! captures to code that reads them differently — and the failure is silent,
//! because both sides agree about the name. Identity is therefore derived from
//! the normalized implementation, its resolved references, its capture schema
//! and the platform ABI.
//!
//! **Code identity and capture-schema identity are separate.** Two handlers can
//! compile to identical behaviour while taking different captures, and one
//! handler can keep its capture schema while changing behaviour. Collapsing
//! them makes one of those two cases undetectable.
//!
//! # This version is strict on purpose
//!
//! No structural-similarity inference. Two records that look alike are not
//! evidence that one can be read as the other. Compatibility comes from an
//! exact identity match or from an **explicit, checked migration** tied to both
//! schema hashes. Everything else fails closed with a recovery action.
//!
//! # What this crate is not
//!
//! Behaviour, not a compile-time guarantee — like `pw-tasks` (ADR-0016) and
//! `pw-resource`. A passing test here does not make a mixed-build deployment a
//! compile error. The compiler's half of E7V is artifact agreement **within one
//! build**, which is a different question and lives in `pw-core`.

use std::collections::BTreeMap;
use std::fmt;

// --- identity ----------------------------------------------------------------

/// A content hash. Opaque on purpose: nothing may compare parts of one, or
/// order two, or infer proximity from a shared prefix.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash(String);

impl Hash {
    pub fn of(content: &str) -> Hash {
        // FNV-1a. Not cryptographic — this detects a version difference, not an
        // adversary — and it is stable across machines and Rust versions, which
        // `DefaultHasher` is not.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in content.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Hash(format!("{h:016x}"))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The runtime ABI the manifest was written against.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlatformAbi(pub u32);

/// The coherent set of generated artifacts a document came from.
///
/// Content addressing identifies each *piece*; this identifies the *set*, which
/// is what a streamed patch must agree with.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BuildId(pub String);

/// A handler's content identity. See the module docs on why this is not a path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HandlerId(pub Hash);

impl HandlerId {
    /// Derive identity from what actually determines behaviour.
    ///
    /// The capture schema is included because a handler that reads its captures
    /// differently is a different handler, even with identical source text.
    pub fn derive(
        normalized_implementation: &str,
        resolved_references: &[&str],
        capture_schema: &SchemaHash,
        abi: &PlatformAbi,
    ) -> HandlerId {
        let mut refs: Vec<&str> = resolved_references.to_vec();
        // Sorted: the order two references happen to appear in is not a
        // behavioural difference, and leaving it in would make an unrelated
        // edit reject every resume.
        refs.sort_unstable();
        HandlerId(Hash::of(&format!(
            "{}\u{1}{}\u{1}{}\u{1}{}",
            normalized_implementation,
            refs.join("\u{2}"),
            capture_schema.0,
            abi.0
        )))
    }
}

/// The shape of what a handler captures.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SchemaHash(pub Hash);

impl SchemaHash {
    /// From the field names and type names, in declaration order.
    pub fn of_fields(fields: &[(&str, &str)]) -> SchemaHash {
        let text: Vec<String> = fields.iter().map(|(n, t)| format!("{n}:{t}")).collect();
        SchemaHash(Hash::of(&text.join(",")))
    }
}

/// How widely a manifest's contents may be read.
///
/// Ordered from widest to narrowest so "equal or stricter" is expressible, but
/// deliberately NOT `Ord` — see [`PrivacyScope::admits`]. Charter §7.8 says
/// labels are a set of restrictions and not a total order, and an ordering here
/// would quietly reintroduce one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyScope {
    /// Served with the public shell; anything cacheable may read it.
    Public,
    /// Scoped to one session.
    Session(String),
    /// Scoped to one user.
    User(String),
    /// Scoped to one organization.
    Organization(String),
}

impl PrivacyScope {
    /// May a manifest written for `self` be resumed into `into`?
    ///
    /// Only when `into` is the same scope or a narrower one. Widening is the
    /// case this exists to reject: a session-scoped manifest resumed into a
    /// public scope is the resume-manifest privacy defect happening at
    /// deployment time instead of at compile time.
    pub fn admits(&self, into: &PrivacyScope) -> bool {
        match (self, into) {
            (a, b) if a == b => true,
            // Public state may be resumed anywhere; it restricts nothing.
            (PrivacyScope::Public, _) => true,
            // Everything else must match exactly. Two different sessions are
            // not "the same or stricter" — they are different principals, and
            // treating one as admitting the other is the cross-tenant defect.
            _ => false,
        }
    }
}

/// One resumable unit, as it appears in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeEntry {
    pub platform_abi: PlatformAbi,
    pub application_build: BuildId,
    pub handler: HandlerId,
    pub capture_schema: SchemaHash,
    pub document_schema: SchemaHash,
    pub privacy_scope: PrivacyScope,
    /// Opaque. Nothing here decodes it — decoding is what the decision
    /// authorises, and doing it first would be the bug.
    pub captures: Vec<u8>,
}

// --- what the running code offers -------------------------------------------

/// An explicit, checked migration between two capture schemas.
///
/// Tied to both hashes, so it cannot be applied to a schema it was not written
/// for. The function is pure and fallible; it may not reach a capability, and
/// the type is what enforces that — it receives bytes and returns bytes.
pub struct Migration {
    pub from: SchemaHash,
    pub to: SchemaHash,
    pub apply: fn(&[u8]) -> Result<Vec<u8>, MigrationError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationError(pub String);

/// What this running build can accept.
#[derive(Default)]
pub struct Runtime {
    pub abi: Vec<PlatformAbi>,
    pub build: Option<BuildId>,
    /// Handlers this build has code for.
    pub handlers: BTreeMap<HandlerId, SchemaHash>,
    /// The document schema each part currently has.
    pub document_schema: Option<SchemaHash>,
    pub scope: Option<PrivacyScope>,
    pub migrations: Vec<Migration>,
}

// --- the decision ------------------------------------------------------------

/// Why a manifest was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    UnsupportedAbi(PlatformAbi),
    UnknownHandler(HandlerId),
    CaptureSchemaMismatch {
        want: SchemaHash,
        got: SchemaHash,
    },
    DocumentSchemaMismatch,
    PrivacyWidened {
        from: PrivacyScope,
        into: PrivacyScope,
    },
    NoMigration {
        from: SchemaHash,
        to: SchemaHash,
    },
    MigrationFailed(MigrationError),
    MalformedManifest(&'static str),
    MixedBuild {
        document: BuildId,
        patch: BuildId,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::UnsupportedAbi(a) => {
                write!(f, "the manifest's platform ABI {} is not supported", a.0)
            }
            Refusal::UnknownHandler(h) => write!(f, "no code for handler {}", h.0),
            Refusal::CaptureSchemaMismatch { want, got } => {
                write!(
                    f,
                    "the handler reads capture schema {}, the manifest holds {}",
                    want.0, got.0
                )
            }
            Refusal::DocumentSchemaMismatch => {
                f.write_str("the document part has a different shape")
            }
            Refusal::PrivacyWidened { from, into } => {
                write!(f, "a {from:?} manifest may not be resumed into {into:?}")
            }
            Refusal::NoMigration { from, to } => {
                write!(f, "no declared migration from {} to {}", from.0, to.0)
            }
            Refusal::MigrationFailed(e) => write!(f, "the migration failed: {}", e.0),
            Refusal::MalformedManifest(why) => write!(f, "the manifest is malformed: {why}"),
            Refusal::MixedBuild { document, patch } => {
                write!(
                    f,
                    "a patch from build {} cannot apply to a document from build {}",
                    patch.0, document.0
                )
            }
        }
    }
}

/// What to do when a manifest cannot be attached.
///
/// The runtime chooses only among actions valid for the construct in question —
/// a destructive command is never in the same list as a read-only widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    /// Fetch the region again and render it.
    RefetchRegion,
    /// Re-render a private slot from the server.
    RerenderPrivateSlot,
    /// Full navigation.
    ReloadDocument,
    /// Ask the user to act again — never replayed automatically.
    RetryInteraction,
    /// Nothing safe is possible without the user deciding.
    RequireUserConfirmation,
    /// Nothing safe is possible at all.
    RejectIrrecoverable,
}

/// What kind of thing is being resumed. Decides which recoveries are legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    /// A read-only public region.
    PublicRegion,
    /// A private slot rendered for one principal.
    PrivateSlot,
    /// Local input the user typed and has not submitted.
    UnsavedInput,
    /// A mutation that has not run yet.
    PendingCommand,
    /// A handle to something held open elsewhere.
    OpenResource,
}

impl Construct {
    /// The recoveries this construct permits, most preferred first.
    ///
    /// A `PendingCommand` never contains an automatic replay: re-running a
    /// mutation the user did not re-request is how a version mismatch becomes a
    /// double charge. It asks for a fresh interaction instead.
    pub fn recoveries(self) -> &'static [Recovery] {
        match self {
            Construct::PublicRegion => &[Recovery::RefetchRegion, Recovery::ReloadDocument],
            Construct::PrivateSlot => &[Recovery::RerenderPrivateSlot, Recovery::ReloadDocument],
            // Preserved separately when its own schema still matches; otherwise
            // the user is asked before anything is discarded.
            Construct::UnsavedInput => {
                &[Recovery::RequireUserConfirmation, Recovery::ReloadDocument]
            }
            Construct::PendingCommand => &[
                Recovery::RetryInteraction,
                Recovery::RequireUserConfirmation,
            ],
            // A handle is not a description of a resource, it IS the resource.
            // Nothing on the other side can be reconnected to it.
            Construct::OpenResource => &[Recovery::RejectIrrecoverable],
        }
    }
}

/// The typed answer. Never a boolean — a boolean cannot carry why, and "why"
/// is what a deployment incident is diagnosed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Attach directly. The captures are valid under the handler's schema.
    Resume,
    /// Attach after applying the named migration.
    Migrate { produced: Vec<u8> },
    /// Do not attach. Take the recovery action.
    Refuse {
        why: Refusal,
        recovery: Recovery,
        /// The causal chain, for a compatibility trace.
        trace: Vec<String>,
    },
}

impl Decision {
    pub fn attaches(&self) -> bool {
        !matches!(self, Decision::Refuse { .. })
    }
}

/// Decide whether a manifest entry may be attached to this runtime's code.
///
/// Order matters: the cheapest and most fundamental checks first, so a trace
/// reads as a chain of narrowing rather than a list of unrelated failures.
pub fn decide(entry: &ResumeEntry, rt: &Runtime, construct: Construct) -> Decision {
    let mut trace = Vec::new();
    let refuse = |why: Refusal, trace: Vec<String>| Decision::Refuse {
        recovery: *construct
            .recoveries()
            .first()
            .unwrap_or(&Recovery::RejectIrrecoverable),
        why,
        trace,
    };

    // 0. The manifest must be well formed before anything reads it.
    if entry.captures.is_empty() && entry.capture_schema != SchemaHash::of_fields(&[]) {
        trace.push("manifest declares a non-empty schema and carries no bytes".into());
        return refuse(
            Refusal::MalformedManifest("captures are empty under a non-empty schema"),
            trace,
        );
    }

    // 1. ABI. An unsupported ABI means nothing below can even be interpreted.
    if !rt.abi.contains(&entry.platform_abi) {
        trace.push(format!("abi {} not in {:?}", entry.platform_abi.0, rt.abi));
        return refuse(Refusal::UnsupportedAbi(entry.platform_abi.clone()), trace);
    }
    trace.push(format!("abi {} supported", entry.platform_abi.0));

    // 2. Handler identity. A name would match here; content does not.
    let Some(handler_schema) = rt.handlers.get(&entry.handler) else {
        trace.push("no code with this handler identity".into());
        return refuse(Refusal::UnknownHandler(entry.handler.clone()), trace);
    };
    trace.push("handler identity found".into());

    // 3. Privacy, BEFORE any decoding. A widened scope must not reach the
    //    point where bytes are interpreted, even to fail.
    if let Some(into) = &rt.scope
        && !entry.privacy_scope.admits(into)
    {
        trace.push(format!(
            "{:?} does not admit {:?}",
            entry.privacy_scope, into
        ));
        return refuse(
            Refusal::PrivacyWidened {
                from: entry.privacy_scope.clone(),
                into: into.clone(),
            },
            trace,
        );
    }
    trace.push("privacy scope is equal or narrower".into());

    // 4. The document part must still have the shape the manifest was written
    //    against, or the captures describe positions that no longer exist.
    if let Some(doc) = &rt.document_schema
        && *doc != entry.document_schema
    {
        trace.push("document part schema differs".into());
        return refuse(Refusal::DocumentSchemaMismatch, trace);
    }
    trace.push("document part schema matches".into());

    // 5. Captures. Exact match, or an explicit migration between exactly these
    //    two schemas. No structural inference.
    if *handler_schema == entry.capture_schema {
        trace.push("capture schema matches exactly".into());
        return Decision::Resume;
    }
    let migration = rt
        .migrations
        .iter()
        .find(|m| m.from == entry.capture_schema && m.to == *handler_schema);
    let Some(migration) = migration else {
        trace.push("no migration between these two schemas".into());
        return refuse(
            Refusal::NoMigration {
                from: entry.capture_schema.clone(),
                to: handler_schema.clone(),
            },
            trace,
        );
    };
    trace.push("explicit migration found".into());
    match (migration.apply)(&entry.captures) {
        Ok(produced) => Decision::Migrate { produced },
        Err(e) => {
            trace.push(format!("migration returned an error: {}", e.0));
            refuse(Refusal::MigrationFailed(e), trace)
        }
    }
}

/// May a streamed patch be applied to this document?
///
/// Separate from [`decide`] because it is a different question with a different
/// answer shape: a patch carries no captures and attaches no handler, but it
/// must belong to the generation of artifacts the document came from. A server
/// that redeploys mid-stream would otherwise send the second half of a page
/// from a build whose markup the first half does not match.
pub fn patch_applies(document: &BuildId, patch: &BuildId) -> Decision {
    if document == patch {
        return Decision::Resume;
    }
    Decision::Refuse {
        why: Refusal::MixedBuild {
            document: document.clone(),
            patch: patch.clone(),
        },
        // A mixed-generation stream cannot be repaired by re-fetching a region:
        // the document itself belongs to the older generation.
        recovery: Recovery::ReloadDocument,
        trace: vec![format!(
            "document build {} != patch build {}",
            document.0, patch.0
        )],
    }
}
