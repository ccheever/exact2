//! `openAuthSession` and `authCallback` for a Rust source (LLP 1069.006):
//! the same request a TypeScript source's binding makes, and the same
//! callback. The host's `exact-auth:` arm does the rest.
//!
//! @ref LLP 1069.006 D1 (a request, answered with the callback URL) / D2
//! (the callback is known before PAR)

use exact_runner::auth::Session;
use exact_runner::{DataError, Outcome, Request, Store};

/// The request for `openAuthSession(url, {callback, state, ephemeral})`:
/// return it as `Answer::Later`, and read the reply with [`reply`].
pub fn open_auth_session(url: &str, callback: &str, state: &str, ephemeral: bool) -> Request {
    let session = Session {
        url: url.into(),
        callback: callback.into(),
        state: state.into(),
        ephemeral,
    };
    Request::auth(session.to_json().into_bytes())
}

/// The session's answer: `Ok` the callback URL (a 200: a callback arrived,
/// which may still carry an OAuth `error`), `Err((status, message))`
/// otherwise — 499 cancelled, 403 a grant, 409 already open, 428 popup
/// blocked, 501 no system browser, 502 anything else.
pub fn reply(outcome: &Outcome) -> Result<String, (u16, String)> {
    match outcome {
        Outcome::Response(r) if r.status == 200 => {
            Ok(String::from_utf8_lossy(&r.body).into_owned())
        }
        Outcome::Response(r) => Err((r.status, String::from_utf8_lossy(&r.body).into_owned())),
        Outcome::Failed { message, .. } => Err((502, message.clone())),
        _ => Err((502, "not an auth session's reply".into())),
    }
}

/// `authCallback()`: this carrier's callback, from the app's `auth.callback`
/// grants (`grants` is the source's own). A device read, as the carrier is
/// the device's fact: the bake compiles no answer that asks. On the web a
/// Rust source has no page origin to form `/.exact/auth/callback` from yet,
/// so there it is unavailable.
pub fn callback(store: &Store, grants: &str) -> Result<String, DataError> {
    store.observe_external_read();
    if cfg!(target_arch = "wasm32") {
        return Err(DataError::Unavailable(
            "authCallback: a Rust source on the web cannot name its callback yet (LLP 1069.006)"
                .into(),
        ));
    }
    exact_runner::auth::carrier_callback(grants, None).ok_or_else(|| {
        DataError::Unavailable("authCallback: the grants name no auth.callback".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::Response;

    #[test]
    fn the_request_and_its_reply() {
        let request = open_auth_session("https://x.test/a", "x.app:/cb", "s", false);
        assert!(request.is_auth());
        let session = Session::from_json(&String::from_utf8(request.body).unwrap()).unwrap();
        assert_eq!(
            (session.callback.as_str(), session.state.as_str()),
            ("x.app:/cb", "s")
        );
        let ok = Outcome::Response(Response {
            status: 200,
            headers: vec![],
            body: b"x.app:/cb?code=c&state=s".to_vec(),
        });
        assert_eq!(reply(&ok).unwrap(), "x.app:/cb?code=c&state=s");
        let cancelled = Outcome::Response(Response {
            status: 499,
            headers: vec![],
            body: b"cancelled".to_vec(),
        });
        assert_eq!(reply(&cancelled).unwrap_err(), (499, "cancelled".into()));
        let store = Store::new("", []);
        let grants = "auth.callback https://x.test/.exact/auth/callback\nauth.callback x.app:/cb";
        assert_eq!(callback(&store, grants).unwrap(), "x.app:/cb");
        assert_eq!(store.reads(), 1, "the carrier is the device's fact");
    }
}
