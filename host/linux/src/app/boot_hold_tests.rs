//! A request the boot makes is held under the agent as a later one is (LLP
//! 1146 D4): a sign-in an app starts at launch, which the Linux host, with no
//! system browser, answers 501 outside the agent.
use super::*;
use crate::agent::handle;
use crate::presenter::PainterChoice;
use exact_runner::{Answer, DataError, Request, Store, Value};

#[derive(Default)]
struct SignsIn;
impl DataSource for SignsIn {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(&mut self, _: &mut Store, source: &str, _: &[Value]) -> Result<Answer, DataError> {
        if source != "signIn" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let session = r#"{"url":"http://127.0.0.1:4400/p/spec/authorize?client_id=c","callback":"app.test:/cb","state":"s","ephemeral":false}"#;
        Ok(Answer::Later(Request::auth(session.as_bytes().to_vec())))
    }
    fn grants(&self) -> &str {
        "auth.session http://127.0.0.1:4400\nauth.callback app.test:/cb"
    }
}

/// The app booted with the agent driving it or not.
fn boot(agent: bool) -> Presenter<SignsIn> {
    let plan = contract::compile(
        "component App\n  resource back = signIn() as shape string\n  view\n    text back\n",
    )
    .unwrap();
    AGENT_BOOT.set(agent);
    let booted = Presenter::boot_with(
        &plan.encode(),
        SignsIn,
        (300., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    );
    AGENT_BOOT.set(false);
    booted.unwrap().0
}

/// Whether a sign-in is held for the agent's `authorize`.
fn held(p: &mut Presenter<SignsIn>) -> bool {
    let state: serde_json::Value = serde_json::from_str(&handle(p, r#"{"op":"state"}"#)).unwrap();
    (state["pending"].as_array().into_iter().flatten()).any(|h| h["device"]["capability"] == "auth")
}

#[test]
fn a_sign_in_the_boot_starts_is_held_under_the_agent() {
    assert!(held(&mut boot(true)), "held for `authorize`");
    assert!(!held(&mut boot(false)), "no system browser here: 501");
}
