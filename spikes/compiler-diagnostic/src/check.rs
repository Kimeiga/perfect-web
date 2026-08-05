//! The one semantic rule this spike implements, chosen because it is the
//! smallest honest instance of the project's central claim.
//!
//! Charter §7.8 lists `SharedCache<Cart@Session>` as something that **must
//! fail**. Charter §16.3 gives the exact diagnostic that failure should produce.
//! This module reproduces that diagnostic from real parsed spans, so Milestone 0
//! can answer "is the target diagnostic quality reachable?" with output rather
//! than with a promise.

use crate::diag::{Diagnostic, Severity};
use crate::parser::{CachePartition, QueryDecl, Visibility};

pub fn check(decl: &QueryDecl) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    // Rule PW0100 — a non-Public query may not be materialized in a shared cache.
    if let Some((CachePartition::Shared, cache_span)) = &decl.cache
        && decl.visibility != Visibility::Public
    {
        let label = decl.visibility.privacy_label();
        out.push(
            Diagnostic::error(
                "PW0100",
                format!(
                    "cannot materialize `{}` in a shared public cache",
                    decl.name
                ),
            )
            .primary(
                cache_span.clone(),
                "this cache is shared across all sessions",
            )
            .context(
                decl.visibility_span.clone(),
                format!(
                    "`{}` is declared here, so its result is labeled `{label}`",
                    decl.name
                ),
            )
            .note(format!(
                "a shared cache may contain only `Public` values; this query's result is `{label}`"
            ))
            .help("change this to `cache private`, or move the value into a private streamed slot")
            .help("if the value really is public, declare the query `public`"),
        );
    }

    // Rule PW0101 — `read_your_writes` is meaningless for a public shared read.
    if let (Visibility::Public, Some((consistency, cspan))) = (decl.visibility, &decl.consistency)
        && consistency == "read_your_writes"
    {
        out.push(
            Diagnostic::error(
                "PW0101",
                "`read_your_writes` requires a session-scoped query",
            )
            .primary(
                cspan.clone(),
                "this consistency policy needs a writer identity",
            )
            .context(
                decl.visibility_span.clone(),
                "declared `public` here, so there is no session to read the writes of",
            )
            .help("use `consistency snapshot` for public data, or declare the query `session`"),
        );
    }

    // Rule PW0102 — a session query with a nonzero freshness window is a stale-cart bug.
    if decl.visibility == Visibility::Session
        && let Some((n, unit, fspan)) = &decl.freshness
        && *n > 0
    {
        out.push(
                    Diagnostic::error(
                        "PW0102",
                        format!("session query `{}` declares a {n}.{unit} staleness window", decl.name),
                    )
                    .primary(fspan.clone(), "session data served this stale can be wrong for the user")
                    .context(decl.visibility_span.clone(), "declared `session` here")
                    .help("use `freshness 0.seconds` with `consistency read_your_writes` for session-owned state"),
                );
    }

    // Rule PW0200 (warning) — a shared materialization with no freshness policy and
    // no invalidation source can only ever be invalidated by hand. Charter §9.4 wants
    // event-driven invalidation; §9.2 lists `freshness` as part of a resource
    // declaration. A warning, not an error, because Milestone 6 owns the real rule.
    if let Some((CachePartition::Shared, cache_span)) = &decl.cache
        && decl.visibility == Visibility::Public
        && decl.freshness.is_none()
    {
        out.push(
                Diagnostic::new(
                    Severity::Warning,
                    "PW0200",
                    format!(
                        "shared materialization of `{}` declares no freshness policy",
                        decl.name
                    ),
                )
                .primary(cache_span.clone(), "nothing here says when this may go stale")
                .note("Milestone 6 replaces time-based freshness with typed invalidation events; until then a shared entry with neither is unbounded")
                .help("add `freshness <n>.seconds`, or an `invalidates_on <Event>` clause once Milestone 6 lands"),
            );
    }

    out
}

/// The `pw explain` surface in miniature (charter §14 M2 task 13). Prints the
/// semantic facts a developer would otherwise have to infer by reading code:
/// value types, privacy label, cache partition, consistency, freshness.
///
/// This exists in the spike to test one specific claim: that the compiler can
/// report placement/privacy facts **without** exposing generated-file paths
/// (charter §14 M2 gate). Nothing here mentions Koka, Marko, or a build dir.
pub fn explain(decl: &QueryDecl, source: &str) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();

    let _ = writeln!(s, "query        {}", decl.name);
    let _ = writeln!(
        s,
        "declared at  line {}",
        source[..decl.name_span.start].lines().count().max(1)
    );
    let _ = writeln!(s, "privacy      {}", decl.visibility.privacy_label());

    if decl.params.is_empty() {
        let _ = writeln!(s, "key          ()  (no parameters — one global instance)");
    } else {
        let key = decl
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(s, "key          ({key})");
        for p in &decl.params {
            let _ = writeln!(
                s,
                "             └ `{}` spans bytes {}..{} of the source",
                &source[p.span.clone()],
                p.span.start,
                p.span.end
            );
        }
    }

    let _ = writeln!(
        s,
        "freshness    {}",
        decl.freshness
            .as_ref()
            .map(|(n, u, _)| format!("{n}.{u}"))
            .unwrap_or_else(|| "(unset — implementation default)".into())
    );
    let _ = writeln!(
        s,
        "consistency  {}",
        decl.consistency
            .as_ref()
            .map(|(c, _)| c.clone())
            .unwrap_or_else(|| "(unset)".into())
    );
    let _ = writeln!(
        s,
        "cache        {}",
        match decl.cache {
            Some((CachePartition::Shared, _)) => "shared   (readable across sessions)",
            Some((CachePartition::Private, _)) => "private  (partitioned per session)",
            None => "(unset)",
        }
    );

    // Derived placement — the thing the developer never writes down. Charter §7.9.
    let placement = match (decl.visibility, decl.cache.as_ref().map(|(c, _)| *c)) {
        (Visibility::Public, Some(CachePartition::Shared)) => {
            "Edge   (public + shared cache => materializable at the edge)"
        }
        (Visibility::Public, _) => "Edge or Origin  (public, but not shared-cached)",
        (Visibility::Session, _) => "Origin (session-scoped read; never shared-cached)",
        (Visibility::Private, _) => "Origin (user-scoped read; never shared-cached)",
    };
    let _ = writeln!(s, "placement    {placement}");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::render;
    use crate::parser::parse;

    /// Semantic diagnostics only, and only errors — warnings are asserted
    /// separately so that adding a lint never silently breaks a rule test.
    fn check_src(src: &str) -> Vec<Diagnostic> {
        let out = parse(src);
        assert!(
            out.diagnostics.is_empty(),
            "parse errors: {:?}",
            out.diagnostics
        );
        check(&out.decl.expect("decl"))
            .into_iter()
            .filter(|d| d.is_error())
            .collect()
    }

    fn warnings_of(src: &str) -> Vec<Diagnostic> {
        let out = parse(src);
        check(&out.decl.expect("decl"))
            .into_iter()
            .filter(|d| !d.is_error())
            .collect()
    }

    #[test]
    fn session_query_in_shared_cache_is_rejected() {
        let d = check_src("session query cart(id: SessionId) cache shared");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, "PW0100");
        // Charter §16.3: the diagnostic must name origin AND boundary.
        assert_eq!(d[0].labels.len(), 2);
        assert!(d[0].labels.iter().any(|l| l.primary));
        assert!(d[0].labels.iter().any(|l| !l.primary));
        assert!(!d[0].helps.is_empty(), "must offer a legal alternative");
    }

    #[test]
    fn public_query_in_shared_cache_is_accepted() {
        let d = check_src("public query store(id: StoreId) freshness 30.seconds cache shared");
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn shared_cache_without_freshness_warns_but_does_not_error() {
        let src = "public query store(id: StoreId) cache shared";
        assert!(check_src(src).is_empty(), "must not be an error");
        let w = warnings_of(src);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, "PW0200");
        assert!(!w[0].is_error());
    }

    #[test]
    fn explain_reports_derived_placement_without_naming_generated_files() {
        let src = "session query cart(id: SessionId)\n    consistency read_your_writes\n    cache private";
        let out = parse(src);
        let text = explain(&out.decl.unwrap(), src);
        assert!(text.contains("Session<SessionId>"), "{text}");
        assert!(text.contains("placement    Origin"), "{text}");
        assert!(text.contains("id: SessionId"), "{text}");
        // Charter §14 M2 gate: no generated-file paths may leak into user output.
        for leak in ["koka", "Koka", "marko", "Marko", ".pw-build", "generated/"] {
            assert!(
                !text.contains(leak),
                "explain output leaked `{leak}`:\n{text}"
            );
        }
    }

    #[test]
    fn private_query_in_shared_cache_is_rejected() {
        let d = check_src("private query profile(id: UserId) cache shared");
        assert_eq!(d[0].code, "PW0100");
        assert!(d[0].notes[0].contains("User<UserId>"));
    }

    #[test]
    fn read_your_writes_on_public_is_rejected() {
        let d = check_src("public query store(id: StoreId) consistency read_your_writes");
        assert_eq!(d[0].code, "PW0101");
    }

    #[test]
    fn stale_session_query_is_rejected() {
        let d = check_src("session query cart(id: SessionId) freshness 30.seconds");
        assert_eq!(d[0].code, "PW0102");
    }

    #[test]
    fn zero_freshness_session_query_is_accepted() {
        let d = check_src(
            "session query cart(id: SessionId) freshness 0.seconds consistency read_your_writes cache private",
        );
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn rendered_diagnostic_meets_charter_16_3_shape() {
        let src = "session query cart(id: SessionId)\n    freshness 0.seconds\n    cache shared";
        let out = parse(src);
        let ds = check(&out.decl.unwrap());
        let text = render(src, "cart.pw", &ds, false);

        // what rule was violated
        assert!(text.contains("PW0100"), "{text}");
        // which boundary made it invalid
        assert!(text.contains("shared across all sessions"), "{text}");
        // where the label originated
        assert!(text.contains("is declared here"), "{text}");
        // relevant inferred label
        assert!(text.contains("Session<SessionId>"), "{text}");
        // at least one legal alternative
        assert!(text.contains("cache private"), "{text}");
    }
}
