//! The host services LLP 1133.025's desktop modules use (D11 item 2): the
//! process's one hot-key handler, the taskbar button and the launch. Each
//! platform object is one per process, so the host keeps it and a package
//! reaches it through its [`Context`](crate::package::Context), keyed by the
//! calling session.
//!
//! A host that has these installs its [`Services`] before the loop starts
//! (the Windows host, `host/windows/src/desktop.rs`); this Linux host binds
//! none (LLP 1133.025 D14), so [`services`] is `None` and the packages
//! answer `NotSupportedError`.
//!
//! A hot key's press is an activation the host raises (D11 item 3): the
//! host's callback calls [`activate`], and the presenter's next turn raises
//! the registration's event with a person's origin on the runner, which a
//! package's own `emit` never gets.

use std::cell::RefCell;
use std::rc::Rc;

/// Why a chord is not registered, in `@exact/global-shortcuts`' words.
pub type Refusal = &'static str;

/// The login item as the platform keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Login {
    /// `enabled`, `disabled`, `requires-approval` or `unknown`.
    pub state: &'static str,
    /// Whether the registration asked for a launch with no window.
    pub hidden: bool,
}

/// What a desktop host keeps for every session of the process. Called on
/// the loop's thread.
pub trait Services {
    /// Register `keys` (an `aria-keyshortcuts` chord) for `session`'s `id`;
    /// a press raises `extension`'s `event` with `[id, {id, keys}]`.
    fn register_hotkey(
        &self,
        session: &str,
        id: &str,
        keys: &str,
        extension: &'static str,
        event: &'static str,
    ) -> Result<(), Refusal>;
    /// `session` no longer reads `keys` for `id`.
    fn unregister_hotkey(&self, session: &str, id: &str, keys: &str);
    /// Whether `keys` could be registered now, keeping nothing.
    fn check_hotkey(&self, keys: &str) -> Result<(), Refusal>;
    /// Ask for the person's attention on the app's taskbar button or tile;
    /// `Err` says why nothing was asked.
    fn attention(&self, critical: bool) -> Result<(), String>;
    /// The login item's registration as it stands.
    fn login(&self) -> Login;
    /// Register the login item (`Some(hidden)`) or remove it (`None`).
    fn set_login(&self, register: Option<bool>) -> Result<Login, String>;
    /// Open the system's settings for the login item.
    fn open_login_settings(&self) -> Result<(), String>;
    /// This launch was the login item's: `Some(hidden)`, read before the
    /// session booted.
    fn login_launch(&self) -> Option<bool>;
}

/// A press the host heard, waiting for the presenter's turn.
struct Activation {
    extension: &'static str,
    event: &'static str,
    args: String,
}

thread_local! {
    static SERVICES: RefCell<Option<Rc<dyn Services>>> = const { RefCell::new(None) };
    static ACTIVATIONS: RefCell<Vec<Activation>> = const { RefCell::new(Vec::new()) };
}

/// The host's services, from now on, for every session on this thread.
pub fn install(services: Rc<dyn Services>) {
    SERVICES.with(|s| *s.borrow_mut() = Some(services));
}

/// The host's services, or `None` where the host keeps none.
pub fn services() -> Option<Rc<dyn Services>> {
    SERVICES.with(|s| s.borrow().clone())
}

/// The host's platform callback heard a person's act for `extension`'s
/// `event`: raised on the next turn with a person's origin. `args` is a
/// JSON array. The host wakes its loop after.
pub fn activate(extension: &'static str, event: &'static str, args: String) {
    ACTIVATIONS.with(|a| {
        a.borrow_mut().push(Activation {
            extension,
            event,
            args,
        })
    });
}

/// The activations heard since the last turn, raised on `runner`.
pub(crate) fn raise<D: exact_runner::DataSource>(runner: &mut exact_runner::Runner<D>) {
    let heard = ACTIVATIONS.with(|a| std::mem::take(&mut *a.borrow_mut()));
    for a in heard {
        let origin = exact_runner::CallOrigin::App;
        if let Err(e) = runner.raise_module_event(a.extension, a.event, &a.args, origin) {
            runner.log(e);
        }
    }
}

#[cfg(test)]
#[path = "desktop_tests.rs"]
mod tests;
