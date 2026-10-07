//! **Who a request is, and what `requires` is told** (track `identity`,
//! `docs/PARALLEL.md`, ADR-0253).
//!
//! The identity track owns this module: sign-up, sign-in and sign-out, a
//! session's principal, and the `requires` evaluator. `main.rs` reaches it
//! at two marked seams, and nowhere else:
//!
//! - `Identity::answer`, asked for every request after its body is read and
//!   before any other route: the track's own routes.
//! - `Identity::requires`, asked by `Server::run` for each predicate a
//!   command's `requires` names, before the command runs.
//!
//! Until the track lands this is the development model `Server::run` held
//! inline: every session this server issued is the signed-in principal, and
//! any other predicate is refused. Real identity verification belongs to a
//! deployment (the charter), behind an interface this track defines.

use std::net::TcpStream;

/// What the identity track keeps for one server.
#[derive(Default)]
pub struct Identity {}

impl Identity {
    /// **A request the identity track answers**, before any other route.
    /// `true` when it answered on `stream`; `false` leaves the request to
    /// the routes after it.
    #[allow(clippy::too_many_arguments)]
    pub fn answer(
        &self,
        _method: &str,
        _route: &str,
        _headers: &str,
        _session: &str,
        _fresh: bool,
        _body: &[u8],
        _stream: &mut TcpStream,
    ) -> bool {
        false
    }

    /// **What `requires predicate` is told for `session`**: `Ok(true)` where
    /// the session's principal holds it, `Ok(false)` where it does not, and
    /// an error for a predicate this deployment does not know, so that an
    /// unknown predicate cannot run.
    pub fn requires(&self, predicate: &str, session: &str) -> Result<bool, String> {
        match predicate {
            // A DEVELOPMENT identity model, not authentication: every
            // session the local server issued is the signed-in principal.
            "SignedIn" => Ok(!session.is_empty()),
            other => Err(format!(
                "the development deployment has no authorization predicate `{other}`"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The development model the track starts from, as `Server::run` held it
    /// inline before ADR-0253: a session is signed in, no session is not,
    /// and a predicate the deployment does not know is an error.
    #[test]
    fn requires_starts_as_the_development_model() {
        let identity = Identity::default();
        assert_eq!(identity.requires("SignedIn", "s-1"), Ok(true));
        assert_eq!(identity.requires("SignedIn", ""), Ok(false));
        assert!(identity.requires("Admin", "s-1").is_err());
    }
}
