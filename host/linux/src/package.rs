//! What a package's Rust implementation of a module sees of exact2 on the
//! Linux and Windows hosts (LLP 1133.001 D5's `rust-static`, built by LLP
//! 1133.022 D10 item 5). It is the Rust counterpart of Apple's
//! `ExactPackage.swift` and Android's `ExtensionContext`.
//!
//! A package declares its implementation in `linux/exact.json` or
//! `windows/exact.json` (`"platform": "linux"`, `"sources": {"rust":
//! ["lib.rs"]}`, `"class": "Echo"`). The app's build compiles that file as a
//! module of the app's own host crate (`contract::native::rust_packages`)
//! and hands the host a table of [`Entry`]; the host makes one instance per
//! session, on its first call, with that session's [`Context`].
//!
//! The runner's calls reach the instance on the loop's thread, between
//! turns: the native slot's one handler queues each body and wakes the
//! loop, and the loop's turn hands a package's bodies to its instance and
//! a shell's (Android's reader) to the shell (`presenter/requests.rs`). The
//! bodies are the ones every host's implementations take
//! (`host/apple/modules/ExactExtensions.swift` lists them). An instance is
//! not `Send`; a [`Reply`] and a [`Notifier`] are, so work on another thread
//! answers and announces through them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub use serde_json::{json, Value as Json};

/// A package's implementation: what the host calls. Only [`Package::new`]
/// and [`Package::later`] are required.
pub trait Package: 'static {
    /// The session's instance, made on the session's first call of it.
    fn new(context: Context) -> Self
    where
        Self: Sized;
    /// A source's read or a send, by its declared name, with the keyword
    /// arguments the call wrote; `reply` once. Under the agent, `preferred`
    /// is the agent's value for the name (LLP 1133.009 D10 item 11). The
    /// call's origin and a send's number are on the context.
    fn later(&mut self, name: &str, args: &Json, preferred: Option<&Json>, reply: Reply);
    /// Data code's synchronous form (LLP 1133.002 D14). These hosts answer
    /// none yet: a synchronous form needs a call on the caller's thread.
    fn call(&mut self, _name: &str, _args: &Json) -> Json {
        json!({ "error": "NotSupportedError" })
    }
    /// Live source `name` has no reader left (`args` `None`), or no reader
    /// reads it with `args` any longer (LLP 1133.009 D10 item 12).
    fn release(&mut self, _name: &str, _args: Option<&Json>) {}
    /// The agent's preferred values for this interface's names and the
    /// runner's permission states, when bound under the agent and whenever
    /// one changes.
    fn preferred(&mut self, _values: &Json, _permissions: &Json) {}
    /// Handles a dropped reply carried (LLP 1133.009 D10 item 10).
    fn dispose(&mut self, _ids: &[String]) {}
    /// The send numbered `request` was let go: its reply has no reader.
    fn cancel(&mut self, _request: u64) {}
    /// The app went to the foreground or left it; the context's
    /// `foreground` already says so.
    fn foreground(&mut self, _shown: bool) {}
    /// The session ends; nothing is called after.
    fn destroy(&mut self) {}
}

/// One package an app's build composed: its extension (the interface's
/// name after its scope) and how its instance is made.
#[derive(Clone, Copy)]
pub struct Entry {
    /// The extension the runner's bodies name.
    pub extension: &'static str,
    make: fn(Context) -> Box<dyn Package>,
}

impl Entry {
    /// `P` as the implementation of `extension`.
    pub fn of<P: Package>(extension: &'static str) -> Entry {
        Entry {
            extension,
            make: |context| Box::new(P::new(context)),
        }
    }

    pub(crate) fn make(&self, context: Context) -> Box<dyn Package> {
        (self.make)(context)
    }
}

/// The answer to one call, sent once from any thread: an object, or an
/// error as `{"error": code, "message"?}`.
pub struct Reply(pub(crate) exact_runner::Reply);

impl Reply {
    pub(crate) fn new(reply: exact_runner::Reply) -> Reply {
        Reply(reply)
    }

    /// Answer with `answer`. An answer that is not an object (a list) is
    /// sent as `{"exact:answer": value}`, a key no shape's field can take.
    pub fn send(self, answer: Json) {
        let answer = match answer {
            Json::Object(_) => answer,
            other => json!({ "exact:answer": other }),
        };
        self.0
            .send(exact_runner::Outcome::Response(exact_runner::Response {
                status: 200,
                headers: Vec::new(),
                body: answer.to_string().into_bytes(),
            }));
    }

    /// Fail with the error `code` (`NotSupportedError`, `NotAllowedError`,
    /// …) and an optional message.
    pub fn error(self, code: &str, message: Option<&str>) {
        let mut answer = json!({ "error": code });
        if let Some(message) = message {
            answer["message"] = json!(message);
        }
        self.send(answer);
    }
}

/// What a package says to the runner from any thread: a topic changed, or
/// one of its interface's events happened.
#[derive(Clone)]
pub struct Notifier {
    native: exact_runner::Native,
    extension: &'static str,
    seq: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl Notifier {
    /// The readers of `topic` (`extension.name`) are asked again (LLP 1016.002).
    pub fn changed(&self, topic: &str) {
        self.native.changed(topic);
    }

    /// Raise the interface's event `name` with `args`, a JSON array (LLP
    /// 1133.004 D6): delivered in order, once, with the device's origin.
    pub fn emit(&self, name: &str, args: Json) {
        let seq = (self.seq).fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let body = json!({ "extension": self.extension, "event": name, "args": args, "seq": seq });
        self.native.changed(&format!("exact-event:{body}"));
    }
}

/// The session an instance belongs to. Read on the loop's thread.
#[derive(Clone)]
pub struct Context(pub(crate) Rc<Shared>);

pub(crate) struct Shared {
    pub(crate) extension: &'static str,
    pub(crate) agent: bool,
    pub(crate) session: String,
    pub(crate) origin: RefCell<String>,
    pub(crate) request: Cell<Option<u64>>,
    pub(crate) foreground: Cell<bool>,
    pub(crate) now: Rc<Cell<f64>>,
    pub(crate) lines: Rc<RefCell<Vec<String>>>,
    pub(crate) notifier: Notifier,
}

impl Context {
    pub(crate) fn new(
        extension: &'static str,
        agent: bool,
        native: exact_runner::Native,
        now: Rc<Cell<f64>>,
        lines: Rc<RefCell<Vec<String>>>,
    ) -> Context {
        let notifier = Notifier {
            native,
            extension,
            seq: Default::default(),
        };
        Context(Rc::new(Shared {
            extension,
            agent,
            session: "1".into(),
            origin: RefCell::new("app".into()),
            request: Cell::new(None),
            foreground: Cell::new(true),
            now,
            lines,
            notifier,
        }))
    }

    /// The extension this instance implements.
    pub fn extension(&self) -> &'static str {
        self.0.extension
    }

    /// Under the agent, unless the drive bound the implementation as in
    /// production (`--bind`): an implementation that is its own substitute
    /// then opens no device (LLP 1133.006 D8).
    pub fn agent(&self) -> bool {
        self.0.agent
    }

    /// The session's id, unique in the process.
    pub fn session(&self) -> &str {
        &self.0.session
    }

    /// Who made the call being answered now: `app`, `tool`, `wake` or
    /// `device`. Set before each [`Package::later`].
    pub fn origin(&self) -> String {
        self.0.origin.borrow().clone()
    }

    /// The runner's number for the send being answered now, which
    /// [`Package::cancel`] names; `None` for a read.
    pub fn request(&self) -> Option<u64> {
        self.0.request.get()
    }

    /// Whether the app is in the foreground, as the runner's admission
    /// reads it.
    pub fn foreground(&self) -> bool {
        self.0.foreground.get()
    }

    /// The session's clock as of the loop's last turn, ms: the agent's
    /// under the agent.
    pub fn now(&self) -> f64 {
        self.0.now.get()
    }

    /// A journal line in a development build.
    pub fn log(&self, line: impl Into<String>) {
        let line = format!("{}: {}", self.0.extension, line.into());
        self.0.lines.borrow_mut().push(line);
    }

    /// What answers and announces from any thread.
    pub fn notifier(&self) -> Notifier {
        self.0.notifier.clone()
    }

    /// The readers of `topic` are asked again.
    pub fn changed(&self, topic: &str) {
        self.0.notifier.changed(topic);
    }

    /// Raise the interface's event `name` with `args`, a JSON array.
    pub fn emit(&self, name: &str, args: Json) {
        self.0.notifier.emit(name, args);
    }
}
