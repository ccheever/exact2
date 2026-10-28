//! The host's local-network fact and the rule every fetch hop and WebSocket
//! open passes (LLP 1133.036 D6, D8).
//!
//! On iOS 14+ and macOS 15+ a connection to a device on the local network
//! can show the system's Local Network alert as surely as a Bonjour browse
//! (TN3179), so a reach asked by a tool invocation, a wake or a package's
//! event, whose destination could prompt, is refused before it is sent:
//! `person-required` while the person has not decided, and unsent while
//! they have denied it. A person's input and the app's own foreground code
//! proceed; their reach is the ask.
//!
//! The fact is one per process, the host's: a package reports its browses'
//! and resolves' outcomes into it, and on Apple the host's probe does. Under
//! the agent the runner stamps the substitute's state on each request
//! instead ([`Reach::substitute`]), and the destination's risk is the
//! grant's static test, since the substitute reads no interface (D19).

use crate::CallOrigin;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// A permission state, as LLP 1133.002 D8 words it. `Unknown` means "a reach
/// may ask": Apple reports no state, and each launch starts here (D8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Access is allowed, or the target has no local network privacy.
    Granted,
    /// The person has not decided, and the platform says so.
    Prompt,
    /// The person or the platform refused access.
    Denied,
    /// No evidence this launch: a reach may ask.
    Unknown,
}

impl State {
    /// Its word in `localNetworkStatus().state`.
    pub fn word(self) -> &'static str {
        match self {
            State::Granted => "granted",
            State::Prompt => "prompt",
            State::Denied => "denied",
            State::Unknown => "unknown",
        }
    }

    /// The state a word names.
    pub fn parse(word: &str) -> Option<State> {
        Some(match word {
            "granted" => State::Granted,
            "prompt" => State::Prompt,
            "denied" => State::Denied,
            "unknown" => State::Unknown,
            _ => return None,
        })
    }

    fn from_u8(n: u8) -> State {
        match n {
            0 => State::Granted,
            1 => State::Prompt,
            2 => State::Denied,
            _ => State::Unknown,
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            State::Granted => 0,
            State::Prompt => 1,
            State::Denied => 2,
            State::Unknown => 3,
        }
    }
}

/// `granted` until a host whose target prompts says so at [`launch`].
static FACT: AtomicU8 = AtomicU8::new(0);
static PROMPTS: AtomicBool = AtomicBool::new(false);

/// Whether this host's target has local network privacy, so that a reach to
/// the local network can show a prompt: iOS 14 and later and macOS 15 and
/// later (TN3179). Such a host says so at launch, and the fact starts at
/// `unknown` (D8); elsewhere no reach has prompt risk, the rule refuses
/// nothing, and the fact is `granted`.
pub fn launch(prompts: bool) {
    PROMPTS.store(prompts, Ordering::Release);
    report(if prompts {
        State::Unknown
    } else {
        State::Granted
    });
}

/// [`launch`], the first time this process asks: a host calls it as it
/// starts its I/O, so the fact is set once per launch however many sessions
/// start.
pub fn launch_once(prompts: impl FnOnce() -> bool) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| launch(prompts()));
}

/// Whether a reach on this host can prompt ([`launch`]).
pub fn prompts() -> bool {
    PROMPTS.load(Ordering::Acquire)
}

/// The host's local-network fact now.
pub fn fact() -> State {
    State::from_u8(FACT.load(Ordering::Acquire))
}

/// Set the host's fact: at launch, where the target has no local network
/// privacy (`Granted`), and after, from evidence. Answers whether it moved.
pub fn report(state: State) -> bool {
    FACT.swap(state.to_u8(), Ordering::AcqRel) != state.to_u8()
}

/// Who asked a request, for the rule this module states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reach {
    /// The origin of the answer that asked it (LLP 1133.002 D12).
    pub origin: CallOrigin,
    /// Under the agent, the substitute's local-network state, which stands
    /// for the host's fact; `None` in production.
    pub substitute: Option<State>,
}

impl Reach {
    /// Whether this request is refused at a hop whose destination has prompt
    /// risk (`risk`): `None` proceeds, else the reason, which follows
    /// `NotAllowedError: ` in the failure's message.
    pub fn refusal(self, risk: bool) -> Option<&'static str> {
        if self.origin == CallOrigin::App || !risk {
            return None;
        }
        match self.substitute.unwrap_or_else(fact) {
            State::Granted => None,
            State::Denied => Some("local network access is denied"),
            State::Prompt | State::Unknown => Some("person-required"),
        }
    }

    /// Whether a hop's destination has prompt risk under the agent: the
    /// grant's static test without loopback (D5), since the substitute reads
    /// no interface. `host` is as a URL serializes it.
    pub fn static_risk(host: &str) -> bool {
        exact_grants::is_local_network(host, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_tool_wake_or_device_hop_with_risk_is_refused_by_the_state() {
        let tool = |state| Reach {
            origin: CallOrigin::Tool,
            substitute: Some(state),
        };
        assert_eq!(tool(State::Prompt).refusal(true), Some("person-required"));
        assert_eq!(tool(State::Unknown).refusal(true), Some("person-required"));
        assert_eq!(
            tool(State::Denied).refusal(true),
            Some("local network access is denied")
        );
        assert_eq!(tool(State::Granted).refusal(true), None);
        assert_eq!(tool(State::Prompt).refusal(false), None);
        for origin in [CallOrigin::Wake, CallOrigin::Device] {
            let reach = Reach {
                origin,
                substitute: Some(State::Prompt),
            };
            assert_eq!(reach.refusal(true), Some("person-required"));
        }
        let app = Reach {
            origin: CallOrigin::App,
            substitute: Some(State::Denied),
        };
        assert_eq!(app.refusal(true), None);
    }

    #[test]
    fn under_the_agent_risk_is_the_static_test_without_loopback() {
        assert!(Reach::static_risk("192.168.1.40"));
        assert!(Reach::static_risk("wled-a1b2c3.local"));
        assert!(!Reach::static_risk("127.0.0.1"));
        assert!(!Reach::static_risk("example.test"));
    }
}
