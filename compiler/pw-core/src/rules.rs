//! Declaration-level semantic rules.
//!
//! These are the rules decidable from a declaration *header* alone — policy
//! contradictions, placement-versus-effect conflicts, privacy-versus-cache
//! conflicts. They need no expression analysis, so they can run today against
//! the real corpus rather than waiting for body parsing.
//!
//! What this is **not**: the effect checker. A rule like "a network request
//! inside a view" (corpus R-001) lives in a function *body*, which E2 captures
//! as a token range and does not parse. Those stay unenforced, and
//! `docs/milestones/E2.md` says so rather than implying otherwise.
//!
//! Every rule carries a stable `PW####` code, a primary span, and — where the
//! rule involves two places — an origin span, per charter §16.3. The E0
//! diagnostic spike established that shape; this applies it to real files.
//!
//! # E6F: this reads the HIR
//!
//! It used to read `pw_syntax::ast::SourceFile`, a second declaration tree
//! produced by a second parser. Two parsers deciding what a program means is
//! two programs, and E6 showed what that costs: the tree grammar learned to
//! parse a `materialize` block's policies and this path did not, so the same
//! file had policies to one analysis and none to another.
//!
//! Architect ruling, 2026-08-06:
//!
//! > There is only one parser that decides what a `.pw` program means.
//!
//! The rules themselves did not change. What changed is where they read from.

use crate::hir::{Decl, DeclKind, Hir, Policy, Span};

use crate::diagnostics::{Detector, Diagnostic};

/// Declaration-level rules build `Diagnostic` values directly. The local
/// builder below only exists to keep each rule readable.
type Finding = Diagnostic;

fn err(
    code: &'static str,
    invariant: &'static str,
    message: impl Into<String>,
    span: Span,
) -> Diagnostic {
    Diagnostic::error(code, invariant, Detector::DeclarationRule, message, span)
}

fn warn(
    code: &'static str,
    invariant: &'static str,
    message: impl Into<String>,
    span: Span,
) -> Diagnostic {
    Diagnostic::warning(code, invariant, Detector::DeclarationRule, message, span)
}

fn policy<'a>(policies: &'a [Policy], name: &str) -> Option<&'a Policy> {
    policies.iter().find(|p| p.name == name)
}

/// The visibility keyword as written, or `""`.
fn vis(decl: &Decl) -> &str {
    decl.visibility.as_deref().unwrap_or("")
}

fn privacy_label(v: &str) -> &'static str {
    match v {
        "public" => "Public",
        "session" => "Session<SessionId>",
        "private" => "User<UserId>",
        _ => "unlabelled",
    }
}

/// The declaration keyword, for a message that names what the reader wrote.
///
/// A `DeclKind` and a noun are not the same thing: the kind is what the
/// compiler decided, the noun is what the author typed, and a diagnostic that
/// says "session query" when the source says `subscription` is describing a
/// different program from the one on screen.
fn noun_of(kind: DeclKind) -> &'static str {
    match kind {
        DeclKind::Query => "query",
        DeclKind::Command => "command",
        DeclKind::Subscription => "subscription",
        DeclKind::Resource => "resource",
        DeclKind::Materialize => "materialize",
        DeclKind::View => "view",
        DeclKind::Component => "component",
        DeclKind::Page => "page",
        DeclKind::Task => "task",
        DeclKind::Event => "event",
        DeclKind::Prelude => "prelude",
        DeclKind::Effect => "effect",
        DeclKind::Fn => "fn",
        DeclKind::Type | DeclKind::Opaque => "type",
        DeclKind::Let => "let",
        DeclKind::Import => "import",
        DeclKind::Other => "declaration",
    }
}

/// The declarations `check_resource` applies to: the ones with a policy block
/// describing a cached, placed or retried operation.
fn is_resource_like(kind: DeclKind) -> bool {
    matches!(
        kind,
        DeclKind::Query
            | DeclKind::Command
            | DeclKind::Subscription
            | DeclKind::Resource
            | DeclKind::Materialize
    )
}

/// Effects a placement cannot satisfy, and why.
fn placement_conflict(placement: &str, effect: &str) -> Option<(&'static str, &'static str)> {
    let browser_only = effect.starts_with("device")
        || effect.starts_with("layout")
        || effect.starts_with("dom")
        || effect.starts_with("observe")
        || effect.starts_with("paint")
        || effect.starts_with("animation");
    match placement {
        "browser" if effect.starts_with("database") => Some((
            "the browser world grants no database capability",
            "move this to an origin-placed query",
        )),
        "browser" if effect.starts_with("secret") => Some((
            "the browser world grants no secret capability",
            "keep secret access at the origin",
        )),
        "edge" if effect.starts_with("secret") => Some((
            "the edge world grants no secret capability",
            "move this operation to `origin`, or request an edge-scoped credential",
        )),
        "origin" if browser_only => Some((
            "the origin world grants no browser or device capabilities",
            "this must run in the browser",
        )),
        "build" if effect.starts_with("clock.wall") || effect.starts_with("random") => Some((
            "static generation requires a deterministic effect row",
            "remove the nondeterministic read, or drop the `build` placement",
        )),
        _ => None,
    }
}

fn check_effects_against_placement(decl: &Decl, out: &mut Vec<Finding>) {
    let (Some(placement), Some(row)) = (
        policy(&decl.policies, "placement"),
        decl.declared_effects.as_ref(),
    ) else {
        return;
    };
    let target = placement.value.split_whitespace().next().unwrap_or("");
    for e in row {
        // `path`, not `written`: the conflict is with the effect FAMILY, and
        // `style.mutate<LayoutAffect>` is as unavailable at `origin` as
        // `style.mutate` is. The message uses the written form, because that is
        // what the reader typed.
        if let Some((why, help)) = placement_conflict(target, &e.path) {
            out.push(
                err(
                    crate::codes::DECLARED_PLACEMENT_CANNOT_GRANT.id,
                    crate::codes::DECLARED_PLACEMENT_CANNOT_GRANT.invariant,
                    format!(
                        "effect `{}` is not available at placement `{target}`",
                        e.written
                    ),
                    e.span.clone(),
                )
                .related(
                    placement.span.clone(),
                    format!("placement `{target}` declared here"),
                )
                .explain(why)
                .repair(help),
            );
        }
    }
}

fn check_resource(decl: &Decl, out: &mut Vec<Finding>) {
    let name = decl.name.as_str();
    let noun = noun_of(decl.kind);
    let visibility = vis(decl);
    let policies: &[Policy] = &decl.policies;
    let label = privacy_label(visibility);
    let name_span: Span = decl.name_span.clone();

    // --- PW0100: a non-Public value in a shared cache -----------------------
    // The charter §16.3 worked example, now on real files.
    if let Some(cache) = policy(policies, "cache")
        && cache.value.starts_with("shared")
        && matches!(visibility, "session" | "private")
    {
        out.push(
            err(
                "PW0100",
                "a shared cache may contain only Public values",
                format!("cannot materialize `{name}` in a shared public cache"),
                cache.span.clone(),
            )
            .related(
                name_span.clone(),
                format!("`{name}` is declared here, so its result is labeled `{label}`"),
            )
            .explain(format!(
                "a shared cache may contain only `Public` values; this {noun}'s result is `{label}`"
            ))
            .repair(
                "change this to `cache private`, or move the value into a private streamed slot",
            ),
        );
    }

    // --- PW0101: read_your_writes on a public read --------------------------
    if let Some(c) = policy(policies, "consistency")
        && c.value.starts_with("read_your_writes")
        && visibility == "public"
    {
        out.push(
            err(
                "PW0101",
                "read-your-writes requires a session-scoped read",
                "`read_your_writes` requires a session-scoped query",
                c.span.clone(),
            )
            .related(
                name_span.clone(),
                "declared `public` here, so there is no session to read the writes of",
            )
            .repair("use `consistency snapshot` for public data, or declare the query `session`"),
        );
    }

    // --- PW0102: a stale session read ---------------------------------------
    if let Some(f) = policy(policies, "freshness")
        && visibility == "session"
        && !f.value.starts_with('0')
    {
        out.push(
            err(
                "PW0102",
                "session-owned state may not be served stale",
                format!("session {noun} `{name}` declares a {} staleness window", f.value),
                f.span.clone(),
            )
            .related(name_span.clone(), "declared `session` here")
            .repair("use `freshness 0.seconds` with `consistency read_your_writes` for session-owned state"),
        );
    }

    // --- PW0303: retry without idempotency ----------------------------------
    if let Some(r) = policy(policies, "retry")
        && noun == "command"
        && policy(policies, "idempotent_by").is_none()
        && !r.value.starts_with("transport_only")
    {
        out.push(
            err(
                "PW0312",
                "a retryable mutation must be idempotent",
                format!("`{name}` declares a retry policy but is not idempotent"),
                r.span.clone(),
            )
            .related(name_span.clone(), "declared here with no `idempotent_by` key")
            .explain("retrying a non-idempotent mutation can apply it more than once")
            .repair("add `idempotent_by InteractionId`, or restrict the policy to `retry transport_only(..)`"),
        );
    }

    // --- PW0304: unbounded retry --------------------------------------------
    if let Some(r) = policy(policies, "retry")
        && (r.value.contains("forever") || r.value.contains("unbounded"))
    {
        out.push(
            err("PW0313", "a retry policy must be bounded", "`retry forever` is not a permitted policy", r.span.clone())
                .explain("an unbounded retry is a self-inflicted denial of service")
                .repair("declare a finite bound with jitter, e.g. `bounded_exponential(max = 3, jitter = true)`"),
        );
    }

    // --- PW0327: `rollback` is derived, not written -------------------------
    //
    // Architect ruling, 2026-08-11 (ADR-0025), retiring the rule this code
    // used to be:
    //
    // > "an optimistic transition is a function from the current value to the
    // > next, and rollback is its inverse" — the first half is excellent. The
    // > second is generally false.
    //
    // A cart holding `Apple x 3`, optimistically `+2`, is `Apple x 5`; a
    // hand-written `remove(apple)` does not restore `Apple x 3`, and even
    // `subtract(2)` fails once there are concurrent updates, normalization, or
    // derived fields. The platform knows the exact value it held, so requiring
    // the author to describe an inverse was requiring a description of
    // something the runtime will not use.
    //
    // The code is REUSED rather than retired, because its subject is the same
    // declaration and a reader looking it up should find what replaced it.
    if let Some(r) = policy(policies, "rollback") {
        out.push(
            err(
                "PW0327",
                "an optimistic transition's reversal is derived, not written",
                format!("`{name}` declares a `rollback`"),
                r.span.clone(),
            )
            .explain(
                "the platform restores the resource value it held before the speculative one, \
                 which it knows exactly. A written inverse describes a different operation: \
                 reversing `add(2)` with `remove(..)` does not restore the previous value once \
                 there are concurrent updates, normalization, or derived fields",
            )
            .repair("delete the `rollback` clause"),
        );
    }

    // --- PW0306: a keyed query with no stale-work policy --------------------
    if noun == "query"
        && !decl.params.is_empty()
        && policy(policies, "on_key_change").is_none()
        && policy(policies, "concurrency").is_some()
    {
        let span = policy(policies, "concurrency").unwrap().span.clone();
        out.push(
            err(
                "PW0325",
                "a keyed query must declare a stale-work policy",
                format!("`{name}` declares no policy for superseded keys"),
                span,
            )
            .related(
                name_span.clone(),
                "this query is keyed, so its key can change",
            )
            .repair("declare `on_key_change cancel | supersede | keep`"),
        );
    }

    // --- PW0307: a subscription escaping its component ----------------------
    if let Some(s) = policy(policies, "scope")
        && matches!(noun, "subscription" | "resource")
        && (s.value.starts_with("application") || s.value.starts_with("session"))
    {
        out.push(
            // ONE code per invariant (architect ruling). `PW2004` is canonical:
            // "a resource cannot outlive the scope that owns it". This detector
            // finds it from the declaration header; `pw-core`'s scope graph finds
            // the same invariant from handle flow. `PW0326` is a DEPRECATED ALIAS
            // kept only until the corpus migrates — see `canonical_code`.
            err(
                "PW2004",
                "a resource cannot outlive the scope that owns it",
                format!("{noun} `{name}` declares `{}` scope", s.value),
                s.span.clone(),
            )
            .reason("declared_scope_exceeds_owner")
            .related(name_span.clone(), "created inside a component")
            .explain(
                "the subscription would outlive the component and push updates into a dead scope",
            )
            .repair(
                "use `scope component`, or hoist the declaration to the scope you actually want",
            ),
        );
    }

    // --- PW0200 (warning): a shared cache with nothing to invalidate it -----
    if let Some(cache) = policy(policies, "cache")
        && cache.value.starts_with("shared")
        && visibility == "public"
        && policy(policies, "freshness").is_none()
        && policy(policies, "invalidates_on").is_none()
        && policy(policies, "invalidates").is_none()
    {
        out.push(
            warn(
                "PW0200",
                "a shared materialization needs a freshness or invalidation source",
                format!("shared materialization of `{name}` declares no freshness policy"),
                cache.span.clone(),
            )
            .explain("with neither a freshness window nor an invalidation source, a shared entry is unbounded")
            .repair("add `freshness <n>.seconds`, or an `invalidates_on <Event>` clause"),
        );
    }
}

/// **PW0332 — an effect is not a callable, so it does not name an operation.**
///
/// Its own pass, run for EVERY declaration. It first sat inside
/// `check_resource`, which is called only for `is_resource_like(kind)` — so an
/// effect never reached it and the rule was silent. That is the enumeration
/// pattern `docs/RISK_QUEUE.md` records, caught here by the discriminator half
/// of its own test rather than by review.
/// **PW0332 — an effect is not a callable, so it does not name an operation.**
///
/// Its own pass, run for EVERY declaration. It first sat inside
/// [`check_resource`], which is called only when `is_resource_like(kind)` — so
/// an effect never reached it and the rule was silent while looking present.
/// That is the enumeration pattern `docs/RISK_QUEUE.md` records, and what
/// caught it was the discriminator half of its own test rather than review.
fn check_effect_names_no_operation(decl: &Decl, out: &mut Vec<Finding>) {
    // `effect session.read { host "pw:host/session#read" }` parsed, was
    // recorded in the ontology, and was read by NOTHING. It is the fossil of
    // deriving an operation from a capability, which ADR-0026 killed:
    //
    //     effect      what computation does
    //     capability  the authority
    //     operation   the callable ABI
    //
    // While it existed, a reader walking declarations could answer *which
    // operation is this* with an effect's clause instead of a callable's — and
    // one did, rendering the effect's empty signature over the function's real
    // one. Architect ruling, 2026-08-20:
    //
    // > Don't merely stop consuming it. You've learned repeatedly that
    // > semantically dead syntax survives for a long time if it still parses.
    //
    // So it is a diagnostic and not a silent ignore. The two failures are
    // different and must not be observationally equivalent: an UNKNOWN policy
    // head is the parser's business; a KNOWN head on a declaration that cannot
    // mean it is this.
    if decl.kind != DeclKind::Effect {
        return;
    }
    let Some(h) = policy(&decl.policies, "host") else {
        return;
    };
    let name = decl.name.as_str();
    out.push(
        err(
            crate::codes::EFFECT_NAMES_AN_OPERATION.id,
            crate::codes::EFFECT_NAMES_AN_OPERATION.invariant,
            format!("effect `{name}` declares a `host` operation"),
            h.span.clone(),
        )
        .explain(
            "a capability authorizes an operation and does not identify one (ADR-0026). Two \
             callables can require one authority and have different ABIs, so an effect cannot \
             stand for either of them. The operation belongs on the `fn` that performs it",
        )
        .repair(
            "delete the clause, and write `host \"interface#operation\"` on the bodiless `fn` \
             the deployment supplies",
        ),
    );
}

/// Run every declaration-level rule over one program's HIR.
///
/// Nested declarations included (`all_decls`), which the AST walk did not do:
/// a `fn` inside a component is a declaration with an effect row, and its
/// placement conflict is the same conflict.
pub fn check(hir: &Hir) -> Vec<Finding> {
    let mut out = Vec::new();
    for (_, decl) in hir.all_decls() {
        if is_resource_like(decl.kind) {
            check_resource(decl, &mut out);
        }
        check_effects_against_placement(decl, &mut out);
        check_effect_names_no_operation(decl, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_syntax::parse_tree;

    fn findings(src: &str) -> Vec<Finding> {
        let p = parse_tree(src);
        assert!(p.ok(), "parse errors: {:?}", p.errors);
        check(&crate::lower::lower_file(src, &p.green))
    }

    fn codes(src: &str) -> Vec<&'static str> {
        findings(src).iter().map(|f| f.code).collect()
    }

    #[test]
    fn session_query_in_a_shared_cache_is_rejected_with_both_spans() {
        let src = "module cart\nsession query Cart(s: SessionId) -> Cart\n    cache shared\n{\n    Carts.current(s)\n}\n";
        let f = findings(src);
        let pw0100 = f.iter().find(|f| f.code == "PW0100").expect("PW0100");
        assert!(
            !pw0100.related.is_empty(),
            "charter §16.3 requires an origin span"
        );
        assert!(
            pw0100
                .explanation
                .as_ref()
                .unwrap()
                .contains("Session<SessionId>")
        );
        assert!(pw0100.repairs[0].description.contains("cache private"));
        // The spans must point at real text.
        assert_eq!(&src[pw0100.primary_span.clone()], "cache shared");
    }

    #[test]
    fn a_public_shared_cache_is_accepted() {
        let src = "module s\npublic query Store(id: StoreId) -> Store\n    freshness 30.seconds\n    cache shared\n{\n    Stores.get(id)\n}\n";
        assert!(!codes(src).contains(&"PW0100"));
    }

    #[test]
    fn read_your_writes_on_a_public_query_is_rejected() {
        let src = "module s\npublic query Store(id: StoreId) -> Store\n    consistency read_your_writes\n{\n    0\n}\n";
        assert!(codes(src).contains(&"PW0101"));
    }

    #[test]
    fn a_stale_session_query_is_rejected_but_a_fresh_one_is_not() {
        let stale = "module c\nsession query Cart(s: SessionId) -> Cart\n    freshness 30.seconds\n{\n    0\n}\n";
        assert!(codes(stale).contains(&"PW0102"));
        let fresh = "module c\nsession query Cart(s: SessionId) -> Cart\n    freshness 0.seconds\n{\n    0\n}\n";
        assert!(!codes(fresh).contains(&"PW0102"));
    }

    #[test]
    fn retry_without_idempotency_is_rejected() {
        let bad = "module p\ncommand charge(o: OrderId) -> Receipt\n    requires SignedIn\n    retry bounded_exponential(max = 5, jitter = true)\n{\n    0\n}\n";
        assert!(codes(bad).contains(&"PW0312"));
        let good = "module p\ncommand charge(o: OrderId) -> Receipt\n    requires SignedIn\n    idempotent_by InteractionId\n    retry bounded_exponential(max = 5, jitter = true)\n{\n    0\n}\n";
        assert!(!codes(good).contains(&"PW0312"));
    }

    #[test]
    fn unbounded_retry_is_rejected() {
        let src =
            "module s\npublic query Store(id: StoreId) -> Store\n    retry forever\n{\n    0\n}\n";
        assert!(codes(src).contains(&"PW0313"));
    }

    #[test]
    fn a_written_rollback_is_rejected_and_an_optimistic_alone_is_not() {
        // The inversion of the rule this code used to carry, and it is a real
        // inversion rather than a deletion: `optimistic` alone was the error
        // and is now correct; `rollback` was required and is now refused.
        let bad = "module c\ncommand add(i: ItemId) -> Cart\n    optimistic Cart() as c => c.add(i)\n    rollback c => c.remove(i)\n{\n    0\n}\n";
        assert!(codes(bad).contains(&"PW0327"));
        let good = "module c\ncommand add(i: ItemId) -> Cart\n    optimistic Cart() as c => c.add(i)\n{\n    0\n}\n";
        assert!(!codes(good).contains(&"PW0327"));
    }

    #[test]
    fn an_effect_naming_a_host_operation_is_rejected_and_a_fn_naming_one_is_not() {
        // The discriminator the architect asked for: a KNOWN policy on a
        // declaration that cannot mean it. The same clause on the callable that
        // performs the effect is correct and must stay silent — otherwise this
        // would read as "`host` is refused", which is the opposite of ADR-0026.
        let bad = "module p\n\neffect session.read {\n    capability session.read\n    \
                   host \"pw:host/session#read\"\n}\n";
        assert!(codes(bad).contains(&"PW0332"), "{:?}", codes(bad));

        let good = "module p\n\nfn current_session() -> Session<SessionId> !{ session.read }\n    \
                    host \"pw:host/session#read\"\n";
        assert!(!codes(good).contains(&"PW0332"), "{:?}", codes(good));

        // And an effect without the clause is unaffected, so the rule is about
        // the clause and not about effects.
        let plain = "module p\n\neffect session.read {\n    capability session.read\n}\n";
        assert!(!codes(plain).contains(&"PW0332"));
    }

    #[test]
    fn a_subscription_declaring_application_scope_is_rejected() {
        let src = "module o\nsubscription Tracking(o: OrderId) -> Stream\n    scope application\n{\n    0\n}\n";
        assert!(
            codes(src).contains(&"PW2004"),
            "PW2004 is the canonical invariant code"
        );
        let ok = "module o\nsubscription Tracking(o: OrderId) -> Stream\n    scope component\n{\n    0\n}\n";
        assert!(!codes(ok).contains(&"PW2004"));
    }

    #[test]
    fn an_effect_the_placement_cannot_grant_is_rejected() {
        let src = "module e\nview Receipt(o: OrderId) !{ secret<Payments> }\n    placement edge\n{\n    0\n}\n";
        let f = findings(src);
        let e = f.iter().find(|f| f.code == "PW5005").expect("PW0323");
        assert!(e.message.contains("secret"), "{}", e.message);
        assert!(
            !e.related.is_empty(),
            "must name where the placement was declared"
        );
        assert!(
            e.explanation
                .as_ref()
                .unwrap()
                .contains("edge world grants no secret")
        );
    }

    #[test]
    fn a_browser_only_effect_at_origin_is_rejected() {
        let src = "module d\nview Estimate(s: SessionId) !{ device.location }\n    placement origin\n{\n    0\n}\n";
        assert!(codes(src).contains(&"PW5005"));
    }

    #[test]
    fn a_shared_cache_with_no_invalidation_warns_but_does_not_error() {
        let src =
            "module s\npublic query Store(id: StoreId) -> Store\n    cache shared\n{\n    0\n}\n";
        let f = findings(src);
        let w = f.iter().find(|f| f.code == "PW0200").expect("PW0200");
        assert!(!w.is_error(), "this is advisory until E6");
    }

    #[test]
    fn a_finding_records_its_detector_and_reason_as_metadata() {
        // The developer learns the invariant; tooling learns which pass found
        // it and which repair applies.
        let src = "module o\nsubscription Tracking(o: OrderId) -> Stream\n    scope application\n{\n    0\n}\n";
        let f = findings(src);
        let v = f.iter().find(|f| f.code == "PW2004").expect("PW2004");
        assert_eq!(v.reason, "declared_scope_exceeds_owner");
        assert_eq!(v.detector, Detector::DeclarationRule);
    }

    #[test]
    fn the_rules_can_go_green_as_well_as_red() {
        // docs/RISK_QUEUE.md: a check that cannot fail is not a check — and one
        // that always fails is just as useless.
        let clean = "module s\npublic query Store(id: StoreId) -> Store\n    freshness 30.seconds\n    consistency snapshot\n    cache shared\n    invalidates_on StoreChanged(id)\n    on_key_change cancel\n    concurrency one_per_key\n{\n    Stores.get(id)\n}\n";
        assert!(findings(clean).is_empty(), "{:?}", findings(clean));
    }
}

#[cfg(test)]
mod corpus_tests {
    use super::*;
    use crate::diagnostics::canonical_code;
    use pw_syntax::parse_tree;
    use std::path::{Path, PathBuf};

    fn corpus(bucket: &str) -> Vec<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join(bucket);
        let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("corpus dir")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "pw"))
            .collect();
        v.sort();
        v
    }

    /// The property that matters most: a rule that fires on a program the
    /// corpus says is CORRECT is worse than a rule that misses one.
    #[test]
    fn no_accepted_corpus_file_produces_an_error() {
        let mut offenders = Vec::new();
        for path in corpus("accepted") {
            let src = std::fs::read_to_string(&path).expect("read");
            let p = parse_tree(&src);
            assert!(p.ok(), "{} failed to parse: {:?}", path.display(), p.errors);
            let hir = crate::lower::lower_file(&src, &p.green);
            for f in check(&hir).into_iter().filter(|f| f.is_error()) {
                offenders.push(format!(
                    "{}: [{}] {}",
                    path.file_name().unwrap().to_string_lossy(),
                    f.code,
                    f.message
                ));
            }
        }
        assert!(
            offenders.is_empty(),
            "false positives on accepted programs:\n{}",
            offenders.join("\n")
        );
    }

    /// Where a rejected file IS caught, it must be caught by the rule its
    /// `@rule` header declares — not merely by something.
    #[test]
    fn every_caught_rejection_matches_its_declared_rule() {
        let mut mismatches = Vec::new();
        let mut caught = 0;
        for path in corpus("rejected") {
            let src = std::fs::read_to_string(&path).expect("read");
            let p = parse_tree(&src);
            // `@rule:` is a doc attribute in a comment, so it is read from the
            // source text rather than from either tree.
            let Some(expected) = src
                .lines()
                .find_map(|l| l.trim().strip_prefix("// @rule:"))
                .map(str::trim)
            else {
                continue;
            };
            let hir = crate::lower::lower_file(&src, &p.green);
            let found: Vec<&str> = check(&hir)
                .into_iter()
                .filter(|f| f.is_error())
                .map(|f| f.code)
                .collect();
            if found.is_empty() {
                continue; // needs body parsing or type checking; see below
            }
            caught += 1;
            let expected = canonical_code(expected);
            if !found.contains(&expected) {
                mismatches.push(format!(
                    "{}: declares @rule {expected}, got {found:?}",
                    path.file_name().unwrap().to_string_lossy()
                ));
            }
        }
        assert!(caught > 0, "no rejected file was caught at all");
        assert!(
            mismatches.is_empty(),
            "caught by the wrong rule:\n{}",
            mismatches.join("\n")
        );
    }

    /// Records how much of the rejected corpus declaration-level rules cover.
    ///
    /// This is a **ratchet**, not a target. It exists so that coverage cannot
    /// silently regress, and so the honest number is in the test suite rather
    /// than only in prose. Raise the floor when a milestone raises the coverage.
    #[test]
    fn declaration_level_coverage_of_the_rejected_corpus_does_not_regress() {
        let mut caught = 0;
        let mut total = 0;
        for path in corpus("rejected") {
            total += 1;
            let src = std::fs::read_to_string(&path).expect("read");
            let p = parse_tree(&src);
            let hir = crate::lower::lower_file(&src, &p.green);
            if check(&hir).iter().any(|f| f.is_error()) {
                caught += 1;
            }
        }
        // E2 checks declaration HEADERS only. The rest need body parsing (E2
        // continuation), effect checking (E1), or privacy/placement solving (E5).
        //
        // **The floor moved from 4 to 3 on 2026-08-11, and it is a movement to
        // record rather than a regression.** `R-029` was caught here for
        // `optimistic` without `rollback`; ADR-0025 retired that invariant —
        // the platform restores the value it held, so a written inverse
        // describes a different operation — and the fixture now carries
        // `optimistic_not_pure`, which is an EFFECT question. `check.rs`
        // catches it, `rules.rs` cannot, and a header rule that could would be
        // guessing at what a call performs.
        //
        // A floor going down is the shape `docs/RISK_QUEUE.md` warns about, so
        // the compensating assertion is below: R-029 is still caught, by the
        // whole checker, for its declared code.
        assert!(
            caught >= 3,
            "declaration-level coverage regressed: {caught}/{total} rejected files caught"
        );
        assert!(
            total >= 40,
            "expected the full rejected corpus, saw {total}"
        );

        // The fixture that left this count is still caught, by the checker as
        // a whole. Without this, lowering the floor would be indistinguishable
        // from losing a catch.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files: Vec<(String, String)> = Vec::new();
        for d in [
            "packages/pw-std",
            "packages/pw-platform-web",
            "examples/lib",
        ] {
            let mut ps: Vec<std::path::PathBuf> = std::fs::read_dir(root.join(d))
                .expect("dir")
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
            std::fs::read_to_string(root.join("examples/domain.pw")).expect("domain"),
        ));
        let r029 = std::fs::read_dir(root.join("examples/rejected"))
            .expect("rejected")
            .map(|e| e.expect("entry").path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("R-029")
            })
            .expect("R-029");
        files.push((
            "R-029.pw".to_string(),
            std::fs::read_to_string(&r029).expect("read"),
        ));
        let codes: Vec<&str> = crate::check::check_sources(&files)
            .iter()
            .flat_map(|(_, ds)| ds.iter())
            .map(|d| d.code)
            .collect();
        assert!(
            codes.contains(&"PW0330"),
            "R-029 left the declaration-level count and must still be caught \
             for its declared invariant: {codes:?}"
        );
    }
}
