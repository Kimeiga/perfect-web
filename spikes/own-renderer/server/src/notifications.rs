//! **The reader's user, and what others do that involves them** (track
//! `notifications`, ADR-XXXX; docs/PARALLEL.md, the integrator's rulings of
//! 2026-10-07).
//!
//! The typed principal: `context.current_user()` is the platform's
//! operation `pw:host/principal#read`, which the host answers beside
//! `pw:host/session#read`, from the identity's principals. Its value is a
//! handle, `User<UserId>`, and on the wire it is the user's id (a privacy
//! qualifier is ABI-transparent): the principal's `user`, or the session's
//! guest where no one signed in.

use super::*;
use crate::identity::Principals;

/// **The platform's principal operation**, by its key.
pub(crate) const PRINCIPAL_READ: &str = "pw:host/principal#read";

/// **The user `session` reads as**: its principal's, or its guest's. What
/// `current_user()` answers, and what a page's binding is given for it.
pub(crate) fn user_of(principals: &Principals, session: &str) -> String {
    principals.user_of(session)
}

/// **The platform's principal operation for one request**: the session's
/// user, and nothing else the component could ask it for, as the session
/// operation answers the session.
pub(crate) fn principal_operation(principals: &Principals, session: &str) -> HostFn {
    let user = user_of(principals, session);
    Arc::new(move |_args: &[Val]| Ok(vec![Val::String(user.clone())]))
}

/// **Whether a binding's argument, as a page's plan writes it, is the
/// reader's user**: `current_user()`, or `context.current_user()`.
pub(crate) fn is_current_user(arg: &str) -> bool {
    matches!(arg, "current_user()" | "context.current_user()")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_principal_operation_answers_the_sessions_user_and_a_guests_where_none_signed_in() {
        let principals = Principals::default();
        let read = principal_operation(&principals, "s-1");
        // The guest model: every session its own guest.
        assert_eq!(
            read(&[]).expect("read"),
            vec![Val::String("u-s-1".to_string())]
        );
        // Two sessions, two users: one session's handle is never another's.
        let other = principal_operation(&principals, "s-2");
        assert_ne!(read(&[]).expect("read"), other(&[]).expect("read"));
    }

    #[test]
    fn a_plans_argument_is_the_reader_by_its_call_alone() {
        assert!(is_current_user("current_user()"));
        assert!(is_current_user("context.current_user()"));
        assert!(!is_current_user("current_session()"));
        assert!(!is_current_user("user"));
    }
}
