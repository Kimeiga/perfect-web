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

use pw_syntax::ast::{Decl, DeclKind, EffectRow, Policy, SourceFile, Visibility};
use pw_syntax::lexer::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    /// Where the conflicting property was introduced. Charter §16.3's "where the
    /// value originated".
    pub origin: Option<Span>,
    pub origin_label: Option<String>,
    pub note: Option<String>,
    pub help: Option<String>,
    pub is_error: bool,
}

impl Finding {
    fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            message: message.into(),
            span,
            origin: None,
            origin_label: None,
            note: None,
            help: None,
            is_error: true,
        }
    }
    fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            is_error: false,
            ..Self::error(code, message, span)
        }
    }
    fn origin(mut self, span: Span, label: impl Into<String>) -> Self {
        self.origin = Some(span);
        self.origin_label = Some(label.into());
        self
    }
    fn note(mut self, n: impl Into<String>) -> Self {
        self.note = Some(n.into());
        self
    }
    fn help(mut self, h: impl Into<String>) -> Self {
        self.help = Some(h.into());
        self
    }
}

fn policy<'a>(policies: &'a [Policy], name: &str) -> Option<&'a Policy> {
    policies.iter().find(|p| p.keyword.name == name)
}

fn privacy_label(v: Visibility) -> &'static str {
    match v {
        Visibility::Public => "Public",
        Visibility::Session => "Session<SessionId>",
        Visibility::Private => "User<UserId>",
        Visibility::Unspecified => "unlabelled",
    }
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

fn check_effects_against_placement(
    policies: &[Policy],
    effects: Option<&EffectRow>,
    out: &mut Vec<Finding>,
) {
    let (Some(placement), Some(row)) = (policy(policies, "placement"), effects) else {
        return;
    };
    let target = placement.value.split_whitespace().next().unwrap_or("");
    for e in &row.effects {
        if let Some((why, help)) = placement_conflict(target, &e.name) {
            out.push(
                Finding::error(
                    "PW0323",
                    format!(
                        "effect `{}` is not available at placement `{target}`",
                        e.name
                    ),
                    e.span.clone(),
                )
                .origin(
                    placement.span.clone(),
                    format!("placement `{target}` declared here"),
                )
                .note(why)
                .help(help),
            );
        }
    }
}

fn check_resource(
    name: &str,
    noun: &str,
    visibility: Visibility,
    policies: &[Policy],
    decl: &Decl,
    out: &mut Vec<Finding>,
) {
    let label = privacy_label(visibility);
    let name_span = decl
        .name
        .as_ref()
        .map(|n| n.span.clone())
        .unwrap_or(decl.span.clone());

    // --- PW0100: a non-Public value in a shared cache -----------------------
    // The charter §16.3 worked example, now on real files.
    if let Some(cache) = policy(policies, "cache")
        && cache.value.starts_with("shared")
        && matches!(visibility, Visibility::Session | Visibility::Private)
    {
        out.push(
            Finding::error(
                "PW0100",
                format!("cannot materialize `{name}` in a shared public cache"),
                cache.span.clone(),
            )
            .origin(
                name_span.clone(),
                format!("`{name}` is declared here, so its result is labeled `{label}`"),
            )
            .note(format!(
                "a shared cache may contain only `Public` values; this {noun}'s result is `{label}`"
            ))
            .help("change this to `cache private`, or move the value into a private streamed slot"),
        );
    }

    // --- PW0101: read_your_writes on a public read --------------------------
    if let Some(c) = policy(policies, "consistency")
        && c.value.starts_with("read_your_writes")
        && visibility == Visibility::Public
    {
        out.push(
            Finding::error(
                "PW0101",
                "`read_your_writes` requires a session-scoped query",
                c.span.clone(),
            )
            .origin(
                name_span.clone(),
                "declared `public` here, so there is no session to read the writes of",
            )
            .help("use `consistency snapshot` for public data, or declare the query `session`"),
        );
    }

    // --- PW0102: a stale session read ---------------------------------------
    if let Some(f) = policy(policies, "freshness")
        && visibility == Visibility::Session
        && !f.value.starts_with('0')
    {
        out.push(
            Finding::error(
                "PW0102",
                format!("session {noun} `{name}` declares a {} staleness window", f.value),
                f.span.clone(),
            )
            .origin(name_span.clone(), "declared `session` here")
            .help("use `freshness 0.seconds` with `consistency read_your_writes` for session-owned state"),
        );
    }

    // --- PW0303: retry without idempotency ----------------------------------
    if let Some(r) = policy(policies, "retry")
        && noun == "command"
        && policy(policies, "idempotent_by").is_none()
        && !r.value.starts_with("transport_only")
    {
        out.push(
            Finding::error(
                "PW0312",
                format!("`{name}` declares a retry policy but is not idempotent"),
                r.span.clone(),
            )
            .origin(name_span.clone(), "declared here with no `idempotent_by` key")
            .note("retrying a non-idempotent mutation can apply it more than once")
            .help("add `idempotent_by InteractionId`, or restrict the policy to `retry transport_only(..)`"),
        );
    }

    // --- PW0304: unbounded retry --------------------------------------------
    if let Some(r) = policy(policies, "retry")
        && (r.value.contains("forever") || r.value.contains("unbounded"))
    {
        out.push(
            Finding::error("PW0313", "`retry forever` is not a permitted policy", r.span.clone())
                .note("an unbounded retry is a self-inflicted denial of service")
                .help("declare a finite bound with jitter, e.g. `bounded_exponential(max = 3, jitter = true)`"),
        );
    }

    // --- PW0305: optimistic without rollback --------------------------------
    if let Some(o) = policy(policies, "optimistic")
        && policy(policies, "rollback").is_none()
    {
        out.push(
            Finding::error(
                "PW0327",
                format!("`{name}` declares an optimistic transition with no rollback path"),
                o.span.clone(),
            )
            .note("a rejected mutation would leave the UI permanently inconsistent with the server")
            .help("add a `rollback` clause describing how the optimistic change is reversed"),
        );
    }

    // --- PW0306: a keyed query with no stale-work policy --------------------
    if noun == "query"
        && !decl_params_empty(decl)
        && policy(policies, "on_key_change").is_none()
        && policy(policies, "concurrency").is_some()
    {
        let span = policy(policies, "concurrency").unwrap().span.clone();
        out.push(
            Finding::error(
                "PW0325",
                format!("`{name}` declares no policy for superseded keys"),
                span,
            )
            .origin(
                name_span.clone(),
                "this query is keyed, so its key can change",
            )
            .help("declare `on_key_change cancel | supersede | keep`"),
        );
    }

    // --- PW0307: a subscription escaping its component ----------------------
    if let Some(s) = policy(policies, "scope")
        && matches!(noun, "subscription" | "resource")
        && (s.value.starts_with("application") || s.value.starts_with("session"))
    {
        out.push(
            // PW0326 is the declaration-level form; pw-core's PW2004 is the
            // same defect found by the static scope checker on a task graph.
            Finding::error(
                "PW0326",
                format!("{noun} `{name}` declares `{}` scope", s.value),
                s.span.clone(),
            )
            .origin(name_span.clone(), "created inside a component")
            .note("the subscription would outlive the component and push updates into a dead scope")
            .help("use `scope component`, or hoist the declaration to the scope you actually want"),
        );
    }

    // --- PW0200 (warning): a shared cache with nothing to invalidate it -----
    if let Some(cache) = policy(policies, "cache")
        && cache.value.starts_with("shared")
        && visibility == Visibility::Public
        && policy(policies, "freshness").is_none()
        && policy(policies, "invalidates_on").is_none()
        && policy(policies, "invalidates").is_none()
    {
        out.push(
            Finding::warning(
                "PW0200",
                format!("shared materialization of `{name}` declares no freshness policy"),
                cache.span.clone(),
            )
            .note("with neither a freshness window nor an invalidation source, a shared entry is unbounded")
            .help("add `freshness <n>.seconds`, or an `invalidates_on <Event>` clause"),
        );
    }
}

fn decl_params_empty(decl: &Decl) -> bool {
    match &decl.kind {
        DeclKind::Resource { params, .. } | DeclKind::Ui { params, .. } => params.is_empty(),
        DeclKind::Function { params, .. } => params.is_empty(),
        _ => true,
    }
}

/// Run every declaration-level rule over a parsed file.
pub fn check(file: &SourceFile) -> Vec<Finding> {
    let mut out = Vec::new();
    for d in &file.decls {
        let name = d
            .name
            .as_ref()
            .map(|n| n.name.as_str())
            .unwrap_or("<anonymous>");
        match &d.kind {
            DeclKind::Resource {
                noun,
                visibility,
                policies,
                ..
            } => {
                check_resource(name, noun, *visibility, policies, d, &mut out);
                check_effects_against_placement(policies, None, &mut out);
            }
            DeclKind::Ui {
                policies, effects, ..
            } => {
                check_effects_against_placement(policies, effects.as_ref(), &mut out);
            }
            DeclKind::Function { effects, .. } => {
                // A function's placement comes from a policy clause when it has
                // one; bare functions get their placement solved at E5.
                check_effects_against_placement(&[], effects.as_ref(), &mut out);
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_syntax::parser::parse;

    fn findings(src: &str) -> Vec<Finding> {
        let p = parse(src);
        assert!(p.ok(), "parse errors: {:?}", p.errors);
        check(&p.file)
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
            pw0100.origin.is_some(),
            "charter §16.3 requires an origin span"
        );
        assert!(pw0100.note.as_ref().unwrap().contains("Session<SessionId>"));
        assert!(pw0100.help.as_ref().unwrap().contains("cache private"));
        // The spans must point at real text.
        assert_eq!(&src[pw0100.span.clone()], "cache shared");
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
    fn optimistic_without_rollback_is_rejected() {
        let bad =
            "module c\ncommand add(i: ItemId) -> Cart\n    optimistic cart.add(i)\n{\n    0\n}\n";
        assert!(codes(bad).contains(&"PW0327"));
        let good = "module c\ncommand add(i: ItemId) -> Cart\n    optimistic cart.add(i)\n    rollback cart.remove(i)\n{\n    0\n}\n";
        assert!(!codes(good).contains(&"PW0327"));
    }

    #[test]
    fn a_subscription_declaring_application_scope_is_rejected() {
        let src = "module o\nsubscription Tracking(o: OrderId) -> Stream\n    scope application\n{\n    0\n}\n";
        assert!(codes(src).contains(&"PW0326"));
        let ok = "module o\nsubscription Tracking(o: OrderId) -> Stream\n    scope component\n{\n    0\n}\n";
        assert!(!codes(ok).contains(&"PW0326"));
    }

    #[test]
    fn an_effect_the_placement_cannot_grant_is_rejected() {
        let src = "module e\nview Receipt(o: OrderId) !{ secret<Payments> }\n    placement edge\n{\n    0\n}\n";
        let f = findings(src);
        let e = f.iter().find(|f| f.code == "PW0323").expect("PW0323");
        assert!(e.message.contains("secret"), "{}", e.message);
        assert!(
            e.origin.is_some(),
            "must name where the placement was declared"
        );
        assert!(
            e.note
                .as_ref()
                .unwrap()
                .contains("edge world grants no secret")
        );
    }

    #[test]
    fn a_browser_only_effect_at_origin_is_rejected() {
        let src = "module d\nview Estimate(s: SessionId) !{ device.location }\n    placement origin\n{\n    0\n}\n";
        assert!(codes(src).contains(&"PW0323"));
    }

    #[test]
    fn a_shared_cache_with_no_invalidation_warns_but_does_not_error() {
        let src =
            "module s\npublic query Store(id: StoreId) -> Store\n    cache shared\n{\n    0\n}\n";
        let f = findings(src);
        let w = f.iter().find(|f| f.code == "PW0200").expect("PW0200");
        assert!(!w.is_error, "this is advisory until E6");
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
    use pw_syntax::parser::parse;
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
            let p = parse(&src);
            assert!(p.ok(), "{} failed to parse: {:?}", path.display(), p.errors);
            for f in check(&p.file).into_iter().filter(|f| f.is_error) {
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
            let p = parse(&src);
            let Some(expected) = p.file.attr("rule") else {
                continue;
            };
            let found: Vec<&str> = check(&p.file)
                .into_iter()
                .filter(|f| f.is_error)
                .map(|f| f.code)
                .collect();
            if found.is_empty() {
                continue; // needs body parsing or type checking; see below
            }
            caught += 1;
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
            let p = parse(&src);
            if check(&p.file).iter().any(|f| f.is_error) {
                caught += 1;
            }
        }
        // E2 checks declaration HEADERS only. The rest need body parsing (E2
        // continuation), effect checking (E1), or privacy/placement solving (E5).
        assert!(
            caught >= 4,
            "declaration-level coverage regressed: {caught}/{total} rejected files caught"
        );
        assert!(
            total >= 40,
            "expected the full rejected corpus, saw {total}"
        );
    }
}
