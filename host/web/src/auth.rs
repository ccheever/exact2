//! `openAuthSession` on the web (LLP 1069.006 D4): the page's half is
//! `auth-glue.js`, which must open its popup inside the press's own call
//! stack, so the request reaches it as a batch op (`{"op":"auth","ticket"}`)
//! and it asks back synchronously (`exact_auth`): the runner's ruling on the
//! request ([`exact_runner::auth::arm`]), and later the callback it saw,
//! which the runner checks. An answer settled here is delivered by the
//! page's `fulfill` with kind 9 ([`exact_runner::auth::take_settled`]).

use super::Host;
use crate::batch::Batch;
use exact_runner::auth::{self, Arm, Browser};
use exact_runner::{DataSource, RequestOut};

impl<D: DataSource> Host<D> {
    /// Hand an `exact-auth:` request to the page, kept until it asks.
    pub(super) fn emit_auth(&mut self, r: RequestOut, batch: &mut Batch) {
        // An artifact that doesn't link auth grants none (its app declares
        // no `auth.session`): the session is refused, as the grant check
        // would refuse it (LLP 1069.006 D1).
        if self.auth_word.is_none() {
            let why =
                "openAuthSession is not linked into this artifact: the app grants no auth.session";
            return auth::settle(&mut self.runner, r.ticket, 403, why);
        }
        batch.auth(r.ticket);
        self.auth_out.retain(|o| self.runner.holds(o.ticket));
        self.auth_out.push(r);
    }

    /// The page's word (`exact_auth`): `{"op":"arm","ticket","agent",
    /// "origin","popup"}` → `{"settled":true}` | `{"held":true}` |
    /// `{"present":{"url","callback"}}` (a page that can't open a popup —
    /// no activation, or the glue not loaded — is 428 `popup blocked`);
    /// `{"op":"done","ticket","url"}` or `{…,"status","message"}` →
    /// `{"settled":true}`.
    pub fn auth(&mut self, json: &str) -> String {
        match self.auth_word {
            Some(word) => word(self, json),
            None => exact_runner::agent::error("openAuthSession is not linked into this artifact"),
        }
    }

    /// [`Host::auth`], when the artifact links auth.
    pub(crate) fn auth_linked(&mut self, json: &str) -> String {
        use exact_runner::agent::{field_bool, field_num, field_str, quote};
        let ticket = field_num(json, "ticket").unwrap_or(0.0) as u64;
        match field_str(json, "op").as_deref() {
            Some("arm") => {
                let Some(at) = self.auth_out.iter().position(|o| o.ticket == ticket) else {
                    return exact_runner::agent::error("no such auth request");
                };
                let out = self.auth_out.remove(at);
                let origin = field_str(json, "origin");
                let agent = field_bool(json, "agent");
                match auth::arm(
                    &mut self.runner,
                    &out,
                    agent,
                    Browser::Web,
                    origin.as_deref(),
                ) {
                    Arm::Settled => "{\"settled\":true}".into(),
                    Arm::Held => "{\"held\":true}".into(),
                    Arm::Present(_) if !field_bool(json, "popup") => {
                        auth::complete(&mut self.runner, ticket, Err((428, "popup blocked")));
                        "{\"settled\":true}".into()
                    }
                    Arm::Present(session) => {
                        let mut s = String::from("{\"present\":{\"url\":");
                        quote(&session.url, &mut s);
                        s.push_str(",\"callback\":");
                        quote(&session.callback, &mut s);
                        s.push_str("}}");
                        s
                    }
                }
            }
            Some("done") => {
                match field_str(json, "url") {
                    Some(url) => auth::complete(&mut self.runner, ticket, Ok(&url)),
                    None => {
                        let status = field_num(json, "status").unwrap_or(502.0) as u16;
                        let message = field_str(json, "message").unwrap_or_default();
                        auth::complete(&mut self.runner, ticket, Err((status, &message)));
                    }
                }
                "{\"settled\":true}".into()
            }
            _ => exact_runner::agent::error("auth: arm or done"),
        }
    }
}
